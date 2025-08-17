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
    let mut test_chunk = Chunk::default();
    test_chunk
        .set_block(0, 0, 0, world::BlockType::Grass)
        .unwrap();
    test_chunk
        .set_block(1, 0, 0, world::BlockType::RedBlock)
        .unwrap();
    test_chunk
        .set_block(2, 0, 0, world::BlockType::Dirt)
        .unwrap();
    let mut chunkmap = ChunkMap::default();
    chunkmap.insert((0, 0, 0).into(), test_chunk);
    chunkmap.insert((-1, 0, 0).into(), Chunk::default());
    chunkmap
        .set_block((-16, 0, 0).into(), world::BlockType::BlueBlock)
        .unwrap();

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
