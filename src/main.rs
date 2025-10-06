use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};

use bevy::prelude::*;

use crate::states::{GamePlugin, JoiningPlugin, MainMenuPlugin};

mod client;
mod states;

const CONTROL_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8082);

#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
enum AppState {
    MainMenu,
    Joining,
    Game,
}

#[derive(Component)]
struct AutoDespawn(AppState);

#[derive(Resource)]
struct ControlServer {
    listener: TcpListener,
    client: Option<TcpStream>,
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind(CONTROL_ADDR).unwrap();
    listener.set_nonblocking(true).unwrap();

    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .insert_resource(AmbientLight {
            brightness: 400.0,
            ..default()
        })
        .insert_state(AppState::MainMenu)
        .add_plugins((MainMenuPlugin, JoiningPlugin, GamePlugin))
        .insert_resource(ControlServer {
            listener,
            client: None,
        })
        .add_systems(Update, handle_connection)
        .add_systems(OnExit(AppState::Joining), cleanup_state)
        .add_systems(OnExit(AppState::MainMenu), cleanup_state)
        .add_systems(OnExit(AppState::Game), cleanup_state)
        .run();
}

fn cleanup_state(
    mut commands: Commands,
    query: Query<(Entity, &AutoDespawn)>,
    current_state: Res<State<AppState>>,
) {
    for (entity, auto_despawn) in query.iter() {
        if auto_despawn.0 != *current_state.get() {
            commands.entity(entity).despawn();
        }
    }
}

fn handle_connection(mut server: ResMut<ControlServer>) {
    if server.client.is_some() {
        return;
    }
    let Ok((stream, _)) = server.listener.accept() else {
        return;
    };
    server.client = Some(stream);
}
