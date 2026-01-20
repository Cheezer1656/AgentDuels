use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::PlayerInfo;

pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_EYE_HEIGHT: f32 = 1.75;
pub const PLAYER_WIDTH: f32 = 0.6;
pub const PLAYER_SPEED: f32 = 3.0;
pub const PLAYER_JUMP_SPEED: f32 = 10.0;
pub const PLAYER_INTERACT_RANGE: f32 = 3.0;
pub struct PlayerAnimationIndices {
    pub root: u32,
    pub idle: u32,
    pub walk: u32,
    pub swing: u32,
    pub draw_bow: u32,
    pub eat: u32,
}
pub const PLAYER_ANIMATION_INDICES: PlayerAnimationIndices = PlayerAnimationIndices {
    root: 0,
    idle: 1,
    walk: 2,
    swing: 3,
    draw_bow: 4,
    eat: 5,
};
pub const SPAWN_POSITIONS: [Vec3; 2] = [
    Vec3::new(21.5, 1.0 + PLAYER_HEIGHT / 2.0, 0.5),
    Vec3::new(-20.5, 1.0 + PLAYER_HEIGHT / 2.0, 0.5),
];
pub const SPAWN_ROTATIONS: [f32; 2] = [std::f32::consts::PI, 0.0];

/// ID 0 = self, ID 1 = opponent
#[derive(
    Component, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd,
)]
pub struct PlayerID(pub u16);

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
            Item::Bow => 40,
            Item::Arrow => 0,
            Item::Block => 0,
            Item::GoldenApple => 20,
        }
    }
    pub fn damage(&self) -> f32 {
        match self {
            Item::Sword => 4.0,
            Item::Pickaxe => 2.0,
            Item::Bow => 0.5,
            Item::Arrow => 0.5,
            Item::Block => 0.5,
            Item::GoldenApple => 0.5,
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

#[derive(Component, Default, Clone, Copy)]
pub struct PlayerActionsTracker(pub PlayerActions);

#[derive(Component)]
pub struct PlayerBody;

#[derive(Component)]
pub struct PlayerHead;

#[derive(Component)]
pub struct PlayerHand;

#[derive(Component, Clone, Copy)]
pub struct Health(pub f32);

impl Default for Health {
    fn default() -> Self {
        Health(20.0)
    }
}

/// Hurt cooldown in ticks (player can't be hurt again until this reaches 0)
#[derive(Component, Default)]
pub struct HurtCooldown(pub u8);

impl HurtCooldown {
    pub fn start(&mut self) {
        self.0 = 10;
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct Inventory {
    contents: HashMap<Item, u16>,
    selected: Item,
}

impl Inventory {
    pub fn get_count(&self, item: Item) -> u16 {
        *self.contents.get(&item).unwrap_or(&0)
    }

    pub fn remove_item(&mut self, item: Item, amount: u16) {
        *self.contents.entry(item).or_insert(0) = self
            .contents
            .get(&item)
            .unwrap_or(&0)
            .saturating_sub(amount);
    }

    pub fn select_item(&mut self, item: Item) {
        self.selected = item;
    }

    pub fn get_selected_item(&self) -> Item {
        self.selected
    }
}

impl Default for Inventory {
    fn default() -> Self {
        let mut contents = HashMap::new();

        contents.insert(Item::Sword, 1);
        contents.insert(Item::Pickaxe, 1);
        contents.insert(Item::Bow, 1);
        contents.insert(Item::Arrow, 1);
        contents.insert(Item::Block, 128);
        contents.insert(Item::GoldenApple, 8);

        Inventory {
            contents,
            selected: Item::Sword,
        }
    }
}

#[derive(Component, Default)]
pub struct Score(pub u16);

pub struct BreakingStatus {
    pub block_pos: IVec3,
    pub ticks_left: usize,
}

/// Tracker for the player's block breaking status
#[derive(Component, Default)]
pub struct BreakingStatusTracker(pub Option<BreakingStatus>);

pub struct ItemUsageStatus {
    pub item: Item,
    pub ticks_left: usize,
}

impl ItemUsageStatus {
    pub fn new(item: Item) -> Self {
        Self {
            item,
            ticks_left: item.ticks_needed(),
        }
    }
}

/// Tracker for the player's item usage status
#[derive(Component, Default)]
pub struct ItemUsageStatusTracker(pub Option<ItemUsageStatus>);

#[derive(Component, Serialize, Deserialize, Deref, Default, Debug, Clone, Copy)]
pub struct HeadRotation(pub Quat);

#[derive(Component, Serialize, Deserialize, Default, Debug, Clone, Copy)]
pub enum PlayerAnimation {
    #[default]
    None,
    Swing,
    DrawBow,
    Eat,
}

#[derive(Bundle, Default)]
pub struct PlayerBundle {
    pub id: PlayerID,
    pub actions: PlayerActionsTracker,
    pub health: Health,
    pub hurt_cooldown: HurtCooldown,
    pub inventory: Inventory,
    pub score: Score,
    pub breaking_status: BreakingStatusTracker,
    pub item_usage_status: ItemUsageStatusTracker,
    pub transform: Transform,
    pub head_rotation: HeadRotation,
    pub animation: PlayerAnimation,
}
