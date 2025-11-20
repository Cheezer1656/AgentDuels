use std::hash::Hash;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct MatchIDPacket {
    pub id: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HandshakePacket {
    pub protocol_version: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Item {
    Sword,
    Pickaxe,
    Bow,
    Arrow,
    Block,
    GoldenApple,
}

/// Bitflags representing player actions (Is reset every tick)
#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy)]
pub struct PlayerActions {
    pub bits: u16,
    pub rotation: [f32; 2], // yaw, pitch (in radians)
    pub item_change: Option<Item>,
}

impl PlayerActions {
    pub fn as_bytes(self) -> Vec<u8> {
        self.bits.to_le_bytes().iter().chain(self.rotation[0].to_le_bytes().iter()).chain(self.rotation[1].to_le_bytes().iter()).chain([if let Some(item) = self.item_change { item as u8 } else { 0 }].iter()).map(|b| *b).collect()
    }

    pub const MOVE_FORWARD: u16 = 1 << 0;
    pub const MOVE_BACKWARD: u16 = 1 << 1;
    pub const MOVE_LEFT: u16 = 1 << 2;
    pub const MOVE_RIGHT: u16 = 1 << 3;
    pub const JUMP: u16 = 1 << 4;
    pub const ATTACK: u16 = 1 << 5;
    pub const USE_ITEM: u16 = 1 << 6;
    pub const PLACE_BLOCK: u16 = 1 << 7;
    pub const DIG_BLOCK: u16 = 1 << 8;

    pub fn is_set(&self, flag: u16) -> bool {
        self.bits & flag != 0
    }

    pub fn set(&mut self, flag: u16) {
        self.bits |= flag;
    }

    pub fn unset(&mut self, flag: u16) {
        self.bits &= !flag;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerActionsPacket {
    pub prev_actions: PlayerActions,
    pub nonce: u128,
    pub action_hash: [u8; 32],
}