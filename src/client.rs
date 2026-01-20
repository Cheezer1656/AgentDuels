use crate::player::{PlayerActions, PlayerID};
use crate::{GAME_VERSION, SERVER_URL};
use anyhow::{Context, bail};
use bevy::ecs::resource::Resource;
use bevy::utils::default;
use std::thread;
use std::time::Duration;
use tokio::runtime::Builder;
use workflow_websocket::client::{
    ConnectOptions, ConnectStrategy, Message, WebSocket, WebSocketConfig,
};

pub enum GameConnectionMessage {
    SendMessage(Message),
    Disconnect,
}

#[derive(Resource)]
pub struct GameConnection {
    pub socket: WebSocket,
    pub receiver_rx: std::sync::mpmc::Receiver<Message>,
    pub sender_tx: std::sync::mpmc::Sender<GameConnectionMessage>,
    pub match_id: u64,
    pub player_id: PlayerID,
}

impl GameConnection {
    pub fn match_id(&self) -> u64 {
        self.match_id
    }
}

impl GameConnection {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let url = url.to_string();
        let (tx, rx) = std::sync::mpsc::channel::<GameConnection>();

        thread::spawn(move || {
            let rt = Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build Tokio runtime");

            rt.block_on(async {
                let socket = WebSocket::new(
                    Some(url.as_str()),
                    Some(WebSocketConfig {
                        receiver_channel_cap: Some(10),
                        sender_channel_cap: Some(10),
                        ..default()
                    }),
                )?;
                socket
                    .connect(ConnectOptions {
                        block_async_connect: true,
                        strategy: ConnectStrategy::Fallback,
                        ..default()
                    })
                    .await
                    .context("Failed to connect to game server websocket")?;

                let Message::Open = socket.recv().await? else {
                    bail!("Expected Open message on connect");
                };

                println!("Connected to server. Sending game version...");
                socket
                    .send(Message::Binary(GAME_VERSION.to_be_bytes().to_vec()))
                    .await?;

                println!("Waiting for match ID...");
                let msg = socket.recv().await?;
                let Message::Binary(data) = msg else {
                    bail!("Unexpected message: {:?}", msg);
                };
                if data.len() != 8 {
                    bail!("Wrong data length for match ID");
                }
                let match_id = u64::from_be_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);
                println!("Match ID: {}", match_id);

                let msg = socket.recv().await?;
                let Message::Binary(data) = msg else {
                    bail!("Unexpected message: {:?}", msg);
                };
                if data.len() != 2 {
                    bail!("Wrong data length for player ID");
                }
                let player_id = PlayerID(u16::from_be_bytes([data[0], data[1]]));
                println!("Player ID: {}", player_id.0);

                let (receiver_tx, receiver_rx) = std::sync::mpmc::channel();
                let socket_clone = socket.clone();
                tokio::spawn(async move {
                    loop {
                        if let Ok(msg) = socket_clone.recv().await {
                            if receiver_tx.send(msg).is_err() {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                });

                let (sender_tx, mut sender_rx) = std::sync::mpmc::channel();

                tx.send(GameConnection {
                    socket: socket.clone(),
                    match_id,
                    player_id,
                    receiver_rx,
                    sender_tx,
                })
                .expect("Failed to send GameConnection");

                loop {
                    let msg = match sender_rx.try_recv() {
                        Ok(msg) => msg,
                        Err(e) => {
                            if e == std::sync::mpmc::TryRecvError::Empty {
                                tokio::time::sleep(Duration::from_millis(5)).await;
                                continue;
                            } else {
                                break;
                            }
                        }
                    };
                    match msg {
                        GameConnectionMessage::Disconnect => {
                            let _ = socket.disconnect().await;
                            break;
                        }
                        GameConnectionMessage::SendMessage(msg) => {
                            if socket.send(msg).await.is_err() {
                                println!("Failed to send message, disconnecting");
                                break;
                            }
                        }
                    }
                }

                Ok(())
            })
            .unwrap();
        });

        rx.recv()
            .context("Failed to receive GameConnection from thread")
    }
}
