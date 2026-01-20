#![feature(mpmc_channel)]

use crate::player::{HeadRotation, Inventory, PlayerAnimation, PlayerID};
use crate::states::network::ControlMsgC2S;
use crate::world::BlockType;
use avian3d::prelude::PhysicsLayer;
use bevy::math::{IVec3, Vec3};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

pub mod client;
pub mod player;
pub mod states;
pub mod world;

pub const GAME_VERSION: u32 = 0;
pub const SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
pub const SERVER_URL: &str = "ws://127.0.0.1:8081";

#[derive(Component, Serialize, Deserialize, Default, Debug, Clone)]
pub struct PlayerInfo {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub velocity: Vec3,
    pub health_update: Option<f32>,
    pub inventory_update: Option<Inventory>,
    pub animation: PlayerAnimation,
    pub hurt_update: Option<bool>,
}

pub type ArrowID = u32;

#[derive(Serialize, Deserialize, Clone)]
pub enum ArrowEvent {
    Updated {
        id: ArrowID,
        position: Vec3,
        rotation: Quat,
    },
    Despawned(ArrowID),
}

#[derive(Serialize, Deserialize)]
pub struct TickMessage {
    pub tick: u64,
    pub players: [PlayerInfo; 2],
    pub deaths: HashSet<PlayerID>,
    pub goals: Option<PlayerID>,
    pub block_updates: Vec<(IVec3, BlockType)>,
    pub arrow_events: Vec<ArrowEvent>,
    pub game_results: Option<GameResults>,
}

type ClientID = usize;

#[derive(Resource)]
pub struct ControlServer {
    listener: TcpListener,
    client: Option<TcpStream>,
    client_id: ClientID,
    disconnect_queue: Arc<Mutex<Vec<ClientID>>>,
    message_buffer: Arc<Mutex<Vec<ControlMsgC2S>>>,
    tick_start_messages: Option<Vec<u8>>,
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
            stream.write(message.as_slice()).unwrap();
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

#[derive(PhysicsLayer, Default)]
pub enum CollisionLayer {
    #[default]
    Default,
    Player,
    Projectile,
    World,
}

pub const ARROW_HEIGHT: f32 = 0.5;
pub const ARROW_WIDTH: f32 = 0.5;

#[derive(Component, Default)]
pub struct Arrow {
    pub id: ArrowID,
    pub ticks_in_ground: usize,
}

#[derive(Resource, Serialize, Deserialize, Clone)]
pub struct GameResults {
    pub winner: Option<u16>,
    pub reason: String,
}
