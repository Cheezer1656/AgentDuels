use agentduels_protocol::packets::PlayerActions;
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

use crate::states::{
    game::{
        network::{OpponentActionsTracker, PlayerActionsTracker}, player::{PlayerHead, PlayerID}, PLAYER_SPEED
    }, GameUpdate
};

pub struct GameLoopPlugin;

impl Plugin for GameLoopPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(GameUpdate, apply_player_input);
    }
}

fn apply_player_input(
    mut player_query: Query<(Entity, &PlayerID, &mut Transform, &mut LinearVelocity)>,
    mut player_head_query: Query<(&mut Transform, &ChildOf), (With<PlayerHead>, Without<PlayerID>)>,
    actions: Res<PlayerActionsTracker>,
    opp_actions: Res<OpponentActionsTracker>,
) {
    for (player_entity, player, mut transform, mut velocity) in player_query.iter_mut() {
        let actions = if player.0 == 0 {
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
        for (mut head_transform, parent) in player_head_query.iter_mut() {
            if parent.0 != player_entity {
                continue;
            }
            head_transform.rotation = Quat::from_rotation_y(-std::f32::consts::PI / 2.0) * pitch;
        }

        transform.rotation = yaw;
        if player.0 == 0 {
            transform.rotation *= Quat::from_axis_angle(Vec3::Y, std::f32::consts::PI);
        }

        delta = transform.rotation.mul_vec3(delta);
        delta.y = 0.0;
        delta *= 20.0;

        velocity.0.x = delta.x;
        velocity.0.z = delta.z;
    }
}
