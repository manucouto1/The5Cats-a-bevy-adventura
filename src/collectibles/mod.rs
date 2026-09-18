pub mod assets;
pub mod components;
pub mod systems;

use bevy::prelude::*;

use crate::{
    collectibles::{assets::load_collectible_assets, components::SpawnCollectibleEvent},
    enemies::components::EnemyKilledEvent,
    game_state::{GameState, LevelState},
};
use systems::{
    despawn_collectibles, drop_on_enemy_death, magnetic_system, maniac_buff_expiration_system,
    pickup_system, spawn_collectibles, spawn_pop_system,
};

pub struct CollectiblesPlugin;

impl Plugin for CollectiblesPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<EnemyKilledEvent>()
            .add_event::<SpawnCollectibleEvent>()
            .add_systems(OnEnter(LevelState::Loading), load_collectible_assets)
            .add_systems(
                Update,
                (
                    drop_on_enemy_death,
                    spawn_collectibles,
                    spawn_pop_system,
                    magnetic_system,
                    pickup_system,
                    maniac_buff_expiration_system,
                )
                    .chain()
                    .run_if(in_state(GameState::Game))
                    .run_if(in_state(LevelState::LevelLoaded)),
            )
            .add_systems(OnExit(LevelState::LevelLoaded), despawn_collectibles);
    }
}
