use std::io::{Read, Write};

use crate::states::game::GameUpdate;
use crate::states::game::player::{Inventory, PLAYER_HEIGHT, PlayerActionsTracker, PlayerID};
use crate::states::game::world::BlockType;
use crate::states::{GameResults, PostGameUpdate};
use crate::{AppState, ControlServer, client::GameConnection};
use agentduels_protocol::packets::Rotation;
use agentduels_protocol::{
    Item, Packet,
    packets::{PlayerActions, PlayerActionsPacket},
};
use bevy::prelude::*;
use fastrand::Rng;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize)]
pub enum ControlMsgS2C {
    TickStart {
        tick: u64,
        opponent_prev_actions: PlayerActions,
        player_position: Vec3,
        opponent_position: Vec3,
    },
    HealthUpdate {
        player_id: u16,
        new_health: f32,
    },
    Death {
        player_id: u16,
    },
    Goal {
        player_id: u16,
    },
    BlockUpdate(IVec3, BlockType),
    InventoryUpdate {
        player_id: u16,
        new_contents: Inventory,
    },
}

#[derive(Resource, Default)]
pub struct ControlMsgQueue(Vec<ControlMsgS2C>);

impl ControlMsgQueue {
    pub fn push(&mut self, msg: ControlMsgS2C) {
        self.0.push(msg);
    }
}

/// Note: Different actions that use the player's hands cannot be executed together in the same tick. The action that is received first will be executed, and later actions will be discarded.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum ControlMsgC2S {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    Jump,
    /// Rotations do not accumulate within a tick; the last one received is used.
    Rotate(Rotation),
    SelectItem(Item),
    Attack,
    UseItem,
    PlaceBlock,
    DigBlock,
    EndTick,
}

#[derive(Event)]
pub struct OpponentDisconnected;

pub struct NetworkPlugin {
    headless: bool,
}

impl NetworkPlugin {
    pub fn new(headless: bool) -> Self {
        NetworkPlugin { headless }
    }
}

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        let systems = (
            run_game_update,
            process_opponent_actions,
            send_control_start.after(gen_seed).after(advance_rng),
            gen_seed,
            advance_rng,
        );

        app.init_resource::<NetworkState>()
            .init_resource::<ControlMsgQueue>();

        if !self.headless {
            app.add_systems(Update, (systems.run_if(in_state(AppState::Game)),))
                .add_observer(handle_opponent_disconnect);
        } else {
            app.add_systems(Update, systems);
        }
    }
}

fn hash_actions(actions: &PlayerActions, nonce: u128) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&nonce.to_le_bytes());
    hasher.update(actions.as_bytes().as_slice());
    let hash = hasher.finalize().into();
    hash
}

// TODO - Don't block the main thread while waiting for input
fn run_game_update(world: &mut World) {
    let net_state = world.resource::<NetworkState>();
    if net_state.phase != NetworkPhase::AwaitingAction {
        return;
    }
    let prev_actions = net_state.prev_actions;
    let prev_nonce = net_state.nonce;

    let mut control_server = world.resource_mut::<ControlServer>();
    let mut message_buffer = control_server.message_buffer.lock().unwrap();
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
    control_server.tick_start_messages = None;

    let mut new_actions = PlayerActions::default();
    for msg in messages {
        match msg {
            ControlMsgC2S::MoveForward => new_actions.set(PlayerActions::MOVE_FORWARD),
            ControlMsgC2S::MoveBackward => new_actions.set(PlayerActions::MOVE_BACKWARD),
            ControlMsgC2S::MoveLeft => new_actions.set(PlayerActions::MOVE_LEFT),
            ControlMsgC2S::MoveRight => new_actions.set(PlayerActions::MOVE_RIGHT),
            ControlMsgC2S::Jump => new_actions.set(PlayerActions::JUMP),
            ControlMsgC2S::Rotate(rotation) => new_actions.rotation = rotation,
            ControlMsgC2S::SelectItem(item) => new_actions.item_change = Some(item),
            ControlMsgC2S::Attack => new_actions.checked_set(PlayerActions::ATTACK),
            ControlMsgC2S::UseItem => new_actions.checked_set(PlayerActions::USE_ITEM),
            ControlMsgC2S::PlaceBlock => new_actions.checked_set(PlayerActions::PLACE_BLOCK),
            ControlMsgC2S::DigBlock => new_actions.checked_set(PlayerActions::DIG_BLOCK),
            ControlMsgC2S::EndTick => break,
        }
    }

    // Set action tracker to previous actions (so the game update uses them) (we use previous actions for the current tick)
    let (_, mut actions) = world
        .query::<(&PlayerID, &mut PlayerActionsTracker)>()
        .iter_mut(world)
        .filter(|(player_id, _)| player_id.0 == 0)
        .next()
        .unwrap();
    actions.0 = prev_actions;

    world.run_schedule(GameUpdate);
    world.run_schedule(PostGameUpdate);

    let nonce: u128 = rand::random();
    let action_hash: [u8; 32] = hash_actions(&new_actions, nonce);

    world
        .resource_mut::<GameConnection>()
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

fn process_opponent_actions(
    mut net_state: ResMut<NetworkState>,
    mut connection: ResMut<GameConnection>,
    mut player_query: Query<(&PlayerID, &mut PlayerActionsTracker)>,
    mut commands: Commands,
) {
    if net_state.phase != NetworkPhase::AwaitingData {
        return;
    }

    let mut buf = [0; 128];
    let mut len = 0;
    match connection.socket.read(buf.as_mut_slice()) {
        Ok(0) => {
            commands.trigger(OpponentDisconnected);
        }
        Ok(n) => {
            len = n;
        }
        Err(e) => {
            if e.kind() != std::io::ErrorKind::WouldBlock {
                println!("Error reading from socket: {}", e);
                return;
            }
        }
    }

    if let Ok(packet) = connection.codec.read(&buf[..len])
        && let Some(packet) = packet
        && let Packet::PlayerActions(actions_packet) = packet
    {
        if net_state.tick > 0 {
            // Skip the first tick since we have no previous data
            let expected_hash: [u8; 32] =
                hash_actions(&actions_packet.prev_actions, actions_packet.nonce);
            assert_eq!(
                expected_hash, net_state.prev_hash,
                "Opponent's previous actions hash does not match expected hash! (Action: {:?}, Bits: {:b}, Nonce: {})",
                actions_packet.prev_actions, actions_packet.prev_actions.bits, actions_packet.nonce
            );
        }

        let (_, mut opponent_actions) = player_query
            .iter_mut()
            .filter(|(player_id, _)| player_id.0 == 1)
            .next()
            .unwrap();
        opponent_actions.0 = actions_packet.prev_actions;

        net_state.prev_hash = actions_packet.action_hash;
        net_state.tick += 1;
        net_state.phase = NetworkPhase::StartingTick;
    }
}

fn send_control_start(
    mut control_server: ResMut<ControlServer>,
    mut net_state: ResMut<NetworkState>,
    mut control_msg_queue: ResMut<ControlMsgQueue>,
    player_query: Query<(&PlayerID, &PlayerActionsTracker, &Transform)>,
) {
    let (_, _, player_transform) = player_query
        .iter()
        .filter(|(player_id, _, _)| player_id.0 == 0)
        .next()
        .unwrap();
    let (_, opponent_actions, opponent_transform) = player_query
        .iter()
        .filter(|(player_id, _, _)| player_id.0 == 1)
        .next()
        .unwrap();
    if net_state.phase != NetworkPhase::StartingTick || control_server.client.is_none() {
        return;
    }
    net_state.phase = NetworkPhase::AwaitingAction;

    control_msg_queue.0.push(ControlMsgS2C::TickStart {
        tick: net_state.tick,
        opponent_prev_actions: opponent_actions.0,
        player_position: player_transform.translation - Vec3::ZERO.with_y(PLAYER_HEIGHT / 2.0),
        opponent_position: opponent_transform.translation - Vec3::ZERO.with_y(PLAYER_HEIGHT / 2.0),
    });
    let messages = "[".to_string()
        + &control_msg_queue
            .0
            .drain(..)
            .map(|msg| serde_json::to_string(&msg).unwrap())
            .collect::<Vec<_>>()
            .join(",")
        + "]";
    control_server.tick_start_messages = Some(messages.clone());

    let Some(stream) = control_server.client.as_mut() else {
        return;
    };

    stream.write(messages.as_bytes()).unwrap();
}

fn handle_opponent_disconnect(_: On<OpponentDisconnected>, mut commands: Commands) {
    commands.insert_resource(GameResults { winner: None });
    commands.set_state(AppState::EndMenu);
}

/// Random number generator for the game.
#[derive(Resource)]
pub struct GameRng(Rng);

impl GameRng {
    /// Don't let consumers change the internal state to ensure all systems get the same random values regardless of execution order.
    /// The internal RNG is changed each tick by the `advance_rng` system.
    pub fn clone_rng(&self) -> Rng {
        self.0.clone()
    }
}

// Generate the RNG seed based on the match ID.
fn gen_seed(seed: Option<Res<GameRng>>, connection: Res<GameConnection>, mut commands: Commands) {
    if seed.is_some() {
        return;
    }
    let seed = Rng::with_seed(connection.match_id());
    commands.insert_resource(GameRng(seed));
}

fn advance_rng(seed: Option<ResMut<GameRng>>) {
    let Some(mut seed) = seed else {
        return;
    };
    // Change the internal state of the RNG each tick to ensure different random values each tick
    seed.0.u64(0..u64::MAX);
}
