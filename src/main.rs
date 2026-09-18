// A release build on Windows is a GUI app: without this it is linked as a
// console program and opens a terminal window behind the game.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod collectibles;
mod cursor;
mod dev;
mod enemies;
mod game_state;
mod hud;
mod lighting;
mod map;
mod menu;
mod parallax;
mod physics;
mod player;

use crate::audio::GameAudioPlugin;
use crate::collectibles::CollectiblesPlugin;
use crate::cursor::CursorPlugin;
use crate::enemies::EnemiesPlugin;
use crate::game_state::{
    CurrentLevel, GameState, Level, LevelCompleteEvent, LevelState, PlayMode, PlayerStats,
};
use crate::hud::HudPlugin;
use crate::lighting::LightingPlugin;
use crate::map::MapPlugin;
use crate::menu::MenuPlugin;
use crate::parallax::components::MainCamera;
use crate::parallax::systems::camera_follow_system;
use crate::physics::{falling_mode_system, gravity_system, kinematic_character_movement_system};
use crate::player::PlayerPlugin;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode, WindowPlugin, WindowResolution};
use bevy_rapier2d::{
    pipeline::CollisionEvent,
    plugin::{NoUserData, RapierConfiguration, RapierPhysicsPlugin},
    render::RapierDebugRenderPlugin,
};

/// Where the game's `assets/` folder lives.
///
/// Running from cargo it is simply the working directory, but a packaged
/// build is launched from wherever the user double-clicked it, so the
/// folder is looked for next to the executable first (a zip) and then in a
/// macOS bundle's `Contents/Resources` (a `.app`).
pub fn asset_root() -> std::path::PathBuf {
    // In the browser this is a URL path the asset server fetches from, and
    // there is no executable to be next to.
    if cfg!(target_arch = "wasm32") {
        return std::path::PathBuf::from("assets");
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let beside = dir.join("assets");
            if beside.is_dir() {
                return beside;
            }
            let bundled = dir.join("../Resources/assets");
            if bundled.is_dir() {
                return bundled;
            }
        }
    }
    std::path::PathBuf::from("assets")
}

const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 720.0;

/// Camera zoom factor: world units visible per screen pixel at the default
/// window size. <1 zooms in (more pixel detail). The original pygame build
/// used a tight 800x800 viewport so enemies engaged the player at
/// meaningful distances; at 1.0 the player could snipe enemies from outside
/// their detection range.
pub const CAMERA_SCALE: f32 = 0.85;

/// World-space height the camera always shows. The width follows the
/// window's aspect ratio, so resizing or going fullscreen never changes the
/// vertical framing (and never reveals more of the map above/below).
pub const VIEW_HEIGHT: f32 = WINDOW_HEIGHT * CAMERA_SCALE;

/// Advances the level flow on each `LevelCompleteEvent`. In campaign mode
/// the next level loads (the LevelLoaded→Loading cycle fires
/// `OnExit(LevelLoaded)` cleanup and `OnEnter(LevelLoaded)` spawn); after
/// the final level — or after any level in single-level mode — the victory
/// screen takes over.
fn handle_level_complete(
    mut events: EventReader<LevelCompleteEvent>,
    mut current_level: ResMut<CurrentLevel>,
    play_mode: Res<PlayMode>,
    mut game_state: ResMut<NextState<GameState>>,
    mut level_state: ResMut<NextState<LevelState>>,
) {
    if events.read().next().is_none() {
        return;
    }
    let next = match *play_mode {
        PlayMode::Campaign => current_level.0.next(),
        PlayMode::SingleLevel => None,
    };
    match next {
        Some(next) => {
            info!("Level complete: {:?} -> {:?}", current_level.0, next);
            current_level.0 = next;
            level_state.set(LevelState::Loading);
        }
        None => {
            info!("Run complete — victory screen.");
            game_state.set(GameState::Victory);
        }
    }
}

/// Ensures that returning to the main menu (or reaching the victory
/// screen) drops `LevelState` to `Pre` so `OnExit(LevelLoaded)` fires and
/// cleans up per-level entities. Not run on pause/game over, where the
/// level has to linger behind the overlay.
fn reset_level_state(mut level_state: ResMut<NextState<LevelState>>) {
    level_state.set(LevelState::Pre);
}

/// Leaving the victory screen puts the campaign back at level 1.
fn reset_run_after_victory(
    mut current_level: ResMut<CurrentLevel>,
    mut stats: ResMut<PlayerStats>,
) {
    current_level.0 = Level::Level1;
    stats.reset();
}

/// Esc during gameplay opens the pause menu. Leaving the pause menu is
/// handled by the menu module (Resume button / Esc on the pause page).
fn pause_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<GameState>>,
    hud_pause: EventReader<hud::PauseRequested>,
) {
    if keys.just_pressed(KeyCode::Escape) || !hud_pause.is_empty() {
        next.set(GameState::PauseMenu);
    }
}

/// Toggles Rapier's physics pipeline so projectiles, gravity, and
/// collisions freeze while a menu covers the level.
fn set_physics_pause(active: bool) -> impl Fn(Query<&mut RapierConfiguration>) {
    move |mut config: Query<&mut RapierConfiguration>| {
        for mut cfg in &mut config {
            cfg.physics_pipeline_active = active;
        }
    }
}

/// bevy_rapier inserts `Events<CollisionEvent>` with `insert_resource`
/// rather than `add_event`, so the type is never registered and nothing ever
/// swaps its double buffer. The queue then only grows, and any reader that
/// skips a frame — the game sitting in the pause menu, `enemy_damage_system`
/// standing down while the player is invincible — receives every collision
/// it missed the moment it reads again. (`App::add_event` cannot fix this:
/// it does nothing when the resource already exists, which it does by the
/// time our plugins run.) Swapping the buffer here makes collisions expire
/// after one frame like any other Bevy event.
fn expire_collision_events(events: Option<ResMut<Events<CollisionEvent>>>) {
    if let Some(mut events) = events {
        events.update();
    }
}

fn fullscreen_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !keys.just_pressed(KeyCode::F11) {
        return;
    }
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    window.mode = match window.mode {
        WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        _ => WindowMode::Windowed,
    };
}

fn main() {
    let default_plugins = DefaultPlugins
        .build()
        // Same folder the level JSONs are read from, so a packaged build
        // finds its assets wherever it was unzipped, and the browser build
        // fetches them from `assets/` next to the page.
        .set(AssetPlugin {
            file_path: asset_root().to_string_lossy().into_owned(),
            ..default()
        })
        .set(ImagePlugin::default_nearest())
        .set(WindowPlugin {
            primary_window: Some(Window {
                title: "5Gatos".into(),
                resolution: WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT),
                resizable: true,
                // The browser build takes over this canvas; native builds
                // ignore it.
                canvas: Some("#game".into()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        });

    // Quitting from the menu hung the process about three runs in five:
    // `RenderAppChannels`, the resource pipelined rendering keeps, blocks in
    // its `Drop` waiting for the render sub-app to come back from the render
    // thread, and on shutdown that hand-off may never arrive — the app then
    // sits in `World::clear_all` forever with its window still up. Recording
    // render commands on the main thread costs a fraction of a millisecond
    // here and cannot deadlock. The plugin only exists where there are
    // threads to pipeline across, so the browser build never sees it.
    #[cfg(not(target_arch = "wasm32"))]
    let default_plugins =
        default_plugins.disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>();

    App::new()
        .add_plugins(default_plugins)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(32.0))
        .add_systems(First, expire_collision_events)
        // Debug renderer is a heavy per-frame cost (every collider drawn as
        // wireframes). Opt in by setting `RAPIER_DEBUG=1` when launching.
        .add_plugins({
            let mut plugin = RapierDebugRenderPlugin::default();
            plugin.enabled = std::env::var("RAPIER_DEBUG").is_ok();
            plugin
        })
        .init_resource::<CurrentLevel>()
        .init_resource::<PlayMode>()
        .init_resource::<PlayerStats>()
        .init_state::<GameState>()
        .add_plugins(MenuPlugin)
        .add_plugins(CursorPlugin)
        .add_plugins(MapPlugin)
        .add_plugins(PlayerPlugin)
        .add_plugins(EnemiesPlugin)
        .add_plugins(CollectiblesPlugin)
        .add_plugins(GameAudioPlugin)
        .add_plugins(HudPlugin)
        .add_plugins(LightingPlugin)
        .add_plugins((dev::DevPlugin, dev::DevScriptPlugin))
        .add_systems(Startup, setup_camera)
        .add_systems(
            Update,
            // After the zone trigger so a level's first frame already knows
            // which band/zone framing to snap to.
            camera_follow_system
                .after(crate::map::zones::level_zone_trigger_system)
                .run_if(in_state(GameState::Game)),
        )
        // Strict ordering: player input writes velocity, then gravity adds
        // vertical velocity, falling_mode optionally pins it, and finally
        // kinematic_character_movement_system consumes velocity to push the
        // controller. Without this chain, the scheduler was free to run
        // movement before input on some frames, producing the "stuck while
        // walking continuously" stutter.
        .add_systems(
            Update,
            (
                crate::player::systems::player_input_system,
                gravity_system,
                falling_mode_system,
                kinematic_character_movement_system,
            )
                .chain()
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            handle_level_complete.run_if(in_state(GameState::Game)),
        )
        .add_systems(Update, fullscreen_toggle_system)
        .add_systems(OnEnter(GameState::MainMenu), reset_level_state)
        .add_systems(OnEnter(GameState::Victory), reset_level_state)
        .add_systems(OnExit(GameState::Victory), reset_run_after_victory)
        .add_systems(Update, pause_input_system.run_if(in_state(GameState::Game)))
        .add_systems(OnEnter(GameState::PauseMenu), set_physics_pause(false))
        .add_systems(OnExit(GameState::PauseMenu), set_physics_pause(true))
        .add_systems(OnEnter(GameState::GameOver), set_physics_pause(false))
        .add_systems(OnExit(GameState::GameOver), set_physics_pause(true))
        .run();
}

fn setup_camera(mut commands: Commands) {
    // Msaa::Off — MSAA averages tile edges with the transparent backing,
    // producing the visible seams between adjacent tiles in pixel-art mode.
    commands.spawn((
        Camera2d,
        MainCamera,
        // Explicit, so the off-screen lighting cameras can never be picked
        // as the UI's camera.
        bevy::ui::IsDefaultUiCamera,
        bevy::render::view::Msaa::Off,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: VIEW_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}
