use std::net::ToSocketAddrs;

use agentduels_protocol::{Packet, PacketCodec, packets::MatchIDPacket};
use tokio::{
    io::{AsyncWriteExt, copy_bidirectional},
    net::{TcpListener, TcpStream},
};

#[derive(Default)]
pub struct GameServer {
    queue: Option<TcpStream>,
}

impl GameServer {
    pub async fn listen<T: ToSocketAddrs>(&mut self, addr: T) -> anyhow::Result<()> {
        let listener = TcpListener::bind(addr.to_socket_addrs().unwrap().next().unwrap()).await?;

        let codec = PacketCodec::default();
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            if let Some(mut new_socket) = self.queue.take() {
                let packet = Packet::MatchID(MatchIDPacket { id: rand::random() });
                let data = codec.write(&packet).unwrap();

                socket.write_all(&data).await.unwrap();
                new_socket.write_all(&data).await.unwrap();

                tokio::spawn(async move {
                    copy_bidirectional(&mut socket, &mut new_socket)
                        .await
                        .unwrap();
                });
            } else {
                self.queue = Some(socket);
            }
        }
    }
}
