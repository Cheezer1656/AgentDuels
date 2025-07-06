use std::net::ToSocketAddrs;

use agentduels_protocol::{packets::MatchIDPacket, PacketCodec};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpListener, TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::mpsc,
};

#[derive(Default)]
pub struct GameServer {
    queue: Option<(TcpStream, mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>)>,
}

impl GameServer {
    pub async fn listen<T: ToSocketAddrs>(&mut self, addr: T) -> anyhow::Result<()> {
        let listener = TcpListener::bind(addr.to_socket_addrs().unwrap().next().unwrap()).await?;

        let codec = PacketCodec::default();
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let (tx, rx) = mpsc::channel(10);
            if let Some((queue_socket, queue_tx, queue_rx)) = self.queue.take() {
                let packet = MatchIDPacket {
                    id: rand::random()
                };

                let (read, mut write) = socket.into_split();
                if let Err(_) = write.write_all(&codec.write(&packet)?).await {
                    drop(read);
                    drop(write);
                    continue;
                };
                tokio::spawn(async move {
                    GameServer::handle_receiving(read, queue_tx).await;
                });
                tokio::spawn(async move {
                    GameServer::handle_sending(write, rx).await;
                });

                let (read, mut write) = queue_socket.into_split();
                if let Err(_) = write.write_all(&codec.write(&packet)?).await {
                    drop(read);
                    drop(write);
                    continue;
                };
                tokio::spawn(async move {
                    GameServer::handle_receiving(read, tx).await;
                });
                tokio::spawn(async move {
                    GameServer::handle_sending(write, queue_rx).await;
                });
            } else {
                self.queue = Some((socket, tx, rx));
            }
        }
    }

    pub async fn handle_receiving(mut socket: OwnedReadHalf, tx: mpsc::Sender<Vec<u8>>) {
        loop {
            let mut buf = [0; 64];
            match socket.read(&mut buf).await {
                Ok(n) => {
                    if n == 0 {
                        break;
                    }
                    if let Err(_) = tx.send(buf.to_vec()).await {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    pub async fn handle_sending(mut socket: OwnedWriteHalf, mut rx: mpsc::Receiver<Vec<u8>>) {
        loop {
            if let Some(bytes) = rx.recv().await {
                let _ = socket.write_all(bytes.as_slice()).await;
            }
        }
    }
}
