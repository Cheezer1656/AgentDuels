use agentduels_protocol::Item;
use bevy::prelude::*;
use std::collections::HashMap;

pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_EYE_HEIGHT: f32 = 1.6;
pub const PLAYER_WIDTH: f32 = 0.6;
pub const PLAYER_SPEED: f32 = 10.0;
pub const PLAYER_JUMP_SPEED: f32 = 2.0;
pub const PLAYER_INTERACT_RANGE: f32 = 5.0;
pub const SPAWN_POSITIONS: [Vec3; 2] = [Vec3::new(21.5, 1.0 + PLAYER_HEIGHT / 2.0, 0.5), Vec3::new(-21.5, 1.0 + PLAYER_HEIGHT / 2.0, 0.5)];
pub const SPAWN_ROTATIONS: [f32; 2] = [std::f32::consts::PI, 0.0];

/// ID 0 = self, ID 1 = opponent
#[derive(Component, Default)]
pub struct PlayerID(pub u16);

#[derive(Component)]
pub struct PlayerBody;

#[derive(Component)]
pub struct PlayerHead;

#[derive(Component)]
pub struct Health(pub f32);

impl Default for Health {
    fn default() -> Self {
        Health(20.0)
    }
}

/// Hurt cooldown in ticks (player can't be hurt again until this reaches 0)
#[derive(Component, Default)]
pub struct HurtCooldown(pub u8);

#[derive(Component)]
pub struct Inventory {
    contents: HashMap<Item, u16>,
    selected: Item,
    changed: bool,
}

impl Inventory {
    pub fn get_count(&self, item: Item) -> u16 {
        *self.contents.get(&item).unwrap_or(&0)
    }

    pub fn add_item(&mut self, item: Item, amount: u16) {
        *self.contents.entry(item).or_insert(0) += amount;
        self.changed = true;
    }

    pub fn remove_item(&mut self, item: Item, amount: u16) {
        *self.contents.entry(item).or_insert(0) =
            (self.contents.get(&item).unwrap_or(&0) - amount).max(0);
    }

    pub fn select_item(&mut self, item: Item) {
        self.selected = item;
        self.changed = true;
    }

    pub fn get_selected_item(&self) -> Item {
        self.selected
    }

    pub fn has_changed(&self) -> bool {
        self.changed
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
            changed: false,
        }
    }
}

#[derive(Component, Default)]
pub struct Score(pub u16);

#[derive(Bundle, Default)]
pub struct PlayerBundle {
    pub id: PlayerID,
    pub health: Health,
    pub hurt_cooldown: HurtCooldown,
    pub inventory: Inventory,
    pub score: Score,
    pub transform: Transform,
}