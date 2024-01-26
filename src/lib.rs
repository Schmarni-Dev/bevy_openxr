pub mod graphics;
pub mod input;
// pub mod passthrough;
pub mod resource_macros;
pub mod resources;
pub mod xr_init;
pub mod xr_input;

use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{ptr, thread};

use crate::xr_init::{StartXrSession, XrInitPlugin};
use crate::xr_input::hands::hand_tracking::DisableHandTracking;
use crate::xr_input::oculus_touch::ActionSets;
use bevy::app::{AppExit, PluginGroupBuilder};
use bevy::core::TaskPoolThreadAssignmentPolicy;
use bevy::ecs::system::SystemState;
use bevy::input::common_conditions;
use bevy::prelude::*;
use bevy::render::camera::{ManualTextureView, ManualTextureViewHandle, ManualTextureViews};
use bevy::render::extract_resource::ExtractResourcePlugin;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::renderer::{render_system, RenderInstance};
use bevy::render::settings::RenderCreation;
use bevy::render::{Render, RenderApp, RenderPlugin, RenderSet};
use bevy::tasks::available_parallelism;
use bevy::transform::systems::{propagate_transforms, sync_simple_transforms};
use bevy::window::{PresentMode, PrimaryWindow, RawHandleWrapper};
use crossbeam_channel::{RecvError, TryRecvError, TrySendError};
use graphics::extensions::XrExtensions;
use graphics::{XrAppInfo, XrPreferdBlendMode};
use input::XrInput;
use openxr as xr;
// use passthrough::{start_passthrough, supports_passthrough, XrPassthroughLayer};
use resources::*;
use xr::{FormFactor, FrameState, FrameWaiter};
use xr_init::{
    setup_xr, xr_only, xr_render_only, CleanupXrData, XrEarlyInitPlugin, XrShouldRender, XrStatus,
};
use xr_input::controllers::XrControllerType;
use xr_input::hands::emulated::HandEmulationPlugin;
use xr_input::hands::hand_tracking::{HandTrackingData, HandTrackingPlugin};
use xr_input::hands::XrHandPlugins;
use xr_input::xr_camera::XrCameraType;
use xr_input::OpenXrInput;

const VIEW_TYPE: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

pub const LEFT_XR_TEXTURE_HANDLE: ManualTextureViewHandle = ManualTextureViewHandle(1208214591);
pub const RIGHT_XR_TEXTURE_HANDLE: ManualTextureViewHandle = ManualTextureViewHandle(3383858418);

// #[derive(Clone,)]
// pub struct XrFrameStateSyncer {}

/// Adds OpenXR support to an App
#[derive(Default)]
pub struct OpenXrPlugin {
    reqeusted_extensions: XrExtensions,
    prefered_blend_mode: XrPreferdBlendMode,
    app_info: XrAppInfo,
}

impl Plugin for OpenXrPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(XrSessionRunning::new(AtomicBool::new(false)));
        #[cfg(not(target_arch = "wasm32"))]
        match graphics::initialize_xr_instance(
            SystemState::<Query<&RawHandleWrapper, With<PrimaryWindow>>>::new(&mut app.world)
                .get(&app.world)
                .get_single()
                .ok()
                .cloned(),
            self.reqeusted_extensions.clone(),
            self.prefered_blend_mode,
            self.app_info.clone(),
        ) {
            Ok((
                xr_instance,
                oxr_session_setup_info,
                blend_mode,
                device,
                queue,
                adapter_info,
                render_adapter,
                instance,
            )) => {
                debug!("Configured wgpu adapter Limits: {:#?}", device.limits());
                debug!("Configured wgpu adapter Features: {:#?}", device.features());
                warn!("Starting with OpenXR Instance");
                app.insert_resource(ActionSets(vec![]));
                app.insert_resource(xr_instance);
                app.insert_resource(blend_mode);
                app.insert_non_send_resource(oxr_session_setup_info);
                let render_instance = RenderInstance(instance.into());
                app.insert_resource(render_instance.clone());
                app.add_plugins(RenderPlugin {
                    render_creation: RenderCreation::Manual(
                        device,
                        queue,
                        adapter_info,
                        render_adapter,
                        render_instance,
                    ),
                });
                let (sender, receiver) = crossbeam_channel::unbounded();
                app.insert_resource(XrFrameStateReceiver(receiver));
                app.insert_resource(XrFrameStateSender(sender));
                app.insert_resource(XrStatus::Disabled);
                app.world.send_event(StartXrSession);
            }
            Err(err) => {
                warn!("OpenXR Instance Failed to initialize: {}", err);
                app.add_plugins(RenderPlugin::default());
                app.insert_resource(XrStatus::NoInstance);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            app.add_plugins(RenderPlugin::default());
            app.insert_resource(XrStatus::Disabled);
        }
        app.add_systems(
            PreUpdate,
            xr_poll_events.run_if(|status: Res<XrStatus>| *status != XrStatus::NoInstance),
        );
        app.add_systems(
            PreUpdate,
            (
                xr_reset_per_frame_resources,
                receive_waited_frame.run_if(xr_only()),
                locate_views.run_if(xr_only()),
                apply_deferred,
            )
                .chain()
                .before(setup_xr)
                .before(xr_poll_events),
        );
        let render_app = app.sub_app_mut(RenderApp);
        render_app.add_systems(
            Render,
            (|| info!("!frame start!")).in_set(RenderSet::ExtractCommands),
        );
        render_app.add_systems(
            Render,
            (|| info!("!frame start 2!")).in_set(RenderSet::PrepareAssets),
        );
        render_app.add_systems(
            Render,
            (|| info!("!frame end!")).in_set(RenderSet::CleanupFlush),
        );
        render_app.add_systems(
            Render,
            (
                (|xr_status: Option<Res<XrStatus>>| info!("{:?}", xr_status)),
                (|xr_status: Res<XrStatus>| info!("{:?}", xr_only()(xr_status))),
                xr_reset_per_frame_resources.run_if(xr_only()),
                xr_wait_frame.run_if(xr_only()),
            )
                .chain()
                .in_set(RenderSet::PrepareFlush),
        );
        render_app.add_systems(
            Render,
            xr_begin_frame
                .run_if(xr_only())
                // .run_if(xr_render_only())
                .in_set(RenderSet::Prepare)
                // .after(RenderSet::PrepareFlush)
                .before(xr_pre_frame),
        );
        render_app.add_systems(
            Render,
            xr_pre_frame
                .run_if(xr_only())
                .run_if(xr_render_only())
                .before(render_system)
                // .after(RenderSet::PrepareFlush),
                .in_set(RenderSet::Prepare),
        );
        render_app.add_systems(
            Render,
            (
                locate_views,
                xr_input::xr_camera::xr_camera_head_sync,
                sync_simple_transforms,
                propagate_transforms,
            )
                .chain()
                .run_if(xr_only())
                .run_if(xr_render_only())
                .in_set(RenderSet::Prepare),
        );
        render_app.add_systems(
            Render,
            xr_end_frame
                .run_if(xr_only())
                .run_if(xr_render_only())
                .in_set(RenderSet::Cleanup),
        );
        render_app.add_systems(
            Render,
            xr_skip_frame
                .run_if(xr_only())
                // .run_if(xr_after_wait_only())
                .run_if(not(xr_render_only()))
                .in_set(RenderSet::Cleanup),
        );

        // let set = {
        //     use RenderSet::*;
        //     &[
        //         ExtractCommands,
        //         PrepareAssets,
        //         ManageViews,
        //         ManageViewsFlush,
        //         Queue,
        //         // QueueMeshes,
        //         PhaseSort,
        //         // Prepare,
        //         PrepareResources,
        //         PrepareResourcesFlush,
        //         PrepareBindGroups,
        //         PrepareFlush,
        //         Render,
        //         RenderFlush,
        //         Cleanup,
        //         CleanupFlush,
        //     ]
        // };
        // for i in 0..set.len() {
        //     let last = set.get(i - 1);
        //     let curr = set.get(i).unwrap();
        //     let next = set.get(i + 1);
        //
        //     let curr1 = curr.clone();
        //     let curr2 = curr.clone();
        //     let mut before = (move || info!("pre {:?}", curr1)).before(curr.clone());
        //     let mut after = (move || info!("post {:?}", curr2)).after(curr.clone());
        //     if let Some(next) = next {
        //         after = after.before(next.clone());
        //     }
        //     if let Some(last) = last {
        //         before = before.after(last.clone());
        //     }
        //     render_app.add_systems(Render, (before, after));
        // }
    }
}
fn xr_skip_frame(
    xr_swapchain: Res<XrSwapchain>,
    xr_frame_state: Res<XrFrameState>,
    environment_blend_mode: Res<XrEnvironmentBlendMode>,
) {
    let swapchain: &Swapchain = &xr_swapchain;
    // swapchain.begin().unwrap();
    match swapchain {
        Swapchain::Vulkan(swap) => {
            swap.stream
                .lock()
                .unwrap()
                .end(
                    xr_frame_state.predicted_display_time,
                    **environment_blend_mode,
                    &[],
                )
                .unwrap();
        }
    }
}

#[derive(Default)]
pub struct DefaultXrPlugins {
    pub reqeusted_extensions: XrExtensions,
    pub prefered_blend_mode: XrPreferdBlendMode,
    pub app_info: XrAppInfo,
}

impl PluginGroup for DefaultXrPlugins {
    fn build(self) -> PluginGroupBuilder {
        DefaultPlugins
            .build()
            .set(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions {
                    compute: TaskPoolThreadAssignmentPolicy {
                        // set the minimum # of compute threads
                        // to the total number of available threads
                        min_threads: available_parallelism(),
                        max_threads: std::usize::MAX, // unlimited max threads
                        percent: 1.0,                 // this value is irrelevant in this case
                    },
                    // keep the defaults for everything else
                    ..default()
                },
            })
            // .disable::<PipelinedRenderingPlugin>()
            .disable::<RenderPlugin>()
            .add_before::<RenderPlugin, _>(OpenXrPlugin {
                prefered_blend_mode: self.prefered_blend_mode,
                reqeusted_extensions: self.reqeusted_extensions,
                app_info: self.app_info.clone(),
            })
            .add_after::<OpenXrPlugin, _>(OpenXrInput::new(XrControllerType::OculusTouch))
            .add_after::<OpenXrPlugin, _>(XrInitPlugin)
            .add_before::<OpenXrPlugin, _>(XrEarlyInitPlugin)
            .add(XrHandPlugins)
            .add(XrResourcePlugin)
            .set(WindowPlugin {
                #[cfg(not(target_os = "android"))]
                primary_window: Some(Window {
                    transparent: true,
                    present_mode: PresentMode::AutoNoVsync,
                    title: self.app_info.name.clone(),
                    ..default()
                }),
                #[cfg(target_os = "android")]
                primary_window: None,
                #[cfg(target_os = "android")]
                exit_condition: bevy::window::ExitCondition::DontExit,
                #[cfg(target_os = "android")]
                close_when_requested: true,
                ..default()
            })
    }
}

fn xr_reset_per_frame_resources(mut should: ResMut<XrShouldRender>) {
    **should = false;
    info!("reset_resources");
}

fn xr_poll_events(
    instance: Option<Res<XrInstance>>,
    session: Option<Res<XrSession>>,
    session_running: Res<XrSessionRunning>,
    mut app_exit: EventWriter<AppExit>,
    mut cleanup_xr: EventWriter<CleanupXrData>,
) {
    if let (Some(instance), Some(session)) = (instance, session) {
        let _span = info_span!("xr_poll_events");
        while let Some(event) = instance.poll_event(&mut Default::default()).unwrap() {
            use xr::Event::*;
            match event {
                SessionStateChanged(e) => {
                    // Session state change is where we can begin and end sessions, as well as
                    // find quit messages!
                    info!("entered XR state {:?}", e.state());
                    match e.state() {
                        xr::SessionState::READY => {
                            info!("Calling Session begin :3");
                            session.begin(VIEW_TYPE).unwrap();
                            session_running.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        xr::SessionState::STOPPING => {
                            session.end().unwrap();
                            session_running.store(false, std::sync::atomic::Ordering::Relaxed);
                            cleanup_xr.send_default();
                        }
                        xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                            // app_exit.send(AppExit);
                            return;
                        }

                        _ => {}
                    }
                }
                InstanceLossPending(_) => {
                    app_exit.send_default();
                }
                EventsLost(e) => {
                    warn!("lost {} XR events", e.lost_event_count());
                }
                _ => {}
            }
        }
    }
}

fn xr_begin_frame(swapchain: Res<XrSwapchain>) {
    let _span = info_span!("xr_begin_frame").entered();
    info!("begin_frame");
    swapchain.begin();
}

fn receive_waited_frame(
    receiver: Res<XrFrameStateReceiver>,
    mut frame_state: ResMut<XrFrameState>,
    mut should_render: ResMut<XrShouldRender>,
) {
    info!("Pre Receive Frame");
    loop {
        match receiver.try_recv() {
            Ok(state) => {
                *frame_state = state;
                **should_render = frame_state.should_render;
                break;
            }
            Err(TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(1));
            }
            v => {
                v.unwrap();
            }
        }
    }
    info!("Post Receive Frame");
}

pub fn xr_wait_frame(
    frame_state: Option<ResMut<XrFrameState>>,
    session: Res<XrSession>,
    mut should_render: ResMut<XrShouldRender>,
    mut commands: Commands,
    sender: Res<XrFrameStateSender>,
) {
    info!("Called xr_wait_frame");
    {
        let _span = info_span!("xr_wait_frame").entered();
        info!("Pre Frame Wait");
        fn cvt(x: xr::sys::Result) -> Result<xr::sys::Result, xr::sys::Result> {
            if x.into_raw() >= 0 {
                Ok(x)
            } else {
                Err(x)
            }
        }
        fn wait_frame(session: &XrSession) -> eyre::Result<FrameState> {
            let out = unsafe {
                let mut x = xr::sys::FrameState::out(ptr::null_mut());
                cvt((session.instance().fp().wait_frame)(
                    session.as_raw(),
                    ptr::null(),
                    x.as_mut_ptr(),
                ))?;
                x.assume_init()
            };
            Ok(FrameState {
                predicted_display_time: out.predicted_display_time,
                predicted_display_period: out.predicted_display_period,
                should_render: out.should_render.into(),
            })
        }
        let state: XrFrameState = match wait_frame(session.into_inner()) {
            Ok(a) => a.into(),
            Err(e) => {
                warn!("error: {}", e);
                return;
            }
        };
        if let Err(TrySendError::Disconnected(_)) = sender.try_send(
            FrameState {
                predicted_display_time: xr::Time::from_nanos(
                    state.predicted_display_time.as_nanos()
                        + state.predicted_display_period.as_nanos(),
                ),
                ..*state
            }
            .into(),
        ) {
            panic!("Framestate Channel Disconnected, TODO: Make this Semi Recoverable?");
        }
        info!("Post Frame Wait");
        **should_render = state.should_render;
        match frame_state {
            Some(mut f) => *f = state,
            None => commands.insert_resource(state),
        }
    }
}

pub fn xr_pre_frame(
    resolution: Res<XrResolution>,
    format: Res<XrFormat>,
    swapchain: Res<XrSwapchain>,
    mut manual_texture_views: ResMut<ManualTextureViews>,
) {
    {
        let _span = info_span!("xr_acquire_image").entered();
        swapchain.acquire_image().unwrap()
    }
    {
        let _span = info_span!("xr_wait_image").entered();
        swapchain.wait_image().unwrap();
    }
    {
        let _span = info_span!("xr_update_manual_texture_views").entered();
        let (left, right) = swapchain.get_render_views();
        let left = ManualTextureView {
            texture_view: left.into(),
            size: **resolution,
            format: **format,
        };
        let right = ManualTextureView {
            texture_view: right.into(),
            size: **resolution,
            format: **format,
        };
        manual_texture_views.insert(LEFT_XR_TEXTURE_HANDLE, left);
        manual_texture_views.insert(RIGHT_XR_TEXTURE_HANDLE, right);
    }
}

fn xr_dummy_frame_cycle(
    environment_blend_mode: Res<XrEnvironmentBlendMode>,
    swapchain: Res<XrSwapchain>,
    xr_frame_state: Res<XrFrameState>,
) {
    info!("dummy_frame_cycle");
    swapchain.begin().unwrap();
    fn end_frame(
        swapchain: &Swapchain,
        environment_blend_mode: &XrEnvironmentBlendMode,
        xr_frame_state: &XrFrameState,
    ) {
        let _ = match swapchain {
            Swapchain::Vulkan(swapchain) => swapchain.stream.lock().unwrap().end(
                xr_frame_state.predicted_display_time,
                **environment_blend_mode,
                &[],
            ),
        };
    }
    end_frame(&swapchain, &environment_blend_mode, &xr_frame_state)
}

pub fn xr_end_frame(
    xr_frame_state: Res<XrFrameState>,
    views: Res<XrViews>,
    input: Res<XrInput>,
    swapchain: Res<XrSwapchain>,
    resolution: Res<XrResolution>,
    environment_blend_mode: Res<XrEnvironmentBlendMode>,
) {
    #[cfg(target_os = "android")]
    {
        let ctx = ndk_context::android_context();
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.unwrap();
        let env = vm.attach_current_thread_as_daemon();
    }
    info!("end_frame");
    {
        let _span = info_span!("xr_release_image").entered();
        swapchain.release_image().unwrap();
    }
    {
        let _span = info_span!("xr_end_frame").entered();
        let result = swapchain.end(
            xr_frame_state.predicted_display_time,
            &views,
            &input.stage,
            **resolution,
            **environment_blend_mode,
            // passthrough_layer.map(|p| p.into_inner()),
        );
        match result {
            Ok(_) => {}
            Err(e) => warn!("error: {}", e),
        }
    }
}

pub fn locate_views(
    mut views: ResMut<XrViews>,
    input: Res<XrInput>,
    session: Res<XrSession>,
    xr_frame_state: Res<XrFrameState>,
) {
    let _span = info_span!("xr_locate_views").entered();
    **views = match session.locate_views(
        VIEW_TYPE,
        xr_frame_state.predicted_display_time,
        &input.stage,
    ) {
        Ok(this) => this,
        Err(err) => {
            warn!("error: {}", err);
            return;
        }
    }
    .1;
}
