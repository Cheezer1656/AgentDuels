use agentduels::states::GamePlugin;
use agentduels::states::network::OpponentDisconnected;
use agentduels::{
    ControlServer, SERVER_URL, client::GameConnection, handle_connection, handle_disconnects,
};
use bevy::DefaultPlugins;
use bevy::app::App;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{RenderCreation, WgpuSettings};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};

const CONTROL_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8083);

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind(CONTROL_ADDR).unwrap();
    listener.set_nonblocking(true).unwrap();

    let connection = GameConnection::connect(SERVER_URL).await.unwrap();

    App::new()
        .add_plugins(DefaultPlugins.set(RenderPlugin {
            synchronous_pipeline_compilation: true,
            render_creation: RenderCreation::Automatic(WgpuSettings {
                backends: None,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(connection)
        .insert_resource(ControlServer::new(listener))
        .add_systems(FixedUpdate, (handle_connection, handle_disconnects))
        .add_plugins(GamePlugin::new(true))
        .add_observer(handle_opponent_disconnect)
        .run();
}

fn handle_opponent_disconnect(
    _: On<OpponentDisconnected>,
    mut exit_writer: MessageWriter<AppExit>,
) {
    exit_writer.write(AppExit::Success);
}
