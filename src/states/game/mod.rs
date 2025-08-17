use bevy::{
    input::mouse::MouseMotion,
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};

use crate::{
    AppState, AutoDespawn,
    states::game::world::{Chunk, ChunkMap, WorldPlugin},
};

mod world;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WorldPlugin)
            .add_systems(
                OnEnter(AppState::Game),
                (replace_camera, setup, cursor_grab),
            )
            .add_systems(OnExit(AppState::Game), cursor_ungrab)
            .add_systems(Update, (move_cam));
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

fn setup(mut commands: Commands) {
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

    commands.spawn((chunkmap, AutoDespawn(AppState::Game)));
}

fn cursor_grab(mut q_windows: Query<&mut Window, With<PrimaryWindow>>) {
    let Ok(mut primary_window) = q_windows.single_mut() else {
        return;
    };

    primary_window.cursor_options.grab_mode = CursorGrabMode::Confined;
    primary_window.cursor_options.visible = false;
}

fn cursor_ungrab(mut q_windows: Query<&mut Window, With<PrimaryWindow>>) {
    let Ok(mut primary_window) = q_windows.single_mut() else {
        return;
    };

    primary_window.cursor_options.grab_mode = CursorGrabMode::None;
    primary_window.cursor_options.visible = false;
}

fn move_cam(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut camera: Query<(&mut Transform, &Camera3d)>,
) {
    let Ok((mut transform, _)) = camera.single_mut() else {
        return;
    };

    let mut delta = Vec3::ZERO;
    if keyboard_input.pressed(KeyCode::KeyW) {
        delta.z -= 0.1;
    } else if keyboard_input.pressed(KeyCode::KeyS) {
        delta.z += 0.1;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        delta.x -= 0.1;
    } else if keyboard_input.pressed(KeyCode::KeyD) {
        delta.x += 0.1;
    } else if keyboard_input.pressed(KeyCode::Space) {
        transform.translation.y += 0.1;
    } else if keyboard_input.pressed(KeyCode::ShiftLeft) {
        transform.translation.y -= 0.1;
    }

    for event in mouse_motion.read() {
        let yaw = Quat::from_rotation_y(-event.delta.x * 0.001);
        let pitch = Quat::from_rotation_x(-event.delta.y * 0.001);
        transform.rotation = yaw * transform.rotation * pitch;
    }

    let y = delta.y;
    delta = transform.rotation.mul_vec3(delta);
    delta.y = y;

    transform.translation += delta;
}
