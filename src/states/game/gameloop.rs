use crate::player::{
    Inventory, PlayerBody
    , PlayerHand, PlayerHead, Score, PLAYER_ANIMATION_INDICES,
};
use crate::player::{PlayerAnimation, PlayerID};
use crate::states::game::{BlueScoreMarker, RedScoreMarker, TPSMarker};
use crate::states::network::TickEvent;
use crate::world::ChunkMap;
use crate::{AppState, Arrow, ArrowEvent, AutoDespawn};
use bevy::prelude::*;

pub struct GameLoopPlugin;

impl Plugin for GameLoopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LastTick>()
            .add_systems(
            Update,
            (
                update_player_positions,
                apply_damage_tint,
                update_animations,
                update_inventories,
                update_item_model.after(update_inventories),
                update_arrows,
                update_chunkmap,
                update_scores,
                update_scoreboard.after(update_scores),
                update_tps,
            ),
        );
    }
}

fn update_player_positions(
    mut tick_events: MessageReader<TickEvent>,
    mut player_query: Query<(&PlayerID, &mut Transform, &Children)>,
    mut player_body_query: Query<&mut Transform, (With<PlayerBody>, Without<PlayerID>)>,
    mut player_head_query: Query<
        &mut Transform,
        (With<PlayerHead>, Without<PlayerID>, Without<PlayerBody>),
    >,
    children_query: Query<&Children>,
) {
    for tick_event in tick_events.read() {
        for (player_id, mut transform, children) in player_query.iter_mut() {
            let player_info = &tick_event.players[player_id.0 as usize];

            transform.translation = player_info.position;

            for child in children.iter() {
                let Ok(mut body_transform) = player_body_query.get_mut(child) else {
                    continue;
                };
                // Set body rotation yaw only
                body_transform.rotation = Quat::from_rotation_y(player_info.yaw);

                for grandchild in children_query.iter_descendants(child) {
                    let Ok(mut head_transform) = player_head_query.get_mut(grandchild) else {
                        continue;
                    };
                    // Set head rotation pitch only
                    head_transform.rotation = Quat::from_rotation_x(player_info.pitch);
                }
            }
        }
    }
}

fn apply_damage_tint(
    mut tick_events: MessageReader<TickEvent>,
    player_query: Query<(Entity, &PlayerID)>,
    children_query: Query<&Children>,
    material_query: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for tick_event in tick_events.read() {
        for (entity, player_id) in player_query.iter() {
            let player_info = &tick_event.players[player_id.0 as usize];
            let Some(is_hurt) = player_info.hurt_update else {
                continue;
            };
            for child in children_query.iter_descendants(entity) {
                let Ok(mesh_material) = material_query.get(child) else {
                    continue;
                };

                let material = materials.get_mut(&mesh_material.0).unwrap();
                material.base_color = if is_hurt {
                    Color::srgb_u8(255, 0, 0)
                } else {
                    Color::WHITE
                };
            }
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
    mut tick_events: MessageReader<TickEvent>,
    player_query: Query<
        (
            Entity,
            &PlayerID,
        ),
        With<PlayerID>,
    >,
    children: Query<&Children>,
    mut anim_player_query: Query<&mut AnimationPlayer>,
) {
    for tick_event in tick_events.read() {
        for (entity, player_id) in player_query.iter() {
            let player_info = &tick_event.players[player_id.0 as usize];
            let vel = player_info.velocity;

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
                    animation.set_speed((vel.x.abs() + vel.z.abs()) * 0.7);
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

                match player_info.animation {
                    PlayerAnimation::Swing => 'inner: {
                        if is_animation_playing(&anim_player, PLAYER_ANIMATION_INDICES.swing.into()) {
                            break 'inner;
                        }
                        stop_hand_animations(&mut anim_player);
                        anim_player
                            .start(PLAYER_ANIMATION_INDICES.swing.into())
                            .set_speed(3.0);
                    }
                    PlayerAnimation::DrawBow => 'inner: {
                        if is_animation_playing(
                            &anim_player,
                            PLAYER_ANIMATION_INDICES.draw_bow.into(),
                        ) {
                            break 'inner;
                        }
                        stop_hand_animations(&mut anim_player);
                        anim_player.start(PLAYER_ANIMATION_INDICES.draw_bow.into());
                    }
                    PlayerAnimation::Eat => 'inner: {
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

fn update_inventories(
    mut tick_events: MessageReader<TickEvent>,
    player_query: Query<(Entity, &PlayerID)>,
    mut inventories: Query<&mut Inventory>,
) {
    for tick_event in tick_events.read() {
        for (entity, player_id) in player_query.iter() {
            let player_info = &tick_event.players[player_id.0 as usize];
            let Some(inv_update) = &player_info.inventory_update else {
                continue;
            };
            let mut inventory = inventories.get_mut(entity).unwrap();
            *inventory = inv_update.clone();
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
        for child in children.iter_descendants(entity) {
            let Ok(entity) = player_hand_query.get(child) else {
                continue;
            };
            commands.entity(entity).despawn_children();

            if inv.get_count(inv.get_selected_item()) > 0 {
                println!(
                    "Updating item model for entity {:?} to {:?}",
                    entity,
                    inv.get_selected_item()
                );
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

fn update_arrows(
    mut tick_events: MessageReader<TickEvent>,
    mut arrow_query: Query<(Entity, &Arrow, &mut Transform)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    for tick_event in tick_events.read(){
        for arrow_event in tick_event.arrow_events.iter() {
            match arrow_event {
                ArrowEvent::Updated { id, position, rotation } => {
                    let rotation = rotation * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2); // Adjust for model orientation
                    let mut updated = false;
                    for (_, arrow, mut transform) in arrow_query.iter_mut() {
                        if arrow.id != *id {
                            continue;
                        }
                        transform.translation = *position;
                        transform.rotation = rotation;
                        updated = true;
                    }
                    if !updated {
                        // Spawn new arrow
                        commands.spawn((
                            AutoDespawn(AppState::Game),
                            Arrow {
                                id: *id,
                                ..default()
                            },
                            Transform {
                                translation: *position,
                                rotation,
                                ..default()
                            },
                            SceneRoot(assets.load("models/items/Arrow.gltf#Scene0")),
                        ));
                    }
                }
                ArrowEvent::Despawned(id) => {
                    for (entity, arrow, _) in arrow_query.iter_mut() {
                        if arrow.id != *id {
                            continue;
                        }
                        if let Ok(mut entity_commands) = commands.get_entity(entity) {
                            entity_commands.despawn();
                        }
                    }
                }
            }
        }
    }
}

fn update_chunkmap(
    mut tick_events: MessageReader<TickEvent>,
    mut chunkmap: Single<&mut ChunkMap>,
) {
    for tick_event in tick_events.read() {
        for (block_pos, block_type) in tick_event.block_updates.iter() {
            let _ = chunkmap.set_block(*block_pos, *block_type);
        }
    }
}

fn update_scores(
    mut tick_events: MessageReader<TickEvent>,
    mut scores: Query<(&PlayerID, &mut Score)>,
) {
    for tick_event in tick_events.read() {
        for (player_id, mut score) in scores.iter_mut() {
            let Some(scoring_player_id) = tick_event.goals else {
                continue;
            };
            if scoring_player_id.0 != player_id.0 {
                continue;
            }
            score.0 += 1;
        }
    }
}

fn update_scoreboard(
    mut red_score: Single<(&mut TextSpan,), With<RedScoreMarker>>,
    mut blue_score: Single<(&mut TextSpan,), (With<BlueScoreMarker>, Without<RedScoreMarker>)>,
    score_query: Query<(&PlayerID, &Score), Changed<Score>>,
) {
    for (player_id, score) in score_query.iter() {
        if player_id.0 == 0 {
            red_score.0.0 = score.0.to_string();
        } else {
            blue_score.0.0 = score.0.to_string();
        }
    }
}

#[derive(Resource, Default)]
struct LastTick(f32);

fn update_tps(
    mut tick_events: MessageReader<TickEvent>,
    mut tps_text: Single<&mut TextSpan, With<TPSMarker>>,
    mut last_tick: ResMut<LastTick>,
    time: Res<Time>,
) {
    for _ in tick_events.read() {
        let tps = 1.0 / (time.elapsed_secs() - last_tick.0);
        last_tick.0 = time.elapsed_secs();

        tps_text.0 = tps.to_string();
    }
}
