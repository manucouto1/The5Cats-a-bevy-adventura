use bevy::prelude::*;

/// Emitted once when the player touches an `EndLevel` tile. Consumers drive
/// the game-state transition (next level or victory screen); this event just
/// announces the trigger.
#[derive(Event, Debug)]
pub struct LevelCompleteEvent;

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum GameState {
    #[default]
    MainMenu,
    Game,
    PauseMenu,
    GameOver,
    Victory,
}

/// Whether the current run is the full campaign (levels chain into each
/// other) or a single level picked from the "Levels" screen (which jumps
/// straight to the victory screen when it ends). Mirrors the pygame scene
/// stack: `execute_game` queued all four levels, `execute_level` only one.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayMode {
    #[default]
    Campaign,
    SingleLevel,
}

/// Tallies shown on the victory screen. Persist across levels within a
/// run; reset whenever a new run starts from the menu or after game over.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PlayerStats {
    pub kitty_points: u32,
    pub hearts: u32,
    pub cookies: u32,
}

impl PlayerStats {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum LevelState {
    #[default]
    Pre,
    Loading,
    LevelLoaded,
}
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Default)]
pub enum Level {
    #[default]
    Level1,
    Level2,
    Level3,
    Level4,
}

impl Level {
    pub fn number(&self) -> u8 {
        match self {
            Level::Level1 => 1,
            Level::Level2 => 2,
            Level::Level3 => 3,
            Level::Level4 => 4,
        }
    }

    pub fn from_number(n: u8) -> Level {
        match n {
            2 => Level::Level2,
            3 => Level::Level3,
            4 => Level::Level4,
            _ => Level::Level1,
        }
    }

    pub fn is_final(&self) -> bool {
        matches!(self, Level::Level4)
    }
}

/// Current level the player is on. Updated when a level completes.
/// Asset loaders read this to resolve their input paths.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct CurrentLevel(pub Level);

/// Reads one of the level's JSON files.
///
/// Native builds read it off the disk, which is what keeps the levels
/// editable without a recompile. The browser has no filesystem, so there
/// the same files are compiled into the binary — a few hundred KB of JSON
/// against a wasm module measured in megabytes.
pub fn read_level_file(path: &str) -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read_to_string(path).ok()
    }
    #[cfg(target_arch = "wasm32")]
    {
        // Paths are built from the asset root, which is "assets" here.
        let tail = path.strip_prefix("assets/").unwrap_or(path);
        embedded_level_file(tail).map(str::to_owned)
    }
}

#[cfg(target_arch = "wasm32")]
macro_rules! embedded_levels {
    ($($tail:literal),* $(,)?) => {
        fn embedded_level_file(tail: &str) -> Option<&'static str> {
            match tail {
                $($tail => Some(include_str!(concat!("../assets/", $tail))),)*
                _ => None,
            }
        }
    };
}

#[cfg(target_arch = "wasm32")]
embedded_levels!(
    "levels/level1/level1.json",
    "levels/level1/level1_active_object.json",
    "levels/level1/level1_events.json",
    "levels/level1/level1_gaps.json",
    "levels/level1/level1_lights.json",
    "levels/level2/level2.json",
    "levels/level2/level2_active_object.json",
    "levels/level2/level2_events.json",
    "levels/level2/level2_gaps.json",
    "levels/level2/level2_lights.json",
    "levels/level3/level3.json",
    "levels/level3/level3_active_object.json",
    "levels/level3/level3_events.json",
    "levels/level3/level3_gaps.json",
    "levels/level3/level3_lights.json",
    "levels/level4/level4.json",
    "levels/level4/level4_active_object.json",
    "levels/level4/level4_events.json",
    "levels/level4/level4_gaps.json",
    "levels/level4/level4_lights.json",
);

pub struct LevelPaths {
    /// Tilemap JSON (`level{N}.json`). Contains tile positions and dimensions;
    /// per-layer behavior (ground / damage / falling / pipe / bouncy / end_level)
    /// is driven by each layer's `path` field.
    pub config: String,
    /// Single JSON that contains both the hero spawn (`hero` field) and
    /// the list of enemies (`enemies` field) for this level.
    pub active_objects: String,
    /// JSON listing level-triggered events (e.g. the `EndLevel` goal tile).
    /// Levels 2-4 put EndLevel here instead of in the tilemap.
    pub events: String,
    /// JSON listing zones that change camera/gravity behavior. Only level 4
    /// defines action-bearing zones (`FallingCamera`); other levels have
    /// empty or Y-range-only data, ignored by the loader.
    pub gaps: String,
    /// JSON describing the level's lighting: ambient/GI settings and the
    /// authored light sources. Missing file = the level renders unlit.
    pub lights: String,
    pub tiles: String,
    pub background: Vec<String>,
}

impl LevelPaths {
    fn from_level_dir(n: u8, bg_count: u8) -> Self {
        // `dir` is read straight off the disk, so it has to be a real path;
        // `rel` goes to the asset server, which is already rooted there.
        let dir = crate::asset_root().join(format!("levels/level{n}")).display().to_string();
        let dir = format!("{dir}/");
        let rel = format!("levels/level{n}/");
        let background = (1..=bg_count)
            .map(|i| format!("{rel}background/{i}.png"))
            .collect();
        Self {
            config: format!("{dir}level{n}.json"),
            active_objects: format!("{dir}level{n}_active_object.json"),
            events: format!("{dir}level{n}_events.json"),
            gaps: format!("{dir}level{n}_gaps.json"),
            lights: format!("{dir}level{n}_lights.json"),
            tiles: format!("{rel}level{n}.png"),
            background,
        }
    }
}

impl Level {
    pub fn get_path(&self) -> LevelPaths {
        match self {
            Level::Level1 => LevelPaths::from_level_dir(1, 6),
            Level::Level2 => LevelPaths::from_level_dir(2, 6),
            Level::Level3 => LevelPaths::from_level_dir(3, 6),
            Level::Level4 => LevelPaths::from_level_dir(4, 5),
        }
    }

    /// Returns the next level in sequence, or `None` after the last level.
    pub fn next(&self) -> Option<Level> {
        match self {
            Level::Level1 => Some(Level::Level2),
            Level::Level2 => Some(Level::Level3),
            Level::Level3 => Some(Level::Level4),
            Level::Level4 => None,
        }
    }
}
