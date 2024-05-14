use bevy::{
    prelude::*,
    render::{
        camera::{
            ManualTextureView, ManualTextureViewHandle, ManualTextureViews, RenderTarget, Viewport,
        },
        renderer::RenderDevice,
    },
};
use bevy_xr::camera::XrCamera;
use wasm_bindgen::JsCast;
use web_sys::{XrFrame, XrView};
use wgpu::{TextureFormat, TextureUsages};

use crate::{
    projection::WxrProjection,
    runner::WxrFrame,
    session::{WxrSession, WxrSessionCreated, WxrWebGlLayer},
    space::{WxrReferenceSpace, WxrReferenceSpacePlugin},
};

pub struct WxrRenderPlugin;

impl Plugin for WxrRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (spawn_cameras, create_textures).run_if(on_event::<WxrSessionCreated>()),
        );
        app.add_systems(
            PostUpdate,
            update_cameras.run_if(resource_exists::<WxrFrame>).run_if(resource_exists::<WxrReferenceSpace>),
        );
    }
}

fn update_cameras(
    layer: Res<WxrWebGlLayer>,
    frame: Res<WxrFrame>,
    ref_space: Res<WxrReferenceSpace>,
    mut cameras: Query<(&mut Camera, &mut Transform, &mut WxrProjection), With<WxrCamera>>,
) {
    let views = frame
        .0
        .get_viewer_pose(&ref_space)
        .unwrap()
        .views()
        .to_vec()
        .into_iter()
        .map(|v| v.dyn_into::<XrView>().unwrap());
    for (view, (mut cam, mut transform, mut proj)) in views.zip(cameras.iter_mut()) {
        let viewport = layer.get_viewport(&view).unwrap();
        let view_pos = view.transform().position();
        let view_rot = view.transform().orientation();
        transform.translation = Vec3::new(
            view_pos.x() as f32,
            view_pos.y() as f32,
            view_pos.z() as f32,
        );
        transform.rotation = Quat::from_xyzw(
            view_rot.x() as f32,
            view_rot.y() as f32,
            view_rot.z() as f32,
            view_rot.w() as f32,
        );
        cam.viewport = Some(Viewport {
            physical_position: UVec2 {
                x: viewport.x() as u32,
                y: viewport.y() as u32,
            },
            physical_size: UVec2 {
                x: viewport.width() as u32,
                y: viewport.height() as u32,
            },
            ..default()
        });
        cam.is_active = true;
        *proj = WxrProjection::from_view(&view);
    }
}
pub const WEBXR_MANUAL_TEXTURE_VIEW: ManualTextureViewHandle = ManualTextureViewHandle(931310957);

#[derive(Component, Clone, Copy)]
pub struct WxrCamera;

fn spawn_cameras(mut cmds: Commands) {
    cmds.spawn((
        Camera3dBundle {
            camera: Camera {
                target: RenderTarget::TextureView(WEBXR_MANUAL_TEXTURE_VIEW),
                ..Default::default()
            },
            ..Default::default()
        },
        WxrProjection::default(),
        WxrCamera,
    ));
    cmds.spawn((
        Camera3dBundle {
            camera: Camera {
                target: RenderTarget::TextureView(WEBXR_MANUAL_TEXTURE_VIEW),
                ..Default::default()
            },
            ..Default::default()
        },
        WxrProjection::default(),
        WxrCamera,
    ));
}
const RGBA: u32 = 0x1908;
const UNSIGNED_BYTE: u32 = 0x1401;
const TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;
fn create_textures(
    layer: Res<WxrWebGlLayer>,
    render_device: Res<RenderDevice>,
    mut texture_views: ResMut<ManualTextureViews>,
) {
    let Some(framebuffer) = layer.framebuffer() else {return;};
    let texture = unsafe {
        render_device
            .wgpu_device()
            .create_texture_from_hal::<wgpu_hal::gles::Api>(
                wgpu_hal::gles::Texture {
                    inner: wgpu_hal::gles::TextureInner::ExternalFramebuffer { inner: framebuffer },
                    mip_level_count: 1,
                    array_layer_count: 1,
                    format: TEXTURE_FORMAT,
                    format_desc: wgpu_hal::gles::TextureFormatDesc {
                        internal: RGBA, // TODO: Test alternatives.
                        external: RGBA,
                        data_type: UNSIGNED_BYTE,
                    },
                    copy_size: wgpu_hal::CopyExtent {
                        width: layer.framebuffer_width(),
                        height: layer.framebuffer_height(),
                        depth: 1,
                    },
                    drop_guard: None,
                },
                &wgpu::TextureDescriptor {
                    label: Some("webxr framebuffer (color)"),
                    size: wgpu::Extent3d {
                        width: layer.framebuffer_width(),
                        height: layer.framebuffer_height(),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: TEXTURE_FORMAT,
                    view_formats: &[TEXTURE_FORMAT],
                    usage: TextureUsages::RENDER_ATTACHMENT
                        // | TextureUsages::TEXTURE_BINDING
                        | TextureUsages::COPY_SRC,
                },
            )
    };
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    texture_views.insert(
        WEBXR_MANUAL_TEXTURE_VIEW,
        ManualTextureView::with_default_format(
            texture_view.into(),
            UVec2 {
                x: layer.framebuffer_width(),
                y: layer.framebuffer_height(),
            },
        ),
    );
}
