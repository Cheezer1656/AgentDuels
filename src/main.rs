use agentduels::states::{EndMenuPlugin, GamePlugin, JoiningPlugin, MainMenuPlugin};
use agentduels::{AppState, AutoDespawn, ControlServer, handle_connection, handle_disconnects};
use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};

const CONTROL_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8082);

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind(CONTROL_ADDR).unwrap();
    listener.set_nonblocking(true).unwrap();

    App::new()
        .add_plugins((
            DefaultPlugins.set(ImagePlugin::default_nearest()),
            #[cfg(debug_assertions)]
            EguiPlugin::default(),
            #[cfg(debug_assertions)]
            WorldInspectorPlugin::new(),
        ))
        .insert_resource(GlobalAmbientLight {
            brightness: 400.0,
            ..default()
        })
        .insert_state(AppState::MainMenu)
        .add_plugins((
            MainMenuPlugin,
            JoiningPlugin,
            EndMenuPlugin,
            GamePlugin::new(false),
        ))
        .insert_resource(ControlServer::new(listener))
        .add_systems(FixedUpdate, (handle_connection, handle_disconnects))
        .add_systems(OnExit(AppState::Joining), cleanup_state)
        .add_systems(OnExit(AppState::MainMenu), cleanup_state)
        .add_systems(OnExit(AppState::Game), cleanup_state)
        .add_systems(OnExit(AppState::EndMenu), cleanup_state)
        .run();
}

fn cleanup_state(
    mut commands: Commands,
    query: Query<(Entity, &AutoDespawn)>,
    current_state: Res<State<AppState>>,
) {
    for (entity, auto_despawn) in query.iter() {
        if auto_despawn.0 != *current_state.get() {
            commands.entity(entity).despawn();
        }
    }
}
