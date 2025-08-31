use crate::enemies::components::EnemyCharacter;
use bevy::prelude::*;

#[derive(Bundle)]
pub struct EnemyBundle {
    pub enemy_character: EnemyCharacter,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
}

impl EnemyBundle {
    pub fn new(position: Vec3) -> Self {
        Self {
            enemy_character: EnemyCharacter,
            transform: Transform::from_translation(position),
            global_transform: GlobalTransform::default(),
            visibility: Visibility::default(),
        }
    }
}
