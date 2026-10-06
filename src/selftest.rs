//! Scripted smoke test, enabled with `WK_SELFTEST=1`: walk to the wall, grab
//! the first hold with the left hand, pull up, add the right hand on the next
//! hold, jump off, and log what the body did along the way.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::level::Handhold;
use crate::player::Player;
use crate::tuning::EYE_OFFSET;
use crate::view::{CursorLocked, PlayerCamera};

pub fn enabled() -> bool {
    std::env::var_os("WK_SELFTEST").is_some()
}

#[derive(Default)]
pub struct Script {
    next_log: f32,
    at_wall: Option<f32>,
    max_height: f32,
    shots: usize,
}

pub fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut script: Local<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut locked: ResMut<CursorLocked>,
    mut players: Query<&mut Player>,
    holds: Query<&GlobalTransform, With<Handhold>>,
) {
    let Ok(mut player) = players.single_mut() else {
        return;
    };
    locked.0 = true;
    let t = time.elapsed_secs();
    let eye = player.cur_pos + Vec3::Y * EYE_OFFSET;
    script.max_height = script.max_height.max(player.cur_pos.y);

    // The n-th lowest handhold of the main wall's first pitch.
    let hold_at = |n: usize| {
        let mut points: Vec<Vec3> = holds
            .iter()
            .map(|h| h.translation())
            .filter(|p| p.x.abs() < 1.0 && (-2.5..-1.5).contains(&p.z))
            .collect();
        points.sort_by(|a, b| a.y.total_cmp(&b.y));
        points.get(n).copied()
    };
    let look_at =|player: &mut Player, target: Vec3| {
        let dir = (target - eye).normalize_or_zero();
        player.yaw = (-dir.x).atan2(-dir.z);
        player.pitch = dir.y.clamp(-1.0, 1.0).asin();
    };
    let mut set_key = |key: KeyCode, down: bool| {
        if down { keys.press(key) } else { keys.release(key) }
    };

    match script.at_wall {
        None => {
            // Phase 1: walk up to the wall.
            let arrived = player.cur_pos.z < -1.3 || t > 6.0;
            set_key(KeyCode::KeyW, t > 0.5 && !arrived);
            if arrived {
                script.at_wall = Some(t);
                info!("selftest: reached wall at t={t:.2} pos={:.2}", player.cur_pos);
            }
        }
        Some(start) => {
            let phase = t - start;
            // One picture per stage: reaching, one hand on, both hands on.
            let shot_times = [0.05, 2.5, 4.5];
            if script.shots < shot_times.len() && phase >= shot_times[script.shots] {
                let path = format!("target/selftest_{}.png", script.shots);
                commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
                script.shots += 1;
            }
            if phase < 3.0 {
                // Phase 2: left hand on the first hold, then pull up.
                if let Some(target) = hold_at(0) {
                    look_at(&mut player, target);
                }
                mouse.press(MouseButton::Left);
                set_key(KeyCode::KeyW, phase > 1.0);
            } else if phase < 5.0 {
                // Phase 3: right hand on the second hold, keep pulling.
                if let Some(target) = hold_at(1) {
                    look_at(&mut player, target);
                }
                mouse.press(MouseButton::Right);
            } else if phase < 5.3 {
                set_key(KeyCode::Space, true);
            } else if phase < 7.0 {
                set_key(KeyCode::Space, false);
                set_key(KeyCode::KeyW, false);
                mouse.release(MouseButton::Left);
                mouse.release(MouseButton::Right);
            } else {
                info!("selftest: done, max body height {:.2}", script.max_height);
                std::process::exit(0);
            }
        }
    }

    if t >= script.next_log {
        script.next_log = t + if script.at_wall.is_none() { 0.06 } else { 0.25 };
        info!(
            "selftest t={t:5.2} pos=({:6.2},{:6.2},{:6.2}) speed={:5.2} m/s vel_y={:7.4} {:?} grounded={} hands=[{} {:.2}, {} {:.2}]",
            player.cur_pos.x,
            player.cur_pos.y,
            player.cur_pos.z,
            player.speed_mps,
            player.vel.y,
            player.state,
            player.is_grounded,
            if player.hands[0].is_hanging() { "HOLD" } else { "free" },
            player.hands[0].grip,
            if player.hands[1].is_hanging() { "HOLD" } else { "free" },
            player.hands[1].grip,
        );
    }
}

/// `WK_OVERVIEW=1` saves bird's-eye pictures of the whole level and exits.
pub fn overview_enabled() -> bool {
    std::env::var_os("WK_OVERVIEW").is_some()
}

pub fn overview(
    mut commands: Commands,
    time: Res<Time>,
    mut shots: Local<usize>,
    mut cameras: Query<&mut Transform, With<PlayerCamera>>,
) {
    let views = [
        (Vec3::new(0.0, 55.0, 70.0), Vec3::new(0.0, 8.0, 0.0)),
        (Vec3::new(60.0, 40.0, -35.0), Vec3::new(5.0, 8.0, 5.0)),
        (Vec3::new(-60.0, 35.0, 30.0), Vec3::new(-10.0, 6.0, 0.0)),
        (Vec3::new(0.0, 14.0, 6.0), Vec3::new(8.0, 8.0, 30.0)),
    ];
    // Hold each view for a second so the picture is taken from a settled frame.
    let t = time.elapsed_secs() - 1.0;
    let index = (t.max(0.0)) as usize;
    if index >= views.len() {
        std::process::exit(0);
    }
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    let (eye, target) = views[index];
    *camera = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    if t.fract() > 0.5 && *shots == index {
        let path = format!("target/overview_{index}.png");
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *shots += 1;
    }
}
