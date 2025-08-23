use bevy::prelude::*;

#[derive(Component)]
pub struct Player {
    pub id: u16,
}

impl Player {
    pub fn new(id: u16) -> Self {
        Self { id }
    }
}