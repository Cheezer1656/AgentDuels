use std::io::{Read, Write};

use crate::{ControlMsgC2S, ControlMsgS2C, ControlServer, client::GameConnection};
use agentduels_protocol::{
    Packet,
    packets::{PlayerActions, PlayerActionsPacket},
};
use bevy::prelude::*;
use fastrand::Rng;
use crate::states::PostGameUpdate;
use crate::states::game::GameUpdate;

#[derive(Resource, Default, Debug)]
struct NetworkState {
    tick: u64,
    phase: NetworkPhase,
    prev_actions: PlayerActions, // Our actions from the last tick
    nonce: u128,                 // Our nonce from the last tick
    prev_hash: [u8; 32], // The hash of the opponent's latest actions (sent in the last tick)
}

#[derive(Default, Debug, PartialEq, Eq)]
enum NetworkPhase {
    #[default]
    StartingTick,
    AwaitingAction,
    AwaitingData,
}

#[derive(Resource, Default, Clone, Copy)]
pub struct PlayerActionsTracker(pub PlayerActions);

#[derive(Message)]
pub struct PacketEvent(Packet);

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
                (
                    run_game_update,
                    receive_packets,
                    process_opponent_actions,
                    send_control_start.after(gen_seed).after(regen_seed),
                    gen_seed,
                    regen_seed,
                )
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

    let mut message_buffer = world
        .resource::<ControlServer>()
        .message_buffer
        .lock()
        .unwrap();
    let Some(end_idx) = message_buffer
        .iter()
        .position(|m| *m == ControlMsgC2S::EndTick)
    else {
        // We haven't received the end of the tick yet
        return;
    };
    let messages = message_buffer[..=end_idx].to_vec();
    message_buffer.drain(..=end_idx);
    drop(message_buffer);

    let mut new_actions = PlayerActions::default();
    for msg in messages {
        match msg {
            ControlMsgC2S::MoveForward => new_actions.set(PlayerActions::MOVE_FORWARD),
            ControlMsgC2S::MoveBackward => new_actions.set(PlayerActions::MOVE_BACKWARD),
            ControlMsgC2S::MoveLeft => new_actions.set(PlayerActions::MOVE_LEFT),
            ControlMsgC2S::MoveRight => new_actions.set(PlayerActions::MOVE_RIGHT),
            ControlMsgC2S::Jump => new_actions.set(PlayerActions::JUMP),
            ControlMsgC2S::Rotate(x, y) => new_actions.rotation = [x, y],
            ControlMsgC2S::SelectItem(item) => new_actions.item_change = Some(item),
            ControlMsgC2S::PlaceBlock => new_actions.set(PlayerActions::PLACE_BLOCK),
            ControlMsgC2S::EndTick => break,
        }
    }

    // Set action tracker to previous actions (so the game update uses them) (we use previous actions for the current tick)
    let mut actions = world.resource_mut::<PlayerActionsTracker>();
    actions.0 = prev_actions;

    world.run_schedule(GameUpdate);
    world.run_schedule(PostGameUpdate);
    let mut connection = world.resource_mut::<GameConnection>();

    let nonce: u128 = rand::random();
    let mut hasher = blake3::Hasher::new();
    hasher.update(&nonce.to_le_bytes());
    hasher.update(new_actions.as_bytes().as_slice());
    let action_hash: [u8; 32] = hasher.finalize().into();

    connection
        .send_packet(Packet::PlayerActions(Box::new(PlayerActionsPacket {
            prev_actions,
            nonce: prev_nonce,
            action_hash,
        })))
        .unwrap();

    let mut net_state = world.resource_mut::<NetworkState>();
    net_state.prev_actions = new_actions;
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
        if let Packet::PlayerActions(actions_packet) = packet {
            if net_state.tick > 0 {
                // Skip the first tick since we have no previous data
                let mut hasher = blake3::Hasher::new();
                hasher.update(&actions_packet.nonce.to_le_bytes());
                hasher.update(&actions_packet.prev_actions.as_bytes());
                let expected_hash: [u8; 32] = hasher.finalize().into();
                if expected_hash != net_state.prev_hash {
                    panic!("Opponent's previous actions hash does not match expected hash");
                }
            }

            opponent_actions.0 = actions_packet.prev_actions;

            net_state.prev_hash = actions_packet.action_hash;
            net_state.tick += 1;
            net_state.phase = NetworkPhase::StartingTick;
        }
    }
}

fn send_control_start(
    mut control_server: ResMut<ControlServer>,
    mut net_state: ResMut<NetworkState>,
    opponent_actions: Res<OpponentActionsTracker>,
) {
    if net_state.phase != NetworkPhase::StartingTick {
        return;
    }
    net_state.phase = NetworkPhase::AwaitingAction;

    let stream = control_server.client.as_mut().unwrap();

    stream
        .write(
            serde_json::to_string(&ControlMsgS2C::TickStart {
                tick: net_state.tick,
                opponent_prev_actions: opponent_actions.0,
            })
            .unwrap()
            .as_bytes(),
        )
        .unwrap();
}

/// Random number generator seeded each tick based on both players' nonces
#[derive(Resource)]
pub struct GameRng(fastrand::Rng);

impl GameRng {
    /// Don't let consumers change the internal state to ensure all systems get the same random values regardless of execution order.
    /// The internal RNG is changed each tick by the `regen_seed` system.
    pub fn clone_rng(&self) -> Rng {
        self.0.clone()
    }
}

fn gen_seed(seed: Option<Res<GameRng>>, net_state: Res<NetworkState>, mut packet_ev: MessageReader<PacketEvent>, mut commands: Commands) {
    if seed.is_some() {
        return;
    }
    for PacketEvent(packet) in packet_ev.read() {
        if let Packet::PlayerActions(actions_packet) = packet {
            // Combine both nonces to generate the seed
            let rng = fastrand::Rng::with_seed((net_state.nonce ^ actions_packet.nonce) as u64);
            commands.insert_resource(GameRng(rng));
        };
    }
}

fn regen_seed(seed: Option<ResMut<GameRng>>) {
    let Some(mut seed) = seed else {
        return;
    };
    // Change the internal state of the RNG each tick to ensure different random values each tick
    seed.0.u64(0..u64::MAX);
}