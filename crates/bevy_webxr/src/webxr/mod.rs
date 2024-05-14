pub mod instance;
pub mod runner;
pub mod session;
pub mod signals;
pub mod window;
pub mod future_util;
pub mod render;
pub mod projection;
pub mod space;
use bevy::{app::PluginGroupBuilder, prelude::*, render::RenderPlugin, winit::WinitPlugin};

use self::{instance::WxrInstancePlugin, projection::WxrProjectionPlugin, render::WxrRenderPlugin, runner::WxrWindowPlugin, session::WxrSessionPlugin, space::WxrReferenceSpacePlugin};

pub fn add_xr_plugins<G: PluginGroup>(plugins: G) -> PluginGroupBuilder {
    plugins
        .build()
        .disable::<WinitPlugin>()
        .add_before::<RenderPlugin, _>(WxrWindowPlugin)
        .set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some("#bevy_canvas".to_string()),
                ..default()
            }),
            ..default()
        })
        .add(bevy_xr::session::XrSessionPlugin)
        .add(WxrSessionPlugin)
        .add(WxrInstancePlugin)
        .add(WxrProjectionPlugin)
        .add(WxrReferenceSpacePlugin)
        .add(WxrRenderPlugin)
}
