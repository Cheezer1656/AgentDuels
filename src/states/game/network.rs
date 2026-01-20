use crate::client::GameConnectionMessage;
use crate::player::{Inventory, Item, PlayerActions, Rotation};
use crate::world::BlockType;
use crate::{AppState, ControlServer, GameResults, TickMessage, client::GameConnection};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Resource, Default, Debug)]
pub struct NetworkState {
    tick: u64,
}

#[derive(Message, Deref)]
pub struct TickEvent(pub TickMessage);

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
    /// In headless mode, you are responsible for resetting the network state between games.
    headless: bool,
}

impl NetworkPlugin {
    pub fn new(headless: bool) -> Self {
        NetworkPlugin { headless }
    }
}

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkState>()
            .add_message::<TickEvent>();

        let systems = (start_tick, end_tick);

        if !self.headless {
            app.add_systems(Update, systems.run_if(in_state(AppState::Game)))
                .add_systems(OnExit(AppState::Game), reset_state);
        } else {
            app.add_systems(Update, systems);
        }
    }
}

fn reset_state(mut net_state: ResMut<NetworkState>) {
    *net_state = NetworkState::default();
}

fn start_tick(
    game_connection: Res<GameConnection>,
    mut control_server: ResMut<ControlServer>,
    mut commands: Commands,
) {
    if !game_connection.socket.is_connected() {
        commands.insert_resource(GameResults {
            winner: None,
            reason: "Disconnected".to_string(),
        });
        commands.set_state(AppState::EndMenu);
        return;
    }

    let Ok(msg) = game_connection.receiver_rx.try_recv() else {
        return;
    };
    let data = match msg {
        workflow_websocket::client::Message::Binary(data) => data,
        _ => {
            let _ = game_connection
                .sender_tx
                .send(GameConnectionMessage::Disconnect);
            commands.insert_resource(GameResults {
                winner: None,
                reason: "Disconnected".to_string(),
            });
            commands.set_state(AppState::EndMenu);
            return;
        }
    };
    let Ok(msg) = postcard::from_bytes::<TickMessage>(&data) else {
        let _ = game_connection
            .sender_tx
            .send(GameConnectionMessage::Disconnect);
        commands.insert_resource(GameResults {
            winner: None,
            reason: "Disconnected".to_string(),
        });
        commands.set_state(AppState::EndMenu);
        return;
    };
    let tick_start_msg = format!("[{},{}]", game_connection.player_id.0, serde_json::to_string(&msg).unwrap()).into_bytes();
    if let Some(client) = &mut control_server.client {
        client.write(tick_start_msg.as_slice()).unwrap();
    }

    if let Some(game_results) = msg.game_results {
        commands.insert_resource(game_results);
        commands.set_state(AppState::EndMenu);
        return;
    }

    // Set this after so that clients that connect in the end menu don't have stale tick start messages
    control_server.tick_start_messages = Some(tick_start_msg);

    commands.write_message(TickEvent(msg));
}

fn end_tick(game_connection: Res<GameConnection>, mut control_server: ResMut<ControlServer>) {
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

    let mut actions = PlayerActions::default();
    for msg in messages {
        match msg {
            ControlMsgC2S::MoveForward => actions.set(PlayerActions::MOVE_FORWARD),
            ControlMsgC2S::MoveBackward => actions.set(PlayerActions::MOVE_BACKWARD),
            ControlMsgC2S::MoveLeft => actions.set(PlayerActions::MOVE_LEFT),
            ControlMsgC2S::MoveRight => actions.set(PlayerActions::MOVE_RIGHT),
            ControlMsgC2S::Jump => actions.set(PlayerActions::JUMP),
            ControlMsgC2S::Rotate(rotation) => actions.rotation = rotation,
            ControlMsgC2S::SelectItem(item) => actions.item_change = Some(item),
            ControlMsgC2S::Attack => actions.checked_set(PlayerActions::ATTACK),
            ControlMsgC2S::UseItem => actions.checked_set(PlayerActions::USE_ITEM),
            ControlMsgC2S::PlaceBlock => actions.checked_set(PlayerActions::PLACE_BLOCK),
            ControlMsgC2S::DigBlock => actions.checked_set(PlayerActions::DIG_BLOCK),
            ControlMsgC2S::EndTick => break,
        }
    }

    let msg = workflow_websocket::client::Message::Binary(postcard::to_allocvec(&actions).unwrap());
    game_connection
        .sender_tx
        .send(GameConnectionMessage::SendMessage(msg))
        .unwrap();
}
