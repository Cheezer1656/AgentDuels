use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct MatchIDPacket {
    pub id: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HandshakePacket {
    pub protocol_version: u32,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy)]
pub struct PlayerActions {
    pub bits: u8,
    pub rotation: [f32; 2], // yaw, pitch
}

impl PlayerActions {
    pub const MOVE_FORWARD: u8 = 1 << 0;
    pub const MOVE_BACKWARD: u8 = 1 << 1;
    pub const MOVE_LEFT: u8 = 1 << 2;
    pub const MOVE_RIGHT: u8 = 1 << 3;
    pub const JUMP: u8 = 1 << 4;
    pub const ATTACK: u8 = 1 << 5;
    pub const USE_ITEM: u8 = 1 << 6;

    pub fn is_set(&self, flag: u8) -> bool {
        self.bits & flag != 0
    }

    pub fn set(&mut self, flag: u8) {
        self.bits |= flag;
    }

    pub fn unset(&mut self, flag: u8) {
        self.bits &= !flag;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerActionsPacket {
    pub prev_actions: PlayerActions,
    pub nonce: u128,
    pub action_hash: [u8; 32]
}
