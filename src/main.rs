use std::{
    io::Read,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

use agentduels_protocol::packets::PlayerActions;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize)]
pub enum ControlMsgS2C {
    TickStart {
        tick: u64,
        opponent_prev_actions: PlayerActions,
    },
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum ControlMsgC2S {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    /// Rotations do not accumulate within a tick; the last one received is used.
    Rotate(f32, f32),
    EndTick,
}

#[derive(Resource)]
struct ControlServer {
    listener: TcpListener,
    client: Option<TcpStream>,
    message_buffer: Arc<Mutex<Vec<ControlMsgC2S>>>,
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
            message_buffer: Arc::new(Mutex::new(Vec::new())),
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
    let Ok((mut stream, _)) = server.listener.accept() else {
        return;
    };
    if let Some(client) = &server.client {
        client.shutdown(std::net::Shutdown::Both).unwrap();
    }
    server.client = Some(stream.try_clone().unwrap());
    let message_buffer = server.message_buffer.clone();
    thread::spawn(move || {
        let mut buf = [0; 128];
        loop {
            let n = stream.read(&mut buf).unwrap();
            if n == 0 {
                println!("Control connection closed");
                break;
            }
            for msg in serde_json::Deserializer::from_slice(&buf[..n])
                .into_iter::<ControlMsgC2S>()
                .flatten()
            {
                message_buffer.lock().unwrap().push(msg);
            }
            buf.fill(0);
        }
    });
}
