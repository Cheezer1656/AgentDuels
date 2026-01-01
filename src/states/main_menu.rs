use bevy::prelude::*;

use crate::{AppState, AutoDespawn, ControlServer};

#[derive(Component, Default)]
struct PlayButton {
    enabled: bool,
}

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
                    (update_button, update_connection_text)
                        .run_if(resource_changed::<ControlServer>),
                )
                    .run_if(in_state(AppState::MainMenu)),
            );
    }
}

fn setup(mut commands: Commands, server: Option<Res<ControlServer>>) {
    commands.spawn((Camera2d::default(), AutoDespawn(AppState::MainMenu)));

    let (text, color) = if let Some(server) = server {
        if server.client.is_some() {
            ("Client connected", Srgba::GREEN)
        } else {
            ("No client connected", Srgba::RED)
        }
    } else {
        ("No client connected", Srgba::RED)
    };

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
                PlayButton::default(),
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
                Text::new(text),
                TextColor(Color::Srgba(color)),
            )
        ],
    ));
}

fn button_press(
    mut next_state: ResMut<NextState<AppState>>,
    button_query: Query<(&PlayButton, &Interaction), Changed<Interaction>>,
) {
    if let Ok((play_button, interaction)) = button_query.single() {
        if play_button.enabled == true && *interaction == Interaction::Pressed {
            next_state.set(AppState::Joining);
        }
    }
}

fn update_button(
    mut butten_query: Query<(&mut PlayButton, &mut BackgroundColor)>,
    server: Res<ControlServer>,
) {
    if let Ok((mut play_button, mut bg_color)) = butten_query.single_mut() {
        if server.client.is_some() {
            play_button.enabled = true;
            bg_color.0 = Color::Srgba(Srgba::GREEN);
        } else {
            play_button.enabled = false;
            bg_color.0 = Color::srgb_u8(50, 50, 50);
        }
    }
}

fn update_connection_text(
    server: Res<ControlServer>,
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
