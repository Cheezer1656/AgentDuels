use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct MatchIDPacket {
    pub id: u32,
}

#[derive(Serialize, Deserialize)]
pub struct HandshakePacket {
    pub protocol_version: u32,
}
