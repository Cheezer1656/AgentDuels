use crate::client::GameConnection;
use crate::states::ButtonBundle;
use crate::{AppState, AutoDespawn, GameResults};
use bevy::prelude::*;

#[derive(Component)]
struct MainMenuButton;

#[derive(Component)]
struct PlayAgainButton;

/// The plugin for the end menu state
/// This requires the GameResults resource to be set before entering the state
pub struct EndMenuPlugin;

impl Plugin for EndMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::EndMenu), setup)
            .add_systems(Update, button_press.run_if(in_state(AppState::EndMenu)));
    }
}

fn setup(mut commands: Commands, game_results: Res<GameResults>, game_connection: Res<GameConnection>, asset_server: Res<AssetServer>) {
    commands.spawn((Camera2d::default(), AutoDespawn(AppState::EndMenu)));

    commands.spawn((
        AutoDespawn(AppState::EndMenu),
        Sprite {
            image: asset_server.load("textures/atlas.png"),
            image_mode: SpriteImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 2.0,
            },
            color: Color::srgb_u8(80, 80, 80),
            rect: Some(Rect {
                min: Vec2::new(0.0, 16.0),
                max: Vec2::new(16.0, 32.0),
            }),
            custom_size: Some(Vec2::new(1920.0, 1080.0)),
            ..default()
        },
        Transform::from_scale(Vec3::splat(5.0)),
    ));

    commands.spawn((
        AutoDespawn(AppState::EndMenu),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![
            (
                Text::new(match game_results.winner {
                    Some(player_id) => {
                        if player_id == game_connection.player_id.0 {
                            "You win!"
                        } else {
                            "You lose!"
                        }
                    },
                    None => game_results.reason.as_str(),
                }),
                TextFont {
                    font: asset_server.load("fonts/LeagueSpartan-Bold.ttf"),
                    font_size: if game_results.winner.is_some() {
                        100.0
                    } else {
                        75.0
                    },
                    ..default()
                },
            ),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::top(Val::Px(50.0)),
                    ..default()
                },
                children![
                    (
                        PlayAgainButton,
                        ButtonBundle::new(UiRect::right(Val::Px(5.0))),
                        children![Text::new("Play Again?"),],
                    ),
                    (
                        MainMenuButton,
                        ButtonBundle::new(UiRect::left(Val::Px(5.0))),
                        children![Text::new("Main Menu"),],
                    ),
                ]
            )
        ],
    ));
}

fn button_press(
    mut next_state: ResMut<NextState<AppState>>,
    button_query: Query<
        (
            &Interaction,
            Option<&MainMenuButton>,
            Option<&PlayAgainButton>,
        ),
        Changed<Interaction>,
    >,
) {
    if let Ok((interaction, main_menu, play_again)) = button_query.single() {
        if *interaction != Interaction::Pressed {
            return;
        }
        if main_menu.is_some() {
            next_state.set(AppState::MainMenu);
        } else if play_again.is_some() {
            next_state.set(AppState::Joining);
        }
    }
}
