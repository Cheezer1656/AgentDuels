use serde::{Deserialize, Serialize};
use std::hash::Hash;

#[derive(Serialize, Deserialize, Debug)]
pub struct MatchIDPacket {
    pub id: u64,
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

impl Item {
    pub fn to_string(&self) -> &'static str {
        match self {
            Item::Sword => "Sword",
            Item::Pickaxe => "Pickaxe",
            Item::Bow => "Bow",
            Item::Arrow => "Arrow",
            Item::Block => "Block",
            Item::GoldenApple => "GoldenApple",
        }
    }
    pub fn ticks_needed(&self) -> usize {
        match self {
            Item::Sword => 0,
            Item::Pickaxe => 0,
            Item::Bow => 25,
            Item::Arrow => 0,
            Item::Block => 0,
            Item::GoldenApple => 20,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy, PartialEq)]
pub struct Rotation {
    pub yaw: f32,
    pub pitch: f32,
}

/// Bitflags representing player actions (Is reset every tick)
#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy)]
pub struct PlayerActions {
    pub bits: u16,
    pub rotation: Rotation,
    pub item_change: Option<Item>,
}

impl PlayerActions {
    pub fn as_bytes(self) -> Vec<u8> {
        self.bits
            .to_le_bytes()
            .iter()
            .chain(self.rotation.yaw.to_le_bytes().iter())
            .chain(self.rotation.pitch.to_le_bytes().iter())
            .chain(
                [if let Some(item) = self.item_change {
                    item as u8
                } else {
                    0
                }]
                .iter(),
            )
            .copied()
            .collect()
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
    pub const HAND_ACTION_MASK: u16 =
        Self::ATTACK + Self::USE_ITEM + Self::PLACE_BLOCK + Self::DIG_BLOCK;

    pub fn is_set(&self, flag: u16) -> bool {
        self.bits & flag != 0
    }

    pub fn set(&mut self, flag: u16) {
        self.bits |= flag;
    }

    pub fn checked_set(&mut self, flag: u16) {
        if flag & Self::HAND_ACTION_MASK != 0 && self.bits & Self::HAND_ACTION_MASK != 0 {
            return;
        }
        self.set(flag);
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
