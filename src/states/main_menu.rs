use bevy::prelude::*;

use crate::{AppState, AutoDespawn, ControlServer};

#[derive(Component)]
struct PlayButton;

#[derive(Component)]
pub struct ConnectionText;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MainMenu), setup)
            .add_systems(
                Update,
                (
                    button_press,
                    update_connection_text.run_if(resource_changed::<ControlServer>),
                )
                    .run_if(in_state(AppState::MainMenu)),
            );
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d::default());

    commands.spawn((
        AutoDespawn(AppState::MainMenu),
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
                Text::new("AgentDuels"),
                TextFont::default().with_font_size(100.0),
            ),
            (
                PlayButton,
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
                children![Text::new("Join Game"),],
            ),
            (
                ConnectionText,
                Text::new("No client connected"),
                TextColor(Color::Srgba(Srgba::RED)),
            )
        ],
    ));
}

fn button_press(
    mut next_state: ResMut<NextState<AppState>>,
    button_query: Query<&Interaction, (With<PlayButton>, Changed<Interaction>)>,
) {
    if let Ok(interaction) = button_query.single() {
        if *interaction == Interaction::Pressed {
            next_state.set(AppState::Joining);
        }
    }
}

fn update_connection_text(
    server: ResMut<ControlServer>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<ConnectionText>>,
) {
    if let Ok((mut text, mut color)) = text_query.single_mut() {
        if server.client.is_some() {
            text.0 = "Client connected".to_string();
            color.0 = Color::Srgba(Srgba::GREEN);
        } else {
            text.0 = "No client connected".to_string();
            color.0 = Color::Srgba(Srgba::RED);
        }
    }
}
