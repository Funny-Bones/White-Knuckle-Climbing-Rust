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

/// Whether any of the scripted modes is running (they log sound cues).
pub fn any_enabled() -> bool {
    enabled() || walk_enabled() || ledge_enabled() || hands_enabled()
}

#[derive(Default)]
pub struct Script {
    next_log: f32,
    at_wall: Option<f32>,
    max_height: f32,
    shots: usize,
    frames: u32,
    slowest_frame: f32,
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
    // Skip the first second, which is dominated by start-up hitches.
    if t > 1.0 {
        script.frames += 1;
        script.slowest_frame = script.slowest_frame.max(time.delta_secs());
    }

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
                info!(
                    "selftest: {:.0} fps average, slowest frame {:.1} ms",
                    script.frames as f32 / (t - 1.0),
                    script.slowest_frame * 1000.0
                );
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

/// `WK_WALK=1` walks around open ground for a while, counting the moments
/// where the body loses its speed without being told to stop, then exits.
pub fn walk_enabled() -> bool {
    std::env::var_os("WK_WALK").is_some()
}

#[derive(Default)]
pub struct WalkLog {
    next_log: f32,
    stalls: u32,
    stalled: bool,
}

pub fn walk(
    time: Res<Time>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut locked: ResMut<CursorLocked>,
    mut players: Query<&mut Player>,
    mut log: Local<WalkLog>,
) {
    let Ok(mut player) = players.single_mut() else {
        return;
    };
    locked.0 = true;
    let t = time.elapsed_secs();
    if t > 40.0 {
        info!("walk: done, {} stalls (bumping into a structure counts as one)", log.stalls);
        std::process::exit(0);
    }
    // Wander in wide loops south of the spawn, sprinting half the time.
    player.yaw = std::f32::consts::PI + (t * 0.35).sin() * 1.4;
    keys.press(KeyCode::KeyW);
    if (t as i32 / 5) % 2 == 1 {
        keys.press(KeyCode::ShiftLeft);
    } else {
        keys.release(KeyCode::ShiftLeft);
    }

    let stalled = t > 2.0 && player.is_grounded && player.speed_mps < 1.0;
    if stalled && !log.stalled {
        log.stalls += 1;
        info!("walk: stalled at t={t:.2} pos={:.1}", player.cur_pos);
    }
    log.stalled = stalled;
    if t >= log.next_log {
        log.next_log = t + 5.0;
        info!("walk t={t:5.1} pos={:.1} speed={:.2}", player.cur_pos, player.speed_mps);
    }
}

/// `WK_LEDGE=1` stands in front of a three metre block, grabs its top edge
/// with both hands, pulls up and over, and reports where the body ends up.
pub fn ledge_enabled() -> bool {
    std::env::var_os("WK_LEDGE").is_some()
}

pub fn ledge(
    mut commands: Commands,
    time: Res<Time>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut locked: ResMut<CursorLocked>,
    mut players: Query<(&mut Transform, &mut Player)>,
    mut stage: Local<u32>,
    mut next_log: Local<f32>,
) {
    let Ok((mut transform, mut player)) = players.single_mut() else {
        return;
    };
    locked.0 = true;
    let t = time.elapsed_secs();
    // The fifth stepping block: 1.6 m wide, 3.0 m tall, centred here.
    let block = Vec3::new(-19.2, 3.0, 22.0);
    if *stage == 0 {
        let start = Vec3::new(block.x, 1.2, block.z - 1.6);
        transform.translation = start;
        player.teleport(start, std::f32::consts::PI);
        *stage = 1;
    }
    let eye = player.cur_pos + Vec3::Y * EYE_OFFSET;
    // Look at the wall just under its top edge.
    let dir = (Vec3::new(block.x, block.y - 0.15, block.z - 0.8) - eye).normalize_or_zero();
    if t > 1.0 && t < 4.5 {
        mouse.press(MouseButton::Left);
        mouse.press(MouseButton::Right);
        if t < 2.0 {
            player.yaw = (-dir.x).atan2(-dir.z);
            player.pitch = dir.y.clamp(-1.0, 1.0).asin();
        } else {
            // Pulling up only works while looking up; looking down lowers you.
            player.pitch = 0.5;
        }
    }
    if t > 2.0 && t < 4.5 {
        keys.press(KeyCode::KeyW);
    }
    if t > 4.5 {
        keys.release(KeyCode::KeyW);
        mouse.release(MouseButton::Left);
        mouse.release(MouseButton::Right);
        player.pitch = 0.0;
    }
    if *stage == 1 && t > 1.8 {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk("target/ledge.png"));
        *stage = 2;
    }
    if t >= *next_log {
        *next_log = t + 0.25;
        info!(
            "ledge t={t:4.2} pos={:.2} {:?} grounded={} hands=[{}, {}]",
            player.cur_pos,
            player.state,
            player.is_grounded,
            player.hands[0].is_hanging(),
            player.hands[1].is_hanging(),
        );
    }
    if t > 6.5 {
        let on_top = player.is_grounded && player.cur_pos.y > block.y + 1.0;
        info!("ledge: done, standing on top of the block: {on_top}");
        std::process::exit(0);
    }
}

/// `WK_HANDS=1` photographs the hand poses: resting, walking, running,
/// reaching with one button held, and crouch-walking.
pub fn hands_enabled() -> bool {
    std::env::var_os("WK_HANDS").is_some()
}

pub fn hands(
    mut commands: Commands,
    time: Res<Time>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut locked: ResMut<CursorLocked>,
    mut players: Query<&mut Player>,
    mut shots: Local<usize>,
) {
    let Ok(mut player) = players.single_mut() else {
        return;
    };
    locked.0 = true;
    let t = time.elapsed_secs();
    let stages = ["rest", "walk", "run", "reach", "crouch"];
    let stage = ((t - 1.0).max(0.0) / 1.5) as usize;
    // Face away from the wall so nothing is in reach, then turn towards open
    // ground for the crouch walk.
    player.yaw = if stage < 4 { std::f32::consts::PI } else { std::f32::consts::FRAC_PI_2 };
    player.pitch = 0.0;
    if stage >= stages.len() {
        std::process::exit(0);
    }
    let set = |keys: &mut ButtonInput<KeyCode>, key, down: bool| {
        if down { keys.press(key) } else { keys.release(key) }
    };
    set(&mut keys, KeyCode::KeyW, stage == 1 || stage == 2 || stage == 4);
    set(&mut keys, KeyCode::ShiftLeft, stage == 2);
    set(&mut keys, KeyCode::ControlLeft, stage == 4);
    if stage == 3 {
        mouse.press(MouseButton::Left);
    } else {
        mouse.release(MouseButton::Left);
    }
    if *shots == stage && (t - 1.0) - stage as f32 * 1.5 > 1.2 {
        let path = format!("target/hands_{}.png", stages[stage]);
        info!(
            "hands: {} speed={:.2} m/s body height={:.2} eye={:.2}",
            stages[stage],
            player.speed_mps,
            player.height,
            player.eye_offset()
        );
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *shots += 1;
    }
}
