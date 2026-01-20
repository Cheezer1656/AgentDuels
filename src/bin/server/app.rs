use crate::read_until_binary;
use agentduels::player::{
    BreakingStatus, BreakingStatusTracker, HeadRotation, Health, HurtCooldown, Inventory, Item,
    ItemUsageStatus, ItemUsageStatusTracker, PLAYER_EYE_HEIGHT, PLAYER_HEIGHT,
    PLAYER_INTERACT_RANGE, PLAYER_JUMP_SPEED, PLAYER_SPEED, PLAYER_WIDTH, PlayerActions,
    PlayerActionsTracker, PlayerAnimation, PlayerBundle, PlayerID, SPAWN_POSITIONS,
    SPAWN_ROTATIONS, Score,
};
use agentduels::world::{BlockType, ChunkMap, WorldPlugin, init_map};
use agentduels::{
    ARROW_HEIGHT, ARROW_WIDTH, AppState, Arrow, ArrowEvent, AutoDespawn, CollisionLayer,
    GameResults, PlayerInfo, TickMessage,
};
use anyhow::bail;
use avian3d::PhysicsPlugins;
use avian3d::prelude::{
    ActiveCollisionHooks, Collider, CollisionEventsEnabled, CollisionHooks, CollisionLayers,
    CollisionStart, Collisions, Friction, GravityScale, LinearDamping, LinearVelocity, LockedAxes,
    Restitution, RigidBody, SpatialQuery, SpatialQueryFilter, SweptCcd,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use rand::prelude::IteratorRandom;
use std::collections::{HashMap, HashSet};
use std::net::TcpStream;
use std::ops::RangeInclusive;
use tungstenite::WebSocket;

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

#[derive(Resource, Default)]
struct Deaths(HashSet<PlayerID>);

#[derive(Resource, Default)]
struct Goals(Option<PlayerID>);

#[derive(Resource, Default)]
struct BlockUpdates(Vec<(IVec3, BlockType)>);

#[derive(Resource, Default)]
struct InventoryChanges(HashMap<PlayerID, Inventory>);

#[derive(Resource, Default, Clone)]
struct ArrowEvents(Vec<ArrowEvent>);

pub fn start_app(mut websockets: [WebSocket<TcpStream>; 2]) -> anyhow::Result<()> {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins((
            WorldPlugin::new(true),
            PhysicsPlugins::new(PostUpdate)
                .with_collision_hooks::<ArrowHooks>()
                .build(),
        ))
        .init_resource::<Deaths>()
        .init_resource::<Goals>()
        .init_resource::<BlockUpdates>()
        .init_resource::<InventoryChanges>()
        .init_resource::<ArrowEvents>()
        .add_observer(update_score)
        .add_observer(reset_players_after_goal)
        .add_observer(reset_health_after_death)
        .add_observer(reset_player_position_on_death)
        .add_observer(reset_player_inv_on_death)
        .add_observer(send_death_events)
        .add_observer(send_goal_events)
        .add_systems(Startup, setup)
        .add_systems(PreUpdate, send_animations)
        .add_systems(
            Update,
            (
                change_item_in_inv,
                move_players,
                place_block.after(change_item_in_inv).after(move_players),
                update_breaking_status
                    .after(change_item_in_inv)
                    .after(move_players),
                break_block.after(update_breaking_status),
                attack
                    .after(change_item_in_inv)
                    .after(move_players)
                    .after(tick_hurt_cooldown),
                update_item_usage_status.after(change_item_in_inv),
                eat_golden_apple.after(update_item_usage_status),
                shoot_arrow.after(update_item_usage_status),
                send_arrow_updates.after(shoot_arrow),
                manage_arrows,
                tick_hurt_cooldown,
                check_goal.after(move_players),
                check_for_win.after(check_goal),
                check_for_deaths,
                kill_oob_players.after(move_players),
            ),
        )
        .add_systems(PostUpdate, (update_info, send_inventory_updates));

    let mut tick = 0;
    loop {
        // Reset per-tick resources
        let world = app.world_mut();
        world.resource_mut::<Deaths>().0.clear();
        world.resource_mut::<Goals>().0 = None;
        world.resource_mut::<BlockUpdates>().0.clear();
        world.resource_mut::<InventoryChanges>().0.clear();

        // Tick the app
        app.update();

        // Send the state updates to the clients
        let world = app.world_mut();
        let msg = postcard::to_allocvec(&TickMessage {
            tick,
            players: world
                .query::<(&PlayerID, &PlayerInfo)>()
                .iter_mut(world)
                .sort_by::<(&PlayerID, &PlayerInfo)>(|(player_id1, _), (player_id2, _)| {
                    player_id1.cmp(player_id2)
                })
                .map(|(_, info)| info.clone())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            deaths: world.resource::<Deaths>().0.clone(),
            goals: world.resource::<Goals>().0,
            block_updates: world.resource::<BlockUpdates>().0.clone(),
            arrow_events: world.resource::<ArrowEvents>().0.clone(),
            game_results: world.get_resource::<GameResults>().map(|x| x.clone()),
        })?;
        // println!("Sending tick {}", tick);
        for ws in websockets.iter_mut() {
            ws.send(tungstenite::Message::Binary(msg.clone().into()))?;
        }

        if world.get_resource::<GameResults>().is_some() {
            println!("Game over, closing connections");
            for ws in websockets.iter_mut() {
                ws.close(None)?;
            }
            break Ok(());
        }

        // Receive player actions from clients
        let mut query = world.query::<(&PlayerID, &mut PlayerActionsTracker)>();
        // println!("Receiving tick {}", tick);
        for (player_id, ws) in websockets.iter_mut().enumerate() {
            match read_until_binary(ws) {
                Ok(data) => {
                    let actions: PlayerActions = postcard::from_bytes(&data)?;
                    // println!("Actions from player {}: {:?}", player_id, actions);
                    let Some((_, mut actions_tracker)) = query
                        .iter_mut(world)
                        .find(|(id, _)| id.0 == player_id as u16)
                    else {
                        bail!("Could not find player with ID {}", player_id);
                    };
                    actions_tracker.0 = actions;
                }
                Err(e) => {
                    return Err(e.context("Failed to read player actions"));
                }
            }
        }

        tick += 1;
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((init_map(), AutoDespawn(AppState::Game)));

    for i in 0..2_i32 {
        commands.spawn((
            PlayerBundle {
                id: PlayerID(i as u16),
                transform: Transform::from_translation(SPAWN_POSITIONS[i as usize]),
                head_rotation: HeadRotation(Quat::from_rotation_y(SPAWN_ROTATIONS[i as usize])),
                ..default()
            },
            PlayerInfo::default(),
            RigidBody::Dynamic,
            Collider::cuboid(PLAYER_WIDTH, PLAYER_HEIGHT, PLAYER_WIDTH),
            CollisionLayers::new(
                CollisionLayer::Player,
                [CollisionLayer::World, CollisionLayer::Projectile],
            ),
            SweptCcd::default(),
            LockedAxes::ROTATION_LOCKED,
            Friction::new(0.0),
            Restitution::new(0.0),
            LinearDamping(2.0),
            GravityScale(3.0),
        ));
    }
}

fn update_info(
    mut player_query: Query<(
        &mut PlayerInfo,
        &Transform,
        &HeadRotation,
        &LinearVelocity,
        Ref<Health>,
        Ref<Inventory>,
        &PlayerAnimation,
        Ref<HurtCooldown>,
    )>,
) {
    for (
        mut info,
        transform,
        head_rotation,
        vel,
        health,
        inv,
        animation,
        hurt_cooldown,
    ) in player_query.iter_mut()
    {
        info.position = transform.translation;
        info.yaw = head_rotation.0.to_euler(EulerRot::YXZ).0;
        info.pitch = -head_rotation.0.to_euler(EulerRot::YXZ).2;
        info.velocity = vel.0;
        info.health_update = if health.is_changed() {
            Some(health.0)
        } else {
            None
        };
        info.inventory_update = if inv.is_changed() {
            Some(inv.clone())
        } else {
            None
        };
        info.animation = *animation;
        info.hurt_update = if hurt_cooldown.is_changed() {
            Some(hurt_cooldown.0 > 0)
        } else {
            None
        };
    }
}

fn change_item_in_inv(mut player_query: Query<(&PlayerActionsTracker, &mut Inventory)>) {
    for (actions, mut inventory) in player_query.iter_mut() {
        if let Some(item) = actions.0.item_change {
            inventory.select_item(item);
        }
    }
}

fn move_players(
    mut player_query: Query<(
        Entity,
        &PlayerID,
        &PlayerActionsTracker,
        &mut HeadRotation,
        &mut LinearVelocity,
    )>,
    collisions: Collisions,
) {
    for (entity, player_id, actions, mut rotation, mut vel) in player_query.iter_mut() {
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

        let yaw = Quat::from_rotation_y(if player_id.0 == 1 { actions.0.rotation.yaw } else { actions.0.rotation.yaw + std::f32::consts::PI });
        let pitch = Quat::from_rotation_z(
            actions
                .0
                .rotation
                .pitch
                .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2),
        );
        rotation.0 = yaw * pitch;

        let mut on_ground = false;
        for contact_pair in collisions.collisions_with(entity) {
            if contact_pair.total_normal_impulse().y > 0.1 {
                on_ground = true;
                break;
            }
        }

        let jump = actions.0.is_set(PlayerActions::JUMP) && on_ground;
        let speed = if jump {
            PLAYER_SPEED * 2.0
        } else {
            PLAYER_SPEED
        };
        let mut delta = (rotation.0 * dir * speed) - vel.0;
        if !on_ground {
            delta *= 0.01;
        }

        delta.y = if jump {
            PLAYER_JUMP_SPEED - vel.0.y
        } else {
            0.0
        };

        vel.0 += delta;
    }
}

fn raycast_for_block(
    player_pos: Vec3,
    player_rot: Quat,
    chunk_map: &ChunkMap,
) -> Option<(IVec3, IVec3)> {
    let origin = player_pos + Vec3::new(0.0, -PLAYER_HEIGHT / 2.0 + PLAYER_EYE_HEIGHT, 0.0); // -half player height + eye height
    let mut pos = origin;
    let dir_inv = 1.0 / (player_rot * Vec3::X).normalize();

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
        } else if (pos - origin).length_squared() > (PLAYER_INTERACT_RANGE * PLAYER_INTERACT_RANGE) {
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
        Entity,
        &PlayerID,
        &PlayerActionsTracker,
        &mut Inventory,
        &HeadRotation,
        &Transform,
    )>,
    mut chunk_map: Single<&mut ChunkMap>,
    mut block_updates: ResMut<BlockUpdates>,
) {
    let mut placements = Vec::new();
    for (entity, player_id, actions, inv, rotation, transform) in player_query.iter_mut() {
        if actions.0.is_set(PlayerActions::PLACE_BLOCK) {
            if inv.get_selected_item() == Item::Block && inv.get_count(Item::Block) > 0 {
                let Some((block_pos, face)) =
                    raycast_for_block(transform.translation, rotation.0, &chunk_map)
                else {
                    continue;
                };

                let block_pos = block_pos + face;
                let block_type = if player_id.0 == 0 {
                    BlockType::RedBlock
                } else {
                    BlockType::BlueBlock
                };
                placements.push((entity, block_pos, block_type));
            }
        }
    }
    'outer: for (entity, block_pos, block_type) in placements {
        for (_, _, _, _, _, transform) in player_query.iter() {
            let foot_pos = (transform.translation - Vec3::ZERO.with_y(PLAYER_HEIGHT / 2.0)).floor().as_ivec3() + IVec3::Y;
            if block_pos == foot_pos || block_pos == foot_pos + IVec3::Y {
                continue 'outer;
            }
        }

        chunk_map.set_block(block_pos, block_type).unwrap();
        block_updates.0.push((block_pos, block_type));

        player_query.get_mut(entity).unwrap().3.remove_item(Item::Block, 1);
    }
}

fn update_breaking_status(
    mut player_query: Query<(
        &PlayerActionsTracker,
        &Inventory,
        &mut BreakingStatusTracker,
        &HeadRotation,
        &Transform,
    )>,
    chunk_map: Single<&ChunkMap>,
) {
    for (actions, inv, mut breaking_status_tracker, rotation, transform) in player_query.iter_mut()
    {
        if actions.0.is_set(PlayerActions::DIG_BLOCK) {
            if let Some((block_pos, _)) =
                raycast_for_block(transform.translation, rotation.0, &chunk_map)
            {
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

fn break_block(
    player_query: Query<&BreakingStatusTracker, Changed<BreakingStatusTracker>>,
    mut chunk_map: Single<&mut ChunkMap>,
    mut block_updates: ResMut<BlockUpdates>,
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
        block_updates
            .0
            .push((breaking_status.block_pos, BlockType::Air));
    }
}

fn attack(
    player_query: Query<(
        Entity,
        &PlayerActionsTracker,
        &Inventory,
        &HeadRotation,
        &Transform,
    )>,
    mut player_query_2: Query<
        (&mut Health, &mut HurtCooldown, &mut LinearVelocity),
        With<PlayerID>,
    >,
    spatial_query: SpatialQuery,
) {
    let mut hit_queue: Vec<(Entity, f32, Vec3)> = Vec::new();
    for (entity, actions, inv, rotation, transform) in player_query.iter() {
        if actions.0.is_set(PlayerActions::ATTACK) {
            let origin = transform.translation
                + Vec3::new(0.0, -PLAYER_HEIGHT / 2.0 + PLAYER_EYE_HEIGHT, 0.0); // -half player height + eye height
            let dir = rotation.0 * Vec3::X;

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
                    Vec3::new(dir.x, 0.5, dir.z).normalize() * 10.0,
                ));
            }
        }
    }
    fastrand::shuffle(hit_queue.as_mut_slice());
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
    player_query: Query<(&ItemUsageStatusTracker, &HeadRotation, &Transform)>,
    mut commands: Commands,
) {
    for (item_usage_tracker, rotation, transform) in player_query.iter() {
        let Some(item_usage) = item_usage_tracker.0.as_ref() else {
            continue;
        };
        if item_usage.item != Item::Bow || item_usage.ticks_left > 0 {
            continue;
        }

        let dir = rotation.0 * Vec3::Z.normalize();
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
            ))
            .observe(handle_arrow_collision)
            .observe(send_arrow_despawns);
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

/// Check if any player has reached their goal area
/// Only one player can score at a time; if multiple are in the goal area, one is chosen at random
fn check_goal(player_query: Query<(Entity, &PlayerID, &Transform)>, mut commands: Commands) {
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
    if let Some(chosen_entity) = entities.get(fastrand::usize(..entities.len().max(1))) {
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
                reason: String::new(),
            });
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
    mut player_query: Query<(
        &PlayerID,
        &mut Transform,
        &mut HeadRotation,
        &mut LinearVelocity,
    )>,
) {
    let (player_id, mut transform, mut rotation, mut vel) = player_query.get_mut(event.0).unwrap();
    transform.translation = SPAWN_POSITIONS[player_id.0 as usize];
    rotation.0 = Quat::from_rotation_y(SPAWN_ROTATIONS[player_id.0 as usize]);
    vel.0 = Vec3::ZERO;
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

fn send_animations(
    mut player_query: Query<(&PlayerActionsTracker, &Inventory, &mut PlayerAnimation)>,
) {
    for (actions, inv, mut animation) in player_query.iter_mut() {
        if actions.0.is_set(PlayerActions::ATTACK)
            || actions.0.is_set(PlayerActions::DIG_BLOCK)
            || (actions.0.is_set(PlayerActions::PLACE_BLOCK)
                && inv.get_selected_item() == Item::Block
                && inv.get_count(Item::Block) > 0)
        {
            *animation = PlayerAnimation::Swing;
        } else if actions.0.is_set(PlayerActions::USE_ITEM)
            && inv.get_count(inv.get_selected_item()) > 0
        {
            match inv.get_selected_item() {
                Item::Bow => {
                    *animation = PlayerAnimation::DrawBow;
                }
                Item::GoldenApple => {
                    *animation = PlayerAnimation::Eat;
                }
                _ => {}
            }
        } else {
            *animation = PlayerAnimation::None;
        }
    }
}

fn send_death_events(
    event: On<DeathEvent>,
    player_query: Query<&PlayerID>,
    mut deaths: ResMut<Deaths>,
) {
    let player_id = player_query.get(event.0).unwrap().0;
    deaths.0.insert(PlayerID(player_id));
}

fn send_goal_events(
    event: On<GoalEvent>,
    player_query: Query<&PlayerID>,
    mut goals: ResMut<Goals>,
) {
    let player_id = player_query.get(event.0).unwrap().0;
    goals.0 = Some(PlayerID(player_id));
}

fn send_arrow_updates(
    mut arrow_events: ResMut<ArrowEvents>,
    arrow_query: Query<(&Arrow, &Transform), Changed<Arrow>>,
) {
    for (arrow, transform) in arrow_query.iter() {
        arrow_events.0.push(ArrowEvent::Updated {
            id: arrow.id,
            position: transform.translation,
            rotation: transform.rotation,
        });
    }
}

fn send_arrow_despawns(
    remove_event: On<Remove>,
    arrow_query: Query<&Arrow>,
    mut arrow_events: ResMut<ArrowEvents>,
) {
    let Ok(arrow) = arrow_query.get(remove_event.entity) else {
        return;
    };
    arrow_events.0.push(ArrowEvent::Despawned(arrow.id));
}

fn send_inventory_updates(
    player_query: Query<(&PlayerID, &Inventory), Changed<Inventory>>,
    mut inv_changes: ResMut<InventoryChanges>,
) {
    for (player_id, inv) in player_query.iter() {
        inv_changes.0.insert(*player_id, inv.clone());
    }
}
