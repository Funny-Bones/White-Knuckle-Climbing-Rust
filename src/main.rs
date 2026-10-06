//! A Bevy reimplementation of the two-handed climbing from White Knuckle.

mod hands;
mod level;
mod player;
mod selftest;
mod tuning;
mod view;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "White Knuckle climbing (Bevy)".into(),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule())
    .insert_resource(Time::<Fixed>::from_hz(tuning::FIXED_HZ))
    .insert_resource(ClearColor(Color::srgb(0.03, 0.035, 0.05)))
    .init_resource::<hands::AimTarget>()
    .init_resource::<view::CursorLocked>()
    .add_systems(Startup, (level::setup_level, view::setup_view, spawn_player))
    .add_systems(
        FixedUpdate,
        player::player_movement.before(PhysicsSet::SyncBackend),
    )
    .add_systems(
        Update,
        (
            view::cursor_grab,
            view::mouse_look,
            selftest::run.run_if(selftest::enabled),
            view::respawn,
            view::camera_follow,
            hands::update_hands,
            view::update_hand_visuals,
            view::update_hud,
        )
            .chain(),
    );
    app.run();
}

fn spawn_player(mut commands: Commands) {
    player::spawn_player(&mut commands, level::SPAWN_POS, level::SPAWN_YAW);
}
