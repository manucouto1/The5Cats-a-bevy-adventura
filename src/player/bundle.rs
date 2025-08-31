use crate::{
    physics::AffectedByGravity,
    player::components::{Health, PlayerCharacter},
};
use bevy::prelude::*;

#[derive(Bundle)]
pub struct PlayerBundle {
    pub player_character: PlayerCharacter,
    pub global_transform: GlobalTransform,
    pub health: Health,
    pub visibility: Visibility,
    pub transform: Transform,
    pub gravity: AffectedByGravity,
}

impl PlayerBundle {
    pub fn new(transform: Transform) -> Self {
        Self {
            transform: transform,
            player_character: PlayerCharacter,
            health: Health::default(),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
            gravity: AffectedByGravity,
        }
    }
}
