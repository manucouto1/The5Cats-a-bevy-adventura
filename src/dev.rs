//! Developer conveniences. None of this affects normal play:
//!
//! * `F12` saves a PNG screenshot of the window into `snapshots/`.
//! * `THE5CATS_LEVEL=<1-4>` starts the game on that level (skips the menu
//!   when combined with `THE5CATS_AUTOPLAY=1`).
//! * `THE5CATS_AUTOPLAY=1` presses "Play" automatically once the menu is
//!   ready.
//! * `THE5CATS_SHOT_AFTER=<secs>` takes a screenshot after that many
//!   seconds and then quits the app. Useful for headless visual checks.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::game_state::{CurrentLevel, GameState, Level, LevelState};

pub struct DevPlugin;

#[derive(Resource)]
struct AutoShot {
    timer: Timer,
    fired: bool,
    /// Frames to wait after the screenshot is requested before exiting so
    /// the render thread has time to write the file.
    exit_countdown: u32,
}

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, apply_env_level)
            .add_systems(Update, (screenshot_on_f12, autoplay, autoshot));
        if let Some(secs) = std::env::var("THE5CATS_SHOT_AFTER")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            app.insert_resource(AutoShot {
                timer: Timer::from_seconds(secs, TimerMode::Once),
                fired: false,
                exit_countdown: 30,
            });
        }
    }
}

fn apply_env_level(mut current: ResMut<CurrentLevel>) {
    let Ok(v) = std::env::var("THE5CATS_LEVEL") else {
        return;
    };
    current.0 = match v.trim() {
        "2" => Level::Level2,
        "3" => Level::Level3,
        "4" => Level::Level4,
        _ => Level::Level1,
    };
    info!("Dev: starting on {:?}", current.0);
}

fn snapshot_path(prefix: &str) -> std::path::PathBuf {
    let _ = std::fs::create_dir_all("snapshots");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    std::path::PathBuf::from(format!("snapshots/{prefix}_{stamp}.png"))
}

fn screenshot_on_f12(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::F12) {
        let path = snapshot_path("shot");
        info!("Saving screenshot to {}", path.display());
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

fn autoplay(
    game_state: Res<State<GameState>>,
    mut next_game: ResMut<NextState<GameState>>,
    mut next_level: ResMut<NextState<LevelState>>,
    mut done: Local<bool>,
) {
    if *done || std::env::var("THE5CATS_AUTOPLAY").is_err() {
        return;
    }
    if *game_state.get() == GameState::MainMenu {
        *done = true;
        next_game.set(GameState::Game);
        next_level.set(LevelState::Loading);
    }
}

fn autoshot(
    mut commands: Commands,
    time: Res<Time>,
    shot: Option<ResMut<AutoShot>>,
    mut exit: EventWriter<AppExit>,
) {
    let Some(mut shot) = shot else { return };
    if !shot.fired {
        shot.timer.tick(time.delta());
        if shot.timer.finished() {
            shot.fired = true;
            let path = std::env::var("THE5CATS_SHOT_PATH")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| snapshot_path("auto"));
            info!("Dev: auto screenshot to {}", path.display());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
    } else if shot.exit_countdown > 0 {
        shot.exit_countdown -= 1;
    } else {
        exit.write(AppExit::Success);
    }
}

// ---------------------------------------------------------------------------
// Scripted input: `THE5CATS_SCRIPT="wait 1; hold KeyD 2; tap Escape; shot pause"`
// Commands (separated by `;`):
//   wait <secs>            idle
//   hold <KeyCode> <secs>  hold a key down for a while
//   tap <KeyCode>          press + release next frame
//   click                  press the left mouse button for one frame
//   press <n>              set the nth menu button to Pressed (no mouse needed)
//   mouse <x> <y>          place the pointer at those logical window coords
//   shot <name>            save snapshots/<name>.png
//   tp <tile_x> <tile_y>   teleport the player to those tile coordinates
//   kill                   set the player's HP to 0
//   goal                   fire LevelCompleteEvent
//   hat                    drop the end-game hat on the player (real win path)
//   boss <phase>           put the boss into that phase (1-3) with 1 HP
//   quit                   exit the app
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct DevScript {
    steps: Vec<String>,
    index: usize,
    /// Time left on the current wait/hold step.
    remaining: f32,
    holding: Option<KeyCode>,
    /// Frames left holding the left mouse button. bevy_ui samples the mouse
    /// in its own PreUpdate set, which may run before this system, so a
    /// one-frame press can be missed entirely.
    holding_click: u32,
    started: bool,
    /// Frames to keep running after a `quit` so pending screenshots flush.
    quit_countdown: Option<u32>,
}

pub struct DevScriptPlugin;

impl Plugin for DevScriptPlugin {
    fn build(&self, app: &mut App) {
        if let Ok(script) = std::env::var("THE5CATS_SCRIPT") {
            app.insert_resource(DevScript {
                steps: script
                    .split(';')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                index: 0,
                remaining: 0.0,
                holding: None,
                holding_click: 0,
                started: false,
                quit_countdown: None,
            })
            .add_systems(
                PreUpdate,
                run_dev_script
                    .after(bevy::input::InputSystem)
                    // Before bevy_ui samples the mouse: it reacts to
                    // `just_pressed`, so a synthetic click injected after it
                    // would never reach a menu button.
                    .before(bevy::ui::UiSystem::Focus),
            );
        }
    }
}

fn parse_key(name: &str) -> Option<KeyCode> {
    Some(match name {
        "KeyA" | "A" => KeyCode::KeyA,
        "KeyD" | "D" => KeyCode::KeyD,
        "KeyW" | "W" => KeyCode::KeyW,
        "KeyS" | "S" => KeyCode::KeyS,
        "Space" => KeyCode::Space,
        "Escape" | "Esc" => KeyCode::Escape,
        "Left" => KeyCode::ArrowLeft,
        "Right" => KeyCode::ArrowRight,
        "Up" => KeyCode::ArrowUp,
        "Down" => KeyCode::ArrowDown,
        "F11" => KeyCode::F11,
        _ => return None,
    })
}

/// Everything the cheat commands poke at, bundled so the script system
/// stays under Bevy's system-parameter limit.
#[derive(bevy::ecs::system::SystemParam)]
struct DevWorld<'w, 's> {
    player: Query<
        'w,
        's,
        (
            &'static mut crate::player::components::Health,
            &'static mut Transform,
        ),
        (
            With<crate::player::components::PlayerCharacter>,
            Without<crate::enemies::components::EnemyCharacter>,
            Without<crate::parallax::components::MainCamera>,
        ),
    >,
    camera: Query<
        'w,
        's,
        &'static Transform,
        (
            With<crate::parallax::components::MainCamera>,
            Without<crate::player::components::PlayerCharacter>,
            Without<crate::enemies::components::EnemyCharacter>,
        ),
    >,
    map: Option<Res<'w, crate::map::assets::GameAssets>>,
    wool: Query<'w, 's, Entity, With<crate::cursor::components::WoolBall>>,
    collectibles: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static crate::collectibles::components::Collectible,
        ),
        (
            Without<crate::player::components::PlayerCharacter>,
            Without<crate::enemies::components::EnemyCharacter>,
            Without<crate::parallax::components::MainCamera>,
        ),
    >,
    menu_buttons: Query<
        'w,
        's,
        (
            &'static mut Interaction,
            &'static crate::menu::components::MenuButtonAction,
        ),
    >,
    bosses: Query<
        'w,
        's,
        (
            Entity,
            &'static mut Transform,
            &'static mut crate::player::components::Health,
            &'static mut crate::enemies::components::FinalBoss,
        ),
        (
            With<crate::enemies::components::EnemyCharacter>,
            Without<crate::player::components::PlayerCharacter>,
        ),
    >,
    enemies: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static crate::enemies::components::EnemyType,
        ),
        (
            With<crate::enemies::components::EnemyCharacter>,
            With<crate::enemies::components::Active>,
            Without<crate::enemies::components::FinalBoss>,
            Without<crate::player::components::PlayerCharacter>,
        ),
    >,
    level_complete: EventWriter<'w, crate::game_state::LevelCompleteEvent>,
    killed: EventWriter<'w, crate::enemies::components::EnemyKilledEvent>,
    spawn_collectible: EventWriter<'w, crate::collectibles::components::SpawnCollectibleEvent>,
    zones: Option<Res<'w, crate::map::zones::LevelZones>>,
    armed: ResMut<'w, crate::map::zones::BossFallArmed>,
    level_mode: Res<'w, crate::map::zones::LevelMode>,
    menu_page: ResMut<'w, crate::menu::MenuPage>,
    player_entity: Query<'w, 's, Entity, With<crate::player::components::PlayerCharacter>>,
    all_enemies: Query<
        'w,
        's,
        (
            &'static crate::enemies::components::EnemyType,
            &'static Transform,
            Has<crate::enemies::components::Active>,
            Has<crate::physics::FallingMode>,
        ),
        (
            With<crate::enemies::components::EnemyCharacter>,
            Without<crate::player::components::PlayerCharacter>,
            Without<crate::enemies::components::FinalBoss>,
        ),
    >,
}

fn run_dev_script(
    mut commands: Commands,
    time: Res<Time>,
    mut script: ResMut<DevScript>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    game_state: Res<State<GameState>>,
    level_state: Res<State<LevelState>>,
    mut exit: EventWriter<AppExit>,
    mut w: DevWorld,
) {
    if let Some(n) = script.quit_countdown.as_mut() {
        if *n == 0 {
            exit.write(AppExit::Success);
        } else {
            *n -= 1;
        }
        return;
    }
    // Scripts start once gameplay is actually running (or immediately when
    // the script is meant for menus, i.e. no autoplay).
    if !script.started {
        let in_game =
            *game_state.get() == GameState::Game && *level_state.get() == LevelState::LevelLoaded;
        if std::env::var("THE5CATS_AUTOPLAY").is_ok() && !in_game {
            return;
        }
        script.started = true;
    }

    // Mouse bookkeeping first: the wait/hold branches below return early,
    // and leaving the button held through a `wait` meant the next `click`
    // pressed an already-pressed button, which never counts as
    // `just_pressed` and so never reached a menu button.
    if script.holding_click > 0 {
        script.holding_click -= 1;
        if script.holding_click == 0 {
            mouse.release(MouseButton::Left);
        }
    }

    // Release a key held last frame once its time is up.
    if let Some(key) = script.holding {
        script.remaining -= time.delta_secs();
        if script.remaining > 0.0 {
            return;
        }
        keys.release(key);
        script.holding = None;
    } else if script.remaining > 0.0 {
        script.remaining -= time.delta_secs();
        return;
    }
    // Clear any tap or click from the previous frame.
    for key in keys.get_pressed().copied().collect::<Vec<_>>() {
        if script.holding != Some(key) {
            keys.release(key);
        }
    }
    if script.holding_click == 0 {
        for button in mouse.get_pressed().copied().collect::<Vec<_>>() {
            mouse.release(button);
        }
    }

    let Some(step) = script.steps.get(script.index).cloned() else {
        return;
    };
    script.index += 1;
    let parts: Vec<&str> = step.split_whitespace().collect();
    info!("Dev script: {step}");
    match parts.as_slice() {
        ["wait", secs] => script.remaining = secs.parse().unwrap_or(1.0),
        ["hold", key, secs] => {
            if let Some(k) = parse_key(key) {
                keys.press(k);
                script.holding = Some(k);
                script.remaining = secs.parse().unwrap_or(1.0);
            }
        }
        ["tap", key] => {
            if let Some(k) = parse_key(key) {
                keys.press(k);
            }
        }
        ["click"] => {
            mouse.press(MouseButton::Left);
            script.holding_click = 6;
        }
        // Drives the same path a real pointer does: `cursor_position` is
        // what bevy_ui hit-tests menu buttons with.
        ["mouse", x, y] => {
            if let (Ok(x), Ok(y), Ok(mut window)) =
                (x.parse::<f32>(), y.parse::<f32>(), windows.single_mut())
            {
                window.set_cursor_position(Some(Vec2::new(x, y)));
            }
        }
        // Menu buttons are driven by bevy_ui from the cursor position, which
        // a headless script has no way to move — poke the Interaction the
        // focus system would have set instead.
        ["press", n] => {
            let index: usize = n.parse().unwrap_or(0);
            let mut actions: Vec<String> = Vec::new();
            for (i, (mut interaction, action)) in w.menu_buttons.iter_mut().enumerate() {
                actions.push(format!("{i}:{action:?}"));
                if i == index {
                    *interaction = Interaction::Pressed;
                }
            }
            info!(
                "Dev: menu buttons [{}], pressing {index}",
                actions.join(", ")
            );
        }
        ["shot", name] => {
            let _ = std::fs::create_dir_all("snapshots");
            let path = std::path::PathBuf::from(format!("snapshots/{name}.png"));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        ["kill"] => {
            for (mut hp, _) in &mut w.player {
                hp.current = 0;
            }
        }
        ["god"] => {
            for entity in &w.player_entity {
                commands
                    .entity(entity)
                    .insert(crate::player::components::Invincibility::new(1.0e6));
            }
        }
        // The end-game hat has no magnet, so a scripted run can never walk
        // into it; drop it where the player already stands.
        ["hat"] => {
            for (_, tf) in &w.player {
                w.spawn_collectible
                    .write(crate::collectibles::components::SpawnCollectibleEvent {
                        kind: crate::collectibles::components::Collectible::EndGame,
                        position: tf.translation,
                        pop: Vec2::ZERO,
                    });
            }
        }
        ["goal"] => {
            w.level_complete
                .write(crate::game_state::LevelCompleteEvent);
        }
        ["boss", phase] => {
            let phase: u8 = phase.parse().unwrap_or(1);
            for (_, _, mut hp, mut boss) in &mut w.bosses {
                boss.phase = phase;
                hp.current = 1;
                hp.max = match phase {
                    2 => 36,
                    _ => 18,
                };
            }
        }
        ["bosshit"] => {
            let zones = w.zones.as_deref().cloned();
            for (entity, mut tf, mut hp, mut boss) in &mut w.bosses {
                hp.current = 0;
                boss.has_been_hit = true;
                crate::cursor::systems::on_boss_phase_defeated(
                    &mut commands,
                    entity,
                    &mut tf,
                    &mut hp,
                    &mut boss,
                    zones.as_ref(),
                    &mut w.armed,
                    &mut w.spawn_collectible,
                );
            }
        }
        ["killall"] => {
            for (entity, tf, kind) in &w.enemies {
                w.killed
                    .write(crate::enemies::components::EnemyKilledEvent {
                        position: tf.translation,
                        kind: kind.clone(),
                    });
                commands.entity(entity).despawn();
            }
        }
        // Teleport the player to tile coordinates, the same space the level
        // JSONs use. Handy for reaching a camera band or a boss phase
        // without playing the level up to it.
        ["tp", x, y] => {
            let (Ok(tx), Ok(ty)) = (x.parse::<f32>(), y.parse::<f32>()) else {
                warn!("Dev: tp needs two tile coordinates");
                return;
            };
            let Some(map) = w.map.as_deref() else { return };
            let tile = map.tile_size_px;
            let world = Vec3::new(
                tx * tile - (map.map_width_tiles as f32 * tile / 2.0) + tile / 2.0,
                -ty * tile + (map.map_height_tiles as f32 * tile / 2.0) - tile / 2.0,
                10.0,
            );
            for (_, mut tf) in &mut w.player {
                tf.translation.x = world.x;
                tf.translation.y = world.y;
            }
        }
        ["where"] => {
            for (hp, tf) in &w.player {
                let camera = w
                    .camera
                    .single()
                    .map(|c| (c.translation.x, c.translation.y))
                    .unwrap_or_default();
                info!(
                    "Dev: player at ({:.0}, {:.0}) camera at ({:.0}, {:.0}) mode={:?} hp={} wool={}",
                    tf.translation.x,
                    tf.translation.y,
                    camera.0,
                    camera.1,
                    *w.level_mode,
                    hp.current,
                    w.wool.iter().count(),
                );
                for (_, btf, bhp, boss) in &w.bosses {
                    info!(
                        "Dev:   boss at ({:.0}, {:.0}) phase={} hp={} diving={} hover_y={:.0}",
                        btf.translation.x,
                        btf.translation.y,
                        boss.phase,
                        bhp.current,
                        boss.diving,
                        boss.hover_y
                    );
                }
                for (ctf, kind) in &w.collectibles {
                    info!(
                        "Dev:   collectible {:?} at ({:.0}, {:.0})",
                        kind, ctf.translation.x, ctf.translation.y
                    );
                }
                for (kind, etf, active, falling) in &w.all_enemies {
                    if etf
                        .translation
                        .truncate()
                        .distance(tf.translation.truncate())
                        < 120.0
                    {
                        info!(
                            "Dev:   near {:?} at ({:.0}, {:.0}) active={} falling={}",
                            kind, etf.translation.x, etf.translation.y, active, falling
                        );
                    }
                }
            }
        }
        ["page", name] => {
            *w.menu_page = match *name {
                "controls" => crate::menu::MenuPage::Controls,
                "options" => crate::menu::MenuPage::Options,
                "levels" => crate::menu::MenuPage::Levels,
                _ => crate::menu::MenuPage::Main,
            };
        }
        ["quit"] => script.quit_countdown = Some(20),
        other => warn!("Dev script: unknown step {other:?}"),
    }
}
