use std::{
    cell::RefCell,
    ptr::NonNull,
    rc::Rc,
    sync::mpsc::{self, Receiver},
};

use bevy::{
    app::{AppExit, PluginsState},
    ecs::event::ManualEventReader,
    input::{
        keyboard::{Key, KeyboardInput},
        mouse::MouseButtonInput,
    },
    prelude::*,
    window::{PrimaryWindow, RawHandleWrapper, RequestRedraw},
};
use bevy_xr::session::XrStatusChanged;
use raw_window_handle::{WebCanvasWindowHandle, WebDisplayHandle};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlCanvasElement, WebGlContextAttributes, XrFrame, XrSystem};

use crate::{
    future_util::ToJsFuture,
    session::WxrSession,
    window::convert::{key_event_code_to_key_code, key_event_key_to_logical_key},
};

pub struct WxrWindowPlugin;

impl Plugin for WxrWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WxrCanvasId>();
        setup_main_window(app);
        app.set_runner(setup_runner);
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct WxrSystem(pub XrSystem);

// SAFETY: idk
unsafe impl Send for WxrSystem {}
unsafe impl Sync for WxrSystem {}

async fn check_for_availability(world: &mut World) {
    let xr = web_sys::window().unwrap().navigator().xr();
    if xr.is_undefined() {
        world.remove_resource::<crate::signals::ShowVrEnterButton>();
        world.remove_resource::<crate::signals::ShowArEnterButton>();
        if let Some(status) = world.get_resource_mut::<bevy_xr::session::XrSharedStatus>() {
            status.set(bevy_xr::session::XrStatus::Unavailable);
        }
        world.send_event(XrStatusChanged(bevy_xr::session::XrStatus::Unavailable));

        return;
    }
    if xr
        .is_session_supported(web_sys::XrSessionMode::ImmersiveVr)
        .to_future()
        .await
        .unwrap()
        .as_bool()
        .unwrap()
    {
        world.insert_resource(crate::signals::ShowVrEnterButton);
    } else {
        world.remove_resource::<crate::signals::ShowVrEnterButton>();
    }
    if xr
        .is_session_supported(web_sys::XrSessionMode::ImmersiveAr)
        .to_future()
        .await
        .unwrap()
        .as_bool()
        .unwrap()
    {
        world.insert_resource(crate::signals::ShowArEnterButton);
    } else {
        world.remove_resource::<crate::signals::ShowArEnterButton>();
    }
    if xr
        .is_session_supported(web_sys::XrSessionMode::Inline)
        .to_future()
        .await
        .unwrap()
        .as_bool()
        .unwrap()
    {
        world.insert_resource(crate::signals::ShowInlineEnterButton);
    } else {
        world.remove_resource::<crate::signals::ShowInlineEnterButton>();
    }

    world.insert_resource(WxrSystem(xr));
}

#[derive(Resource, Clone, Copy, Debug, Deref)]
pub struct WxrCanvasId(pub &'static str);
#[derive(Debug, Deref)]
pub struct WxrCanvas(pub Box<web_sys::HtmlCanvasElement>);
impl Default for WxrCanvasId {
    fn default() -> Self {
        Self("bevy_canvas")
    }
}

pub enum FrameLoopSource {
    Canvas,
    WxrSession,
}
struct WxrAppRunnerState {
    frame_source: FrameLoopSource,
}

impl Drop for WxrCanvas {
    fn drop(&mut self) {
        warn!("DROPING CANVAS!!!");
    }
}

enum WxrCanvasEvent {
    KeyboardInput(KeyboardInput),
    TextInput(ReceivedCharacter),
    MouseMoved(CursorMoved),
    MouseButton(MouseButtonInput),
}

fn setup_main_window(app: &mut App) {
    let canvas_id = app.world.get_resource::<WxrCanvasId>().unwrap();
    let doc = web_sys::window().unwrap().document().unwrap();
    let canvas = doc
        .get_element_by_id(canvas_id)
        .map(|c| c.dyn_into::<HtmlCanvasElement>().unwrap())
        .unwrap_or_else(|| {
            let canvas = doc
                .create_element("canvas")
                .unwrap()
                .dyn_into::<HtmlCanvasElement>()
                .unwrap();
            doc.body().unwrap().append_child(&canvas).unwrap();
            canvas.set_id(canvas_id);
            canvas
        });
    app.insert_non_send_resource(WxrCanvas(canvas.into()));
    let window = app
        .world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(&app.world);
    let canvas = app.world.get_non_send_resource::<WxrCanvas>().unwrap();
    let w = NonNull::from({
        let js_value: &JsValue = &canvas.0;
        js_value
    });
    let mut webgl_layer_attrs = WebGlContextAttributes::new();
    webgl_layer_attrs.alpha(true);
    webgl_layer_attrs.xr_compatible(true);
    webgl_layer_attrs.power_preference(web_sys::WebGlPowerPreference::HighPerformance);
    unsafe {
        let e = w.as_ptr();
        (*e).dyn_ref::<HtmlCanvasElement>()
            .unwrap()
            .get_context_with_context_options("webgl2", &webgl_layer_attrs)
            .unwrap()
            .unwrap();
    }

    let raw_handle =
        raw_window_handle::RawWindowHandle::WebCanvas(WebCanvasWindowHandle::new(w.cast()));
    let display_handle = WebDisplayHandle::new();
    app.world.entity_mut(window).insert(RawHandleWrapper {
        window_handle: raw_handle,
        display_handle: raw_window_handle::RawDisplayHandle::Web(display_handle),
    });
}
struct WxrEventRecv(Receiver<WxrCanvasEvent>);

fn setup_runner(mut app: App) {
    if app.plugins_state() == PluginsState::Ready {
        app.finish();
        app.cleanup();
    }
    // prepare structures to access data in the world
    // let mut app_exit_event_reader = ManualEventReader::<AppExit>::default();
    // let mut redraw_event_reader = ManualEventReader::<RequestRedraw>::default();
    let (tx, rx) = mpsc::channel();
    app.insert_non_send_resource(WxrEventRecv(rx));
    spawn_local(async move {
        setup_canvas_callbacks(&mut app, tx);
        check_for_availability(&mut app.world).await;
        let app = Rc::new(RefCell::new(app));
        setup_canvas_frame_handler(app.clone());
        setup_xr_session_frame_handler(app.clone());
        request_animation_frame(&app.borrow());
    });
}

fn setup_canvas_callbacks(app: &mut App, sender: mpsc::Sender<WxrCanvasEvent>) {
    // let canvas = app.world.get_nn_send_resource::<WxrCanvas>().unwrap();
    let window_entity = app
        .world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(&app.world);
    let sender_outer = sender;
    let sender = sender_outer.clone();
    add_event("keydown", move |e: web_sys::KeyboardEvent| {
        let logical_key = key_event_key_to_logical_key(&e.key());
        match &logical_key {
            Key::Character(char) => {
                sender
                    .send(WxrCanvasEvent::TextInput(ReceivedCharacter {
                        window: window_entity,
                        char: char.clone(),
                    }))
                    .unwrap();
            }
            Key::Space => {
                sender
                    .send(WxrCanvasEvent::TextInput(ReceivedCharacter {
                        window: window_entity,
                        char: " ".into(),
                    }))
                    .unwrap();
            }
            Key::Enter => {
                sender
                    .send(WxrCanvasEvent::TextInput(ReceivedCharacter {
                        window: window_entity,
                        char: "\r".into(),
                    }))
                    .unwrap();
            }
            Key::Tab => {
                sender
                    .send(WxrCanvasEvent::TextInput(ReceivedCharacter {
                        window: window_entity,
                        char: "\t".into(),
                    }))
                    .unwrap();
            }
            _ => {}
        }
        let event = KeyboardInput {
            key_code: key_event_code_to_key_code(&e.code()),
            logical_key,
            state: bevy::input::ButtonState::Pressed,
            window: window_entity,
        };
        sender.send(WxrCanvasEvent::KeyboardInput(event)).unwrap();
    });
    let sender = sender_outer.clone();
    add_event("keyup", move |e: web_sys::KeyboardEvent| {
        let event = KeyboardInput {
            key_code: key_event_code_to_key_code(&e.code()),
            logical_key: key_event_key_to_logical_key(&e.key()),
            state: bevy::input::ButtonState::Released,
            window: window_entity,
        };
        sender.send(WxrCanvasEvent::KeyboardInput(event)).unwrap();
    });
    let sender = sender_outer.clone();
    add_event("mousemove", move |e: web_sys::MouseEvent| {
        sender
            .send(WxrCanvasEvent::MouseMoved(CursorMoved {
                window: window_entity,
                position: Vec2::new(e.x() as f32, e.y() as f32),
                delta: Some(Vec2::new(e.offset_x() as f32, e.offset_y() as f32)),
            }))
            .unwrap();
    });
    let sender = sender_outer.clone();
    add_event("mousedown", move |e: web_sys::MouseEvent| {
        let button = match e.button() {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            4 => MouseButton::Forward,
            5 => MouseButton::Back,
            other => MouseButton::Other(other.try_into().unwrap()),
        };
        sender
            .send(WxrCanvasEvent::MouseButton(MouseButtonInput {
                button,
                state: bevy::input::ButtonState::Pressed,
                window: window_entity,
            }))
            .unwrap();
    });
    let sender = sender_outer.clone();
    add_event("mouseup", move |e: web_sys::MouseEvent| {
        let button = match e.button() {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            4 => MouseButton::Forward,
            5 => MouseButton::Back,
            other => MouseButton::Other(other.try_into().unwrap()),
        };
        sender
            .send(WxrCanvasEvent::MouseButton(MouseButtonInput {
                button,
                state: bevy::input::ButtonState::Released,
                window: window_entity,
            }))
            .unwrap();
    });
}

// there is probably a better way to keep the Closure alive than leaking it
fn add_event<T: wasm_bindgen::convert::FromWasmAbi + 'static, F: FnMut(T) + 'static>(
    name: &str,
    closure: F,
) {
    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback(
            name,
            Box::leak(Box::new(Closure::wrap(
                Box::new(closure) as Box<dyn FnMut(_)>
            )))
            .as_ref()
            .unchecked_ref(),
        )
        .unwrap();
}

pub struct WxrRunnerXrFrameEventHandler(Closure<dyn FnMut(f64, XrFrame)>);
pub struct WxrRunnerCanvasFrameEventHandler(Closure<dyn FnMut(f64)>);
fn setup_xr_session_frame_handler(app: Rc<RefCell<App>>) {
    app.clone()
        .borrow_mut()
        .world
        .insert_non_send_resource(WxrRunnerXrFrameEventHandler(Closure::new(
            move |time, xr_frame| {
                let mut app = app.borrow_mut();
                info!("Session Frame");
                common_update(&mut app);
            },
        )));
}

fn request_animation_frame(app: &App) {
    if let Some(session) = app.world.get_resource::<WxrSession>() {
        let handler = app
            .world
            .get_non_send_resource::<WxrRunnerXrFrameEventHandler>()
            .unwrap();
        info!("requesting XR frame");
        session.request_animation_frame(handler.0.as_ref().unchecked_ref());
    } else {
        let handler = app
            .world
            .get_non_send_resource::<WxrRunnerCanvasFrameEventHandler>()
            .unwrap();
        info!("requesting Window frame");
        web_sys::window()
            .unwrap()
            .request_animation_frame(handler.0.as_ref().unchecked_ref())
            .unwrap();
    }
}

fn setup_canvas_frame_handler(app: Rc<RefCell<App>>) {
    app.clone()
        .borrow_mut()
        .world
        .insert_non_send_resource(WxrRunnerCanvasFrameEventHandler(Closure::new(
            move |time| {
                let mut app = app.borrow_mut();
                info!("Canvas Frame: {time}");
                common_update(&mut app);
            },
        )));
}

fn common_update(app: &mut App) {
    request_animation_frame(app);
    if app.plugins_state() == PluginsState::Ready {
        app.finish();
        app.cleanup();
    }
    let rx = app
        .world
        .remove_non_send_resource::<WxrEventRecv>()
        .unwrap();
    while let Ok(e) = rx.0.try_recv() {
        match e {
            WxrCanvasEvent::KeyboardInput(event) => {
                app.world.send_event(event);
            }
            WxrCanvasEvent::TextInput(event) => {
                app.world.send_event(event);
            }
            WxrCanvasEvent::MouseMoved(event) => {
                app.world.send_event(event);
            }
            WxrCanvasEvent::MouseButton(event) => {
                app.world.send_event(event);
            }
        }
    }
    app.world.insert_non_send_resource(rx);
    app.update();
}

// fn request_animation_frame(f: &Closure<dyn FnMut()>) {
//     web_sys::window()
//         .unwrap()
//         .request_animation_frame(f.as_ref().unchecked_ref())
//         .expect("should register `requestAnimationFrame` OK");
// }
