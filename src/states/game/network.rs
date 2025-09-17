use std::io::Read;

use crate::client::GameConnection;
use agentduels_protocol::{
    Packet,
    packets::{PlayerActions, PlayerActionsPacket},
};
use bevy::prelude::*;

use crate::states::{game::GameUpdate};

#[derive(Resource, Default)]
struct NetworkState {
    tick: u64,
    phase: NetworkPhase,
    prev_actions: PlayerActions, // Our actions from the last tick
    nonce: u128, // Our nonce from the last tick
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

#[derive(Event)]
pub struct PacketEvent(Packet);

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkState>()
            .init_resource::<PlayerActionsTracker>()
            .init_resource::<OpponentActionsTracker>()
            .add_event::<PacketEvent>()
            .insert_resource(IncomingBuffer(Vec::new()))
            .add_systems(
                Update,
                (run_game_update, receive_packets, process_opponent_actions).run_if(in_state(crate::AppState::Game)),
            );
    }
}

fn run_game_update(world: &mut World) {
    let net_state = world.resource::<NetworkState>();
    if net_state.phase != NetworkPhase::AwaitingAction {
        return;
    }
    let prev_actions = net_state.prev_actions;
    let prev_nonce = net_state.nonce;

    world.run_schedule(GameUpdate);

    let action = world.resource::<PlayerActionsTracker>().0;
    let mut connection = world.resource_mut::<GameConnection>();

    let nonce: u128 = rand::random();
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[action.0]);
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
    mut packet_ev: EventWriter<PacketEvent>,
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

fn process_opponent_actions(mut packet_ev: EventReader<PacketEvent>, mut net_state: ResMut<NetworkState>, mut opponent_actions: ResMut<OpponentActionsTracker>) {
    if net_state.phase != NetworkPhase::AwaitingData {
        return;
    }
    for PacketEvent(packet) in packet_ev.read() {
        if let Packet::PlayerActions(actions) = packet {
            if net_state.tick > 0 { // Skip the first tick since we have no previous data
                let mut hasher = blake3::Hasher::new();
                hasher.update(&[actions.prev_actions.0]);
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