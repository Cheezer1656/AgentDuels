use ::bevy::prelude::*;

mod networking;

#[derive(Component)]
struct ConnectionText;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d::default());

    commands.spawn((
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
                BorderColor(Color::BLACK),
                BorderRadius::MAX,
                BackgroundColor(Color::Srgba(Srgba::GREEN)),
                children![Text::new("Join Game"),],
            ),
            (
                ConnectionText,
                Text::new("No program connected"),
                TextColor(Color::Srgba(Srgba::RED)),
            )
        ],
    ));
}