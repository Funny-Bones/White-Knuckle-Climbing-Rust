//! The player body: ground movement, hanging, pulling up and jumping.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::level::{Handhold, WORLD_GROUP};
use crate::tuning::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PlayerState {
    Grounded,
    #[default]
    Airborne,
    Hanging,
}

/// A point on a handhold, stored in the handhold's local space.
#[derive(Clone, Copy, Debug)]
pub struct Hold {
    pub entity: Entity,
    pub local: Vec3,
}

#[derive(Debug)]
pub struct Hand {
    /// What this hand is currently hanging from.
    pub hold: Option<Hold>,
    /// Last hold the crosshair was on, still grabbable while `memory_time > 0`.
    pub memory: Option<Hold>,
    pub memory_time: f32,
    /// The hand cannot grab while this is positive.
    pub grab_wait: f32,
    /// How long the hand has been holding on; the catch firms up over time.
    pub hold_time: f32,
    pub regen_wait: f32,
    pub grip: f32,
    pub drag_mult: f32,
}

impl Default for Hand {
    fn default() -> Self {
        Self {
            hold: None,
            memory: None,
            memory_time: 0.0,
            grab_wait: 0.0,
            hold_time: 0.0,
            regen_wait: 0.0,
            grip: GRIP_MAX,
            drag_mult: 0.0,
        }
    }
}

impl Hand {
    pub fn is_hanging(&self) -> bool {
        self.hold.is_some()
    }

    pub fn grip_fraction(&self) -> f32 {
        (self.grip / GRIP_MAX).clamp(0.0, 1.0)
    }

    pub fn release(&mut self) {
        self.regen_wait = REGEN_WAIT;
        if self.hold.take().is_some() {
            self.grab_wait += RELEASE_GRAB_WAIT;
            self.memory = None;
            self.memory_time = 0.0;
        }
    }
}

#[derive(Component, Debug)]
pub struct Player {
    /// Velocity in the original's units; see [`VEL_SCALE`].
    pub vel: Vec3,
    pub yaw: f32,
    /// Positive looks up.
    pub pitch: f32,
    pub state: PlayerState,
    pub is_grounded: bool,
    pub time_since_grounded: f32,
    pub hands: [Hand; 2],
    /// Body positions at the last two physics ticks, for camera interpolation.
    pub prev_pos: Vec3,
    pub cur_pos: Vec3,
    /// Actual speed over the last tick, in metres per second.
    pub speed_mps: f32,
    /// Number of jumps made, so other systems can notice a new one.
    pub jump_count: u32,

    move_axis: Vec3,
    climb_dir: Vec3,
    jump_dir: Vec3,
    hit_normal: Vec3,
    has_hit: bool,
    hit_time: f32,
    is_sliding: bool,
    has_jumped: bool,
    /// Limit velocity to what the last move actually achieved (set when grounded).
    clamp_to_last_move: bool,
}

impl Player {
    pub fn new(pos: Vec3, yaw: f32) -> Self {
        Self {
            vel: Vec3::ZERO,
            yaw,
            pitch: 0.0,
            state: PlayerState::Airborne,
            is_grounded: false,
            time_since_grounded: 0.0,
            hands: [Hand::default(), Hand::default()],
            prev_pos: pos,
            cur_pos: pos,
            speed_mps: 0.0,
            jump_count: 0,
            move_axis: Vec3::ZERO,
            climb_dir: Vec3::Y,
            jump_dir: Vec3::Y,
            hit_normal: Vec3::ZERO,
            has_hit: false,
            hit_time: 0.0,
            is_sliding: false,
            has_jumped: false,
            clamp_to_last_move: false,
        }
    }

    pub fn is_hanging(&self) -> bool {
        self.hands.iter().any(Hand::is_hanging)
    }

    pub fn look_rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0)
    }

    pub fn teleport(&mut self, pos: Vec3, yaw: f32) {
        let jump_count = self.jump_count;
        *self = Self::new(pos, yaw);
        self.jump_count = jump_count;
    }

    fn grounded_or_coyote(&self) -> bool {
        self.is_grounded || self.time_since_grounded < COYOTE_TIME
    }

    /// A surface was touched during the last move.
    fn on_surface_hit(&mut self, normal: Vec3) {
        self.hit_normal = normal;
        self.has_hit = true;
        self.hit_time = HIT_MEMORY;
        // Moving into anything too steep to stand on kills the velocity into it.
        if self.vel.normalize_or_zero().dot(normal) <= 0.0
            && angle_deg(Vec3::Y, normal) > SLOPE_LIMIT_DEG
        {
            self.vel = project_on_plane(self.vel, normal);
        }
    }

    fn jump(&mut self, holds: &Query<(&GlobalTransform, &Handhold)>) {
        let mut can_jump = self.state != PlayerState::Hanging;
        let mut jump_mult_sum = 0.0;
        let mut hanging_hands = 0;

        if self.state == PlayerState::Hanging {
            let strain = if self.hands.iter().all(Hand::is_hanging) {
                TWO_HAND_JUMP_STRAIN
            } else {
                1.0
            };
            let on_ground = self.grounded_or_coyote();
            for hand in &mut self.hands {
                let Some(hold) = hand.hold else { continue };
                let Ok((_, handhold)) = holds.get(hold.entity) else {
                    hand.release();
                    can_jump = true;
                    continue;
                };
                hanging_hands += 1;
                if handhold.allow_airborne_jump || on_ground {
                    can_jump = true;
                    jump_mult_sum += handhold.jump_mult;
                    if handhold.drop_on_jump {
                        hand.grip -= CLIMB_JUMP_DRAIN * handhold.jump_strain_mult * strain;
                        hand.release();
                    }
                }
            }
        }
        if !can_jump {
            return;
        }

        let jump_mult = if hanging_hands > 0 {
            jump_mult_sum / hanging_hands as f32
        } else {
            1.0
        };
        // Hands that let go no longer count, so a normal jump off a hold goes
        // straight up; only holds that keep their grip launch along the wall.
        let dir = if self.is_hanging() { self.climb_dir } else { self.jump_dir };
        let impulse = dir.normalize_or_zero() * JUMP_HEIGHT * -GRAVITY * jump_mult;

        if self.vel.y < 0.0 {
            self.vel.y = 0.0;
        }
        if self.climb_dir.dot(Vec3::Y) > 0.0 {
            self.climb_dir = Vec3::Y;
        }
        self.vel.y = self.vel.y.max(impulse.y);
        self.vel.x += impulse.x;
        self.vel.z += impulse.z;
        self.time_since_grounded = COYOTE_TIME;
        self.has_jumped = true;
        self.is_grounded = false;
        self.jump_count += 1;
    }
}

/// Everything the hang step needs to know about this tick.
struct HangInput {
    pos: Vec3,
    input: Vec2,
    right: Vec3,
    cam_forward: Vec3,
    wall_ahead: bool,
    bracing: bool,
    dt: f32,
}

impl Player {
    /// Applies the forces of every hand that is holding on and returns the
    /// pull towards the held points. Also replaces the walking input with
    /// climbing input.
    fn hang(
        &mut self,
        h: HangInput,
        holds: &Query<(&GlobalTransform, &Handhold)>,
        cur_speed: &mut f32,
        cur_drag: &mut f32,
    ) -> Vec3 {
        if self.state != PlayerState::Hanging {
            return Vec3::ZERO;
        }
        let up = Vec3::Y;
        *cur_speed = SPEED;

        if !self.is_grounded {
            self.climb_dir = if self.has_hit { up_along_surface(self.hit_normal) } else { up };
            if self.has_hit && angle_deg(-up, self.hit_normal) < 10.0 {
                // Under a ceiling: sag slightly instead of climbing.
                self.climb_dir = -up * 0.15;
            } else if self.climb_dir.distance_squared(up) > 1e-6 {
                self.climb_dir += self.hit_normal * 0.1;
            }
        } else {
            self.climb_dir = up;
        }
        let climb_dir = self.climb_dir;

        let forward_axis = if h.wall_ahead {
            h.cam_forward.lerp(-climb_dir, 0.3).normalize_or_zero()
        } else {
            h.cam_forward + climb_dir
        };
        self.move_axis = h.right * h.input.x * 0.5 + forward_axis * h.input.y;

        let mut pull = Vec3::ZERO;
        let mut climb = 0.0;
        let mut speed_mult = 0.0;
        let look_up = h.cam_forward.dot(up);

        for i in 0..self.hands.len() {
            let Some(hold) = self.hands[i].hold else { continue };
            let Ok((hold_transform, handhold)) = holds.get(hold.entity) else {
                self.hands[i].release();
                continue;
            };

            let mut arm = hold_transform.transform_point(hold.local) - h.pos;
            let length = arm.length();
            arm = if length <= HANG_REST_LENGTH {
                Vec3::ZERO
            } else {
                arm / length * (length - HANG_REST_LENGTH)
            };
            if length > INTERACT_DISTANCE * 1.25 {
                arm *= 4.0;
                if length > INTERACT_DISTANCE * 2.0 {
                    self.hands[i].release();
                }
            }
            pull += arm;
            if self.hit_normal != Vec3::ZERO {
                pull += self.hit_normal * WALL_PUSH;
            }

            // Forward pulls up; looking down flips it so forward lowers you.
            let drive = if look_up >= 0.0 {
                h.input.y
            } else {
                h.input.y.lerp(-h.input.y, look_up.abs())
            };
            climb = (climb + drive * CLIMB_ACCEL).clamp(0.0, CLIMB_MAX) * handhold.climb_mult;

            // Holding on bleeds off vertical speed, harder the longer the grip.
            let catch = h.dt * 8.0 * self.hands[i].hold_time.clamp(0.0, 2.0) * handhold.hang_vel_adjust;
            self.vel.x += climb_dir.x * climb * h.dt;
            self.vel.y = self.vel.y.lerp(climb * climb_dir.y, catch.clamp(0.0, 1.0));
            self.vel.z += climb_dir.z * climb * h.dt;
            self.vel.y += handhold.climb_grav * h.dt;

            *cur_drag = cur_drag.max(DRAG * self.hands[i].drag_mult);
            speed_mult = handhold.speed_mult;

            if self.move_axis.length() < 0.05 && h.bracing {
                self.vel.y = 0.0;
                self.vel = self.vel.lerp(Vec3::ZERO, (h.dt * 5.0).min(1.0));
            }
        }

        *cur_speed *= speed_mult;
        pull
    }
}

pub fn spawn_player(commands: &mut Commands, pos: Vec3, yaw: f32) {
    commands.spawn((
        Player::new(pos, yaw),
        Transform::from_translation(pos),
        Collider::capsule_y(CAPSULE_HEIGHT * 0.5 - CAPSULE_RADIUS, CAPSULE_RADIUS),
        KinematicCharacterController {
            offset: CharacterLength::Absolute(0.02),
            max_slope_climb_angle: SLOPE_LIMIT_DEG.to_radians(),
            min_slope_slide_angle: SLOPE_LIMIT_DEG.to_radians(),
            autostep: Some(CharacterAutostep {
                max_height: CharacterLength::Absolute(0.3),
                min_width: CharacterLength::Absolute(0.2),
                include_dynamic_bodies: false,
            }),
            snap_to_ground: None,
            apply_impulse_to_dynamic_bodies: false,
            // Handholds are grabbed, not stood on.
            filter_groups: Some(CollisionGroups::new(Group::ALL, WORLD_GROUP)),
            ..default()
        },
    ));
}

pub fn player_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    rapier: ReadRapierContext,
    mut players: Query<(
        Entity,
        &Transform,
        &mut Player,
        &mut KinematicCharacterController,
        Option<&KinematicCharacterControllerOutput>,
    )>,
    holds: Query<(&GlobalTransform, &Handhold)>,
) {
    let Ok((entity, transform, mut player, mut controller, output)) = players.single_mut() else {
        return;
    };
    let Ok(rapier) = rapier.single() else {
        return;
    };
    let p = &mut *player;
    let dt = time.delta_secs();
    let up = Vec3::Y;
    let pos = transform.translation;
    let world_only = QueryFilter::default()
        .exclude_collider(entity)
        .groups(CollisionGroups::new(Group::ALL, WORLD_GROUP));

    p.prev_pos = p.cur_pos;
    p.cur_pos = pos;

    // Results of the move requested last tick.
    let mut controller_grounded = false;
    if let Some(output) = output {
        controller_grounded = output.grounded;
        for collision in &output.collisions {
            if let Some(details) = collision.hit.details {
                let normal = details.normal1.normalize_or_zero();
                if normal != Vec3::ZERO {
                    p.on_surface_hit(normal);
                }
            }
        }
        let moved = output.effective_translation;
        p.speed_mps = moved.length() / dt;
        if p.clamp_to_last_move {
            p.vel = p.vel.clamp(-moved.abs(), moved.abs());
        }
    }
    if p.hit_time > 0.0 {
        p.hit_time -= dt;
    } else {
        p.hit_normal = Vec3::ZERO;
        p.has_hit = false;
        p.climb_dir = up;
        p.jump_dir = up;
    }
    for hand in &mut p.hands {
        hand.regen_wait = (hand.regen_wait - dt).max(0.0);
    }

    // Walking off an edge should not carry the downward push from the ground.
    if p.is_grounded && !controller_grounded && p.vel.y < 0.0 && !p.is_sliding {
        p.vel.y = 0.0;
    }
    p.is_grounded = controller_grounded;

    let mut input = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        input.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        input.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        input.x += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        input.x -= 1.0;
    }
    let sprinting = keys.pressed(KeyCode::ShiftLeft);
    let bracing = keys.pressed(KeyCode::ControlLeft);

    let yaw_rotation = Quat::from_rotation_y(p.yaw);
    let flat_forward = yaw_rotation * Vec3::NEG_Z;
    let mut forward = flat_forward;
    let mut right = yaw_rotation * Vec3::X;
    if p.is_grounded {
        // Walk along the ground rather than into it.
        if let Some((_, hit)) =
            rapier.cast_ray_and_get_normal(pos, -up, GROUND_PROBE, true, world_only)
        {
            if hit.normal.dot(up) > 0.4 {
                forward = project_on_plane(forward, hit.normal).normalize_or_zero();
                right = project_on_plane(right, hit.normal).normalize_or_zero();
            }
        }
    }

    // `state` is still last tick's here, as in the original.
    let wish = if p.state == PlayerState::Hanging { Vec3::ZERO } else { forward * input.y }
        + right * input.x;
    p.move_axis = p.move_axis.lerp(wish, (dt * 100.0).min(1.0)).clamp_length_max(1.0);
    p.jump_dir = up;

    let mut cur_speed = SPEED;
    let mut cur_drag = DRAG;
    let hanging = p.is_hanging();
    p.state = if hanging { PlayerState::Hanging } else { PlayerState::Airborne };

    if !controller_grounded && !hanging && p.hit_time <= 0.0 {
        p.hit_normal = up;
    }

    let slope = angle_deg(up, p.hit_normal);
    p.is_sliding = (SLOPE_LIMIT_DEG..=SLOPE_MAX_DEG).contains(&slope);
    if p.is_sliding {
        p.vel += p.hit_normal * dt * SLIDE_SPEED * (slope / 90.0) * dt;
        cur_speed *= 0.5;
        if slope > 20.0 && slope < 90.0 {
            p.jump_dir = p.hit_normal.lerp(up, 0.9).normalize_or_zero();
        }
    }

    if p.is_grounded {
        if !hanging {
            p.state = PlayerState::Grounded;
        }
        p.time_since_grounded = 0.0;
    } else {
        p.time_since_grounded += dt;
    }

    p.vel += up * GRAVITY * dt;

    if p.is_grounded {
        if !p.is_sliding {
            p.vel.y = p.vel.y.max(-0.05);
            cur_drag *= if p.move_axis.length() > f32::EPSILON {
                GROUND_MOVE_DRAG_MULT
            } else {
                GROUND_FRICTION_MULT
            };
        }
    } else {
        cur_speed *= AIR_CONTROL;
        if hanging && p.hit_time > 0.0 {
            // Pressed against the wall: shift slides, ctrl clamps on.
            if sprinting {
                cur_drag *= WALL_FRICTION_MULT;
            } else if bracing {
                cur_drag *= WALL_FRICTION_MULT * 30.0;
                p.vel = p.vel.lerp(Vec3::ZERO, (dt * 5.0).min(1.0));
            } else {
                cur_drag *= WALL_FRICTION_MULT * 3.0;
            }
        }
    }

    if sprinting && p.grounded_or_coyote() && !p.is_sliding {
        if p.is_grounded {
            cur_speed = SPRINT_SPEED;
        }
    } else if bracing && !p.is_sliding {
        cur_speed = CROUCH_SPEED;
    }

    let eye = pos + up * EYE_OFFSET;
    let cam_forward = p.look_rotation() * Vec3::NEG_Z;
    let wall_ahead = hanging
        && rapier
            .cast_ray(eye, flat_forward, WALL_AHEAD_DISTANCE, true, world_only)
            .is_some();
    let pull = p.hang(
        HangInput { pos, input, right, cam_forward, wall_ahead, bracing, dt },
        &holds,
        &mut cur_speed,
        &mut cur_drag,
    );

    p.vel += (p.move_axis * cur_speed + pull * 1.5) * dt * ACCEL_SCALE;

    // Quadratic drag, never removing more than half the speed in one tick.
    let speed = p.vel.length();
    let drag = -p.vel.normalize_or_zero() * cur_drag * speed * speed * dt;
    p.vel += drag.clamp_length_max(speed * 0.5);

    if p.time_since_grounded < COYOTE_TIME || p.state == PlayerState::Hanging {
        if keys.pressed(KeyCode::Space) {
            if !p.has_jumped {
                p.jump(&holds);
            }
        } else {
            p.has_jumped = false;
        }
    }

    // Against a near-vertical surface, slide along it instead of pushing in.
    let vertical_slide = p.hit_time >= 0.0
        && p.vel.normalize_or_zero().dot(p.hit_normal) <= 0.0
        && slope >= 85.0;
    if vertical_slide {
        p.vel = project_on_plane(p.vel, p.hit_normal);
    }

    controller.translation = Some(p.vel * dt * VEL_SCALE);
    p.clamp_to_last_move = !vertical_slide && p.is_grounded;
}

/// The direction along a surface that points most upward.
fn up_along_surface(normal: Vec3) -> Vec3 {
    project_on_plane(Vec3::Y, normal.normalize_or_zero()).normalize_or_zero()
}

fn project_on_plane(v: Vec3, normal: Vec3) -> Vec3 {
    let len_sq = normal.length_squared();
    if len_sq < 1e-12 { v } else { v - normal * (v.dot(normal) / len_sq) }
}

/// Angle between two vectors in degrees; zero if either has no length.
fn angle_deg(a: Vec3, b: Vec3) -> f32 {
    let denom = (a.length_squared() * b.length_squared()).sqrt();
    if denom < 1e-12 {
        0.0
    } else {
        (a.dot(b) / denom).clamp(-1.0, 1.0).acos().to_degrees()
    }
}
