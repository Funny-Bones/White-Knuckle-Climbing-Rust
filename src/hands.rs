//! Per-hand grabbing and grip stamina.

use bevy::prelude::*;
use bevy_rapier3d::parry::query::ShapeCastOptions;
use bevy_rapier3d::parry::shape::Ball;
use bevy_rapier3d::prelude::*;

use crate::level::{Handhold, Holds, WORLD_GROUP};
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
    holds: Holds,
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
    // Handholds win; failing that, the top edge of whatever is being looked at.
    aim.0 = find_handhold(&rapier, entity, camera.translation, forward, radius, &holds)
        .or_else(|| find_ledge(&rapier, entity, camera.translation, forward, radius));

    let is_grounded = player.is_grounded;
    let loss_rate = if player.hands.iter().all(|h| h.is_hanging()) {
        TWO_HAND_LOSS_RATE
    } else {
        1.0
    };

    for (hand, button) in player.hands.iter_mut().zip(HAND_BUTTONS) {
        let pressed = locked.0 && mouse.pressed(button);
        // Count the lockout down to just below zero and no further. Left to
        // run on, one long frame would bank enough negative time to cancel the
        // next lockout, and a hand that let go for a jump would grab straight
        // back on.
        if hand.grab_wait >= 0.0 {
            hand.grab_wait = (hand.grab_wait - dt).max(-f32::EPSILON);
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
                        hand.drag_mult = Handhold::or_ledge(handhold).drag_mult;
                    }
                }
            }
        }

        // Grip drains while hanging in the air and recovers otherwise.
        let strain = hand
            .hold
            .and_then(|hold| holds.get(hold.entity).ok())
            .map(|(_, handhold)| Handhold::or_ledge(handhold).strain_rate);
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
    holds: &Holds,
) -> Option<(Entity, Vec3)> {
    let not_player = QueryFilter::default().exclude_collider(player);
    let is_hold = |entity: Entity| holds.get(entity).is_ok_and(|(_, handhold)| handhold.is_some());
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

/// How far in from the edge a ledge is tested and gripped.
const LEDGE_INSET: f32 = 0.12;
/// How far above the point being looked at a ledge's top may be.
const LEDGE_RISE: f32 = 0.55;

/// Finds a ledge to grab when looking along `dir`: a walkable top just above
/// the wall under the crosshair, or the edge a swept sphere catches when
/// aiming slightly over it.
fn find_ledge(
    rapier: &RapierContext,
    player: Entity,
    origin: Vec3,
    dir: Vec3,
    radius: f32,
) -> Option<(Entity, Vec3)> {
    let world = QueryFilter::default()
        .exclude_collider(player)
        .groups(CollisionGroups::new(Group::ALL, WORLD_GROUP));
    let flat_dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();

    // The top of the surface at `point`, whose outward normal is `normal`.
    let ledge_above = |point: Vec3, normal: Vec3| {
        // Tops are stood on and undersides have no ledge; only walls count.
        if normal.y > 0.7 || normal.y < -0.3 {
            return None;
        }
        let outward = Vec3::new(normal.x, 0.0, normal.z);
        let inward = if outward.length() > 0.2 { -outward.normalize() } else { flat_dir };
        if inward == Vec3::ZERO {
            return None;
        }
        // Drop a ray from above, just inside the wall. Starting inside solid
        // rock means the wall carries on upward and there is no ledge here.
        let above = point + inward * LEDGE_INSET + Vec3::Y * LEDGE_RISE;
        let (entity, top) =
            rapier.cast_ray_and_get_normal(above, Vec3::NEG_Y, LEDGE_RISE + 0.15, true, world)?;
        if top.time_of_impact < 0.03 || top.normal.y < 0.7 {
            return None;
        }
        // Grip close to the edge, and only where there is room for a hand.
        let grip = top.point - inward * (LEDGE_INSET * 0.6);
        if rapier.cast_ray(grip + Vec3::Y * 0.02, Vec3::Y, 0.3, true, world).is_some() {
            return None;
        }
        (grip.distance(origin) <= INTERACT_DISTANCE).then_some((entity, grip))
    };

    if let Some((_, hit)) = rapier.cast_ray_and_get_normal(origin, dir, INTERACT_DISTANCE, true, world) {
        if let Some(ledge) = ledge_above(hit.point, hit.normal) {
            return Some(ledge);
        }
    }
    let ball = Ball::new(radius);
    let options = ShapeCastOptions {
        max_time_of_impact: INTERACT_DISTANCE,
        target_distance: 0.0,
        stop_at_penetration: false,
        compute_impact_geometry_on_penetration: true,
    };
    let (_, hit) = rapier.cast_shape(origin, Quat::IDENTITY, dir, &ball, options, world)?;
    let details = hit.details?;
    // Catching an edge gives a diagonal normal; treat it as the wall below.
    // A normal that is nearly straight up is just the floor.
    let wall_normal = Vec3::new(details.normal1.x, 0.0, details.normal1.z);
    if details.normal1.y > 0.9 || wall_normal.length() < 0.2 {
        return None;
    }
    let wall_normal = wall_normal.normalize();
    ledge_above(details.witness1 - Vec3::Y * 0.05, wall_normal)
}
