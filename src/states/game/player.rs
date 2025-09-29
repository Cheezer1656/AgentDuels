use bevy::prelude::*;

#[derive(Component)]
pub struct Player {
    pub id: u16, // ID 0 = self, ID 1 = opponent
}

impl Player {
    pub fn new(id: u16) -> Self {
        Self { id }
    }
}
