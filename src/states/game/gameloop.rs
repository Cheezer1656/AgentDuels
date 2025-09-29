use agentduels_protocol::packets::PlayerActions;
use avian3d::prelude::AngularVelocity;
use bevy::prelude::*;

use crate::states::{
    GameUpdate,
    game::{
        PLAYER_SPEED,
        network::{OpponentActionsTracker, PlayerActionsTracker},
        player::Player,
        world::{BlockType, ChunkMap},
    },
};

pub struct GameLoopPlugin;

impl Plugin for GameLoopPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(GameUpdate, (handle_player_input, test_movement));
    }
}

fn handle_player_input(
    mut player_query: Query<(&Player, &mut Transform, &mut AngularVelocity)>,
    actions: Res<PlayerActionsTracker>,
    opp_actions: Res<OpponentActionsTracker>,
) {
    for (player, mut transform, mut velocity) in player_query.iter_mut() {
        let actions = if player.id == 0 {
            actions.0
        } else {
            opp_actions.0
        };

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

        let yaw = Quat::from_axis_angle(Vec3::Y, actions.rotation[0]);
        let pitch = Quat::from_axis_angle(Vec3::X, actions.rotation[1]);
        transform.rotation = yaw * transform.rotation * pitch;

        delta = transform.rotation.mul_vec3(delta);
        delta.y = 0.0;
        delta *= 20.0;

        velocity.0.x = -delta.z;
        velocity.0.z = -delta.x;
    }
}

fn test_movement(mut actions: ResMut<PlayerActionsTracker>) {
    actions.0.set(PlayerActions::MOVE_FORWARD);
    // actions.0.rotation[0] = 0.1;
}
