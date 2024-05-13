use std::sync::mpsc::{channel, Receiver, Sender};

use bevy::{prelude::*, utils::HashSet};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::{XrSession, XrSessionInit, XrSessionMode};

use crate::{future_util::ToJsFuture, runner::WxrSystem};
pub struct WxrSessionPlugin;

#[derive(Clone, Copy, Hash)]
/// Just because a feature is listed here doesn't mean it actually does anything out of the box
pub enum WxrFeature {
    Anchors,
    BoundedFloor,
    DepthSensing,
    DomOverlay,
    HandTracking,
    HitTest,
    Layers,
    LightEstimation,
    LocalFloor,
    SecondaryViews,
    Unbounded,
    Viewer,
    Unknown(&'static str),
}

impl WxrFeature {
    pub const fn as_str(&self) -> &'static str {
        match self {
            WxrFeature::Anchors => "anchors",
            WxrFeature::BoundedFloor => "bounded-floor",
            WxrFeature::DepthSensing => "depth-sensing",
            WxrFeature::DomOverlay => "dom-overlay",
            WxrFeature::HandTracking => "hand-tracking",
            WxrFeature::HitTest => "hit-test",
            WxrFeature::Layers => "layers",
            WxrFeature::LightEstimation => "light-estimation",
            WxrFeature::LocalFloor => "local-floor",
            WxrFeature::SecondaryViews => "secondary-views",
            WxrFeature::Unbounded => "unbounded",
            WxrFeature::Viewer => "viewer",
            WxrFeature::Unknown(feat) => feat,
        }
    }
}

#[derive(Resource, Clone, Default)]
pub struct WxrRequiredFeatures(pub HashSet<&'static str>);
impl WxrRequiredFeatures {
    pub fn enable(&mut self, feat: WxrFeature) -> &mut Self {
        self.0.insert(feat.as_str());
        self
    }
    pub fn in_enabled(&self, feat: WxrFeature) -> bool {
        self.0.contains(feat.as_str())
    }
    pub fn as_js_value(&self) -> JsValue {
        js_sys::Array::from_iter(self.0.iter().map(|v| JsValue::from_str(v))).into()
    }
}
#[derive(Resource, Clone, Default)]
pub struct WxrOptionalFeatures(pub HashSet<&'static str>);
impl WxrOptionalFeatures {
    pub fn enable(&mut self, feat: WxrFeature) -> &mut Self {
        self.0.insert(feat.as_str());
        self
    }
    pub fn in_enabled(&self, feat: WxrFeature) -> bool {
        self.0.contains(feat.as_str())
    }
    pub fn as_js_value(&self) -> JsValue {
        js_sys::Array::from_iter(self.0.iter().map(|v| JsValue::from_str(v))).into()
    }
}

#[derive(Clone, Copy, Default)]
pub enum WxrSessionMode {
    #[default]
    Inline,
    Vr,
    Mr,
}
#[derive(Resource, Clone, Copy, Default, Deref, DerefMut)]
pub struct WxrRequestedSessionMode(pub WxrSessionMode);

impl Plugin for WxrSessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WxrRequiredFeatures>();
        app.init_resource::<WxrOptionalFeatures>();
        let (tx, rx) = channel();
        app.insert_resource(SessionCreatedReader(rx));
        app.insert_resource(SessionCreatedWriter(tx));
        app.init_resource::<WxrRequestedSessionMode>();
        app.add_systems(
            PreUpdate,
            (
                create_session.run_if(on_event::<bevy_xr::session::CreateXrSession>()),
                insert_session,
            )
                .chain(),
        );
    }
}

#[derive(Deref, DerefMut, Resource)]
pub struct WxrSession {
    #[deref]
    session: XrSession,
    mode: WxrSessionMode,
}
// SAFETY: idk probably bad
unsafe impl Send for WxrSession {}
unsafe impl Sync for WxrSession {}

fn insert_session(mut cmds: Commands, recv: Res<SessionCreatedReader>) {
    while let Ok((session, session_mode)) = recv.0.try_recv() {
        info!("Session Created!");
        cmds.insert_resource(WxrSession {
            session,
            mode: session_mode,
        });
    }
}

#[derive(Resource)]
struct SessionCreatedReader(Receiver<(XrSession, WxrSessionMode)>);
// SAFETY: idk probably bad
unsafe impl Send for SessionCreatedReader {}
unsafe impl Sync for SessionCreatedReader {}
#[derive(Resource)]
struct SessionCreatedWriter(Sender<(XrSession, WxrSessionMode)>);
// SAFETY: idk probably bad
unsafe impl Send for SessionCreatedWriter {}
unsafe impl Sync for SessionCreatedWriter {}

fn create_session(
    mode: Res<WxrRequestedSessionMode>,
    system: Res<WxrSystem>,
    required_features: Res<WxrRequiredFeatures>,
    optional_features: Res<WxrOptionalFeatures>,
    pending_promises: ResMut<SessionCreatedWriter>,
) {
    let mut session_create_info = XrSessionInit::new();
    session_create_info.required_features(&required_features.as_js_value());
    session_create_info.optional_features(&optional_features.as_js_value());
    let mode = *mode;
    let promise =
        match mode.0 {
            WxrSessionMode::Vr => system
                .request_session_with_options(XrSessionMode::ImmersiveVr, &session_create_info),
            WxrSessionMode::Mr => system
                .request_session_with_options(XrSessionMode::ImmersiveAr, &session_create_info),
            WxrSessionMode::Inline => {
                system.request_session_with_options(XrSessionMode::Inline, &session_create_info)
            }
        };
    let send = pending_promises.0.clone();
    spawn_local(async move {
        send.send((
            promise
                .to_future()
                .await
                .unwrap()
                .dyn_into::<XrSession>()
                .unwrap(),
            mode.0,
        ))
        .unwrap();
    });
}
