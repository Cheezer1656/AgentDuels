use crate::player::{PLAYER_ANIMATION_INDICES, PlayerBody, PlayerBundle, PlayerHand, SPAWN_POSITIONS, SPAWN_ROTATIONS, Inventory};
use crate::player::{PlayerHead, PlayerID};
use crate::world::{WorldPlugin, init_map};
use crate::{
    AppState, AutoDespawn, ControlServer,
    states::game::{gameloop::GameLoopPlugin, network::NetworkPlugin},
};
use bevy::scene::SceneInstanceReady;
use bevy::{
    ecs::schedule::ScheduleLabel,
    input::mouse::MouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use bevy_inspector_egui::bevy_egui;
use crate::client::GameConnection;

mod gameloop;
pub mod network;

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Clone, Debug)]
pub struct GameUpdate;

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Clone, Debug)]
pub struct PostGameUpdate;

#[derive(Component)]
struct RedScoreMarker;

#[derive(Component)]
struct BlueScoreMarker;

#[derive(Component)]
struct TPSMarker;

#[derive(Component)]
struct ClientStatusMarker;

pub struct GamePlugin {
    headless: bool,
}

impl GamePlugin {
    pub fn new(headless: bool) -> Self {
        Self { headless }
    }
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_schedule(Schedule::new(GameUpdate))
            .add_schedule(Schedule::new(PostGameUpdate))
            .add_plugins((
                WorldPlugin::new(self.headless),
                NetworkPlugin::new(self.headless),
                GameLoopPlugin,
            ));
        if !self.headless {
            app.add_systems(OnEnter(AppState::Game), (setup, cursor_grab))
                .add_systems(OnExit(AppState::Game), cursor_ungrab)
                .add_systems(
                    Update,
                    (toggle_cursor_grab, move_cam).run_if(in_state(AppState::Game)),
                )
                .add_systems(
                    FixedUpdate,
                    update_client_status.run_if(resource_changed::<ControlServer>),
                );
        } else {
            app.add_systems(Startup, setup);
        }
    }
}

fn setup(
    mut commands: Commands,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    assets: Res<AssetServer>,
    control_server: Res<ControlServer>,
    game_connection: Res<GameConnection>,
) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb_u8(48, 193, 255)),
            ..default()
        },
        Msaa::Off,
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
        bevy_egui::PrimaryEguiContext,
        AutoDespawn(AppState::Game),
    ));

    commands.spawn((
        AutoDespawn(AppState::Game),
        Node {
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            height: Val::Percent(100.0),
            width: Val::Percent(100.0),
            ..default()
        },
        children![(
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
                height: Val::Px(90.0),
                width: Val::Px(180.0),
                ..default()
            }
        )],
    ));

    commands.spawn((
        AutoDespawn(AppState::Game),
        Text2d::new("TPS: "),
        TextFont::default(),
        children![(TPSMarker, TextSpan("0".to_string()))],
        Node {
            height: Val::Px(20.0),
            width: Val::Percent(100.0),
            ..default()
        },
    ));

    commands.spawn((
        AutoDespawn(AppState::Game),
        Text2d::new("Client status: "),
        TextFont::default(),
        children![(
            ClientStatusMarker,
            TextSpan(
                (if control_server.client.is_some() {
                    "Connected"
                } else {
                    "Disconnected"
                })
                .to_string()
            ),
        ),],
        Node {
            margin: UiRect::default().with_top(Val::Px(20.0)),
            height: Val::Px(90.0),
            width: Val::Percent(100.0),
            ..default()
        },
    ));

    commands.spawn((init_map(), AutoDespawn(AppState::Game)));

    for i in 0..2_i32 {
        let mut body_transform = Transform::from_xyz(0.0, -0.9, 0.0);
        body_transform.rotation = Quat::from_rotation_y(SPAWN_ROTATIONS[i as usize]);

        let gltf_path = format!("models/{}.gltf#Scene0", if i == game_connection.player_id.0 as i32 { "player" } else { "opponent" });
        let mut graph = AnimationGraph::new();
        for i in 0..5 {
            graph.add_clip(
                assets.load(GltfAssetLabel::Animation(i).from_asset(gltf_path.clone())),
                match i {
                    0..=1 => 0.01,
                    _ => 1.0,
                },
                PLAYER_ANIMATION_INDICES.root.into(),
            );
        }

        commands
            .spawn((
                PlayerBundle {
                    id: PlayerID(i as u16),
                    transform: Transform::from_translation(SPAWN_POSITIONS[i as usize]),
                    ..default()
                },
                Visibility::default(),
                AutoDespawn(AppState::Game),
            ))
            .with_children(|parent| {
                parent
                    .spawn((
                        AnimationGraphHandle(graphs.add(graph)),
                        SceneRoot(assets.load(gltf_path)),
                        PlayerBody,
                        body_transform,
                    ))
                    .observe(play_animation_on_ready)
                    .observe(mark_entities_on_ready);
            });
    }
}

fn play_animation_on_ready(
    scene_ready: On<SceneInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    graph_handles: Query<&AnimationGraphHandle>,
    mut anim_players: Query<&mut AnimationPlayer>,
) {
    if let Ok(graph_handle) = graph_handles.get(scene_ready.entity) {
        for child in children.iter_descendants(scene_ready.entity) {
            if let Ok(mut anim_player) = anim_players.get_mut(child) {
                anim_player
                    .play(PLAYER_ANIMATION_INDICES.idle.into())
                    .repeat();
                anim_player
                    .play(PLAYER_ANIMATION_INDICES.walk.into())
                    .repeat()
                    .pause();

                commands
                    .entity(child)
                    .insert(AnimationGraphHandle(graph_handle.0.clone()));
            }
        }
    }
}

fn mark_entities_on_ready(
    scene_ready: On<SceneInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    mut names: Query<(Entity, &mut Name)>,
    mut player_query: Query<&mut Inventory>,
) {
    let mut marked = false;
    for child in children.iter_descendants(scene_ready.entity) {
        let Ok((entity, name)) = names.get_mut(child) else {
            continue;
        };
        match name.as_str() {
            "head" => {
                commands.entity(entity).insert(PlayerHead);
            }
            "hand" => {
                commands.entity(entity).insert(PlayerHand);
            }
            _ => {}
        }
        marked = true;
    }
    if marked {
        for mut inventory in player_query.iter_mut() {
            inventory.set_changed();
        }
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

fn toggle_cursor_grab(
    mut cursor_opts: Single<&mut CursorOptions, With<PrimaryWindow>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    if keyboard_input.just_pressed(KeyCode::Escape) {
        if cursor_opts.grab_mode == CursorGrabMode::None {
            cursor_opts.grab_mode = CursorGrabMode::Confined;
            cursor_opts.visible = false;
        } else {
            cursor_opts.grab_mode = CursorGrabMode::None;
            cursor_opts.visible = true;
        }
    }
}

fn move_cam(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    cursor_opts: Single<&CursorOptions, With<PrimaryWindow>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut camera: Query<(&mut Transform, &Camera3d)>,
) {
    if cursor_opts.grab_mode == CursorGrabMode::None {
        return;
    }
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

fn update_client_status(
    mut text_query: Query<&mut TextSpan, With<ClientStatusMarker>>,
    control_server: Res<ControlServer>,
) {
    let Ok(mut text_span) = text_query.single_mut() else {
        return;
    };
    text_span.0 = (if control_server.client.is_some() {
        "Connected"
    } else {
        "Disconnected"
    })
    .to_string();
}
