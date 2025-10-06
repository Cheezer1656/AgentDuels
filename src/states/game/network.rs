use std::io::{Read, Write};

use crate::{client::GameConnection, ControlServer};
use agentduels_protocol::{
    Packet,
    packets::{PlayerActions, PlayerActionsPacket},
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::states::game::GameUpdate;

#[derive(Resource, Default)]
struct NetworkState {
    tick: u64,
    phase: NetworkPhase,
    prev_actions: PlayerActions, // Our actions from the last tick
    nonce: u128,                 // Our nonce from the last tick
    prev_hash: [u8; 32], // The hash of the opponent's latest actions (sent in the last tick)
}

#[derive(Default, PartialEq, Eq)]
enum NetworkPhase {
    #[default]
    AwaitingAction,
    AwaitingData,
}

#[derive(Resource, Default, Clone, Copy)]
pub struct PlayerActionsTracker(pub PlayerActions);

#[derive(Message)]
pub struct PacketEvent(Packet);

#[derive(Serialize)]
pub enum ControlMsgS2C {
    TickStart {
        tick: u64,
        opponent_prev_actions: PlayerActions,
    }
}

#[derive(Deserialize)]
pub enum ControlMsgC2S {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    /// Rotations do not accumulate within a tick; the last one received is used.
    Rotate(f32, f32),
    EndTick,
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkState>()
            .init_resource::<PlayerActionsTracker>()
            .init_resource::<OpponentActionsTracker>()
            .add_message::<PacketEvent>()
            .insert_resource(IncomingBuffer(Vec::new()))
            .add_systems(
                Update,
                (run_game_update, receive_packets, process_opponent_actions)
                    .run_if(in_state(crate::AppState::Game)),
            );
    }
}

// TODO - Don't block the main thread while waiting for input
fn run_game_update(world: &mut World) {
    let net_state = world.resource::<NetworkState>();
    if net_state.phase != NetworkPhase::AwaitingAction {
        return;
    }
    let prev_actions = net_state.prev_actions;
    let prev_nonce = net_state.nonce;
    let tick = net_state.tick;

    let mut actions = world.resource_mut::<PlayerActionsTracker>();
    actions.0 = PlayerActions::default(); // Reset actions for this tick

    let opp_actions = world.resource::<OpponentActionsTracker>().0;

    let mut control_server = world
        .resource_mut::<ControlServer>();
    let stream = control_server
        .client
        .as_mut()
        .unwrap();

    stream.write(serde_json::to_string(&ControlMsgS2C::TickStart {
        tick: tick,
        opponent_prev_actions: opp_actions,
    }).unwrap().as_bytes()).unwrap();

    let mut new_actions = PlayerActions::default();
    let mut buf = [0; 1024];
    loop {
        let Ok(n) = stream.read(&mut buf) else {
            return;
        };
        let Ok(msg) = serde_json::from_slice::<ControlMsgC2S>(&buf[..n]) else {
            return;
        };
        match msg {
            ControlMsgC2S::MoveForward => new_actions.set(PlayerActions::MOVE_FORWARD),
            ControlMsgC2S::MoveBackward => new_actions.set(PlayerActions::MOVE_BACKWARD),
            ControlMsgC2S::MoveLeft => new_actions.set(PlayerActions::MOVE_LEFT),
            ControlMsgC2S::MoveRight => new_actions.set(PlayerActions::MOVE_RIGHT),
            ControlMsgC2S::Rotate(x, y) => new_actions.rotation = [x, y],
            ControlMsgC2S::EndTick => break,
        }
        buf.fill(0);
    }

    let mut actions = world.resource_mut::<PlayerActionsTracker>();
    actions.0 = new_actions;

    world.run_schedule(GameUpdate);
    let action = world.resource::<PlayerActionsTracker>().0;
    let mut connection = world.resource_mut::<GameConnection>();

    let nonce: u128 = rand::random();
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[action.bits]);
    let rotation = action.rotation;
    hasher.update(&rotation[0].to_le_bytes());
    hasher.update(&rotation[1].to_le_bytes());
    hasher.update(&nonce.to_le_bytes());
    let action_hash: [u8; 32] = hasher.finalize().into();

    connection
        .send_packet(Packet::PlayerActions(Box::new(PlayerActionsPacket {
            prev_actions,
            nonce: prev_nonce,
            action_hash,
        })))
        .unwrap();

    let mut net_state = world.resource_mut::<NetworkState>();
    net_state.prev_actions = action;
    net_state.nonce = nonce;
    net_state.phase = NetworkPhase::AwaitingData;
}

#[derive(Resource)]
struct IncomingBuffer(Vec<u8>);

fn receive_packets(
    mut buf: ResMut<IncomingBuffer>,
    mut connection: ResMut<GameConnection>,
    mut packet_ev: MessageWriter<PacketEvent>,
) {
    let mut temp = [0u8; 1024];
    if let Ok(bytes_read) = connection.socket.read(&mut temp) {
        buf.0.extend_from_slice(&temp[..bytes_read]);
    }

    if let Ok(packets) = connection.codec.read(&buf.0) {
        for packet in packets {
            packet_ev.write(PacketEvent(packet));
        }
        buf.0.clear();
    }
}

#[derive(Resource, Default, Clone, Copy)]
pub struct OpponentActionsTracker(pub PlayerActions);

fn process_opponent_actions(
    mut packet_ev: MessageReader<PacketEvent>,
    mut net_state: ResMut<NetworkState>,
    mut opponent_actions: ResMut<OpponentActionsTracker>,
) {
    if net_state.phase != NetworkPhase::AwaitingData {
        return;
    }
    for PacketEvent(packet) in packet_ev.read() {
        if let Packet::PlayerActions(actions) = packet {
            if net_state.tick > 0 {
                // Skip the first tick since we have no previous data
                let mut hasher = blake3::Hasher::new();
                hasher.update(&[actions.prev_actions.bits]);
                let rotation = actions.prev_actions.rotation;
                hasher.update(&rotation[0].to_le_bytes());
                hasher.update(&rotation[1].to_le_bytes());
                hasher.update(&actions.nonce.to_le_bytes());
                let expected_hash: [u8; 32] = hasher.finalize().into();
                if expected_hash != net_state.prev_hash {
                    panic!("Opponent's previous actions hash does not match expected hash");
                }
            }

            opponent_actions.0 = actions.prev_actions;

            net_state.prev_hash = actions.action_hash;
            net_state.tick += 1;
            net_state.phase = NetworkPhase::AwaitingAction;
        }
    }
}
