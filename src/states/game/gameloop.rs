use crate::AppState;
use crate::states::game::network::GameRng;
use crate::states::game::player::{
    BreakingStatus, BreakingStatusTracker, Health, HurtCooldown, Inventory, ItemUsageStatus,
    ItemUsageStatusTracker, PLAYER_ANIMATION_INDICES, PLAYER_EYE_HEIGHT, PLAYER_HEIGHT,
    PLAYER_INTERACT_RANGE, PLAYER_JUMP_SPEED, PLAYER_SPEED, PlayerActionsTracker, PlayerBody,
    PlayerHand, SPAWN_POSITIONS, SPAWN_ROTATIONS, Score,
};
use crate::states::game::world::{BlockType, ChunkMap};
use crate::states::game::{BlueScoreMarker, RedScoreMarker, TPSMarker};
use crate::states::network::{ControlMsgQueue, ControlMsgS2C};
use crate::states::{
    ARROW_HEIGHT, ARROW_WIDTH, Arrow, CollisionLayer, GameResults, GameUpdate, PostGameUpdate,
    game::player::{PlayerHead, PlayerID},
};
use agentduels_protocol::{Item, PlayerActions};
use avian3d::prelude::{
    ActiveCollisionHooks, Collider, CollisionEventsEnabled, CollisionHooks, CollisionLayers,
    CollisionStart, Collisions, Friction, GravityScale, LinearVelocity, LockedAxes, Restitution,
    RigidBody, SpatialQuery, SpatialQueryFilter, SweptCcd,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use std::ops::RangeInclusive;

// First goal is for player 0, second for player 1
pub const GOAL_BOUNDS: [(
    RangeInclusive<i32>,
    RangeInclusive<i32>,
    RangeInclusive<i32>,
); 2] = [(-27..=-25, -3..=-1, -1..=1), (25..=27, -3..=-1, -1..=1)];

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
            .add_observer(send_death_events)
            .add_observer(send_goal_events)
            .init_resource::<LastTick>()
            .add_systems(
                GameUpdate,
                (
                    change_item_in_inv,
                    move_player,
                    place_block.after(change_item_in_inv).after(move_player),
                    update_breaking_status
                        .after(change_item_in_inv)
                        .after(move_player),
                    break_block.after(update_breaking_status),
                    attack
                        .after(change_item_in_inv)
                        .after(move_player)
                        .after(tick_hurt_cooldown),
                    update_item_usage_status.after(change_item_in_inv),
                    eat_golden_apple.after(update_item_usage_status),
                    shoot_arrow.after(update_item_usage_status),
                    manage_arrows,
                    tick_hurt_cooldown,
                    apply_damage_tint.after(tick_hurt_cooldown),
                    check_goal.after(move_player),
                    check_for_win.after(check_goal),
                    check_for_deaths,
                    kill_oob_players.after(move_player),
                    update_animations
                        .after(move_player)
                        .after(update_item_usage_status),
                    update_item_model.after(change_item_in_inv),
                ),
            )
            .add_systems(
                PostGameUpdate,
                (
                    send_health_updates,
                    send_inventory_updates,
                    update_scoreboard,
                    update_tps,
                ),
            );
    }
}

fn change_item_in_inv(mut player_query: Query<(&PlayerActionsTracker, &mut Inventory)>) {
    for (actions, mut inventory) in player_query.iter_mut() {
        if let Some(item) = actions.0.item_change {
            inventory.select_item(item);
        }
    }
}

fn move_player(
    mut player_query: Query<(
        Entity,
        &PlayerID,
        &PlayerActionsTracker,
        &mut LinearVelocity,
        &Children,
    )>,
    mut player_body_query: Query<&mut Transform, (With<PlayerBody>, Without<PlayerID>)>,
    mut player_head_query: Query<
        &mut Transform,
        (With<PlayerHead>, Without<PlayerID>, Without<PlayerBody>),
    >,
    children_query: Query<&Children>,
    collisions: Collisions,
) {
    for (entity, player_id, actions, mut vel, children) in player_query.iter_mut() {
        for child in children.iter() {
            let Ok(mut body_transform) = player_body_query.get_mut(child) else {
                continue;
            };

            let mut dir = Vec3::ZERO;
            if actions.0.is_set(PlayerActions::MOVE_FORWARD) {
                dir.x += 1.0;
            }
            if actions.0.is_set(PlayerActions::MOVE_BACKWARD) {
                dir.x -= 1.0;
            }
            if actions.0.is_set(PlayerActions::MOVE_LEFT) {
                dir.z += 1.0;
            }
            if actions.0.is_set(PlayerActions::MOVE_RIGHT) {
                dir.z -= 1.0;
            }

            let yaw = Quat::from_axis_angle(Vec3::Y, actions.0.rotation.yaw);
            let pitch = Quat::from_axis_angle(
                -Vec3::X,
                actions
                    .0
                    .rotation
                    .pitch
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

            let mut on_ground = false;
            for contact_pair in collisions.collisions_with(entity) {
                if contact_pair.total_normal_impulse().y > 0.1 {
                    on_ground = true;
                    break;
                }
            }

            let mut delta = (body_transform.rotation * dir * PLAYER_SPEED) - vel.0;
            if !on_ground {
                delta *= 0.01;
            }

            delta.y = if actions.0.is_set(PlayerActions::JUMP) && on_ground {
                PLAYER_JUMP_SPEED - vel.0.y
            } else {
                0.0
            };

            vel.0 += delta;
        }
    }
}

fn raycast_for_block(
    player_pos: Vec3,
    player_yaw: Quat,
    player_pitch: Quat,
    chunk_map: &ChunkMap,
) -> Option<(IVec3, IVec3)> {
    let origin = player_pos + Vec3::new(0.0, -PLAYER_HEIGHT / 2.0 + PLAYER_EYE_HEIGHT, 0.0); // -half player height + eye height
    let mut pos = origin;
    let dir_inv = 1.0
        / (player_yaw * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) * player_pitch)
            .mul_vec3(Vec3::Z);

    let step = dir_inv.map(|a| a.signum());
    let select = dir_inv.map(|a| 0.5 + 0.5 * a.signum());

    loop {
        let floored_pos = pos.floor();
        let floored_pos_ivec3 = pos.floor().as_ivec3();
        if chunk_map.get_block(floored_pos_ivec3) != BlockType::Air {
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
            .normalize()
            .ceil()
            .as_ivec3();

            return Some((floored_pos_ivec3, face));
        } else if (pos - origin).length_squared() > 5.0 * 5.0 {
            return None;
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
}

fn place_block(
    mut player_query: Query<(
        &PlayerID,
        &PlayerActionsTracker,
        &mut Inventory,
        &Transform,
        &Children,
    )>,
    player_body_query: Query<&Transform, (With<PlayerBody>, Without<PlayerID>)>,
    player_head_query: Query<
        &Transform,
        (With<PlayerHead>, Without<PlayerID>, Without<PlayerBody>),
    >,
    children_query: Query<&Children>,
    mut chunk_map: Single<&mut ChunkMap>,
    mut control_msg_queue: ResMut<ControlMsgQueue>,
) {
    for (player_id, actions, mut inv, transform, children) in player_query.iter_mut() {
        for child in children.iter() {
            let Ok(body_transform) = player_body_query.get(child) else {
                continue;
            };
            if actions.0.is_set(PlayerActions::PLACE_BLOCK) {
                if inv.get_selected_item() == Item::Block && inv.get_count(Item::Block) > 0 {
                    for child in children_query.iter_descendants(child) {
                        let Ok(head_transform) = player_head_query.get(child) else {
                            continue;
                        };
                        let Some((block_pos, face)) = raycast_for_block(
                            transform.translation,
                            body_transform.rotation,
                            head_transform.rotation,
                            &chunk_map,
                        ) else {
                            continue;
                        };

                        let block_pos = block_pos + face;
                        let block_type = if player_id.0 == 0 {
                            BlockType::RedBlock
                        } else {
                            BlockType::BlueBlock
                        };
                        chunk_map.set_block(block_pos, block_type).unwrap();
                        control_msg_queue.push(ControlMsgS2C::BlockUpdate(block_pos, block_type));

                        inv.remove_item(Item::Block, 1);
                    }
                }
            }
        }
    }
}

fn update_breaking_status(
    mut player_query: Query<(
        &PlayerActionsTracker,
        &Inventory,
        &mut BreakingStatusTracker,
        &Transform,
        &Children,
    )>,
    player_body_query: Query<&Transform, (With<PlayerBody>, Without<PlayerID>)>,
    player_head_query: Query<
        &Transform,
        (With<PlayerHead>, Without<PlayerID>, Without<PlayerBody>),
    >,
    children_query: Query<&Children>,
    chunk_map: Single<&ChunkMap>,
) {
    for (actions, inv, mut breaking_status_tracker, transform, children) in player_query.iter_mut()
    {
        for child in children.iter() {
            let Ok(body_transform) = player_body_query.get(child) else {
                continue;
            };
            if actions.0.is_set(PlayerActions::DIG_BLOCK) {
                for child in children_query.iter_descendants(child) {
                    let Ok(head_transform) = player_head_query.get(child) else {
                        continue;
                    };
                    if let Some((block_pos, _)) = raycast_for_block(
                        transform.translation,
                        body_transform.rotation,
                        head_transform.rotation,
                        &chunk_map,
                    ) {
                        if let Some(breaking_status) = breaking_status_tracker.0.as_mut() {
                            if block_pos == breaking_status.block_pos {
                                let ticks_advanced = match inv.get_selected_item() {
                                    Item::Pickaxe => 3,
                                    _ => 1,
                                };
                                if let Some(ticks_left) =
                                    breaking_status.ticks_left.checked_sub(ticks_advanced)
                                {
                                    breaking_status.ticks_left = ticks_left;
                                } else {
                                    breaking_status_tracker.0 = None;
                                }
                            } else {
                                breaking_status_tracker.0 = None;
                            }
                        } else {
                            breaking_status_tracker.0 = Some(BreakingStatus {
                                block_pos,
                                ticks_left: 30,
                            });
                        }
                    } else {
                        breaking_status_tracker.0 = None;
                    }
                }
            }
        }
    }
}

fn break_block(
    player_query: Query<&BreakingStatusTracker, Changed<BreakingStatusTracker>>,
    mut chunk_map: Single<&mut ChunkMap>,
    mut control_msg_queue: ResMut<ControlMsgQueue>,
) {
    for breaking_status_tracker in player_query.iter() {
        let Some(breaking_status) = breaking_status_tracker.0.as_ref() else {
            continue;
        };
        if breaking_status.ticks_left > 0 {
            continue;
        }
        chunk_map
            .set_block(breaking_status.block_pos, BlockType::Air)
            .unwrap();
        control_msg_queue.push(ControlMsgS2C::BlockUpdate(
            breaking_status.block_pos,
            BlockType::Air,
        ));
    }
}

fn attack(
    player_query: Query<(
        Entity,
        &PlayerActionsTracker,
        &Inventory,
        &Transform,
        &Children,
    )>,
    player_body_query: Query<&Transform, (With<PlayerBody>, Without<PlayerID>)>,
    player_head_query: Query<
        &Transform,
        (With<PlayerHead>, Without<PlayerID>, Without<PlayerBody>),
    >,
    children_query: Query<&Children>,
    mut player_query_2: Query<
        (&mut Health, &mut HurtCooldown, &mut LinearVelocity),
        With<PlayerID>,
    >,
    spatial_query: SpatialQuery,
) {
    let mut hit_queue: Vec<(Entity, f32, Vec3)> = Vec::new();
    for (entity, actions, inv, transform, children) in player_query.iter() {
        if actions.0.is_set(PlayerActions::ATTACK) {
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

                    let (_, _, vel) = player_query_2.get(entity).unwrap();
                    let reach = (PLAYER_INTERACT_RANGE / PLAYER_SPEED * vel.0.with_y(0.0).length())
                        .min(PLAYER_INTERACT_RANGE);

                    let hits = spatial_query.ray_hits(
                        origin,
                        Dir3::new(dir).unwrap(),
                        reach,
                        10,
                        true,
                        &SpatialQueryFilter::default(),
                    );
                    for hit in hits.iter() {
                        if hit.entity == entity {
                            continue;
                        }
                        hit_queue.push((
                            hit.entity,
                            inv.get_selected_item().damage(),
                            Vec3::new(dir.x, 0.5, dir.z).normalize() * 20.0,
                        ));
                    }
                }
            }
        }
    }
    for (entity, damage, knockback) in hit_queue {
        if let Ok((mut health, mut hurt_cooldown, mut vel)) = player_query_2.get_mut(entity) {
            if hurt_cooldown.0 > 0 {
                continue;
            }
            health.0 -= damage;
            hurt_cooldown.start();
            vel.0 += knockback;
        }
    }
}

fn update_item_usage_status(
    mut player_query: Query<(
        &PlayerActionsTracker,
        &mut ItemUsageStatusTracker,
        &Inventory,
    )>,
) {
    for (actions, mut item_usage_tracker, inv) in player_query.iter_mut() {
        if let Some(item_usage) = item_usage_tracker.0.as_mut() {
            if actions.0.is_set(PlayerActions::USE_ITEM)
                && inv.get_count(inv.get_selected_item()) > 0
            {
                if inv.get_selected_item() == item_usage.item {
                    if let Some(ticks_left) = item_usage.ticks_left.checked_sub(1) {
                        item_usage.ticks_left = ticks_left;
                    } else {
                        item_usage_tracker.0 = None;
                    }
                } else {
                    item_usage_tracker.0 = Some(ItemUsageStatus::new(inv.get_selected_item()));
                }
            } else {
                item_usage_tracker.0 = None;
            }
        } else {
            if actions.0.is_set(PlayerActions::USE_ITEM)
                && inv.get_count(inv.get_selected_item()) > 0
            {
                item_usage_tracker.0 = Some(ItemUsageStatus::new(inv.get_selected_item()));
            }
        }
    }
}

fn eat_golden_apple(
    mut player_query: Query<(&ItemUsageStatusTracker, &mut Health, &mut Inventory)>,
) {
    for (item_usage_tracker, mut health, mut inv) in player_query.iter_mut() {
        let Some(item_usage) = item_usage_tracker.0.as_ref() else {
            continue;
        };
        if item_usage.item != Item::GoldenApple || item_usage.ticks_left > 0 {
            continue;
        }
        *health = Health::default();
        inv.remove_item(Item::GoldenApple, 1);
        println!("Golden apples left: {}", inv.get_count(Item::GoldenApple));
    }
}

fn shoot_arrow(
    player_query: Query<(&ItemUsageStatusTracker, &Transform, &Children)>,
    player_body_query: Query<&Transform, (With<PlayerBody>, Without<PlayerID>)>,
    player_head_query: Query<
        &Transform,
        (With<PlayerHead>, Without<PlayerID>, Without<PlayerBody>),
    >,
    children_query: Query<&Children>,
    mut commands: Commands,
) {
    for (item_usage_tracker, transform, children) in player_query.iter() {
        let Some(item_usage) = item_usage_tracker.0.as_ref() else {
            continue;
        };
        if item_usage.item != Item::Bow || item_usage.ticks_left > 0 {
            continue;
        }
        for child in children.iter() {
            let Ok(body_transform) = player_body_query.get(child) else {
                continue;
            };
            for child in children_query.iter_descendants(child) {
                let Ok(head_transform) = player_head_query.get(child) else {
                    continue;
                };

                let dir = (body_transform.rotation
                    * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
                    * head_transform.rotation)
                    .mul_vec3(Vec3::Z)
                    .normalize();
                let origin = transform.translation
                    + Vec3::new(0.0, -PLAYER_HEIGHT / 2.0 + PLAYER_EYE_HEIGHT, 0.0) // -half player height + eye height
                    + dir;

                commands
                    .spawn((
                        Arrow::default(),
                        RigidBody::Dynamic,
                        Collider::cuboid(ARROW_WIDTH, ARROW_HEIGHT, ARROW_WIDTH),
                        CollisionLayers::new(
                            CollisionLayer::Projectile,
                            [CollisionLayer::World, CollisionLayer::Player],
                        ),
                        CollisionEventsEnabled,
                        ActiveCollisionHooks::FILTER_PAIRS,
                        SweptCcd::default(),
                        LockedAxes::ROTATION_LOCKED,
                        Transform::from_translation(origin),
                        LinearVelocity(dir * 50.0),
                        Friction::new(100.0),
                        Restitution::new(0.0),
                        GravityScale(5.0),
                        Visibility::default(),
                    ))
                    .observe(handle_arrow_collision);
            }
        }
    }
}

#[derive(SystemParam)]
pub struct ArrowHooks<'w, 's> {
    arrow_query: Query<'w, 's, &'static Arrow>,
}

impl CollisionHooks for ArrowHooks<'_, '_> {
    fn filter_pairs(&self, collider1: Entity, collider2: Entity, _commands: &mut Commands) -> bool {
        if let Ok(arrow) = self.arrow_query.get(collider1) {
            if arrow.ticks_in_ground > 0 {
                return false;
            }
        }
        if let Ok(arrow) = self.arrow_query.get(collider2) {
            if arrow.ticks_in_ground > 0 {
                return false;
            }
        }
        true
    }
}

fn handle_arrow_collision(
    event: On<CollisionStart>,
    arrow_query: Query<(&Arrow, &LinearVelocity)>,
    mut player_query: Query<(&mut Health, &mut HurtCooldown, &mut LinearVelocity), Without<Arrow>>,
    mut commands: Commands,
) {
    let Ok((arrow, arrow_vel)) = arrow_query.get(event.collider1) else {
        return;
    };
    if arrow.ticks_in_ground > 0 {
        return;
    }
    let Ok((mut health, mut hurt_cooldown, mut player_vel)) = player_query.get_mut(event.collider2)
    else {
        return;
    };
    commands.entity(event.collider1).despawn();
    health.0 -= 9.0;
    hurt_cooldown.start();
    player_vel.0 += arrow_vel.0.normalize() * 20.0;
}

fn manage_arrows(
    mut arrow_query: Query<(Entity, &mut Arrow, &LinearVelocity)>,
    mut commands: Commands,
) {
    for (entity, mut arrow, vel) in arrow_query.iter_mut() {
        if vel.length() < 0.1 {
            arrow.ticks_in_ground += 1;
            if arrow.ticks_in_ground > 100 {
                commands.entity(entity).despawn();
            }
        } else {
            arrow.ticks_in_ground = 0;
        }
    }
}

fn tick_hurt_cooldown(mut player_query: Query<&mut HurtCooldown>) {
    for mut hurt_cooldown in player_query.iter_mut() {
        hurt_cooldown.0 = hurt_cooldown.0.saturating_sub(1);
    }
}

fn apply_damage_tint(
    player_query: Query<(Entity, &HurtCooldown), Changed<HurtCooldown>>,
    children_query: Query<&Children>,
    material_query: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, hurt_cooldown) in player_query.iter() {
        for child in children_query.iter_descendants(entity) {
            let Ok(mesh_material) = material_query.get(child) else {
                continue;
            };

            let material = materials.get_mut(&mesh_material.0).unwrap();
            material.base_color = if hurt_cooldown.0 != 0 {
                Color::srgb_u8(255, 0, 0)
            } else {
                Color::WHITE
            };
        }
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
        // Kill the player if they are in their own goal, without scoring
        let (x_range, y_range, z_range) = &GOAL_BOUNDS[player_id.0 as usize ^ 1];
        if x_range.contains(&pos.x) && y_range.contains(&pos.y) && z_range.contains(&pos.z) {
            commands.trigger(DeathEvent(entity));
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

fn check_for_win(player_query: Query<(&PlayerID, &Score), Changed<Score>>, mut commands: Commands) {
    for (player_id, score) in player_query.iter() {
        if score.0 >= 5 {
            commands.insert_resource(GameResults {
                winner: Some(player_id.0),
            });
            commands.set_state(AppState::EndMenu);
        }
    }
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
    mut player_query: Query<(&PlayerID, &mut Transform, &mut LinearVelocity, &Children)>,
    mut player_body_query: Query<
        (&mut Transform, &Children),
        (With<PlayerBody>, Without<PlayerID>),
    >,
    mut player_head_query: Query<
        &mut Transform,
        (With<PlayerHead>, Without<PlayerID>, Without<PlayerBody>),
    >,
) {
    let (player_id, mut transform, mut vel, children) = player_query.get_mut(event.0).unwrap();
    transform.translation = SPAWN_POSITIONS[player_id.0 as usize];
    vel.0 = Vec3::ZERO;

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

fn is_animation_playing(anim_player: &AnimationPlayer, animation: AnimationNodeIndex) -> bool {
    // I'm assuming .animation() will always be Some if the animation is playing, and the .animation() expression won't be evaluated when the first part is false
    anim_player.is_playing_animation(animation)
        && !anim_player.animation(animation).unwrap().is_finished()
        && !anim_player.animation(animation).unwrap().is_paused()
}

fn stop_hand_animations(anim_player: &mut AnimationPlayer) {
    for id in [
        PLAYER_ANIMATION_INDICES.swing,
        PLAYER_ANIMATION_INDICES.draw_bow,
        PLAYER_ANIMATION_INDICES.eat,
    ] {
        if is_animation_playing(&anim_player, id.into()) {
            anim_player
                .animation_mut(id.into())
                .unwrap()
                .rewind()
                .pause();
        }
    }
}

fn update_animations(
    player_query: Query<
        (
            Entity,
            &PlayerActionsTracker,
            &ItemUsageStatusTracker,
            &LinearVelocity,
        ),
        With<PlayerID>,
    >,
    children: Query<&Children>,
    mut anim_player_query: Query<&mut AnimationPlayer>,
) {
    for (entity, actions, item_usage_tracker, vel) in player_query.iter() {
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
            } else {
                if animation.is_paused() {
                    animation.resume();
                }
                animation.set_speed((vel.x.abs() + vel.z.abs()) * 0.25);
            }

            // Stop paused or finished animations to allow other animations to use the transforms freely
            for id in [
                PLAYER_ANIMATION_INDICES.swing,
                PLAYER_ANIMATION_INDICES.draw_bow,
                PLAYER_ANIMATION_INDICES.eat,
            ] {
                if anim_player
                    .animation(id.into())
                    .map(|anim| anim.is_paused())
                    .unwrap_or(false)
                {
                    anim_player.stop(id.into());
                }
                let Some(animation) = anim_player.animation_mut(id.into()) else {
                    continue;
                };
                if animation.is_finished() {
                    animation.rewind().pause();
                }
            }

            if (actions.0.is_set(PlayerActions::ATTACK)
                || actions.0.is_set(PlayerActions::PLACE_BLOCK)
                || actions.0.is_set(PlayerActions::DIG_BLOCK))
                && !is_animation_playing(&anim_player, PLAYER_ANIMATION_INDICES.swing.into())
            {
                stop_hand_animations(&mut anim_player);
                anim_player
                    .start(PLAYER_ANIMATION_INDICES.swing.into())
                    .set_speed(3.0);
            }

            if let Some(item_usage) = item_usage_tracker.0.as_ref() {
                match item_usage.item {
                    Item::Bow => 'inner: {
                        if is_animation_playing(
                            &anim_player,
                            PLAYER_ANIMATION_INDICES.draw_bow.into(),
                        ) {
                            break 'inner;
                        }
                        stop_hand_animations(&mut anim_player);
                        anim_player.start(PLAYER_ANIMATION_INDICES.draw_bow.into());
                    }
                    Item::GoldenApple => 'inner: {
                        if is_animation_playing(&anim_player, PLAYER_ANIMATION_INDICES.eat.into()) {
                            break 'inner;
                        }
                        stop_hand_animations(&mut anim_player);
                        anim_player.start(PLAYER_ANIMATION_INDICES.eat.into());
                    }
                    _ => {}
                }
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
            commands.entity(entity).despawn_children();

            if inv.get_count(inv.get_selected_item()) > 0 {
                let gltf_path = format!(
                    "models/items/{}.gltf#Scene0",
                    inv.get_selected_item().to_string()
                );
                let new_model_entity = commands.spawn(SceneRoot(assets.load(gltf_path))).id();
                commands.entity(entity).add_child(new_model_entity);
            }
        }
    }
}

fn send_health_updates(
    player_query: Query<(&PlayerID, &Health), Changed<Health>>,
    mut control_msg_queue: ResMut<ControlMsgQueue>,
) {
    for (player_id, health) in player_query.iter() {
        control_msg_queue.push(ControlMsgS2C::HealthUpdate {
            player_id: player_id.0,
            new_health: health.0,
        });
    }
}

fn send_death_events(
    event: On<DeathEvent>,
    player_query: Query<&PlayerID>,
    mut control_msg_queue: ResMut<ControlMsgQueue>,
) {
    let player_id = player_query.get(event.0).unwrap().0;
    control_msg_queue.push(ControlMsgS2C::Death { player_id });
}

fn send_goal_events(
    event: On<GoalEvent>,
    player_query: Query<&PlayerID>,
    mut control_msg_queue: ResMut<ControlMsgQueue>,
) {
    let player_id = player_query.get(event.0).unwrap().0;
    control_msg_queue.push(ControlMsgS2C::Goal { player_id });
}

fn send_inventory_updates(
    player_query: Query<(&PlayerID, &Inventory), Changed<Inventory>>,
    mut control_msg_queue: ResMut<ControlMsgQueue>,
) {
    for (player_id, inv) in player_query.iter() {
        control_msg_queue.push(ControlMsgS2C::InventoryUpdate {
            player_id: player_id.0,
            new_contents: inv.clone(),
        })
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

#[derive(Resource, Default)]
struct LastTick(f32);

fn update_tps(
    mut tps_text: Single<&mut TextSpan, With<TPSMarker>>,
    mut last_tick: ResMut<LastTick>,
    time: Res<Time>,
) {
    let tps = 1.0 / (time.elapsed_secs() - last_tick.0);
    last_tick.0 = time.elapsed_secs();

    tps_text.0 = tps.to_string();
}
