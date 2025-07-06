use std::net::SocketAddr;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use agentduels_protocol::{
    PacketCodec,
    packets::{HandshakePacket, MatchIDPacket},
};
use tokio::net::TcpStream;

pub struct GameClient {
    socket: TcpStream,
}

impl GameClient {
    pub async fn connect(addr: SocketAddr) -> anyhow::Result<Self> {
        let mut socket = TcpStream::connect(addr)
            .await
            .expect("Failed to connect to game server");

        let codec = PacketCodec::default();

        let mut buf = [0; 64];
        socket.read(buf.as_mut_slice()).await.unwrap();
        let packet: MatchIDPacket = codec.read(&buf).unwrap();

        println!("Match ID: {}", packet.id);

        let packet = HandshakePacket {
            protocol_version: 1,
        };
        socket
            .write_all(&codec.write(&packet).unwrap())
            .await
            .unwrap();

        let mut buf = [0; 64];
        socket.read(buf.as_mut_slice()).await.unwrap();
        let packet: HandshakePacket = codec.read(&buf).unwrap();

        println!(
            "Other client has protocol version {}",
            packet.protocol_version
        );

        Ok(GameClient { socket })
    }
}
