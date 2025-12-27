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
    match_id: u64,
}

impl GameConnection {
    pub fn match_id(&self) -> u64 {
        self.match_id
    }
}

impl GameConnection {
    pub fn connect(addr: SocketAddr) -> anyhow::Result<Self> {
        let mut socket = TcpStream::connect(addr).expect("Failed to connect to game server");

        let codec = PacketCodec::default();

        let mut buf = [0; 16];
        socket.read(buf.as_mut_slice())?;
        println!("Read {:?} bytes", &buf);
        let Packet::MatchID(ref packet) = codec.read(&buf)?[0] else {
            bail!("Expected MatchID packet");
        };

        println!("Match ID: {}", packet.id);
        let match_id = packet.id;

        let packet = Packet::Handshake(HandshakePacket {
            protocol_version: 1,
        });
        socket.write_all(&codec.write(&packet)?)?;

        let mut buf = [0; 16];
        socket.read(buf.as_mut_slice())?;
        let Packet::Handshake(ref packet) = codec.read(&buf)?[0] else {
            bail!("Expected Handshake packet");
        };

        println!(
            "Other client has protocol version {}",
            packet.protocol_version
        );

        Ok(GameConnection {
            socket,
            codec,
            match_id,
        })
    }

    pub fn send_packet(&mut self, packet: Packet) -> anyhow::Result<()> {
        let data = self.codec.write(&packet)?;
        self.socket.write_all(&data)?;
        Ok(())
    }
}
