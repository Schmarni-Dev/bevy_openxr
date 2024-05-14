use bevy::{
    math::Vec3A,
    prelude::*,
    render::camera::{CameraProjection, CameraProjectionPlugin},
};
use web_sys::XrView;

pub struct WxrProjectionPlugin;

impl Plugin for WxrProjectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CameraProjectionPlugin::<WxrProjection>::default());
    }
}

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct WxrProjection(pub Mat4);
impl WxrProjection {
    pub fn from_view(view: &XrView) -> Self {
        let mut proj = Self(Mat4::from_cols_slice(&view.projection_matrix()));
        proj.0.z_axis.z = 0.0;
        proj.0.w_axis.z = -proj.0.w_axis.z;
        proj
    }
}
impl Default for WxrProjection {
    fn default() -> Self {
        Self(Mat4 {
            x_axis: Vec4::new(2.4142134, 0.0, 0.0, 0.0),
            y_axis: Vec4::new(0.0, 2.4142134, 0.0, 0.0),
            z_axis: Vec4::new(0.0, 0.0, 0.0, -1.0),
            w_axis: Vec4::new(0.0, 0.0, 0.1, 0.0),
        })
    }
}

impl CameraProjection for WxrProjection {
    fn get_projection_matrix(&self) -> Mat4 {
        self.0
    }

    fn update(&mut self, _width: f32, _height: f32) {}

    fn far(&self) -> f32 {
        self.0.w_axis.z / (self.0.z_axis.z + 1.0)
    }

    fn get_frustum_corners(&self, _z_near: f32, _z_far: f32) -> [bevy::math::Vec3A; 8] {
        let mut corners = [
            Vec3A::new(1.0, -1.0, 1.0),   // Bottom-right far
            Vec3A::new(1.0, 1.0, 1.0),    // Top-right far
            Vec3A::new(-1.0, 1.0, 1.0),   // Top-left far
            Vec3A::new(-1.0, -1.0, 1.0),  // Bottom-left far
            Vec3A::new(1.0, -1.0, -1.0),  // Bottom-right near
            Vec3A::new(1.0, 1.0, -1.0),   // Top-right near
            Vec3A::new(-1.0, 1.0, -1.0),  // Top-left near
            Vec3A::new(-1.0, -1.0, -1.0), // Bottom-left near
        ];

        let inverse_matrix = self.0.inverse();
        for (i, corner) in corners.into_iter().enumerate() {
            corners[i] = inverse_matrix.transform_point3a(corner);
        }

        corners
    }
}
