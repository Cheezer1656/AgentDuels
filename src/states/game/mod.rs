use avian3d::{
    PhysicsPlugins,
    prelude::{Collider, Friction, LockedAxes, PhysicsDebugPlugin, Restitution, RigidBody},
};
use bevy::{
    ecs::schedule::ScheduleLabel,
    input::mouse::MouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::{
    AppState, AutoDespawn,
    states::game::{
        gameloop::GameLoopPlugin,
        network::NetworkPlugin,
        player::PlayerID,
        world::{Chunk, ChunkMap, WorldPlugin},
    },
};

mod gameloop;
mod network;
mod player;
mod world;

const PLAYER_SPEED: f32 = 0.1;

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Clone, Debug)]
pub struct GameUpdate;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_schedule(Schedule::new(GameUpdate))
            .add_plugins((
                WorldPlugin,
                NetworkPlugin,
                GameLoopPlugin,
                PhysicsPlugins::default(),
                PhysicsDebugPlugin::default(),
            ))
            .add_systems(
                OnEnter(AppState::Game),
                (replace_camera, set_bg, setup, cursor_grab),
            )
            .add_systems(OnExit(AppState::Game), cursor_ungrab)
            .add_systems(Update, move_cam);
    }
}

fn replace_camera(mut commands: Commands, camera_query: Query<Entity, With<Camera2d>>) {
    for entity in camera_query.iter() {
        commands.entity(entity).despawn();
    }
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn set_bg(mut clear_color: ResMut<ClearColor>) {
    clear_color.0 = Color::srgb_u8(48, 193, 255);
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut chunkmap = ChunkMap::default();

    for x in -2..=2 {
        for y in -1..=1 {
            for z in -1..=1 {
                chunkmap.insert((x, y, z).into(), Chunk::default());
            }
        }
    }

    for x in -20..=20 {
        for y in -8..=0 {
            chunkmap
                .set_block(
                    (x, y, 0).into(),
                    match x {
                        -20..0 => world::BlockType::BlueBlock,
                        0 => world::BlockType::WhiteBlock,
                        1..=20 => world::BlockType::RedBlock,
                        _ => unreachable!(),
                    },
                )
                .unwrap();
        }
    }

    for i in 0..2 {
        for x in 21..=30 {
            for y in -5..=0 {
                for z in -5..=5 {
                    chunkmap
                        .set_block(
                            (x * (i * 2 - 1), y, z).into(),
                            match y {
                                -5..=-3 => world::BlockType::Stone,
                                -2..=-1 => world::BlockType::Dirt,
                                0 => world::BlockType::Grass,
                                _ => unreachable!(),
                            },
                        )
                        .unwrap();
                }
            }
        }
    }

    chunkmap
        .set_block((0, 1, 0).into(), world::BlockType::WhiteBlock)
        .unwrap();

    commands.spawn((chunkmap, AutoDespawn(AppState::Game)));

    let player_mesh = meshes.add(Cuboid::new(0.6, 1.8, 0.6));
    let player_direction_mesh = meshes.add(Cuboid::new(1.0, 0.1, 0.1));

    for i in 0..2_i32 {
        let mut transform = Transform::from_xyz((i * 2 - 1) as f32 * -21.0, 1.4, 0.0);
        if i == 0 {
            transform.rotate_y(std::f32::consts::PI);
        }

        commands.spawn((
            PlayerID(i as u16),
            RigidBody::Dynamic,
            Collider::cuboid(0.6, 1.8, 0.6),
            LockedAxes::ROTATION_LOCKED,
            Friction::new(0.0),
            Restitution::new(0.0),
            Mesh3d(player_mesh.clone()),
            MeshMaterial3d(materials.add(if i == 0 {
                Color::srgb_u8(237, 28, 36)
            } else {
                Color::srgb_u8(47, 54, 153)
            })),
            transform,
            AutoDespawn(AppState::Game),
            children![(
                Mesh3d(player_direction_mesh.clone()),
                MeshMaterial3d(materials.add(Color::srgb_u8(0, 255, 0))),
                Transform::from_xyz(0.3, 0.7, 0.0)
            )],
        ));
    }
}

fn cursor_grab(mut cursor_opts: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor_opts.grab_mode = CursorGrabMode::Confined;
    cursor_opts.visible = false;
}

fn cursor_ungrab(mut cursor_opts: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    cursor_opts.grab_mode = CursorGrabMode::None;
    cursor_opts.visible = true;
}

fn move_cam(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut camera: Query<(&mut Transform, &Camera3d)>,
) {
    let Ok((mut transform, _)) = camera.single_mut() else {
        return;
    };

    let mut delta = Vec3::ZERO;
    for keycode in keyboard_input.get_pressed() {
        match keycode {
            KeyCode::KeyW => delta.z -= 0.1,
            KeyCode::KeyS => delta.z += 0.1,
            KeyCode::KeyA => delta.x -= 0.1,
            KeyCode::KeyD => delta.x += 0.1,
            KeyCode::Space => transform.translation.y += 0.1,
            KeyCode::ShiftLeft => transform.translation.y -= 0.1,
            _ => {}
        }
    }

    for event in mouse_motion.read() {
        // Extract current yaw and pitch
        let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);

        // Update yaw and pitch based on mouse movement
        yaw -= event.delta.x * 0.001;
        pitch = (pitch - event.delta.y * 0.001).clamp(-1.5, 1.5); // Clamp pitch

        // Reconstruct rotation
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    }

    // Only apply yaw to movement direction
    let yaw = Quat::from_rotation_y(transform.rotation.to_euler(EulerRot::YXZ).0);
    delta = yaw.mul_vec3(delta);
    delta.y = 0.0;

    transform.translation += delta;
}
