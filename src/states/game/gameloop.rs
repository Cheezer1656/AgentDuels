use agentduels_protocol::packets::PlayerActions;
use bevy::prelude::*;

use crate::states::{game::{network::{OpponentActionsTracker, PlayerActionsTracker}, player::Player, world::{BlockType, ChunkMap}, PLAYER_SPEED}, GameUpdate, Velocity};

pub struct GameLoopPlugin;

impl Plugin for GameLoopPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(GameUpdate, (apply_physics, handle_player_input, test_movement));
    }
}

fn apply_physics(mut chunkmap_query: Query<&ChunkMap>, mut entity_query: Query<(&mut Velocity, &mut Transform)>) {
    let Ok(chunkmap) = chunkmap_query.single_mut() else {
        return;
    };
    for (mut velocity, mut transform) in entity_query.iter_mut() {
        // Apply gravity
        if chunkmap.get_block(transform.translation.as_ivec3()) == BlockType::Air && velocity.0.y > -0.9 {
            velocity.0.y -= 0.03;
        }

        transform.translation += velocity.0;

        // Reduce horizontal velocity (friction)
        velocity.0.x *= 0.8;
        velocity.0.z *= 0.8;
    }
}

fn handle_player_input(mut player_query: Query<(&Player, &mut Transform, &mut Velocity)>, actions: Res<PlayerActionsTracker>, opp_actions: Res<OpponentActionsTracker>) {
    for (player, mut transform, mut velocity) in player_query.iter_mut() {
        let actions = if player.id == 0 { actions.0 } else { opp_actions.0 };

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

        velocity.0.x = delta.x;
        velocity.0.z = delta.z;
    }
}

fn test_movement(mut actions: ResMut<PlayerActionsTracker>) {
    actions.0.set(PlayerActions::MOVE_FORWARD);
    actions.0.rotation[0] = 0.1;
}