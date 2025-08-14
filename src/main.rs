use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};

use agentduels::SERVER_ADDR;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};

use crate::networking::GameClient;

mod networking;

const CONTROL_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8082);

#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
enum AppState {
    MainMenu,
    Joining,
    Game,
}

#[derive(Component)]
struct PlayButton;

#[derive(Component)]
struct ConnectionText;

#[tokio::main]
async fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_state(AppState::MainMenu)
        .add_systems(Startup, (setup, create_listener))
        .add_systems(
            Update,
            (
                handle_connection,
                check_connection,
                button_press,
                poll_connection.run_if(in_state(AppState::Joining)),
            ),
        )
        .add_systems(OnEnter(AppState::Joining), start_connection)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d::default());

    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![
            (
                Text::new("AgentDuels"),
                TextFont::default().with_font_size(100.0),
            ),
            (
                PlayButton,
                Button::default(),
                Node {
                    width: Val::Px(150.0),
                    height: Val::Px(65.0),
                    border: UiRect::all(Val::Px(5.0)),
                    margin: UiRect::default()
                        .with_top(Val::Px(20.0))
                        .with_bottom(Val::Px(20.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor(Color::BLACK),
                BorderRadius::MAX,
                BackgroundColor(Color::Srgba(Srgba::GREEN)),
                children![Text::new("Join Game"),],
            ),
            (
                ConnectionText,
                Text::new("No client connected"),
                TextColor(Color::Srgba(Srgba::RED)),
            )
        ],
    ));
}

#[derive(Component)]
struct ControlServer {
    listener: TcpListener,
    client: Option<TcpStream>,
}

fn create_listener(mut commands: Commands) {
    let listener = TcpListener::bind(CONTROL_ADDR).unwrap();
    listener.set_nonblocking(true).unwrap();
    commands.spawn((ControlServer {
        listener,
        client: None,
    },));
}

fn handle_connection(
    mut server_query: Query<&mut ControlServer>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<ConnectionText>>,
) {
    if let Ok(mut server) = server_query.single_mut() {
        while let Ok((stream, _)) = server.listener.accept() {
            if server.client.is_none() {
                server.client = Some(stream);
                if let Ok((mut text, mut color)) = text_query.single_mut() {
                    text.0 = "Client connected".to_string();
                    color.0 = Color::Srgba(Srgba::GREEN);
                }
            }
        }
    }
}

fn check_connection(
    server_query: Query<&ControlServer>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<ConnectionText>>,
) {
    let Ok(server) = server_query.single() else {
        return;
    };
    if server.client.is_none() {
        if let Ok((mut text, mut color)) = text_query.single_mut() {
            text.0 = "No client connected".to_string();
            color.0 = Color::Srgba(Srgba::RED);
        }
        return;
    };
}

fn button_press(
    mut next_state: ResMut<NextState<AppState>>,
    button_query: Query<&Interaction, (With<PlayButton>, Changed<Interaction>)>,
) {
    if let Ok(interaction) = button_query.single() {
        if *interaction == Interaction::Pressed {
            next_state.set(AppState::Joining);
        }
    }
}

#[derive(Component)]
struct ConnectingTask(Task<Result<GameClient, anyhow::Error>>);

fn start_connection(mut commands: Commands) {
    println!("Starting connection to game server...");
    let task_pool = AsyncComputeTaskPool::get();
    let task = task_pool.spawn(async move { GameClient::connect(SERVER_ADDR) });
    commands.spawn(ConnectingTask(task));
}

fn poll_connection(
    mut commands: Commands,
    mut task_query: Query<(Entity, &mut ConnectingTask)>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (entity, mut connecting_task) in task_query.iter_mut() {
        if let Some(result) = block_on(future::poll_once(&mut connecting_task.0)) {
            match result {
                Ok(client) => {
                    println!("Connected to game server");
                    commands.spawn(client);
                    next_state.set(AppState::Game);
                }
                Err(e) => {
                    eprintln!("Failed to connect: {}", e);
                    next_state.set(AppState::MainMenu);
                }
            }
            commands.entity(entity).despawn();
        }
    }
}
