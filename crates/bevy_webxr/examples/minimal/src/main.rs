use std::{f32::consts::TAU, ops::DerefMut};

use bevy::prelude::*;
use bevy_webxr::{
    add_xr_plugins, instance::WxrInstancePlugin, signals::{ShowArEnterButton, ShowInlineEnterButton, ShowVrEnterButton}
};

#[bevy_main]
fn main() {
    let mut app = App::new();
    app.add_plugins(add_xr_plugins(DefaultPlugins));
    app.add_plugins(bevy_egui::EguiPlugin);
    app.add_systems(Startup, setup);
    app.add_systems(Update, spin);
    app.add_systems(Update, update_egui);
    app.run();
}

#[derive(Component)]
struct Spin;

fn update_egui(
    mut contexts: bevy_egui::EguiContexts,
    mut check: Local<bool>,
    mut txt: Local<String>,
    vr_button: Option<Res<ShowVrEnterButton>>,
    ar_button: Option<Res<ShowArEnterButton>>,
    inline_button: Option<Res<ShowInlineEnterButton>>,
    mut start_session: EventWriter<bevy_xr::session::CreateXrSession>,
) {
    info!("test");
    bevy_egui::egui::Window::new("Test").show(contexts.ctx_mut(), |ui| {
        ui.checkbox(&mut check, "Said Hello");
        ui.text_edit_singleline(txt.deref_mut());
    });
    bevy_egui::egui::Window::new("XR Menu").show(contexts.ctx_mut(), |ui| {
        if vr_button.is_some() && ui.button("Enter VR").clicked() {
            start_session.send_default();
        }
        if ar_button.is_some() && ui.button("Enter AR").clicked() {
            start_session.send_default();
        }
        if inline_button.is_some() && ui.button("Enter Inline").clicked() {
            start_session.send_default();
        }
    });
}

fn spin(
    mut query: Query<&mut Transform, With<Spin>>,
    time: Res<Time>,
    key: Res<ButtonInput<KeyCode>>,
) {
    if key.pressed(KeyCode::Space) {
        for mut transform in &mut query {
            transform.rotate_y(time.delta_seconds() * TAU);
        }
    }
}

fn setup(
    mut cmds: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    cmds.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });
    cmds.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::from_size(Vec3::splat(1.0)).mesh()),
            material: mats.add(StandardMaterial {
                base_color: Color::PINK,
                // emissive: ,
                ..Default::default()
            }),
            // transform: todo!(),
            ..Default::default()
        },
        Spin,
    ));
}
