use anyhow::bail;
use bevy::ecs::resource::Resource;
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
};

use agentduels_protocol::{Packet, PacketCodec, packets::HandshakePacket};

#[derive(Resource)]
pub struct GameConnection {
    pub socket: TcpStream,
    pub codec: PacketCodec,
}

impl GameConnection {
    pub fn connect(addr: SocketAddr) -> anyhow::Result<Self> {
        let mut socket = TcpStream::connect(addr).expect("Failed to connect to game server");

        let codec = PacketCodec::default();

        let mut buf = [0; 8];
        socket.read(buf.as_mut_slice()).unwrap();
        println!("Read {:?} bytes", &buf);
        let Packet::MatchID(ref packet) = codec.read(&buf).unwrap()[0] else {
            bail!("Expected MatchID packet");
        };

        println!("Match ID: {}", packet.id);

        let packet = Packet::Handshake(HandshakePacket {
            protocol_version: 1,
        });
        socket.write_all(&codec.write(&packet).unwrap()).unwrap();

        let mut buf = [0; 8];
        socket.read(buf.as_mut_slice()).unwrap();
        let Packet::Handshake(ref packet) = codec.read(&buf).unwrap()[0] else {
            bail!("Expected Handshake packet");
        };

        println!(
            "Other client has protocol version {}",
            packet.protocol_version
        );

        Ok(GameConnection { socket, codec })
    }

    pub fn send_packet(&mut self, packet: Packet) -> anyhow::Result<()> {
        let data = self.codec.write(&packet)?;
        self.socket.write_all(&data)?;
        Ok(())
    }
}
