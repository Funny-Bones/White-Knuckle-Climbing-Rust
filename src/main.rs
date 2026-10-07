//! A Bevy reimplementation of the two-handed climbing from White Knuckle.

mod audio;
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
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "White Knuckle climbing (Bevy)".into(),
                    // `WK_NOVSYNC=1` uncaps the frame rate, for measuring it.
                    present_mode: if std::env::var_os("WK_NOVSYNC").is_some() {
                        bevy::window::PresentMode::AutoNoVsync
                    } else {
                        bevy::window::PresentMode::AutoVsync
                    },
                    ..default()
                }),
                ..default()
            })
            // Keep pixel-art sprites crisp.
            .set(ImagePlugin::default_nearest()),
    )
    .add_plugins(RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule())
    .insert_resource(Time::<Fixed>::from_hz(tuning::FIXED_HZ))
    .insert_resource(ClearColor(Color::srgb(0.45, 0.68, 0.95)))
    .init_resource::<hands::AimTarget>()
    .init_resource::<view::CursorLocked>()
    .add_systems(Startup, (level::setup_level, view::setup_view, audio::setup_audio, spawn_player))
    .add_systems(
        FixedUpdate,
        player::player_movement.before(PhysicsSet::SyncBackend),
    )
    .add_systems(
        Update,
        (
            view::attach_sky,
            view::cursor_grab,
            view::mouse_look,
            selftest::run.run_if(selftest::enabled),
            selftest::walk.run_if(selftest::walk_enabled),
            selftest::ledge.run_if(selftest::ledge_enabled),
            selftest::hands.run_if(selftest::hands_enabled),
            view::respawn,
            view::camera_follow,
            hands::update_hands,
            audio::play_sounds,
            view::update_hand_visuals,
            view::update_hud,
            selftest::overview.run_if(selftest::overview_enabled),
        )
            .chain(),
    );
    app.run();
}

fn spawn_player(mut commands: Commands) {
    player::spawn_player(&mut commands, level::SPAWN_POS, level::SPAWN_YAW);
}
