use agentduels_protocol::{Item, PlayerActions};
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use std::ops::RangeInclusive;

use crate::states::game::player::{Inventory, Score};
use crate::states::game::world::{BlockType, ChunkMap};
use crate::states::{
    GameUpdate,
    game::{
        network::{OpponentActionsTracker, PlayerActionsTracker},
        player::{PlayerHead, PlayerID},
    },
};

pub const PLAYER_SPEED: f32 = 0.5;
// First goal is for player 0, second for player 1
pub const GOAL_BOUNDS: [(
    RangeInclusive<i32>,
    RangeInclusive<i32>,
    RangeInclusive<i32>,
); 2] = [(-25..=-23, -3..=-1, -1..=1), (23..=25, -3..=-1, -1..=1)];

#[derive(EntityEvent)]
struct GoalEvent(Entity);

pub struct GameLoopPlugin;

impl Plugin for GameLoopPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(update_score)
            .add_observer(reset_player_positions_on_goal)
            .add_systems(
                GameUpdate,
                (
                    change_item_in_inv,
                    move_player,
                    place_block.after(change_item_in_inv).after(move_player),
                    check_goal.after(move_player),
                ),
            );
    }
}

fn change_item_in_inv(
    mut player_query: Query<(&PlayerID, &mut Inventory)>,
    actions: Res<PlayerActionsTracker>,
    opp_actions: Res<OpponentActionsTracker>,
) {
    for (player_id, mut inventory) in player_query.iter_mut() {
        let actions = if player_id.0 == 0 {
            actions.0
        } else {
            opp_actions.0
        };
        if let Some(item) = actions.item_change {
            inventory.select_item(item);
        }
    }
}

fn move_player(
    mut player_query: Query<(Entity, &PlayerID, &mut Transform, &mut LinearVelocity)>,
    mut player_head_query: Query<(&mut Transform, &ChildOf), (With<PlayerHead>, Without<PlayerID>)>,
    actions: Res<PlayerActionsTracker>,
    opp_actions: Res<OpponentActionsTracker>,
) {
    for (player_entity, player_id, mut transform, mut velocity) in player_query.iter_mut() {
        let actions = if player_id.0 == 0 {
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
        let pitch = Quat::from_axis_angle(
            Vec3::X,
            actions.rotation[1].clamp(
                -std::f32::consts::FRAC_PI_2 + 0.01,
                std::f32::consts::FRAC_PI_2 - 0.01,
            ),
        );
        for (mut head_transform, parent) in player_head_query.iter_mut() {
            if parent.0 != player_entity {
                continue;
            }
            head_transform.rotation = Quat::from_rotation_y(-std::f32::consts::PI / 2.0) * pitch;
        }

        transform.rotation = yaw;
        if player_id.0 == 0 {
            transform.rotation *= Quat::from_axis_angle(Vec3::Y, std::f32::consts::PI);
        }

        delta = transform.rotation.mul_vec3(delta);
        delta.y = 0.0;
        delta *= 20.0;

        velocity.0.x = delta.x;
        velocity.0.z = delta.z;
    }
}

fn place_block(
    player_query: Query<(Entity, &PlayerID, &Inventory, &Transform)>,
    head_query: Query<(&Transform, &ChildOf), With<PlayerHead>>,
    actions: Res<PlayerActionsTracker>,
    opp_actions: Res<OpponentActionsTracker>,
    mut chunk_map: Single<&mut ChunkMap>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (player_entity, player_id, inv, transform) in player_query.iter() {
        let actions = if player_id.0 == 0 {
            actions.0
        } else {
            opp_actions.0
        };
        if actions.is_set(PlayerActions::PLACE_BLOCK) {
            if inv.get_selected_item() == Item::Block && inv.get_count(Item::Block) > 0 {
                let Some((head_transform, _)) = head_query
                    .iter()
                    .find(|(_, parent)| parent.0 == player_entity)
                else {
                    continue;
                };
                let origin = transform.translation + Vec3::new(0.0, -0.9 + 1.6, 0.0); // -half player height + eye height
                let mut pos = origin;
                let dir_inv =
                    1.0 / (transform.rotation * head_transform.rotation).mul_vec3(-Vec3::Z);

                for i in 0..50 {
                    commands.spawn((
                        Mesh3d(meshes.add(Cuboid::new(0.05, 0.05, 0.05))),
                        MeshMaterial3d(materials.add(Color::srgb_u8(243, 255, 255))),
                        Transform::from_translation(origin + 1.0 / dir_inv * (i as f32 * 0.1)),
                    ));
                }

                let step = dir_inv.map(|a| a.signum());
                let select = dir_inv.map(|a| 0.5 + 0.5 * a.signum());
                let mut found = false;
                loop {
                    if chunk_map.get_block(pos.floor().as_ivec3()) != BlockType::Air {
                        found = true;
                        break;
                    } else if (pos - origin).length_squared() > 5.0 * 5.0 {
                        break;
                    }

                    let planes = pos.floor() + select;
                    let t = (planes - origin) * dir_inv;

                    if t.x < t.y {
                        if t.x < t.z {
                            pos.x += step.x;
                        } else {
                            pos.z += step.z;
                        }
                    } else {
                        if t.y < t.z {
                            pos.y += step.y;
                        } else {
                            pos.z += step.z;
                        }
                    }
                }

                if found {
                    let floored_pos = pos.floor();

                    let t1 = (floored_pos - origin) * dir_inv;
                    let t2 = (floored_pos + Vec3::splat(1.0) - origin) * dir_inv;
                    let t_min = t1.min(t2);
                    let t_hit = t_min.x.max(t_min.y).max(t_min.z);

                    let face = (if t_hit == t_min.x {
                        Vec3::new(-step.x, 0.0, 0.0)
                    } else if t_hit == t_min.y {
                        Vec3::new(0.0, -step.y, 0.0)
                    } else {
                        Vec3::new(0.0, 0.0, -step.z)
                    })
                    .normalize();

                    let block_pos = (floored_pos + face).as_ivec3();
                    chunk_map
                        .set_block(
                            block_pos,
                            if player_id.0 == 0 {
                                BlockType::RedBlock
                            } else {
                                BlockType::BlueBlock
                            },
                        )
                        .unwrap();
                }
            }
        }
    }
}

fn check_goal(player_query: Query<(Entity, &PlayerID, &Transform)>, mut commands: Commands) {
    for (entity, player_id, transform) in player_query.iter() {
        let pos = transform.translation.floor().as_ivec3();
        let (x_range, y_range, z_range) = &GOAL_BOUNDS[player_id.0 as usize];
        if x_range.contains(&pos.x) && y_range.contains(&pos.y) && z_range.contains(&pos.z) {
            commands.trigger(GoalEvent(entity));
        }
    }
}

fn update_score(event: On<GoalEvent>, mut player_query: Query<&mut Score>) {
    let Ok(mut score) = player_query.get_mut(event.0) else {
        return;
    };
    score.0 += 1;
}

fn reset_player_positions_on_goal(
    _: On<GoalEvent>,
    mut player_query: Query<(&PlayerID, &mut Transform)>,
) {
    for (player_id, mut transform) in player_query.iter_mut() {
        transform.translation = Vec3::new((player_id.0 as f32 * 2.0 - 1.0) * -21.5, 1.9, 0.5);
        transform.rotation = if player_id.0 == 0 {
            Quat::from_axis_angle(Vec3::Y, std::f32::consts::PI)
        } else {
            Quat::IDENTITY
        };
    }
}
