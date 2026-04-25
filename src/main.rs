mod audio;
mod collectibles;
mod cursor;
mod enemies;
mod game_state;
mod map;
mod menu;
mod parallax;
mod physics;
mod player;
use crate::audio::GameAudioPlugin;
use crate::collectibles::CollectiblesPlugin;
use crate::cursor::CursorPlugin;
use crate::enemies::EnemiesPlugin;
// use crate::enemies::EnemiesPlugin;
use crate::physics::{falling_mode_system, gravity_system, kinematic_character_movement_system};
use crate::player::PlayerPlugin;
use crate::{menu::MenuPlugin, parallax::components::MainCamera};
// use crate::player::PlayerPlugin;
use crate::{map::MapPlugin, parallax::systems::camera_follow_system};
use bevy::prelude::*;
use bevy::window::{WindowPlugin, WindowResolution};
use bevy_rapier2d::{
    plugin::{NoUserData, RapierConfiguration, RapierPhysicsPlugin},
    render::RapierDebugRenderPlugin,
};

const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 720.0;

use crate::game_state::{CurrentLevel, GameState, Level, LevelCompleteEvent, LevelState};

/// Advances `CurrentLevel` on each `LevelCompleteEvent`. If a next level
/// exists, triggers `LevelState::Loading` — the LevelLoaded→Loading cycle
/// fires `OnExit(LevelLoaded)` (cleanup) and `OnEnter(LevelLoaded)` (spawn)
/// for the new level, all within `GameState::Game`. After the last level,
/// returns to the main menu.
fn handle_level_complete(
    mut events: EventReader<LevelCompleteEvent>,
    mut current_level: ResMut<CurrentLevel>,
    mut game_state: ResMut<NextState<GameState>>,
    mut level_state: ResMut<NextState<LevelState>>,
) {
    if events.read().next().is_none() {
        return;
    }
    match current_level.0.next() {
        Some(next) => {
            info!("Level complete: {:?} -> {:?}", current_level.0, next);
            current_level.0 = next;
            level_state.set(LevelState::Loading);
        }
        None => {
            info!("Game complete — back to main menu.");
            current_level.0 = Level::Level1;
            game_state.set(GameState::MainMenu);
            // `reset_level_state_on_exit_game` will force LevelState back
            // to Pre on the Game→MainMenu transition.
        }
    }
}

/// Ensures that returning to the main menu drops `LevelState` to `Pre` so
/// `OnExit(LevelLoaded)` fires and cleans up per-level entities. Runs on
/// entering MainMenu (not on exiting Game) so that pausing — Game→PauseMenu
/// — does NOT wipe the level: entities have to linger behind the pause UI.
fn reset_level_state_on_main_menu(mut level_state: ResMut<NextState<LevelState>>) {
    level_state.set(LevelState::Pre);
}

/// Esc toggles between gameplay and the pause menu. All per-frame systems
/// are already gated on `GameState::Game`, so they stop automatically; the
/// physics pipeline is paused separately by `set_physics_pause`.
fn pause_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    current: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    match current.get() {
        GameState::Game => next.set(GameState::PauseMenu),
        GameState::PauseMenu => next.set(GameState::Game),
        _ => {}
    }
}

/// Toggles Rapier's physics pipeline on pause transitions so projectiles,
/// gravity, and collisions freeze when the pause menu is up. Runs on the
/// state transition itself (OnEnter/OnExit PauseMenu).
fn set_physics_pause(active: bool) -> impl Fn(Query<&mut RapierConfiguration>) {
    move |mut config: Query<&mut RapierConfiguration>| {
        for mut cfg in &mut config {
            cfg.physics_pipeline_active = active;
        }
    }
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "the5cats".into(),
                        // Fixed inner size — winit sends min/max size hints
                        // so well-behaved tiling WMs leave the window alone
                        // (in i3, also add `for_window [class="the5cats"]
                        // floating enable` to the config).
                        resolution: WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(32.0))
        // Debug renderer is a heavy per-frame cost (every collider drawn as
        // wireframes). Opt in by setting `RAPIER_DEBUG=1` when launching.
        .add_plugins({
            let mut plugin = RapierDebugRenderPlugin::default();
            plugin.enabled = std::env::var("RAPIER_DEBUG").is_ok();
            plugin
        })
        .init_resource::<CurrentLevel>()
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
        .add_plugins(CollectiblesPlugin)
        .add_plugins(GameAudioPlugin)
        .add_systems(Startup, setup_camera_and_ui)
        .add_systems(
            Update,
            camera_follow_system.run_if(in_state(GameState::Game)),
        )
        .add_systems(Update, gravity_system.run_if(in_state(GameState::Game)))
        .add_systems(
            Update,
            // Runs after gravity so it overrides velocity.y on falling-mode
            // entities for that frame; must run before the kinematic movement
            // system that consumes velocity.
            falling_mode_system
                .after(gravity_system)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            kinematic_character_movement_system
                .after(falling_mode_system)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            handle_level_complete.run_if(in_state(GameState::Game)),
        )
        .add_systems(OnEnter(GameState::MainMenu), reset_level_state_on_main_menu)
        .add_systems(
            Update,
            pause_input_system
                .run_if(in_state(GameState::Game).or(in_state(GameState::PauseMenu))),
        )
        .add_systems(OnEnter(GameState::PauseMenu), set_physics_pause(false))
        .add_systems(OnExit(GameState::PauseMenu), set_physics_pause(true))
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
