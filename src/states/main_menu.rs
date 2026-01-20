use crate::{AppState, AutoDespawn, ControlServer};
use bevy::prelude::*;

#[derive(Component)]
struct PlayButton {
    enabled: bool,
}

#[derive(Component)]
struct QuitButton;

#[derive(Bundle)]
pub struct ButtonBundle {
    button: Button,
    node: Node,
    border_color: BorderColor,
    background_color: BackgroundColor,
}

impl ButtonBundle {
    pub fn new(margin: UiRect) -> Self {
        ButtonBundle {
            button: Button::default(),
            node: Node {
                width: Val::Px(400.0),
                height: Val::Px(80.0),
                border: UiRect::all(Val::Px(2.5)),
                margin,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(30.0)),
                ..default()
            },
            border_color: BorderColor::all(Color::BLACK),
            background_color: BackgroundColor(Color::Srgba(Srgba::rgb_u8(180, 0, 0))),
        }
    }
}

#[derive(Component)]
pub struct ConnectionText;

#[derive(Component)]
pub struct ConnectionIcon;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MainMenu), setup)
            .add_systems(
                Update,
                (
                    play_button_press,
                    quit_button_press,
                    (update_button, update_connection_display)
                        .run_if(resource_changed::<ControlServer>),
                )
                    .run_if(in_state(AppState::MainMenu)),
            );
    }
}

fn setup(
    mut commands: Commands,
    server: Option<Res<ControlServer>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((Camera2d::default(), AutoDespawn(AppState::MainMenu)));

    commands.spawn((
        AutoDespawn(AppState::MainMenu),
        Mesh2d(meshes.add(Rectangle::from_size(Vec2::new(2050.0, 1286.0)))),
        MeshMaterial2d(materials.add(ColorMaterial {
            texture: Some(asset_server.load("textures/background.png")),
            ..default()
        })),
    ));

    let client_connected = if let Some(server) = &server {
        server.client.is_some()
    } else {
        false
    };

    let (text, color) = if client_connected {
        ("Client connected", Srgba::GREEN)
    } else {
        ("No client connected", Srgba::RED)
    };

    commands.spawn((
        AutoDespawn(AppState::MainMenu),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            margin: UiRect::left(Val::Px(50.0)),
            ..default()
        },
        children![
            (
                Text::new("AgentDuels"),
                TextFont {
                    font: asset_server.load("fonts/LeagueSpartan-Bold.ttf"),
                    font_size: 100.0,
                    ..default()
                },
                Node {
                    margin: UiRect::top(Val::Px(50.0)),
                    ..default()
                }
            ),
            (
                ButtonBundle::new(UiRect::default().with_top(Val::Px(50.0))),
                PlayButton {
                    enabled: client_connected,
                },
                children![Text::new("Join Game"),],
            ),
            (
                ButtonBundle::new(UiRect::default().with_top(Val::Px(10.0))),
                QuitButton,
                children![Text::new("Quit Game"),],
            ),
        ],
    ));

    commands.spawn((
        AutoDespawn(AppState::MainMenu),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(40.0),
            ..default()
        },
        children![(
            ConnectionText,
            Text::new(text),
            TextColor(Color::Srgba(color)),
        ),],
    ));

    commands.spawn((
        AutoDespawn(AppState::MainMenu),
        ConnectionIcon,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            height: Val::Px(22.0),
            width: Val::Px(22.0),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::Srgba(color)),
    ));
}

fn play_button_press(
    mut next_state: ResMut<NextState<AppState>>,
    button_query: Query<(&PlayButton, &Interaction), Changed<Interaction>>,
) {
    if let Ok((play_button, interaction)) = button_query.single() {
        if play_button.enabled == true && *interaction == Interaction::Pressed {
            next_state.set(AppState::Joining);
        }
    }
}

fn quit_button_press(
    button_query: Query<&Interaction, (With<QuitButton>, Changed<Interaction>)>,
    mut exit_writer: MessageWriter<AppExit>,
) {
    if let Ok(interaction) = button_query.single()
        && *interaction == Interaction::Pressed
    {
        exit_writer.write(AppExit::Success);
    }
}

fn update_button(
    mut butten_query: Query<(&mut PlayButton, &mut BackgroundColor)>,
    server: Res<ControlServer>,
) {
    if let Ok((mut play_button, mut bg_color)) = butten_query.single_mut() {
        if server.client.is_some() {
            play_button.enabled = true;
            bg_color.0 = Color::Srgba(Srgba::rgb_u8(180, 0, 0));
        } else {
            play_button.enabled = false;
            bg_color.0 = Color::srgb_u8(83, 83, 83);
        }
    }
}

fn update_connection_display(
    server: Res<ControlServer>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<ConnectionText>>,
    mut icon_query: Query<&mut BackgroundColor, With<ConnectionIcon>>,
) {
    if let Ok((mut text, mut text_color)) = text_query.single_mut()
        && let Ok(mut icon_color) = icon_query.single_mut()
    {
        if server.client.is_some() {
            text.0 = "Client connected".to_string();
            text_color.0 = Color::Srgba(Srgba::GREEN);
            icon_color.0 = Color::Srgba(Srgba::GREEN);
        } else {
            text.0 = "No client connected".to_string();
            text_color.0 = Color::Srgba(Srgba::RED);
            icon_color.0 = Color::Srgba(Srgba::RED);
        }
    }
}
