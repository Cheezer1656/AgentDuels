use agentduels_protocol::packets::PlayerActions;
use bevy::prelude::*;

use crate::states::{game::{network::{OpponentActionsTracker, PlayerActionsTracker}, player::Player, PLAYER_SPEED}, GameUpdate};

pub struct GameLoopPlugin;

impl Plugin for GameLoopPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(GameUpdate, (handle_player_input, test_movement));
    }
}

fn handle_player_input(mut player_query: Query<(&Player, &mut Transform)>, actions: Res<PlayerActionsTracker>, opp_actions: Res<OpponentActionsTracker>) {
    for (player, mut transform) in player_query.iter_mut() {
        let actions = if player.id == 0 { actions.0 } else { opp_actions.0 };

        println!("Player {} actions: {:?}", player.id, actions);

        let mut delta = Vec3::ZERO;

        if actions.is_set(PlayerActions::MOVE_FORWARD) {
            delta.x += PLAYER_SPEED;
        }
        if actions.is_set(PlayerActions::MOVE_BACKWARD) {
            delta.x -= PLAYER_SPEED;
        }
        if actions.is_set(PlayerActions::MOVE_LEFT) {
            delta.z += PLAYER_SPEED;
        }
        if actions.is_set(PlayerActions::MOVE_RIGHT) {
            delta.z -= PLAYER_SPEED;
        }

        transform.translation += delta;
    }
}

fn test_movement(mut actions: ResMut<PlayerActionsTracker>) {
    actions.0.set(PlayerActions::MOVE_BACKWARD);
}