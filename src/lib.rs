use crate::states::network::ControlMsgC2S;
use bevy::prelude::{Component, ResMut, Resource, States};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

pub mod client;
pub mod states;

pub const SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);

type ClientID = usize;

#[derive(Resource)]
pub struct ControlServer {
    listener: TcpListener,
    client: Option<TcpStream>,
    client_id: ClientID,
    disconnect_queue: Arc<Mutex<Vec<ClientID>>>,
    message_buffer: Arc<Mutex<Vec<ControlMsgC2S>>>,
    tick_start_messages: Option<String>,
}

impl ControlServer {
    pub fn new(listener: TcpListener) -> Self {
        ControlServer {
            listener,
            client: None,
            client_id: 0,
            disconnect_queue: Arc::new(Mutex::new(Vec::new())),
            message_buffer: Arc::new(Mutex::new(Vec::new())),
            tick_start_messages: None,
        }
    }
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    MainMenu,
    Joining,
    Game,
    EndMenu,
}

#[derive(Component)]
pub struct AutoDespawn(pub AppState);

pub fn handle_connection(mut server: ResMut<ControlServer>) {
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
        println!("Client {client_id} connected");
        let mut buf = [0; 128];
        loop {
            let n = stream.read(&mut buf).unwrap_or(0);
            if n == 0 {
                println!("Client {client_id} disconnected");
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

pub fn handle_disconnects(mut server: ResMut<ControlServer>) {
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
