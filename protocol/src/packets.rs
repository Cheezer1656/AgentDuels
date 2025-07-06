use agentduels_protocol_macros::Packet;
use serde::{Deserialize, Serialize};

use crate::Packet;

#[derive(Packet, Serialize, Deserialize)]
pub struct MatchIDPacket {
    pub id: u32,
}

#[derive(Packet, Serialize, Deserialize)]
pub struct HandshakePacket {
    pub protocol_version: u32,
}
