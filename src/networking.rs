use bevy::ecs::component::Component;
use std::{io::{Read, Write}, net::{SocketAddr, TcpStream}};

use agentduels_protocol::{
    PacketCodec,
    packets::{HandshakePacket, MatchIDPacket},
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
        let packet: MatchIDPacket = codec.read(&buf).unwrap();

        println!("Match ID: {}", packet.id);

        let packet = HandshakePacket {
            protocol_version: 1,
        };
        socket
            .write_all(&codec.write(&packet).unwrap())

            .unwrap();

        let mut buf = [0; 8];
        socket.read(buf.as_mut_slice()).unwrap();
        let packet: HandshakePacket = codec.read(&buf).unwrap();

        println!(
            "Other client has protocol version {}",
            packet.protocol_version
        );

        Ok(GameClient { socket })
    }
}
