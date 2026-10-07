//! Greybox test park: the original climbing wall plus a spread of structures
//! that each exercise a different part of the climbing.

use std::f32::consts::{PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{CascadeShadowConfigBuilder, Skybox};
use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use bevy_rapier3d::prelude::*;

/// Solid geometry the body collides with.
pub const WORLD_GROUP: Group = Group::GROUP_1;
/// Handholds: hands can find them, the body passes through them.
pub const HANDHOLD_GROUP: Group = Group::GROUP_2;

pub const SPAWN_POS: Vec3 = Vec3::new(0.0, 1.3, 3.0);
pub const SPAWN_YAW: f32 = 0.0;

/// Z of the main climbing wall's face.
const WALL_FACE_Z: f32 = -2.0;
/// Width, height and depth of a handhold, depth being how far it sticks out.
const HOLD_SIZE: Vec3 = Vec3::new(0.5, 0.14, 0.3);
/// Height of the lowest hold on most structures: reachable from the ground.
const FIRST_HOLD: f32 = 2.3;
/// Comfortable vertical spacing between holds.
const HOLD_STEP: f32 = 1.3;

/// Something a hand can hang from. Defaults match the original's handholds.
#[derive(Component, Clone, Debug)]
pub struct Handhold {
    /// Multiplies the body's drag while this is held.
    pub drag_mult: f32,
    pub climb_mult: f32,
    /// Extra vertical acceleration while held.
    pub climb_grav: f32,
    /// Multiplies how fast holding on drains grip.
    pub strain_rate: f32,
    pub jump_mult: f32,
    pub jump_strain_mult: f32,
    pub speed_mult: f32,
    /// How strongly holding on cancels vertical speed.
    pub hang_vel_adjust: f32,
    pub drop_on_jump: bool,
    pub allow_airborne_jump: bool,
}

impl Default for Handhold {
    fn default() -> Self {
        Self::STANDARD
    }
}

/// Everything a hand might be holding: handholds, and plain geometry whose
/// ledge has been grabbed (which has no `Handhold` of its own).
pub type Holds<'w, 's> = Query<'w, 's, (&'static GlobalTransform, Option<&'static Handhold>)>;

impl Handhold {
    pub const STANDARD: Handhold = Handhold {
        drag_mult: 10.0,
        climb_mult: 1.0,
        climb_grav: 0.0,
        strain_rate: 1.0,
        jump_mult: 1.0,
        jump_strain_mult: 1.0,
        speed_mult: 1.0,
        hang_vel_adjust: 1.0,
        drop_on_jump: true,
        allow_airborne_jump: true,
    };

    /// The tuning for a grabbed point: its own if it is a handhold, the
    /// standard values if it is a ledge.
    pub fn or_ledge(handhold: Option<&Handhold>) -> &Handhold {
        handhold.unwrap_or(&Self::STANDARD)
    }

    /// Burns grip two and a half times as fast.
    fn slick() -> Self {
        Self { strain_rate: 2.5, ..default() }
    }

    /// Launches a stronger jump.
    fn spring() -> Self {
        Self { jump_mult: 1.5, ..default() }
    }
}

struct Palette {
    ground: Handle<StandardMaterial>,
    wall: Handle<StandardMaterial>,
    stone: Handle<StandardMaterial>,
    ledge: Handle<StandardMaterial>,
    ramp: Handle<StandardMaterial>,
    hold: Handle<StandardMaterial>,
    slick: Handle<StandardMaterial>,
    spring: Handle<StandardMaterial>,
    goal: Handle<StandardMaterial>,
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
    palette: Palette,
    /// One unit cube shared by every block and hold, sized by its transform,
    /// so the renderer can draw them in batches.
    cube: Handle<Mesh>,
}

impl Builder<'_, '_, '_> {
    fn block(&mut self, material: &Handle<StandardMaterial>, center: Vec3, size: Vec3) {
        self.block_rotated(material, center, size, Quat::IDENTITY);
    }

    fn block_rotated(
        &mut self,
        material: &Handle<StandardMaterial>,
        center: Vec3,
        size: Vec3,
        rotation: Quat,
    ) {
        self.commands.spawn((
            Mesh3d(self.cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(center).with_rotation(rotation).with_scale(size),
            Collider::cuboid(0.5, 0.5, 0.5),
            CollisionGroups::new(WORLD_GROUP, Group::ALL),
        ));
    }

    /// A handhold box; its local Z is the direction it sticks out in.
    fn hold_box(&mut self, center: Vec3, size: Vec3, rotation: Quat, handhold: Handhold) {
        let material = if handhold.strain_rate > 1.0 {
            &self.palette.slick
        } else if handhold.jump_mult > 1.0 {
            &self.palette.spring
        } else {
            &self.palette.hold
        };
        self.commands.spawn((
            Mesh3d(self.cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(center).with_rotation(rotation).with_scale(size),
            Collider::cuboid(0.5, 0.5, 0.5),
            CollisionGroups::new(HANDHOLD_GROUP, Group::ALL),
            handhold,
        ));
    }

    /// A hold sticking out of a surface at `point`, along the surface `normal`.
    fn hold(&mut self, point: Vec3, normal: Vec3, handhold: Handhold) {
        let rotation = Quat::from_rotation_arc(Vec3::Z, normal);
        self.hold_box(point + normal * (HOLD_SIZE.z * 0.5 - 0.02), HOLD_SIZE, rotation, handhold);
    }

    /// A hold on the main wall.
    fn wall_hold(&mut self, x: f32, y: f32, handhold: Handhold) {
        self.hold(Vec3::new(x, y, WALL_FACE_Z), Vec3::Z, handhold);
    }

    /// A bar floating in the air, grabbable from any side.
    fn floating_hold(&mut self, center: Vec3, yaw: f32, handhold: Handhold) {
        self.hold_box(center, Vec3::new(0.9, 0.14, 0.3), Quat::from_rotation_y(yaw), handhold);
    }

    /// A straight column of holds up a vertical face.
    fn ladder(&mut self, base: Vec3, normal: Vec3, top: f32, step: f32, handhold: Handhold) {
        let mut y = FIRST_HOLD;
        while y <= top {
            self.hold(Vec3::new(base.x, y, base.z), normal, handhold.clone());
            y += step;
        }
    }
}

pub fn setup_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let materials = &mut *materials;
    let mut solid = |r: f32, g: f32, b: f32| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: 0.9,
            ..default()
        })
    };
    let palette = Palette {
        // Everything solid is white; slight tints keep neighbours apart.
        ground: solid(1.0, 1.0, 1.0),
        wall: solid(0.95, 0.95, 0.96),
        stone: solid(0.92, 0.91, 0.89),
        ledge: solid(0.84, 0.87, 0.92),
        ramp: solid(0.88, 0.92, 0.89),
        hold: solid(0.95, 0.72, 0.18),
        slick: solid(0.85, 0.22, 0.20),
        spring: solid(0.25, 0.80, 0.40),
        goal: solid(1.0, 0.9, 0.35),
    };
    let cube = meshes.add(Cuboid::from_length(1.0));
    let mut b = Builder { commands: &mut commands, meshes: &mut meshes, materials, palette, cube };

    // The ground is a white floor ruled into one metre squares.
    let ground_size = 160.0;
    let grid = images.add(grid_texture());
    let ground = b.materials.add(StandardMaterial {
        base_color_texture: Some(grid),
        perceptual_roughness: 0.9,
        uv_transform: Affine2::from_scale(Vec2::splat(ground_size / GRID_TILE_METRES)),
        ..default()
    });
    b.palette.ground = ground.clone();
    b.block(&ground, Vec3::new(0.0, -0.5, 0.0), Vec3::new(ground_size, 1.0, ground_size));

    main_wall(&mut b);
    chimney(&mut b);
    pillar_forest(&mut b);
    monkey_bars(&mut b);
    leaning_slab(&mut b);
    ramps(&mut b);
    spiral_islands(&mut b);
    ladder_spire(&mut b);
    arch(&mut b);
    stepping_blocks(&mut b);
    twin_towers(&mut b);
    traverse_wall(&mut b);
    hanging_garden(&mut b);
    great_cube(&mut b);

    lighting(&mut b);
    commands.insert_resource(Sky(images.add(sky_texture())));
}

/// A bright day: a sun that casts long shadows, sky-blue ambient light so
/// shadowed faces stay readable, and coloured lamps that mark the structures.
fn lighting(b: &mut Builder) {
    b.commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.98, 0.94),
            illuminance: 12_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            first_cascade_far_bound: 16.0,
            maximum_distance: 140.0,
            ..default()
        }
        .build(),
        Transform::from_translation(SUN_DIRECTION * 60.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    b.commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.72, 0.84, 1.0),
        brightness: 1100.0,
        ..default()
    });

    let warm = Color::srgb(1.0, 0.62, 0.28);
    let cool = Color::srgb(0.35, 0.75, 1.0);
    let green = Color::srgb(0.45, 1.0, 0.55);
    let red = Color::srgb(1.0, 0.3, 0.25);
    // Position, colour, and whether the lamp casts shadows. None do for now:
    // in daylight their shadows are invisible and each one costs six extra
    // passes over the whole level.
    let lamps = [
        (Vec3::new(-1.5, 27.4, 0.5), warm, false),    // under the main wall's roof
        (Vec3::new(5.2, 15.4, -0.6), cool, false),    // rest ledge
        (Vec3::new(13.75, 9.0, -4.0), warm, false),   // inside the chimney
        (Vec3::new(13.75, 18.5, -4.0), warm, false),
        (Vec3::new(0.0, 9.6, 12.0), cool, false),     // monkey bars
        (Vec3::new(25.6, 13.0, -12.4), cool, false),  // spire rest ledges
        (Vec3::new(25.6, 25.0, -12.4), cool, false),
        (Vec3::new(25.6, 37.0, -12.4), cool, false),
        (Vec3::new(24.0, 51.6, -14.0), warm, false),  // spire summit
        (Vec3::new(27.5, 12.0, -2.0), red, false),    // slick tower
        (Vec3::new(27.5, 12.0, 4.0), green, false),   // spring tower
        (Vec3::new(20.0, 3.0, 11.5), warm, false),    // under the leaning slab
        (Vec3::new(15.0, 8.5, 18.0), cool, false),    // hanging garden
        (Vec3::new(20.0, 8.0, 20.0), warm, false),    // traverse wall
        (Vec3::new(0.0, 14.0, 32.0), cool, false),    // inside the spiral
        (Vec3::new(-11.0, 4.6, 16.0), warm, false),   // under the arch
        (Vec3::new(-19.0, 9.0, -7.0), green, false),  // pillar forest
        (Vec3::new(-28.0, 11.2, -14.6), warm, false), // under the cube's cap
        (Vec3::new(-20.6, 11.2, -22.0), cool, false),
        (Vec3::new(-28.5, 6.5, 6.0), cool, false),    // top of the ramps
    ];
    let bulb = b.meshes.add(Sphere::new(0.12));
    for (position, color, shadows) in lamps {
        let glow = color.to_linear() * 12.0;
        b.commands.spawn((
            PointLight {
                color,
                intensity: 600_000.0,
                range: 22.0,
                shadow_maps_enabled: shadows,
                ..default()
            },
            Transform::from_translation(position),
            Mesh3d(bulb.clone()),
            MeshMaterial3d(b.materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: glow,
                ..default()
            })),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// North: the original wall. Easy zigzag, a rest ledge, slick holds, a
/// spring jump, then under a roof and over its lip.
fn main_wall(b: &mut Builder) {
    let (wall, ledge, goal) = (b.palette.wall.clone(), b.palette.ledge.clone(), b.palette.goal.clone());
    b.block(&wall, Vec3::new(0.0, 20.0, WALL_FACE_Z - 2.0), Vec3::new(14.0, 40.0, 4.0));

    for i in 0..9 {
        let x = if i % 2 == 0 { -0.45 } else { 0.45 };
        b.wall_hold(x, FIRST_HOLD + HOLD_STEP * i as f32, Handhold::default());
    }

    b.wall_hold(1.3, 13.4, Handhold::default());
    b.wall_hold(2.0, 14.5, Handhold::default());
    b.wall_hold(3.0, 15.8, Handhold::default());
    b.block(&ledge, Vec3::new(3.2, 13.6, -1.0), Vec3::new(3.0, 0.4, 2.0));

    for i in 0..4 {
        b.wall_hold(3.8, 16.9 + HOLD_STEP * i as f32, Handhold::slick());
    }
    b.wall_hold(3.2, 22.1, Handhold::spring());
    b.wall_hold(2.6, 25.4, Handhold::default());
    b.wall_hold(1.8, 26.4, Handhold::default());
    b.wall_hold(1.2, 27.3, Handhold::default());

    let roof_center = Vec3::new(0.0, 28.5, 0.0);
    let roof_size = Vec3::new(14.0, 0.6, 4.0);
    b.block(&wall, roof_center, roof_size);
    let underside = roof_center.y - roof_size.y * 0.5;
    for (x, z) in [(1.2, -1.2), (0.9, -0.3), (1.2, 0.6), (0.9, 1.5)] {
        b.hold(Vec3::new(x, underside, z), Vec3::NEG_Y, Handhold::default());
    }
    let lip_z = roof_center.z + roof_size.z * 0.5;
    let top = roof_center.y + roof_size.y * 0.5;
    b.hold(Vec3::new(1.0, roof_center.y, lip_z), Vec3::Z, Handhold::default());
    b.hold(Vec3::new(1.0, top, lip_z - 0.6), Vec3::Y, Handhold::default());
    b.block(&goal, Vec3::new(0.0, top + 0.5, -0.5), Vec3::new(1.0, 1.0, 1.0));
}

/// East of the wall: two facing walls to climb by crossing from one to the other.
fn chimney(b: &mut Builder) {
    let stone = b.palette.stone.clone();
    let height = 20.0;
    let (left_face, right_face) = (12.5, 15.0);
    b.block(&stone, Vec3::new(left_face - 0.5, height * 0.5, -4.0), Vec3::new(1.0, height, 6.0));
    b.block(&stone, Vec3::new(right_face + 0.5, height * 0.5, -4.0), Vec3::new(1.0, height, 6.0));
    for i in 0..8 {
        let y = FIRST_HOLD + 2.4 * i as f32;
        b.hold(Vec3::new(left_face, y, -4.0), Vec3::X, Handhold::default());
        if y + 1.2 < height {
            b.hold(Vec3::new(right_face, y + 1.2, -4.0), Vec3::NEG_X, Handhold::default());
        }
    }
}

/// West: nine pillars of rising height, each wrapped in a spiral of holds.
/// The tops are close enough to jump between.
fn pillar_forest(b: &mut Builder) {
    let stone = b.palette.stone.clone();
    let width = 1.6;
    let faces = [Vec3::Z, Vec3::X, Vec3::NEG_Z, Vec3::NEG_X];
    for row in 0..3 {
        for col in 0..3 {
            let index = row * 3 + col;
            let height = 4.0 + 2.5 * index as f32;
            let (x, z) = (-14.0 - 5.0 * col as f32, -2.0 - 5.0 * row as f32);
            b.block(&stone, Vec3::new(x, height * 0.5, z), Vec3::new(width, height, width));
            let mut step = 0;
            loop {
                let y = FIRST_HOLD + HOLD_STEP * step as f32;
                if y > height - 0.3 {
                    break;
                }
                let normal = faces[(step + index) % 4];
                b.hold(Vec3::new(x, y, z) + normal * width * 0.5, normal, Handhold::default());
                step += 1;
            }
        }
    }
}

/// South of the spawn: two towers joined by a beam to cross hand over hand.
fn monkey_bars(b: &mut Builder) {
    let (wall, ledge) = (b.palette.wall.clone(), b.palette.ledge.clone());
    let (z, height) = (12.0, 8.0);
    for x in [-6.0, 6.0] {
        b.block(&wall, Vec3::new(x, height * 0.5, z), Vec3::new(2.0, height, 2.0));
        b.ladder(Vec3::new(x, 0.0, z - 1.0), Vec3::NEG_Z, height - 0.4, HOLD_STEP, Handhold::default());
    }
    let beam_y = height + 2.6;
    b.block(&ledge, Vec3::new(0.0, beam_y, z), Vec3::new(10.0, 0.4, 0.6));
    for i in 0..8 {
        let x = -4.4 + 8.8 * i as f32 / 7.0;
        b.hold(Vec3::new(x, beam_y - 0.2, z), Vec3::NEG_Y, Handhold::default());
    }
}

/// East: a slab leaning 20 degrees. One side overhangs, the other is an easy
/// lean-back climb.
fn leaning_slab(b: &mut Builder) {
    let wall = b.palette.wall.clone();
    let size = Vec3::new(6.0, 14.0, 0.8);
    let rotation = Quat::from_rotation_x(20f32.to_radians());
    let center = Vec3::new(20.0, 6.7, 8.0);
    b.block_rotated(&wall, center, size, rotation);
    let mut i = 0;
    let mut v = -4.6;
    while v < size.y * 0.5 {
        let u = if i % 2 == 0 { -0.5 } else { 0.5 };
        // Overhanging face.
        let normal = rotation * Vec3::Z;
        b.hold(center + rotation * Vec3::new(u - 1.5, v, size.z * 0.5), normal, Handhold::default());
        // Lean-back face.
        b.hold(center + rotation * Vec3::new(u + 1.5, v, -size.z * 0.5), -normal, Handhold::default());
        v += 1.2;
        i += 1;
    }
}

/// West: a ramp to walk up and a steep one to slide down, each to a platform.
fn ramps(b: &mut Builder) {
    let (ramp, ledge) = (b.palette.ramp.clone(), b.palette.ledge.clone());
    for (x, length, degrees) in [(-26.0, 10.0, 30.0f32), (-31.0, 8.0, 62.0)] {
        let angle = degrees.to_radians();
        let low_z = 14.0;
        let rise = length * angle.sin();
        let run = length * angle.cos();
        b.block_rotated(
            &ramp,
            Vec3::new(x, rise * 0.5, low_z - run * 0.5),
            Vec3::new(3.0, 0.3, length),
            Quat::from_rotation_x(angle),
        );
        b.block(&ledge, Vec3::new(x, rise - 0.25, low_z - run - 1.5), Vec3::new(3.0, 0.5, 3.0));
        // A way back up for the steep one.
        b.ladder(Vec3::new(x, 0.0, low_z - run - 3.0), Vec3::NEG_Z, rise - 0.6, HOLD_STEP, Handhold::default());
        b.block(&ledge, Vec3::new(x, (rise - 0.5) * 0.5, low_z - run - 2.9), Vec3::new(1.0, rise - 0.5, 0.2));
    }
}

/// Far south: floating platforms spiralling upward, with a bar to swing from
/// in each gap.
fn spiral_islands(b: &mut Builder) {
    let (ledge, goal) = (b.palette.ledge.clone(), b.palette.goal.clone());
    let center = Vec3::new(0.0, 0.0, 32.0);
    let radius = 8.0;
    let count = 12;
    let island = |i: usize| {
        let angle = TAU * i as f32 / 9.0;
        center + Vec3::new(angle.sin() * radius, 1.2 + 2.2 * i as f32, -angle.cos() * radius)
    };
    for i in 0..count {
        let pos = island(i);
        let material = if i == count - 1 { &goal } else { &ledge };
        b.block(material, pos, Vec3::new(2.4, 0.4, 2.4));
        if i + 1 < count {
            let next = island(i + 1);
            let middle = (pos + next) * 0.5;
            let yaw = (next.x - pos.x).atan2(next.z - pos.z) + PI * 0.5;
            b.floating_hold(Vec3::new(middle.x, next.y + 1.6, middle.z), yaw, Handhold::default());
        }
    }
}

/// North-east: a fifty metre spire. Longer than two hands of grip, so the
/// rest ledges matter.
fn ladder_spire(b: &mut Builder) {
    let (stone, ledge, goal) = (b.palette.stone.clone(), b.palette.ledge.clone(), b.palette.goal.clone());
    let (x, z, height, width) = (24.0, -14.0, 50.0, 1.4);
    b.block(&stone, Vec3::new(x, height * 0.5, z), Vec3::new(width, height, width));
    let face = Vec3::new(x, 0.0, z + width * 0.5);
    b.ladder(face, Vec3::Z, height - 0.2, 1.5, Handhold::default());
    for rest in [12.0, 24.0, 36.0] {
        b.block(&ledge, Vec3::new(x + 1.3, rest, z + width * 0.5 + 0.5), Vec3::new(1.6, 0.3, 1.2));
    }
    b.block(&goal, Vec3::new(x, height + 0.3, z), Vec3::splat(0.6));
}

/// South-west: an arch to climb up one leg, cross under, and down the other.
fn arch(b: &mut Builder) {
    let stone = b.palette.stone.clone();
    let (z, leg_height) = (16.0, 6.0);
    for x in [-14.0, -8.0] {
        b.block(&stone, Vec3::new(x, leg_height * 0.5, z), Vec3::new(1.5, leg_height, 1.5));
        b.ladder(Vec3::new(x, 0.0, z - 0.75), Vec3::NEG_Z, leg_height - 0.5, HOLD_STEP, Handhold::default());
    }
    b.block(&stone, Vec3::new(-11.0, leg_height + 0.4, z), Vec3::new(7.5, 0.8, 1.5));
    for i in 0..4 {
        b.hold(Vec3::new(-12.5 + i as f32, leg_height, z), Vec3::NEG_Y, Handhold::default());
    }
}

/// South-west: a row of blocks, each a little taller than the last.
fn stepping_blocks(b: &mut Builder) {
    let ramp = b.palette.ramp.clone();
    for i in 0..8 {
        let height = 0.6 * (i + 1) as f32;
        b.block(
            &ramp,
            Vec3::new(-32.0 + 3.2 * i as f32, height * 0.5, 22.0),
            Vec3::new(1.6, height, 1.6),
        );
    }
}

/// Far east: one tower of slick holds, one of widely spaced spring holds.
fn twin_towers(b: &mut Builder) {
    let wall = b.palette.wall.clone();
    let (x, height) = (30.0, 24.0);
    for (z, step, handhold) in [(-2.0, HOLD_STEP, Handhold::slick()), (4.0, 3.4, Handhold::spring())] {
        b.block(&wall, Vec3::new(x, height * 0.5, z), Vec3::new(2.0, height, 2.0));
        b.ladder(Vec3::new(x - 1.0, 0.0, z), Vec3::NEG_X, height - 0.3, step, handhold);
    }
    // A bridge between the tops.
    b.block(&wall, Vec3::new(x, height - 0.2, 1.0), Vec3::new(1.0, 0.4, 4.0));
}

/// South-east: a long sideways traverse well above the ground.
fn traverse_wall(b: &mut Builder) {
    let (wall, ledge) = (b.palette.wall.clone(), b.palette.ledge.clone());
    let (center_x, face_z) = (20.0, 21.5);
    b.block(&wall, Vec3::new(center_x, 5.0, face_z + 0.5), Vec3::new(16.0, 10.0, 1.0));
    b.ladder(Vec3::new(13.0, 0.0, face_z), Vec3::NEG_Z, 6.0, HOLD_STEP, Handhold::default());
    for i in 0..11 {
        let x = 14.2 + 1.2 * i as f32;
        let y = 6.2 + (i as f32 * 0.9).sin() * 0.5;
        b.hold(Vec3::new(x, y, face_z), Vec3::NEG_Z, Handhold::default());
    }
    b.block(&ledge, Vec3::new(27.2, 5.0, face_z - 0.6), Vec3::new(1.4, 0.3, 1.2));
}

/// Overhead near the spawn: a field of bars hanging in the air at mixed
/// heights, for swinging and jumping between.
fn hanging_garden(b: &mut Builder) {
    let ledge = b.palette.ledge.clone();
    b.block(&ledge, Vec3::new(10.0, 0.75, 14.0), Vec3::new(2.0, 1.5, 2.0));
    for row in 0..4 {
        for col in 0..5 {
            let i = row * 5 + col;
            let height = 4.2 + ((i * 7) % 5) as f32 * 0.7 + row as f32 * 1.4;
            let pos = Vec3::new(10.0 + col as f32 * 2.6, height, 14.0 + 2.4 * row as f32);
            let handhold = match i % 7 {
                3 => Handhold::slick(),
                5 => Handhold::spring(),
                _ => Handhold::default(),
            };
            b.floating_hold(pos, if i % 2 == 0 { 0.0 } else { PI * 0.5 }, handhold);
        }
    }
}

/// North-west: a big cube with holds scattered over every side and a ring of
/// holds under an overhanging cap.
fn great_cube(b: &mut Builder) {
    let (wall, ledge) = (b.palette.wall.clone(), b.palette.ledge.clone());
    let center = Vec3::new(-28.0, 6.0, -22.0);
    let side = 12.0;
    b.block(&wall, center, Vec3::splat(side));
    let half = side * 0.5;
    for (face, normal) in [Vec3::Z, Vec3::X, Vec3::NEG_Z, Vec3::NEG_X].into_iter().enumerate() {
        let along = Vec3::Y.cross(normal);
        for row in 0..8 {
            for col in 0..4 {
                // Skip some so every face reads differently.
                if (row * 3 + col * 5 + face) % 4 == 0 {
                    continue;
                }
                let offset = -3.9 + 2.6 * col as f32 + if row % 2 == 0 { 0.0 } else { 0.9 };
                let y = FIRST_HOLD + HOLD_STEP * row as f32;
                let point = Vec3::new(center.x, y, center.z) + normal * half + along * offset;
                b.hold(point, normal, Handhold::default());
            }
        }
    }
    // The cap overhangs by a metre and a half all round.
    let cap_y = side + 0.3;
    b.block(&ledge, Vec3::new(center.x, cap_y, center.z), Vec3::new(side + 3.0, 0.6, side + 3.0));
    for normal in [Vec3::Z, Vec3::X, Vec3::NEG_Z, Vec3::NEG_X] {
        let along = Vec3::Y.cross(normal);
        for i in 0..5 {
            let point = Vec3::new(center.x, side, center.z)
                + normal * (half + 0.75)
                + along * (-4.8 + 2.4 * i as f32);
            b.hold(point, Vec3::NEG_Y, Handhold::default());
        }
        let lip = Vec3::new(center.x, cap_y, center.z) + normal * (half + 1.5);
        b.hold(lip, normal, Handhold::default());
        b.hold(lip + Vec3::Y * 0.3 - normal * 0.6, Vec3::Y, Handhold::default());
    }
}

/// Direction towards the sun; the sky's glow and the shadows both use it.
const SUN_DIRECTION: Vec3 = Vec3::new(0.67, 0.57, 0.47);
/// World size covered by one repeat of the ground texture.
const GRID_TILE_METRES: f32 = 4.0;

/// The sky cube map, for the camera to draw behind everything.
#[derive(Resource)]
pub struct Sky(pub Handle<Image>);

pub fn sky_for_camera(sky: &Sky) -> Skybox {
    Skybox { image: Some(sky.0.clone()), brightness: 1800.0, ..default() }
}

/// A clear blue sky: deep overhead, pale at the horizon, with a glow around
/// the sun. Built as a cube map so no image file is needed.
fn sky_texture() -> Image {
    const SIZE: usize = 128;
    let zenith = Vec3::new(0.10, 0.36, 0.86);
    let horizon = Vec3::new(0.62, 0.80, 0.98);
    let ground = Vec3::new(0.80, 0.86, 0.94);
    let sun = SUN_DIRECTION.normalize();
    let mut data = Vec::with_capacity(SIZE * SIZE * 6 * 4);
    for face in 0..6 {
        for y in 0..SIZE {
            for x in 0..SIZE {
                let u = (x as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
                // Cube map faces in the order +X, -X, +Y, -Y, +Z, -Z.
                let dir = match face {
                    0 => Vec3::new(1.0, -v, -u),
                    1 => Vec3::new(-1.0, -v, u),
                    2 => Vec3::new(u, 1.0, v),
                    3 => Vec3::new(u, -1.0, -v),
                    4 => Vec3::new(u, -v, 1.0),
                    _ => Vec3::new(-u, -v, -1.0),
                }
                .normalize();
                let mut color = if dir.y >= 0.0 {
                    horizon.lerp(zenith, dir.y.powf(0.55))
                } else {
                    horizon.lerp(ground, (-dir.y * 4.0).min(1.0))
                };
                let toward_sun = dir.dot(sun).max(0.0);
                color += Vec3::new(1.0, 0.95, 0.85) * (toward_sun.powf(8.0) * 0.18);
                color = color.lerp(Vec3::ONE, toward_sun.powf(900.0));
                for channel in color.min(Vec3::ONE).to_array() {
                    data.push((channel * 255.0) as u8);
                }
                data.push(255);
            }
        }
    }
    let mut image = Image::new(
        Extent3d { width: SIZE as u32, height: SIZE as u32, depth_or_array_layers: 6 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image.sampler = ImageSampler::linear();
    image
}

/// White with a grey line every metre and a darker one every four. It has a
/// full set of mip levels so the lines fade with distance instead of shimmering.
fn grid_texture() -> Image {
    const SIZE: usize = 256;
    let cell = SIZE / GRID_TILE_METRES as usize;
    let mut level: Vec<[f32; 3]> = Vec::with_capacity(SIZE * SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let edge = |c: usize, width: usize| c % cell < width || c % cell >= cell - width;
            let major = x < 2 || x >= SIZE - 2 || y < 2 || y >= SIZE - 2;
            let shade = if major {
                0.42
            } else if edge(x, 1) || edge(y, 1) {
                0.66
            } else {
                0.93
            };
            level.push([shade, shade, shade * 1.02]);
        }
    }
    let mut data = Vec::new();
    let mut size = SIZE;
    let mut mips = 0;
    loop {
        for texel in &level {
            for channel in texel {
                data.push((channel.min(1.0) * 255.0) as u8);
            }
            data.push(255);
        }
        mips += 1;
        if size == 1 {
            break;
        }
        // Average each 2x2 block for the next level down.
        let half = size / 2;
        let mut next = Vec::with_capacity(half * half);
        for y in 0..half {
            for x in 0..half {
                let mut sum = [0.0f32; 3];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let texel = level[(y * 2 + dy) * size + x * 2 + dx];
                    for c in 0..3 {
                        sum[c] += texel[c] * 0.25;
                    }
                }
                next.push(sum);
            }
        }
        level = next;
        size = half;
    }
    let mut image = Image::new_uninit(
        Extent3d { width: SIZE as u32, height: SIZE as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = mips;
    image.data = Some(data);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 16,
        ..default()
    });
    image
}
