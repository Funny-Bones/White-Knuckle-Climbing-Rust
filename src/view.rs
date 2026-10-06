//! Everything the player sees: camera, hands, aim marker and HUD.

use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::hands::AimTarget;
use crate::level::{SPAWN_POS, SPAWN_YAW};
use crate::player::{Player, PlayerState};
use crate::tuning::*;

#[derive(Component)]
pub struct PlayerCamera;

/// Mouse look and hand buttons only work while the cursor is captured.
#[derive(Resource, Default)]
pub struct CursorLocked(pub bool);

#[derive(Component)]
pub struct HandVisual {
    index: usize,
    /// Position in camera space, so a free hand follows the view exactly.
    local: Vec3,
    /// Sideways tilt, eased towards whatever the current motion asks for.
    roll: f32,
    /// How long the player has been sprinting; the run pose waits a moment.
    sprint_time: f32,
}

#[derive(Component)]
pub struct AimMarker;

#[derive(Component)]
pub struct Crosshair;

#[derive(Component)]
pub struct GripBar(usize);

#[derive(Component)]
pub struct StatusText;

/// The game's own hand art, when it has been extracted into
/// `assets/original` (it is not part of this repository). Without it the
/// hands are drawn as plain boxes.
#[derive(Resource, Default)]
pub struct HandSprites(Option<HandPoses>);

struct HandPoses {
    normal: Handle<Image>,
    open: Handle<Image>,
    grab: Handle<Image>,
    crouch: Option<Handle<Image>>,
    sprint: Option<Handle<Image>>,
}

/// Hands live on their own layer, drawn over the world so that walls and
/// holds never hide them.
const HAND_LAYER: usize = 1;

const SPRITE_DIR: &str = "original";
/// World size of a hand sprite. Together with the rest position this puts the
/// hands large in the bottom corners of the view, wrists off-screen.
const SPRITE_SIZE: f32 = 0.38;
const HAND_BUTTONS: [MouseButton; 2] = [MouseButton::Left, MouseButton::Right];
/// Delay before sprinting switches the hands to the run pose.
const RUN_POSE_DELAY: f32 = 0.3;

const SKIN: Vec3 = Vec3::new(0.86, 0.70, 0.60);
const STRAINED: Vec3 = Vec3::new(0.90, 0.12, 0.10);

fn hand_rest(index: usize, sprites: bool) -> Vec3 {
    let side = if index == 0 { -1.0 } else { 1.0 };
    if sprites {
        Vec3::new(side * 0.41, -0.44, -0.5)
    } else {
        Vec3::new(side * 0.34, -0.30, -0.62)
    }
}

fn load_hand_poses(asset_server: &AssetServer) -> Option<HandPoses> {
    // Same root Bevy resolves asset paths against under `cargo run`.
    let root = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let file = |pose: &str| format!("{SPRITE_DIR}/hand_background_{pose}.png");
    let poses = ["normal", "open", "grab"];
    if !poses.iter().all(|pose| root.join("assets").join(file(pose)).is_file()) {
        return None;
    }
    let optional = |pose: &str| {
        root.join("assets").join(file(pose)).is_file().then(|| asset_server.load(file(pose)))
    };
    Some(HandPoses {
        normal: asset_server.load(file("normal")),
        open: asset_server.load(file("open")),
        grab: asset_server.load(file("grab")),
        crouch: optional("crouch"),
        sprint: optional("sprint"),
    })
}

pub fn setup_view(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((
        PlayerCamera,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: FOV_DEG.to_radians(),
            near: 0.05,
            ..default()
        }),
        Transform::from_translation(SPAWN_POS + Vec3::Y * EYE_OFFSET),
        children![(
            Camera3d::default(),
            Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
            Projection::Perspective(PerspectiveProjection {
                fov: FOV_DEG.to_radians(),
                near: 0.05,
                ..default()
            }),
            RenderLayers::layer(HAND_LAYER),
            // The sprites are flat white pixel art; leave their colours alone.
            Tonemapping::None,
        )],
    ));

    let poses = load_hand_poses(&asset_server);
    let hand_mesh = match poses {
        Some(_) => meshes.add(Rectangle::new(SPRITE_SIZE, SPRITE_SIZE)),
        None => meshes.add(Cuboid::new(0.13, 0.17, 0.07)),
    };
    for index in 0..2 {
        let material = match &poses {
            Some(poses) => StandardMaterial {
                base_color_texture: Some(poses.normal.clone()),
                unlit: true,
                alpha_mode: AlphaMode::Mask(0.5),
                // The right hand is the left one mirrored, which flips its facing.
                cull_mode: None,
                double_sided: true,
                ..default()
            },
            None => StandardMaterial {
                base_color: Color::srgb(SKIN.x, SKIN.y, SKIN.z),
                unlit: true,
                ..default()
            },
        };
        commands.spawn((
            HandVisual {
                index,
                local: hand_rest(index, poses.is_some()),
                roll: 0.0,
                sprint_time: 0.0,
            },
            Mesh3d(hand_mesh.clone()),
            MeshMaterial3d(materials.add(material)),
            Transform::default(),
            RenderLayers::layer(HAND_LAYER),
        ));
    }
    commands.insert_resource(HandSprites(poses));

    commands.spawn((
        AimMarker,
        Mesh3d(meshes.add(Sphere::new(0.025))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 1.0, 0.6),
            unlit: true,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));

    commands.spawn((
        Crosshair,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            width: Val::Px(4.0),
            height: Val::Px(4.0),
            margin: UiRect { left: Val::Px(-2.0), top: Val::Px(-2.0), ..default() },
            ..default()
        },
        BackgroundColor(Color::WHITE),
    ));

    // One grip bar per hand, at the bottom corners clear of the hands.
    for index in 0..2 {
        let offset = Val::Px(24.0);
        let (left, right) = if index == 0 { (offset, Val::Auto) } else { (Val::Auto, offset) };
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(28.0),
                    left,
                    right,
                    width: Val::Px(14.0),
                    height: Val::Px(150.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexEnd,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    GripBar(index),
                    Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(Color::WHITE),
                ));
            });
    }

    commands.spawn((
        Text::new(
            "Click to capture the mouse, Esc to release it\n\
             Left / right mouse: hold to grab with that hand\n\
             W: pull up   A/D: shift sideways   S: lower\n\
             Space: jump (lets go)   Shift: slide on the wall   Ctrl: brace\n\
             R: back to the start",
        ),
        TextFont::from_font_size(15.0),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
    commands.spawn((
        StatusText,
        Text::new(""),
        TextFont::from_font_size(15.0),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(12.0),
            ..default()
        },
    ));
}

pub fn cursor_grab(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut locked: ResMut<CursorLocked>,
) {
    let Ok(mut cursor) = windows.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        locked.0 = false;
    } else if !locked.0 && mouse.just_pressed(MouseButton::Left) {
        locked.0 = true;
    } else {
        return;
    }
    cursor.grab_mode = if locked.0 { CursorGrabMode::Locked } else { CursorGrabMode::None };
    cursor.visible = !locked.0;
}

pub fn mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    locked: Res<CursorLocked>,
    mut players: Query<&mut Player>,
) {
    let Ok(mut player) = players.single_mut() else {
        return;
    };
    if !locked.0 || motion.delta == Vec2::ZERO {
        return;
    }
    player.yaw -= motion.delta.x * MOUSE_SENSITIVITY;
    player.pitch = (player.pitch - motion.delta.y * MOUSE_SENSITIVITY)
        .clamp(-PITCH_DOWN_MAX_DEG.to_radians(), PITCH_UP_MAX_DEG.to_radians());
}

pub fn respawn(keys: Res<ButtonInput<KeyCode>>, mut players: Query<(&mut Transform, &mut Player)>) {
    let Ok((mut transform, mut player)) = players.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::KeyR) || transform.translation.y < -30.0 {
        transform.translation = SPAWN_POS;
        player.teleport(SPAWN_POS, SPAWN_YAW);
    }
}

/// The body only moves on physics ticks; the camera glides between them.
pub fn camera_follow(
    fixed_time: Res<Time<Fixed>>,
    players: Query<&Player>,
    mut cameras: Query<&mut Transform, With<PlayerCamera>>,
) {
    let (Ok(player), Ok(mut camera)) = (players.single(), cameras.single_mut()) else {
        return;
    };
    let body = player.prev_pos.lerp(player.cur_pos, fixed_time.overstep_fraction());
    camera.translation = body + Vec3::Y * EYE_OFFSET;
    camera.rotation = player.look_rotation();
}

pub fn update_hand_visuals(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    locked: Res<CursorLocked>,
    players: Query<&Player>,
    cameras: Query<&Transform, (With<PlayerCamera>, Without<HandVisual>)>,
    holds: Query<&GlobalTransform>,
    sprites: Res<HandSprites>,
    mut hands: Query<(&mut Transform, &mut HandVisual, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Ok(player), Ok(camera)) = (players.single(), cameras.single()) else {
        return;
    };
    let poses = sprites.0.as_ref();
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let speed = player.speed_mps.min(8.0);
    let walking = player.is_grounded && speed > 0.1;
    let sprinting = keys.pressed(KeyCode::ShiftLeft) && player.is_grounded && speed > 0.3;
    let crouching = keys.pressed(KeyCode::ControlLeft);

    for (mut transform, mut visual, material) in &mut hands {
        let hand = &player.hands[visual.index];
        let side = if visual.index == 0 { -1.0 } else { 1.0 };
        let fatigue = 1.0 - hand.grip_fraction();
        let reaching = locked.0 && mouse.pressed(HAND_BUTTONS[visual.index]);
        visual.sprint_time = if sprinting { visual.sprint_time + dt } else { 0.0 };
        let running = visual.sprint_time > RUN_POSE_DELAY && !reaching;
        let held_point = hand
            .hold
            .and_then(|hold| Some(holds.get(hold.entity).ok()?.transform_point(hold.local)));

        let mut roll = 0.0;
        if let Some(point) = held_point {
            // A tiring hand trembles on its hold.
            let seed = visual.index as f32 * 1.7;
            let tremble = Vec3::new(
                (t * 61.0 + seed).sin(),
                (t * 47.0 + seed + 1.3).sin(),
                (t * 53.0 + seed + 2.1).sin(),
            ) * fatigue * 0.02;
            transform.translation = point + tremble;
            visual.local = camera.compute_affine().inverse().transform_point3(point);
        } else {
            let mut target = hand_rest(visual.index, poses.is_some());
            // The two hands move half a cycle apart.
            let offset = visual.index as f32 * std::f32::consts::PI;
            if reaching {
                // Held out towards whatever is ahead.
                target += Vec3::new(-side * 0.14, 0.2, -0.06);
            } else if running {
                // Arms pump: up and forward, then down and back.
                let phase = t * 11.0 + offset;
                target += Vec3::new(0.0, 0.05 + 0.08 * phase.sin(), 0.07 * phase.cos());
                roll = side * 0.18 * phase.sin();
            } else if walking {
                // Sway side to side, with a dip on each footfall.
                let phase = t * 3.5 + offset * 0.5;
                target += Vec3::new(
                    side * 0.004 * speed * phase.sin(),
                    0.006 * speed * (phase * 2.0).sin().abs(),
                    0.0,
                );
                roll = side * 0.012 * speed * phase.sin();
            } else if !player.is_grounded {
                target.y += 0.05;
            }
            visual.local = visual.local.lerp(target, (dt * 12.0).min(1.0));
            transform.translation = camera.transform_point(visual.local);
        }
        visual.roll += (roll - visual.roll) * (dt * 10.0).min(1.0);
        transform.rotation = camera.rotation * Quat::from_rotation_z(visual.roll);
        transform.scale = match poses {
            // The right hand is the left one mirrored.
            Some(_) if visual.index == 1 => Vec3::new(-1.0, 1.0, 1.0),
            Some(_) => Vec3::ONE,
            None if held_point.is_some() => Vec3::splat(0.85),
            None => Vec3::ONE,
        };

        let Some(mut material) = materials.get_mut(&material.0) else {
            continue;
        };
        match poses {
            Some(poses) => {
                // As in the original: fist on a hold, open while the button is
                // held, then crouch and run poses, relaxed otherwise.
                let pose = if held_point.is_some() {
                    &poses.grab
                } else if reaching {
                    &poses.open
                } else if crouching {
                    poses.crouch.as_ref().unwrap_or(&poses.normal)
                } else if running {
                    poses.sprint.as_ref().unwrap_or(&poses.normal)
                } else {
                    &poses.normal
                };
                if material.base_color_texture.as_ref() != Some(pose) {
                    material.base_color_texture = Some(pose.clone());
                }
            }
            None => {
                // Box hands have no poses, so colour carries the state instead.
                let mut color = SKIN.lerp(STRAINED, fatigue);
                if !hand.is_hanging() && hand.grab_wait > 0.0 {
                    color *= 0.45;
                }
                material.base_color = Color::srgb(color.x, color.y, color.z);
            }
        }
    }
}

pub fn update_hud(
    players: Query<&Player>,
    aim: Res<AimTarget>,
    mut markers: Query<(&mut Transform, &mut Visibility), With<AimMarker>>,
    mut crosshairs: Query<&mut BackgroundColor, (With<Crosshair>, Without<GripBar>)>,
    mut bars: Query<(&GripBar, &mut Node, &mut BackgroundColor)>,
    mut status: Query<&mut Text, With<StatusText>>,
) {
    let Ok(player) = players.single() else {
        return;
    };

    if let Ok((mut transform, mut visibility)) = markers.single_mut() {
        match aim.0 {
            Some((_, point)) => {
                transform.translation = point;
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
    if let Ok(mut crosshair) = crosshairs.single_mut() {
        crosshair.0 = if aim.0.is_some() { Color::srgb(1.0, 0.8, 0.2) } else { Color::WHITE };
    }

    for (bar, mut node, mut color) in &mut bars {
        let hand = &player.hands[bar.0];
        let fraction = hand.grip_fraction();
        node.height = Val::Percent(fraction * 100.0);
        let tint = Vec3::ONE.lerp(STRAINED, 1.0 - fraction);
        let alpha = if hand.grab_wait > 0.0 && !hand.is_hanging() { 0.35 } else { 0.9 };
        color.0 = Color::srgba(tint.x, tint.y, tint.z, alpha);
    }

    if let Ok(mut text) = status.single_mut() {
        let state = match player.state {
            PlayerState::Grounded => "grounded",
            PlayerState::Airborne => "airborne",
            PlayerState::Hanging => "hanging",
        };
        let feet = player.cur_pos.y - CAPSULE_HEIGHT * 0.5;
        text.0 = format!("height {feet:5.1} m\n{state}");
    }
}
