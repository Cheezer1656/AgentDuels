use anyhow::bail;
use bevy::ecs::component::Component;
use std::{io::{Read, Write}, net::{SocketAddr, TcpStream}};

use agentduels_protocol::{
    packets::{HandshakePacket, MatchIDPacket}, Packet, PacketCodec
};

#[derive(Component)]
pub struct GameClient {
    socket: TcpStream,
}

impl GameClient {
    pub fn connect(addr: SocketAddr) -> anyhow::Result<Self> {
        let mut socket = TcpStream::connect(addr).expect("Failed to connect to game server");

        let codec = PacketCodec::default();

        let mut buf = [0; 8];
        socket.read(buf.as_mut_slice()).unwrap();
        println!("Read {:?} bytes", &buf);
        let Packet::MatchID(packet) = codec.read(&buf).unwrap() else {
            bail!("Expected MatchID packet");
        };

        println!("Match ID: {}", packet.id);

        let packet = Packet::Handshake(HandshakePacket {
            protocol_version: 1,
        });
        socket
            .write_all(&codec.write(&packet).unwrap())

            .unwrap();

        let mut buf = [0; 8];
        socket.read(buf.as_mut_slice()).unwrap();
        let Packet::Handshake(packet) = codec.read(&buf).unwrap() else {
            bail!("Expected Handshake packet");
        };

        println!(
            "Other client has protocol version {}",
            packet.protocol_version
        );

        Ok(GameClient { socket })
    }
}
