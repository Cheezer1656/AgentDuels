use std::net::ToSocketAddrs;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpListener, TcpStream
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

        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let (tx, rx) = mpsc::channel(10);
            if let Some((queue_socket, queue_tx, queue_rx)) = self.queue.take() {
                let (read, write) = socket.into_split();
                tokio::spawn(async move {
                    GameServer::handle_receiving(read, queue_tx).await;
                });
                tokio::spawn(async move {
                    GameServer::handle_sending(write, queue_rx).await;
                });
                let (read, write) = queue_socket.into_split();
                tokio::spawn(async move {
                    GameServer::handle_receiving(read, tx).await;
                });
                tokio::spawn(async move {
                    GameServer::handle_sending(write, rx).await;
                });
            } else {
                self.queue = Some((socket, tx, rx));
            }
        }
    }

    pub async fn handle_receiving(mut socket: OwnedReadHalf, tx: mpsc::Sender<Vec<u8>>) {
        loop {
            let mut bytes = Vec::new();
            match socket.read(bytes.as_mut_slice()).await {
                Ok(n) => {
                    if n == 0 {
                        break;
                    }
                    let _ = tx.send(bytes);
                }
                Err(_) => break,
            }
        }
    }

    pub async fn handle_sending(mut socket: OwnedWriteHalf, mut rx: mpsc::Receiver<Vec<u8>>) {
        loop {
            if let Ok(bytes) = rx.try_recv() {
                let _ = socket.write_all(bytes.as_slice());
            } else {
                break;
            }
        }
    }
}
