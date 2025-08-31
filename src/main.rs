mod cursor;
mod enemies;
mod game_state;
mod map;
mod menu;
mod parallax;
mod physics;
mod player;
use crate::cursor::CursorPlugin;
use crate::enemies::EnemiesPlugin;
// use crate::enemies::EnemiesPlugin;
use crate::physics::{gravity_system, kinematic_character_movement_system};
use crate::player::PlayerPlugin;
use crate::{menu::MenuPlugin, parallax::components::MainCamera};
// use crate::player::PlayerPlugin;
use crate::{map::MapPlugin, parallax::systems::camera_follow_system};
use bevy::prelude::*;
use bevy_rapier2d::{
    plugin::{NoUserData, RapierPhysicsPlugin},
    render::RapierDebugRenderPlugin,
};

use crate::game_state::{GameState, Level};

#[derive(Resource)]
pub struct CurrentLevel(pub Level);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(32.0))
        .add_plugins(RapierDebugRenderPlugin::default())
        .init_state::<GameState>()
        .add_plugins(MenuPlugin)
        .add_plugins(CursorPlugin)
        // --- Sistemas de Estado ---
        // Menú Principal
        // .add_systems(OnEnter(GameState::MainMenu), systems::spawn_main_menu)
        // .add_systems(Update, systems::menu_input_handling.run_if(in_state(GameState::MainMenu)))
        // .add_systems(OnExit(GameState::MainMenu), systems::despawn_all_entities)
        .add_plugins(MapPlugin)
        .add_plugins(PlayerPlugin)
        .add_plugins(EnemiesPlugin)
        .add_systems(Startup, setup_camera_and_ui)
        .add_systems(
            Update,
            camera_follow_system.run_if(in_state(GameState::Game)),
        )
        .add_systems(Update, gravity_system.run_if(in_state(GameState::Game)))
        .add_systems(
            Update,
            kinematic_character_movement_system.run_if(in_state(GameState::Game)),
        )
        .run();
}

fn setup_camera_and_ui(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera));

    // commands.spawn((
    //     Text::new("Left Arrow: Animate Left Sprite\nRight Arrow: Animate Right Sprite"),
    //     Node {
    //         position_type: PositionType::Absolute,
    //         top: Val::Px(12.0),
    //         left: Val::Px(12.0),
    //         ..default()
    //     },
    // ));
}
