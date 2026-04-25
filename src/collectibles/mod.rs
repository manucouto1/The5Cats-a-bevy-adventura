pub mod assets;
pub mod components;
pub mod systems;

use bevy::prelude::*;

use crate::{
    collectibles::assets::load_collectible_assets,
    enemies::components::EnemyKilledEvent,
    game_state::{GameState, LevelState},
};
use systems::{
    despawn_collectibles, despawn_score_hud, magnetic_system, maniac_buff_expiration_system,
    pickup_system, spawn_collectible_on_death, spawn_score_hud, update_score_hud,
};

/// Running score (kitty points). HUD will read this in task #13.
#[derive(Resource, Default)]
pub struct Score {
    pub kitty_points: u32,
}

pub struct CollectiblesPlugin;

impl Plugin for CollectiblesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>()
            .add_event::<EnemyKilledEvent>()
            .add_systems(OnEnter(LevelState::Loading), load_collectible_assets)
            .add_systems(OnEnter(LevelState::LevelLoaded), spawn_score_hud)
            .add_systems(
                Update,
                (
                    spawn_collectible_on_death,
                    magnetic_system,
                    pickup_system,
                    maniac_buff_expiration_system,
                    update_score_hud,
                )
                    .run_if(in_state(GameState::Game))
                    .run_if(in_state(LevelState::LevelLoaded)),
            )
            .add_systems(
                OnExit(LevelState::LevelLoaded),
                (despawn_collectibles, despawn_score_hud),
            );
    }
}
