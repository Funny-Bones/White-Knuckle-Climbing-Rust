//! Per-hand grabbing and grip stamina.

use bevy::prelude::*;
use bevy_rapier3d::parry::query::ShapeCastOptions;
use bevy_rapier3d::parry::shape::Ball;
use bevy_rapier3d::prelude::*;

use crate::level::Handhold;
use crate::player::{Hold, Player};
use crate::tuning::*;
use crate::view::{CursorLocked, PlayerCamera};

/// The handhold point the crosshair is currently on, if any.
#[derive(Resource, Default)]
pub struct AimTarget(pub Option<(Entity, Vec3)>);

const HAND_BUTTONS: [MouseButton; 2] = [MouseButton::Left, MouseButton::Right];

pub fn update_hands(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    locked: Res<CursorLocked>,
    rapier: ReadRapierContext,
    cameras: Query<&Transform, With<PlayerCamera>>,
    mut players: Query<(Entity, &mut Player)>,
    holds: Query<(&GlobalTransform, &Handhold)>,
    mut aim: ResMut<AimTarget>,
) {
    let (Ok(camera), Ok((entity, mut player)), Ok(rapier)) =
        (cameras.single(), players.single_mut(), rapier.single())
    else {
        return;
    };
    let dt = time.delta_secs();

    // Moving fast widens the catch radius.
    let radius = (player.speed_mps * 0.03).clamp(0.15, 0.3);
    let forward = camera.rotation * Vec3::NEG_Z;
    aim.0 = find_handhold(&rapier, entity, camera.translation, forward, radius, &holds);

    let is_grounded = player.is_grounded;
    let loss_rate = if player.hands.iter().all(|h| h.is_hanging()) {
        TWO_HAND_LOSS_RATE
    } else {
        1.0
    };

    for (hand, button) in player.hands.iter_mut().zip(HAND_BUTTONS) {
        let pressed = locked.0 && mouse.pressed(button);
        if hand.grab_wait >= 0.0 {
            hand.grab_wait -= dt;
        }

        if hand.is_hanging() {
            if !pressed {
                hand.release();
            }
        } else {
            if hand.memory_time > 0.0 {
                hand.memory_time -= dt;
            }
            if let Some((target, point)) = aim.0 {
                if let Ok((transform, _)) = holds.get(target) {
                    let local = transform.affine().inverse().transform_point3(point);
                    hand.memory = Some(Hold { entity: target, local });
                    hand.memory_time = GRAB_FORGIVE;
                }
            }
            // Holding the button grabs as soon as a hold comes into reach.
            if pressed && hand.grab_wait < 0.0 && hand.memory_time > 0.0 {
                if let Some(memory) = hand.memory {
                    if let Ok((_, handhold)) = holds.get(memory.entity) {
                        hand.hold = Some(memory);
                        hand.hold_time = 0.0;
                        hand.drag_mult = handhold.drag_mult;
                    }
                }
            }
        }

        // Grip drains while hanging in the air and recovers otherwise.
        let strain = hand
            .hold
            .and_then(|hold| holds.get(hold.entity).ok())
            .map(|(_, handhold)| handhold.strain_rate);
        if let Some(strain) = strain {
            hand.hold_time += dt;
            if is_grounded {
                hand.grip += dt * REGEN_AIRBORNE;
            } else {
                hand.grip -= dt * loss_rate * strain;
            }
        } else {
            hand.hold_time = 0.0;
            if is_grounded {
                hand.grip += dt * REGEN_GROUNDED;
            } else if hand.regen_wait <= 0.0 {
                hand.grip += dt * REGEN_AIRBORNE;
            }
        }
        hand.grip = hand.grip.clamp(0.0, GRIP_MAX);

        if hand.grip <= GRIP_EXHAUSTED && hand.grab_wait < 0.1 {
            hand.release();
            hand.grab_wait = EXHAUSTED_GRAB_WAIT;
        }
    }
}

/// Finds the handhold a hand would grab when looking along `dir`: whatever is
/// under the crosshair first, then anything a swept sphere catches nearby.
fn find_handhold(
    rapier: &RapierContext,
    player: Entity,
    origin: Vec3,
    dir: Vec3,
    radius: f32,
    holds: &Query<(&GlobalTransform, &Handhold)>,
) -> Option<(Entity, Vec3)> {
    let not_player = QueryFilter::default().exclude_collider(player);
    let is_hold = |entity: Entity| holds.contains(entity);
    let sweep = |max_time_of_impact: f32| ShapeCastOptions {
        max_time_of_impact,
        target_distance: 0.0,
        stop_at_penetration: false,
        compute_impact_geometry_on_penetration: true,
    };
    // Where a swept sphere touched what it hit.
    let contact = |from: Vec3, dir: Vec3, radius: f32, hit: &ShapeCastHit| {
        let center = from + dir * hit.time_of_impact;
        match hit.details {
            Some(details) if details.witness1.distance(center) < radius + 0.3 => details.witness1,
            _ => center,
        }
    };

    let direct = rapier.cast_ray_and_get_normal(origin, dir, INTERACT_DISTANCE, true, not_player);
    if let Some((entity, hit)) = direct {
        if is_hold(entity) {
            return Some((entity, hit.point));
        }
    }

    let ball = Ball::new(radius);
    let swept = rapier.cast_shape(
        origin,
        Quat::IDENTITY,
        dir,
        &ball,
        sweep(INTERACT_DISTANCE),
        not_player,
    );
    if let Some((entity, hit)) = swept {
        let touched = contact(origin, dir, radius, &hit);
        if is_hold(entity) {
            return Some((entity, touched));
        }
        // The sphere caught something else first: look for a hold between
        // that point and where the crosshair lands.
        let end = direct.map_or(origin + dir * INTERACT_DISTANCE, |(_, hit)| hit.point);
        let span = end - touched;
        let length = span.length();
        if length > 1e-4 {
            let line_dir = span / length;
            let line_ball = Ball::new(0.15);
            let only_holds = QueryFilter::default().predicate(&is_hold);
            if let Some((entity, hit)) = rapier.cast_shape(
                touched,
                Quat::IDENTITY,
                line_dir,
                &line_ball,
                sweep(length),
                only_holds,
            ) {
                return Some((entity, contact(touched, line_dir, 0.15, &hit)));
            }
        }
    }

    // Last chance: a hold right next to the point being looked at.
    let (_, hit) = direct?;
    let mut found = None;
    rapier.intersect_shape(hit.point, Quat::IDENTITY, &Ball::new(0.1), not_player, |entity| {
        if is_hold(entity) {
            found = Some((entity, hit.point));
        }
        found.is_none()
    });
    found
}
