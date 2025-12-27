use crate::states::game::gameloop::GOAL_BOUNDS;
use crate::states::game::player::{
    PLAYER_HEIGHT, PLAYER_WIDTH, PlayerBody, PlayerBundle, SPAWN_POSITIONS, SPAWN_ROTATIONS, Score,
};
use crate::{
    AppState, AutoDespawn,
    states::game::{
        gameloop::GameLoopPlugin,
        network::NetworkPlugin,
        player::{PlayerHead, PlayerID},
        world::{Chunk, ChunkMap, WorldPlugin},
    },
};
use avian3d::prelude::{GravityScale, LinearDamping};
use avian3d::{
    PhysicsPlugins,
    prelude::{
        Collider, CollisionLayers, Friction, LockedAxes, PhysicsDebugPlugin, PhysicsLayer,
        Restitution, RigidBody,
    },
};
use bevy::{
    ecs::schedule::ScheduleLabel,
    input::mouse::MouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

mod gameloop;
mod network;
mod player;
mod world;

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Clone, Debug)]
pub struct GameUpdate;

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Clone, Debug)]
pub struct PostGameUpdate;

#[derive(PhysicsLayer, Default)]
pub enum CollisionLayer {
    #[default]
    Entity,
    World,
}

#[derive(Component)]
struct RedScoreMarker;

#[derive(Component)]
struct BlueScoreMarker;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_schedule(Schedule::new(GameUpdate))
            .add_schedule(Schedule::new(PostGameUpdate))
            .add_plugins((
                WorldPlugin,
                NetworkPlugin,
                GameLoopPlugin,
                PhysicsPlugins::new(PostGameUpdate),
                PhysicsDebugPlugin::default(),
            ))
            .add_systems(
                OnEnter(AppState::Game),
                (replace_camera, setup, cursor_grab),
            )
            .add_systems(OnExit(AppState::Game), cursor_ungrab)
            .add_systems(
                Update,
                (move_cam, update_scoreboard).run_if(in_state(AppState::Game)),
            );
    }
}

fn replace_camera(mut commands: Commands, camera_query: Query<Entity, With<Camera2d>>) {
    for entity in camera_query.iter() {
        commands.entity(entity).despawn();
    }
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb_u8(48, 193, 255)),
            ..default()
        },
        Msaa::Off,
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Text2d::new("Score: "),
        TextFont::default(),
        children![
            (
                RedScoreMarker,
                TextSpan("0".to_string()),
                TextColor(Color::srgb_u8(255, 0, 0)),
            ),
            (TextSpan(" - ".to_string()),),
            (
                BlueScoreMarker,
                TextSpan("0".to_string()),
                TextColor(Color::srgb_u8(0, 0, 255)),
            )
        ],
        Node {
            height: Val::Percent(100.0),
            width: Val::Percent(100.0),
            ..default()
        },
    ));

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
                'outer: for z in -5..=5 {
                    for (x_range, y_range, z_range) in GOAL_BOUNDS.iter() {
                        if x_range.contains(&x)
                            && (*y_range.start()..=y_range.end() + 1).contains(&y)
                            && z_range.contains(&z)
                        {
                            continue 'outer;
                        }
                    }
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

    let player_body_mesh = meshes.add(Cuboid::new(PLAYER_WIDTH, 1.3, PLAYER_WIDTH));
    let player_head_mesh = meshes.add(Cuboid::new(0.5, 0.5, 0.5));
    let player_direction_mesh = meshes.add(Cuboid::new(0.1, 0.1, 1.0));

    for i in 0..2_i32 {
        let mut body_transform = Transform::from_xyz(0.0, -0.25, 0.0);
        body_transform.rotation = Quat::from_rotation_y(SPAWN_ROTATIONS[i as usize]);

        commands.spawn((
            PlayerBundle {
                id: PlayerID(i as u16),
                transform: Transform::from_translation(SPAWN_POSITIONS[i as usize]),
                ..default()
            },
            RigidBody::Dynamic,
            Collider::cuboid(PLAYER_WIDTH, PLAYER_HEIGHT, PLAYER_WIDTH),
            CollisionLayers::new(CollisionLayer::Entity, [CollisionLayer::World]),
            LockedAxes::ROTATION_LOCKED,
            Friction::new(0.0),
            Restitution::new(0.0),
            LinearDamping(2.0),
            GravityScale(2.0),
            Visibility::default(),
            AutoDespawn(AppState::Game),
            children![(
                PlayerBody,
                Mesh3d(player_body_mesh.clone()),
                MeshMaterial3d(materials.add(if i == 0 {
                    Color::srgb_u8(237, 28, 36)
                } else {
                    Color::srgb_u8(47, 54, 153)
                })),
                body_transform,
                children![(
                    PlayerHead,
                    Mesh3d(player_head_mesh.clone()),
                    MeshMaterial3d(materials.add(Color::srgb_u8(0, 255, 0))),
                    Transform::from_xyz(0.0, PLAYER_HEIGHT / 2.0, 0.0)
                        .with_rotation(Quat::from_rotation_y(-std::f32::consts::PI / 2.0)),
                    children![(
                        Mesh3d(player_direction_mesh.clone()),
                        MeshMaterial3d(materials.add(Color::srgb_u8(0, 255, 0))),
                        Transform::from_xyz(0.0, 0.0, -0.25)
                    )],
                )]
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

fn update_scoreboard(
    mut red_score: Single<(&mut TextSpan,), With<RedScoreMarker>>,
    mut blue_score: Single<(&mut TextSpan,), (With<BlueScoreMarker>, Without<RedScoreMarker>)>,
    score_query: Query<(&PlayerID, &Score)>,
) {
    let mut red = 0;
    let mut blue = 0;

    for (player_id, score) in score_query.iter() {
        if player_id.0 == 0 {
            red = score.0;
        } else {
            blue = score.0;
        }
    }

    red_score.0.0 = red.to_string();
    blue_score.0.0 = blue.to_string();
}
