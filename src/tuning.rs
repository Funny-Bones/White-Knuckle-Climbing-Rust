//! Tuning values for the climbing controller.
//!
//! These are the values the original game's player object is configured with
//! (read from its serialized scene data, not the script defaults), so the
//! numbers only make sense together with the original's odd unit system:
//! velocity is stored in small "velocity units" and multiplied by
//! [`VEL_SCALE`] when it is turned into movement.

/// Physics tick rate of the original (fixed timestep of 0.02 s).
pub const FIXED_HZ: f64 = 50.0;

/// Metres moved per second for one unit of stored velocity.
pub const VEL_SCALE: f32 = 175.0;
/// Scale applied to every acceleration before it reaches the velocity.
pub const ACCEL_SCALE: f32 = 0.15;

pub const SPEED: f32 = 0.85;
pub const SPRINT_SPEED: f32 = 2.5;
pub const CROUCH_SPEED: f32 = 0.3;
pub const AIR_CONTROL: f32 = 0.3;
/// Jump impulse is `JUMP_HEIGHT * -GRAVITY`.
pub const JUMP_HEIGHT: f32 = 0.4;
pub const GRAVITY: f32 = -0.2;
pub const COYOTE_TIME: f32 = 0.2;

/// Quadratic drag coefficient while airborne.
pub const DRAG: f32 = 8.0;
/// Drag multiplier on the ground while moving.
pub const GROUND_MOVE_DRAG_MULT: f32 = 15.0;
/// Drag multiplier on the ground with no input.
pub const GROUND_FRICTION_MULT: f32 = 100.0;
/// Drag multiplier while hanging and touching a wall (x3 normally, x1 when
/// sprinting, x30 when bracing).
pub const WALL_FRICTION_MULT: f32 = 10.0;

pub const SLOPE_LIMIT_DEG: f32 = 55.0;
pub const SLOPE_MAX_DEG: f32 = 89.0;
pub const SLIDE_SPEED: f32 = 0.5;
/// How long a surface contact keeps counting as "touching".
pub const HIT_MEMORY: f32 = 0.1;

/// Reach of a hand, measured from the camera.
pub const INTERACT_DISTANCE: f32 = 2.5;
/// How long a hold stays grabbable after the crosshair leaves it.
pub const GRAB_FORGIVE: f32 = 0.2;
/// Seconds of grip per hand when hanging from that hand alone.
pub const GRIP_MAX: f32 = 10.0;
/// Grip spent by each hand that lets go for a jump.
pub const CLIMB_JUMP_DRAIN: f32 = 1.0;
/// Jump strain multiplier when both hands were holding on.
pub const TWO_HAND_JUMP_STRAIN: f32 = 0.48;
/// Grip loss rate when both hands are holding on (1.0 with a single hand).
pub const TWO_HAND_LOSS_RATE: f32 = 0.5;
/// A hand lets go when its grip falls to this.
pub const GRIP_EXHAUSTED: f32 = 0.1;
/// Lockout before an exhausted hand can grab again.
pub const EXHAUSTED_GRAB_WAIT: f32 = 1.0;
/// Lockout added every time a hand lets go.
pub const RELEASE_GRAB_WAIT: f32 = 0.25;
/// Delay before a free hand starts recovering grip in the air.
pub const REGEN_WAIT: f32 = 0.17;
pub const REGEN_GROUNDED: f32 = 0.9;
pub const REGEN_AIRBORNE: f32 = 0.75;

/// Distance from the body centre to a held point before the arm starts pulling.
pub const HANG_REST_LENGTH: f32 = 1.2;
/// Target climb velocity gained per hand, and its cap.
pub const CLIMB_ACCEL: f32 = 0.015;
pub const CLIMB_MAX: f32 = 0.02;
/// Push away from the surface being climbed, per hanging hand.
pub const WALL_PUSH: f32 = 0.09;
/// Distance in front of the camera checked for a wall while hanging.
pub const WALL_AHEAD_DISTANCE: f32 = 0.75;

// Body dimensions (the original is a 1.5 m tall, 0.3 m radius capsule scaled by 1.5).
pub const CAPSULE_RADIUS: f32 = 0.45;
pub const CAPSULE_HEIGHT: f32 = 2.25;
/// Camera height above the body centre.
pub const EYE_OFFSET: f32 = 0.675;
/// How far the body is pulled down to stay on the ground while walking.
pub const GROUND_SNAP: f32 = 0.15;
/// Length of the ray used to find the ground under the body centre.
pub const GROUND_PROBE: f32 = 1.5;

pub const FOV_DEG: f32 = 90.0;
/// The original lets the view tip slightly past straight up.
pub const PITCH_UP_MAX_DEG: f32 = 105.0;
pub const PITCH_DOWN_MAX_DEG: f32 = 90.0;
/// Radians of view rotation per pixel of mouse movement (our own value).
pub const MOUSE_SENSITIVITY: f32 = 0.0022;
