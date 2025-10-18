use bevy::prelude::*;

/// ID 0 = self, ID 1 = opponent
#[derive(Component)]
pub struct PlayerID(pub u16);

#[derive(Component)]
pub struct PlayerHead;
