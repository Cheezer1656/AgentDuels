use crate::states::game::network::GameRng;
use crate::states::game::player::{
    Health, HurtCooldown, Inventory, PLAYER_ANIMATION_INDICES, PLAYER_EYE_HEIGHT, PLAYER_HEIGHT,
    PLAYER_INTERACT_RANGE, PLAYER_JUMP_SPEED, PLAYER_SPEED, PLAYER_WIDTH, PlayerBody, PlayerHand,
    SPAWN_POSITIONS, SPAWN_ROTATIONS, Score,
};
use crate::states::game::world::{BlockType, ChunkMap};
use crate::states::game::{BlueScoreMarker, RedScoreMarker, TPSMarker};
use crate::states::{
    GameUpdate, PostGameUpdate,
    game::{
        network::{OpponentActionsTracker, PlayerActionsTracker},
        player::{PlayerHead, PlayerID},
    },
};
use agentduels_protocol::{Item, PlayerActions};
use avian3d::prelude::{LinearVelocity, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use std::ops::RangeInclusive;

// First goal is for player 0, second for player 1
pub const GOAL_BOUNDS: [(
    RangeInclusive<i32>,
    RangeInclusive<i32>,
    RangeInclusive<i32>,
); 2] = [(-25..=-23, -3..=-1, -1..=1), (23..=25, -3..=-1, -1..=1)];

#[derive(EntityEvent)]
struct GoalEvent(Entity);

#[derive(EntityEvent)]
struct DeathEvent(Entity);

pub struct GameLoopPlugin;

impl Plugin for GameLoopPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(update_score)
            .add_observer(reset_players_after_goal)
            .add_observer(reset_health_after_death)
            .add_observer(reset_player_position_on_death)
            .add_observer(reset_player_inv_on_death)
            .add_systems(
                GameUpdate,
                (
                    change_item_in_inv,
                    move_player,
                    place_block.after(change_item_in_inv).after(move_player),
                    attack
                        .after(change_item_in_inv)
                        .after(move_player)
                        .after(tick_hurt_cooldown),
                    tick_hurt_cooldown,
                    check_goal.after(move_player),
                    check_for_deaths,
                    kill_oob_players.after(move_player),
                    update_animation.after(move_player),
                    update_item_model.after(change_item_in_inv),
                ),
            )
            .add_systems(PostGameUpdate, (update_scoreboard, update_tps));
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
    mut player_query: Query<(&PlayerID, &Transform, &mut LinearVelocity, &Children)>,
    mut player_body_query: Query<&mut Transform, (With<PlayerBody>, Without<PlayerID>)>,
    mut player_head_query: Query<
        &mut Transform,
        (With<PlayerHead>, Without<PlayerID>, Without<PlayerBody>),
    >,
    children_query: Query<&Children>,
    actions: Res<PlayerActionsTracker>,
    opp_actions: Res<OpponentActionsTracker>,
    chunk_map: Single<&ChunkMap>,
) {
    for (player_id, transform, mut vel, children) in player_query.iter_mut() {
        for child in children.iter() {
            let Ok(mut body_transform) = player_body_query.get_mut(child) else {
                continue;
            };

            let actions = if player_id.0 == 0 {
                actions.0
            } else {
                opp_actions.0
            };

            let mut dir = Vec3::ZERO;
            if actions.is_set(PlayerActions::MOVE_FORWARD) {
                dir.x += 1.0;
            }
            if actions.is_set(PlayerActions::MOVE_BACKWARD) {
                dir.x -= 1.0;
            }
            if actions.is_set(PlayerActions::MOVE_LEFT) {
                dir.z += 1.0;
            }
            if actions.is_set(PlayerActions::MOVE_RIGHT) {
                dir.z -= 1.0;
            }

            let yaw = Quat::from_axis_angle(Vec3::Y, actions.rotation[0]);
            let pitch = Quat::from_axis_angle(
                -Vec3::X,
                actions.rotation[1]
                    .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2),
            );
            for child in children_query.iter_descendants(child) {
                let Ok(mut head_transform) = player_head_query.get_mut(child) else {
                    continue;
                };
                head_transform.rotation = pitch;
            }

            body_transform.rotation = yaw;
            if player_id.0 == 0 {
                body_transform.rotation *= Quat::from_axis_angle(Vec3::Y, std::f32::consts::PI);
            }

            let p1 = transform.translation
                - Vec3::new(PLAYER_WIDTH / 2.0, PLAYER_HEIGHT / 2.0, PLAYER_WIDTH / 2.0);
            let p2 = p1 + Vec3::new(PLAYER_WIDTH, 0.0, 0.0);
            let p3 = p1 + Vec3::new(0.0, 0.0, PLAYER_WIDTH);
            let p4 = p1 + Vec3::new(PLAYER_WIDTH, 0.0, PLAYER_WIDTH);
            let mut on_ground = false;
            for p in [p1, p2, p3, p4] {
                if chunk_map.get_block(p.floor().as_ivec3()) != BlockType::Air {
                    on_ground = true;
                    break;
                }
            }

            let mut delta = (body_transform.rotation * dir * PLAYER_SPEED) - vel.0;
            if !on_ground {
                delta *= 0.01;
            }

            delta.y = if actions.is_set(PlayerActions::JUMP) && on_ground {
                PLAYER_JUMP_SPEED - vel.0.y
            } else {
                0.0
            };

            vel.0 += delta;
        }
    }
}

fn place_block(
    mut player_query: Query<(&PlayerID, &mut Inventory, &Transform, &Children)>,
    player_body_query: Query<&Transform, (With<PlayerBody>, Without<PlayerID>)>,
    player_head_query: Query<
        &Transform,
        (With<PlayerHead>, Without<PlayerID>, Without<PlayerBody>),
    >,
    children_query: Query<&Children>,
    actions: Res<PlayerActionsTracker>,
    opp_actions: Res<OpponentActionsTracker>,
    mut chunk_map: Single<&mut ChunkMap>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (player_id, mut inv, transform, children) in player_query.iter_mut() {
        let actions = if player_id.0 == 0 {
            actions.0
        } else {
            opp_actions.0
        };
        for child in children.iter() {
            let Ok(body_transform) = player_body_query.get(child) else {
                continue;
            };
            if actions.is_set(PlayerActions::PLACE_BLOCK) {
                if inv.get_selected_item() == Item::Block && inv.get_count(Item::Block) > 0 {
                    for child in children_query.iter_descendants(child) {
                        let Ok(head_transform) = player_head_query.get(child) else {
                            continue;
                        };
                        let origin = transform.translation
                            + Vec3::new(0.0, -PLAYER_HEIGHT / 2.0 + PLAYER_EYE_HEIGHT, 0.0); // -half player height + eye height
                        let mut pos = origin;
                        let dir_inv = 1.0
                            / (body_transform.rotation
                                * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
                                * head_transform.rotation)
                                .mul_vec3(Vec3::Z);

                        // for i in 0..50 {
                        //     commands.spawn((
                        //         Mesh3d(meshes.add(Cuboid::new(0.05, 0.05, 0.05))),
                        //         MeshMaterial3d(materials.add(Color::srgb_u8(243, 255, 255))),
                        //         Transform::from_translation(
                        //             origin + 1.0 / dir_inv * (i as f32 * 0.1),
                        //         ),
                        //     ));
                        // }

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

                            inv.remove_item(Item::Block, 1);
                        }
                    }
                }
            }
        }
    }
}

fn attack(
    player_query: Query<(Entity, &PlayerID, &Transform, &Children)>,
    player_body_query: Query<&Transform, (With<PlayerBody>, Without<PlayerID>)>,
    player_head_query: Query<
        &Transform,
        (With<PlayerHead>, Without<PlayerID>, Without<PlayerBody>),
    >,
    children_query: Query<&Children>,
    mut player_query_2: Query<(&mut Health, &mut HurtCooldown), With<PlayerID>>,
    actions: Res<PlayerActionsTracker>,
    opp_actions: Res<OpponentActionsTracker>,
    spatial_query: SpatialQuery,
) {
    for (entity, player_id, transform, children) in player_query.iter() {
        let actions = if player_id.0 == 0 {
            actions.0
        } else {
            opp_actions.0
        };
        if actions.is_set(PlayerActions::ATTACK) {
            for child in children.iter() {
                let Ok(body_transform) = player_body_query.get(child) else {
                    continue;
                };
                for child in children_query.iter_descendants(child) {
                    let Ok(head_transform) = player_head_query.get(child) else {
                        continue;
                    };
                    let origin = transform.translation
                        + Vec3::new(0.0, -PLAYER_HEIGHT / 2.0 + PLAYER_EYE_HEIGHT, 0.0); // -half player height + eye height
                    let dir = (body_transform.rotation
                        * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
                        * head_transform.rotation)
                        .mul_vec3(Vec3::Z);

                    let hits = spatial_query.ray_hits(
                        origin,
                        Dir3::new(dir).unwrap(),
                        PLAYER_INTERACT_RANGE,
                        10,
                        true,
                        &SpatialQueryFilter::default(),
                    );
                    for hit in hits.iter() {
                        if hit.entity == entity {
                            continue;
                        }
                        if let Ok((mut health, mut hurt_cooldown)) =
                            player_query_2.get_mut(hit.entity)
                        {
                            if hurt_cooldown.0 > 0 {
                                continue;
                            }
                            health.0 -= 5.0;
                            hurt_cooldown.0 = 10;
                            println!(
                                "Player {:?} attacked entity {:?}, new health: {}",
                                entity, hit.entity, health.0
                            );
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn tick_hurt_cooldown(mut player_query: Query<&mut HurtCooldown>) {
    for mut hurt_cooldown in player_query.iter_mut() {
        hurt_cooldown.0 = hurt_cooldown.0.saturating_sub(1);
    }
}

/// Check if any player has reached their goal area
/// Only one player can score at a time; if multiple are in the goal area, one is chosen at random
fn check_goal(
    player_query: Query<(Entity, &PlayerID, &Transform)>,
    rng: Res<GameRng>,
    mut commands: Commands,
) {
    let mut entities = Vec::new();
    for (entity, player_id, transform) in player_query.iter() {
        let pos = transform.translation.floor().as_ivec3();
        let (x_range, y_range, z_range) = &GOAL_BOUNDS[player_id.0 as usize];
        if x_range.contains(&pos.x) && y_range.contains(&pos.y) && z_range.contains(&pos.z) {
            entities.push(entity);
        }
    }
    if let Some(chosen_entity) = rng.clone_rng().choice(entities.iter()) {
        commands.trigger(GoalEvent(*chosen_entity));
    };
}

fn update_score(event: On<GoalEvent>, mut player_query: Query<&mut Score>) {
    let Ok(mut score) = player_query.get_mut(event.0) else {
        return;
    };
    score.0 += 1;
}

// Use DeathEvent to reset players after a goal is scored
fn reset_players_after_goal(
    _: On<GoalEvent>,
    mut player_query: Query<Entity, With<PlayerID>>,
    mut commands: Commands,
) {
    for entity in player_query.iter_mut() {
        commands.trigger(DeathEvent(entity));
    }
}

fn check_for_deaths(player_query: Query<(Entity, &Health)>, mut commands: Commands) {
    for (entity, health) in player_query.iter() {
        if health.0 <= 0.0 {
            commands.trigger(DeathEvent(entity));
        }
    }
}

fn reset_health_after_death(event: On<DeathEvent>, mut player_query: Query<&mut Health>) {
    let Ok(mut health) = player_query.get_mut(event.0) else {
        return;
    };
    health.0 = Health::default().0;
}

fn reset_player_position_on_death(
    event: On<DeathEvent>,
    mut player_query: Query<(&PlayerID, &mut Transform, &Children)>,
    mut player_body_query: Query<
        (&mut Transform, &Children),
        (With<PlayerBody>, Without<PlayerID>),
    >,
    mut player_head_query: Query<
        &mut Transform,
        (With<PlayerHead>, Without<PlayerID>, Without<PlayerBody>),
    >,
) {
    let (player_id, mut transform, children) = player_query.get_mut(event.0).unwrap();
    transform.translation = SPAWN_POSITIONS[player_id.0 as usize];

    for child in children.iter() {
        let Ok((mut body_transform, body_children)) = player_body_query.get_mut(child) else {
            continue;
        };
        body_transform.rotation = Quat::from_rotation_y(SPAWN_ROTATIONS[player_id.0 as usize]);

        for child in body_children.iter() {
            let Ok(mut head_transform) = player_head_query.get_mut(child) else {
                continue;
            };
            head_transform.rotation = Quat::IDENTITY;
        }
    }
}

fn reset_player_inv_on_death(event: On<DeathEvent>, mut player_query: Query<&mut Inventory>) {
    let Ok(mut inventory) = player_query.get_mut(event.0) else {
        return;
    };
    *inventory = Inventory::default();
}

fn kill_oob_players(mut player_query: Query<(&mut Health, &Transform)>) {
    for (mut health, transform) in player_query.iter_mut() {
        if transform.translation.y < -10.0 {
            health.0 = 0.0;
        }
    }
}

fn update_animation(
    player_query: Query<(Entity, &LinearVelocity), With<PlayerID>>,
    children: Query<&Children>,
    mut anim_player_query: Query<&mut AnimationPlayer>,
) {
    for (entity, vel) in player_query.iter() {
        for child in children.iter_descendants(entity) {
            let Ok(mut anim_player) = anim_player_query.get_mut(child) else {
                continue;
            };
            let animation = anim_player
                .animation_mut(PLAYER_ANIMATION_INDICES.walk.into())
                .unwrap();
            if vel.x.abs() < 0.1 && vel.z.abs() < 0.1 {
                if !animation.is_paused() {
                    animation.rewind().pause();
                }
            } else if animation.is_paused() {
                animation.resume();
            }
        }
    }
}

fn update_item_model(
    player_query: Query<(Entity, &Inventory), Changed<Inventory>>,
    player_hand_query: Query<Entity, With<PlayerHand>>,
    children: Query<&Children>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    for (entity, inv) in player_query.iter() {
        println!(
            "Updating item model for entity {:?} to {:?}",
            entity,
            inv.get_selected_item()
        );
        for child in children.iter_descendants(entity) {
            let Ok(entity) = player_hand_query.get(child) else {
                continue;
            };
            let gltf_path = format!(
                "models/items/{}.gltf#Scene0",
                inv.get_selected_item().to_string()
            );
            let new_model_entity = commands.spawn(SceneRoot(assets.load(gltf_path))).id();
            commands
                .entity(entity)
                .despawn_children()
                .add_child(new_model_entity);
        }
    }
}

fn update_scoreboard(
    mut red_score: Single<(&mut TextSpan,), With<RedScoreMarker>>,
    mut blue_score: Single<(&mut TextSpan,), (With<BlueScoreMarker>, Without<RedScoreMarker>)>,
    score_query: Query<(&PlayerID, &Score)>,
) {
    let mut red = 0;
    let mut blue = 0;

    for (player_id, score) in score_query.iter() {
        if player_id.0 == 0 {
            red = score.0;
        } else {
            blue = score.0;
        }
    }

    red_score.0.0 = red.to_string();
    blue_score.0.0 = blue.to_string();
}

fn update_tps(mut tps_text: Single<&mut TextSpan, With<TPSMarker>>, time: Res<Time>) {
    let tps = 1.0 / time.delta_secs();
    tps_text.0 = tps.to_string();
}
