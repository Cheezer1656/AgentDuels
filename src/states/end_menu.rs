use crate::{AppState, AutoDespawn};
use bevy::prelude::*;

#[derive(Resource)]
pub struct GameResults {
    pub(crate) winner: u16,
}

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

fn setup(mut commands: Commands, game_results: Res<GameResults>) {
    commands.spawn((Camera2d::default(), AutoDespawn(AppState::EndMenu)));

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
                Text::new(if game_results.winner == 0 {
                    "You win!"
                } else {
                    "You lose"
                }),
                TextFont::default().with_font_size(100.0),
            ),
            (
                Text::new("Play again?"),
                TextColor(Color::Srgba(Srgba::RED)),
            ),
            (
                PlayAgainButton,
                Button::default(),
                Node {
                    width: Val::Px(150.0),
                    height: Val::Px(65.0),
                    border: UiRect::all(Val::Px(5.0)),
                    margin: UiRect::default()
                        .with_top(Val::Px(20.0))
                        .with_bottom(Val::Px(20.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(Color::BLACK),
                BorderRadius::MAX,
                BackgroundColor(Color::Srgba(Srgba::GREEN)),
                children![Text::new("Yes"),],
            ),
            (
                MainMenuButton,
                Button::default(),
                Node {
                    width: Val::Px(150.0),
                    height: Val::Px(65.0),
                    border: UiRect::all(Val::Px(5.0)),
                    margin: UiRect::default()
                        .with_top(Val::Px(20.0))
                        .with_bottom(Val::Px(20.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(Color::BLACK),
                BorderRadius::MAX,
                BackgroundColor(Color::Srgba(Srgba::GREEN)),
                children![Text::new("No"),],
            ),
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
