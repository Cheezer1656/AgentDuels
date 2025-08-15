use agentduels::SERVER_ADDR;
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future},
};

use crate::{AppState, AutoDespawn, networking::GameClient};

pub struct JoiningPlugin;

impl Plugin for JoiningPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Joining), (setup, start_connection))
            .add_systems(Update, poll_connection.run_if(in_state(AppState::Joining)));
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        AutoDespawn(AppState::Joining),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![Text::new("Joining Game..."),],
    ));
}

#[derive(Component)]
struct ConnectingTask(Task<Result<GameClient, anyhow::Error>>);

fn start_connection(mut commands: Commands, task_query: Query<&ConnectingTask>) {
    if task_query.single().is_ok() {
        // If there's already a connection task, don't start a new one
        return;
    }
    println!("Starting connection to game server...");
    let task_pool = AsyncComputeTaskPool::get();
    let task = task_pool.spawn(async move { GameClient::connect(SERVER_ADDR) });
    commands.spawn(ConnectingTask(task));
}

fn poll_connection(
    mut commands: Commands,
    mut task_query: Query<(Entity, &mut ConnectingTask)>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (entity, mut connecting_task) in task_query.iter_mut() {
        if let Some(result) = block_on(future::poll_once(&mut connecting_task.0)) {
            match result {
                Ok(client) => {
                    println!("Connected to game server");
                    commands.spawn(client);
                    next_state.set(AppState::Game);
                }
                Err(e) => {
                    eprintln!("Failed to connect: {}", e);
                    next_state.set(AppState::MainMenu);
                }
            }
            commands.entity(entity).despawn();
        }
    }
}
