//! Climbing sounds: grabbing, letting go, jumping, landing and the fatigue loop.
//!
//! The clips are the game's own and are not part of this repository; each one
//! is used only if it has been extracted into `assets/original/audio`, so the
//! game simply runs silent without them. Volumes and pitch ranges are the
//! ones the original's player is configured with.

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::player::Player;
use crate::selftest;
use crate::tuning::*;

const AUDIO_DIR: &str = "original/audio";
/// Overall level on top of the original's per-sound volumes (our own value).
const MASTER_GAIN: f32 = 2.0;

/// Downward speeds (m/s) that separate soft, medium and heavy landings.
const SOFT_LANDING: f32 = 1.3;
const MEDIUM_LANDING: f32 = 5.0;
const HEAVY_LANDING: f32 = 20.0;
/// Time in the air before touching down counts as a landing.
const LANDING_AIR_TIME: f32 = 0.2;
const FATIGUE_MAX_VOLUME: f32 = 0.35;

/// A group of interchangeable clips with the volume and pitch range to play them at.
#[derive(Default)]
struct SoundSet {
    clips: Vec<Handle<AudioSource>>,
    volume: f32,
    pitch: (f32, f32),
}

#[derive(Resource, Default)]
pub struct Sounds {
    grab: SoundSet,
    let_go: SoundSet,
    grip_lost: SoundSet,
    jump: SoundSet,
    mantle: SoundSet,
    land_soft: SoundSet,
    land_medium: SoundSet,
    land_heavy: SoundSet,
}

#[derive(Component)]
pub struct FatigueLoop;

/// What the sound system remembers about the previous frame.
pub struct Tracker {
    hanging: [bool; 2],
    jump_count: u32,
    was_grounded: bool,
    air_time: f32,
    fall_speed: f32,
    fatigue_volume: f32,
    rng: u32,
}

impl Default for Tracker {
    fn default() -> Self {
        Self {
            hanging: [false; 2],
            jump_count: 0,
            // Start as if standing, so spawning in does not count as a landing.
            was_grounded: true,
            air_time: 0.0,
            fall_speed: 0.0,
            fatigue_volume: 0.0,
            rng: 0x9E37_79B9,
        }
    }
}

impl Tracker {
    /// Uniform value in `0.0..1.0`.
    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }
}

fn asset_path(name: &str) -> String {
    format!("{AUDIO_DIR}/{name}.wav")
}

fn asset_exists(path: &str) -> bool {
    // Same root Bevy resolves asset paths against under `cargo run`.
    let root = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    root.join("assets").join(path).is_file()
}

pub fn setup_audio(mut commands: Commands, asset_server: Res<AssetServer>) {
    let set = |names: &[&str], volume: f32, pitch: (f32, f32)| SoundSet {
        clips: names
            .iter()
            .map(|name| asset_path(name))
            .filter(|path| asset_exists(path))
            .map(|path| asset_server.load(path))
            .collect(),
        volume,
        pitch,
    };
    commands.insert_resource(Sounds {
        grab: set(&["climb1", "climb2", "climb3", "climb4"], 0.1, (1.0, 1.1)),
        let_go: set(&["letgo1", "letgo2", "letgo3"], 0.2, (0.99, 1.1)),
        grip_lost: set(&["player_climbing_griplost"], 0.65, (0.95, 1.05)),
        jump: set(
            &["player_fem_jump1", "player_fem_jump2", "player_fem_jump3"],
            0.2,
            (0.95, 1.03),
        ),
        mantle: set(&["mantle1"], 0.2, (1.0, 1.0)),
        land_soft: set(&["landsoft2_new"], 0.25, (0.96, 1.03)),
        land_medium: set(&["land_medium1", "land_medium2", "land_medium3"], 0.25, (0.95, 1.0)),
        land_heavy: set(
            &["player_fem_land_heavy_01", "player_fem_land_heavy_02"],
            0.5,
            (0.95, 1.0),
        ),
    });

    let fatigue = asset_path("hand_fatigue_new");
    if asset_exists(&fatigue) {
        commands.spawn((
            FatigueLoop,
            AudioPlayer::new(asset_server.load(fatigue)),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        ));
    }
}

pub fn play_sounds(
    mut commands: Commands,
    time: Res<Time>,
    sounds: Res<Sounds>,
    players: Query<&Player>,
    mut fatigue_loops: Query<&mut AudioSink, With<FatigueLoop>>,
    mut tracker: Local<Tracker>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    let dt = time.delta_secs();
    let tracker = &mut *tracker;
    let mut play = |tracker: &mut Tracker, label: &str, set: &SoundSet| {
        if selftest::enabled() {
            info!("sound: {label}");
        }
        if set.clips.is_empty() {
            return;
        }
        let pick = (tracker.random() * set.clips.len() as f32) as usize;
        let speed = set.pitch.0 + (set.pitch.1 - set.pitch.0) * tracker.random();
        commands.spawn((
            AudioPlayer::new(set.clips[pick.min(set.clips.len() - 1)].clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(set.volume * MASTER_GAIN))
                .with_speed(speed),
        ));
    };

    for (index, hand) in player.hands.iter().enumerate() {
        let hanging = hand.is_hanging();
        if hanging && !tracker.hanging[index] {
            play(tracker, "grab", &sounds.grab);
        } else if !hanging && tracker.hanging[index] {
            // An exhausted hand is locked out for longer than one that let go.
            if hand.grab_wait > EXHAUSTED_GRAB_WAIT * 0.5 {
                play(tracker, "grip lost", &sounds.grip_lost);
            }
            play(tracker, "let go", &sounds.let_go);
        }
        tracker.hanging[index] = hanging;
    }

    if player.jump_count > tracker.jump_count {
        play(tracker, "jump", &sounds.jump);
    }
    tracker.jump_count = player.jump_count;

    if player.is_grounded {
        if !tracker.was_grounded && tracker.air_time > LANDING_AIR_TIME {
            if player.is_hanging() {
                play(tracker, "mantle", &sounds.mantle);
            } else if tracker.fall_speed > HEAVY_LANDING {
                play(tracker, "land heavy", &sounds.land_heavy);
            } else if tracker.fall_speed > MEDIUM_LANDING {
                play(tracker, "land medium", &sounds.land_medium);
            } else if tracker.fall_speed > SOFT_LANDING {
                play(tracker, "land soft", &sounds.land_soft);
            }
        }
        tracker.air_time = 0.0;
    } else {
        tracker.air_time += dt;
        tracker.fall_speed = -player.vel.y * VEL_SCALE;
    }
    tracker.was_grounded = player.is_grounded;

    // The strain loop fades in over the second half of the weakest hand's grip.
    let target = if player.is_hanging() {
        let weakest = player.hands.iter().map(|hand| hand.grip).fold(GRIP_MAX, f32::min);
        ((1.0 - weakest / GRIP_MAX - 0.5) * 2.0).clamp(0.0, 1.0) * FATIGUE_MAX_VOLUME
    } else {
        0.0
    };
    tracker.fatigue_volume += (target - tracker.fatigue_volume) * (dt * 3.0).min(1.0);
    if let Ok(mut sink) = fatigue_loops.single_mut() {
        sink.set_volume(Volume::Linear(tracker.fatigue_volume * MASTER_GAIN));
    }
}
