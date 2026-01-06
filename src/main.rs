use crate::states::network::ControlMsgC2S;
use crate::states::{EndMenuPlugin, GamePlugin, JoiningPlugin, MainMenuPlugin};
use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use std::io::Write;
use std::{
    io::Read,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

mod client;
mod states;

const CONTROL_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8082);

#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
enum AppState {
    MainMenu,
    Joining,
    Game,
    EndMenu,
}

#[derive(Component)]
struct AutoDespawn(AppState);

type ClientID = usize;

#[derive(Resource)]
struct ControlServer {
    listener: TcpListener,
    client: Option<TcpStream>,
    client_id: ClientID,
    disconnect_queue: Arc<Mutex<Vec<ClientID>>>,
    message_buffer: Arc<Mutex<Vec<ControlMsgC2S>>>,
    tick_start_messages: Option<String>,
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind(CONTROL_ADDR).unwrap();
    listener.set_nonblocking(true).unwrap();

    App::new()
        .add_plugins((
            DefaultPlugins.set(ImagePlugin::default_nearest()),
            #[cfg(debug_assertions)]
            EguiPlugin::default(),
            #[cfg(debug_assertions)]
            WorldInspectorPlugin::new(),
        ))
        .insert_resource(AmbientLight {
            brightness: 400.0,
            ..default()
        })
        .insert_state(AppState::MainMenu)
        .add_plugins((MainMenuPlugin, JoiningPlugin, EndMenuPlugin, GamePlugin))
        .insert_resource(ControlServer {
            listener,
            client: None,
            client_id: 0,
            disconnect_queue: Arc::new(Mutex::new(Vec::new())),
            message_buffer: Arc::new(Mutex::new(Vec::new())),
            tick_start_messages: None,
        })
        .add_systems(FixedUpdate, (handle_connection, handle_disconnects))
        .add_systems(OnExit(AppState::Joining), cleanup_state)
        .add_systems(OnExit(AppState::MainMenu), cleanup_state)
        .add_systems(OnExit(AppState::Game), cleanup_state)
        .add_systems(OnExit(AppState::EndMenu), cleanup_state)
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
        let _ = client.shutdown(std::net::Shutdown::Both);
    }
    server.client = Some(stream.try_clone().unwrap());
    server.client_id += 1;
    let client_id = server.client_id;
    let disconnect_queue = server.disconnect_queue.clone();
    let message_buffer = server.message_buffer.clone();
    let tick_start_messages = server.tick_start_messages.clone();
    thread::spawn(move || {
        if let Some(message) = tick_start_messages {
            stream.write(message.as_bytes()).unwrap();
        }
        let mut buf = [0; 128];
        loop {
            let n = stream.read(&mut buf).unwrap_or(0);
            if n == 0 {
                disconnect_queue.lock().unwrap().push(client_id);
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

fn handle_disconnects(mut server: ResMut<ControlServer>) {
    let disconnects = server
        .disconnect_queue
        .lock()
        .unwrap()
        .drain(..)
        .collect::<Vec<_>>();
    for client_id in disconnects {
        if server.client_id == client_id {
            server.client = None;
        }
    }
}
