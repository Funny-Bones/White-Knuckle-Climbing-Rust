//! Greybox test shaft: a wall of handholds, a rest ledge, and a roof to cross.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

/// Solid geometry the body collides with.
pub const WORLD_GROUP: Group = Group::GROUP_1;
/// Handholds: hands can find them, the body passes through them.
pub const HANDHOLD_GROUP: Group = Group::GROUP_2;

pub const SPAWN_POS: Vec3 = Vec3::new(0.0, 1.3, 3.0);
pub const SPAWN_YAW: f32 = 0.0;

/// Z of the climbing wall's face.
const WALL_FACE_Z: f32 = -2.0;

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
        Self {
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
        }
    }
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
}

impl Builder<'_, '_, '_> {
    fn block(&mut self, material: &Handle<StandardMaterial>, center: Vec3, size: Vec3) {
        self.commands.spawn((
            Mesh3d(self.meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(center),
            Collider::cuboid(size.x * 0.5, size.y * 0.5, size.z * 0.5),
            CollisionGroups::new(WORLD_GROUP, Group::ALL),
        ));
    }

    fn hold(
        &mut self,
        material: &Handle<StandardMaterial>,
        center: Vec3,
        size: Vec3,
        handhold: Handhold,
    ) {
        self.commands.spawn((
            Mesh3d(self.meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(center),
            Collider::cuboid(size.x * 0.5, size.y * 0.5, size.z * 0.5),
            CollisionGroups::new(HANDHOLD_GROUP, Group::ALL),
            handhold,
        ));
    }

    /// A hold sticking out of the main wall.
    fn wall_hold(&mut self, material: &Handle<StandardMaterial>, x: f32, y: f32, handhold: Handhold) {
        self.hold(
            material,
            Vec3::new(x, y, WALL_FACE_Z + 0.13),
            Vec3::new(0.5, 0.14, 0.3),
            handhold,
        );
    }
}

pub fn setup_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut solid = |r: f32, g: f32, b: f32| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: 0.9,
            ..default()
        })
    };
    let ground = solid(0.16, 0.17, 0.19);
    let wall = solid(0.30, 0.31, 0.34);
    let ledge = solid(0.22, 0.26, 0.32);
    let hold = solid(0.95, 0.72, 0.18);
    let slick = solid(0.85, 0.22, 0.20);
    let spring = solid(0.25, 0.80, 0.40);
    let goal = solid(1.0, 0.9, 0.35);

    let mut b = Builder { commands: &mut commands, meshes: &mut meshes };

    b.block(&ground, Vec3::new(0.0, -0.5, 0.0), Vec3::new(60.0, 1.0, 60.0));
    b.block(&wall, Vec3::new(0.0, 20.0, WALL_FACE_Z - 2.0), Vec3::new(14.0, 40.0, 4.0));

    // First pitch: an easy zigzag.
    for i in 0..9 {
        let x = if i % 2 == 0 { -0.45 } else { 0.45 };
        b.wall_hold(&hold, x, 2.3 + 1.3 * i as f32, Handhold::default());
    }

    // Traverse right onto a ledge where grip recovers.
    b.wall_hold(&hold, 1.3, 13.4, Handhold::default());
    b.wall_hold(&hold, 2.0, 14.5, Handhold::default());
    b.wall_hold(&hold, 3.0, 15.8, Handhold::default());
    b.block(&ledge, Vec3::new(3.2, 13.6, -1.0), Vec3::new(3.0, 0.4, 2.0));

    // Second pitch: slick holds that burn grip two and a half times as fast.
    for i in 0..4 {
        b.wall_hold(
            &slick,
            3.8,
            16.9 + 1.3 * i as f32,
            Handhold { strain_rate: 2.5, ..default() },
        );
    }

    // A springy hold to launch across the gap above it.
    b.wall_hold(&spring, 3.2, 22.1, Handhold { jump_mult: 1.5, ..default() });
    b.wall_hold(&hold, 2.6, 25.4, Handhold::default());
    b.wall_hold(&hold, 1.8, 26.4, Handhold::default());
    b.wall_hold(&hold, 1.2, 27.3, Handhold::default());

    // Roof: cross underneath, then over the lip.
    let roof_center = Vec3::new(0.0, 28.5, 0.0);
    let roof_size = Vec3::new(14.0, 0.6, 4.0);
    b.block(&wall, roof_center, roof_size);
    let underside = roof_center.y - roof_size.y * 0.5;
    for (x, z) in [(1.2, -1.2), (0.9, -0.3), (1.2, 0.6), (0.9, 1.5)] {
        b.hold(
            &hold,
            Vec3::new(x, underside - 0.07, z),
            Vec3::new(0.5, 0.14, 0.5),
            Handhold::default(),
        );
    }
    let lip_z = roof_center.z + roof_size.z * 0.5;
    b.hold(
        &hold,
        Vec3::new(1.0, roof_center.y, lip_z + 0.13),
        Vec3::new(0.6, 0.14, 0.3),
        Handhold::default(),
    );
    let top = roof_center.y + roof_size.y * 0.5;
    b.hold(
        &hold,
        Vec3::new(1.0, top + 0.07, lip_z - 0.6),
        Vec3::new(0.5, 0.14, 0.3),
        Handhold::default(),
    );
    b.block(&goal, Vec3::new(0.0, top + 0.5, -0.5), Vec3::new(1.0, 1.0, 1.0));

    commands.spawn((
        DirectionalLight { illuminance: 9000.0, ..default() },
        Transform::from_xyz(8.0, 30.0, 14.0).looking_at(Vec3::new(0.0, 12.0, -2.0), Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight {
        brightness: 350.0,
        ..default()
    });
}
