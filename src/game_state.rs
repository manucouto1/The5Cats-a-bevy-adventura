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

/// Current level the player is on. Updated when a level completes.
/// Asset loaders read this to resolve their input paths.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct CurrentLevel(pub Level);

pub struct LevelPaths {
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
    pub tiles: String,
    pub background: Vec<String>,
}

impl LevelPaths {
    fn from_level_dir(n: u8, bg_count: u8) -> Self {
        let dir = format!("assets/levels/level{n}/");
        let rel = format!("levels/level{n}/");
        let background = (1..=bg_count)
            .map(|i| format!("{rel}background/{i}.png"))
            .collect();
        Self {
            config: format!("{dir}level{n}.json"),
            active_objects: format!("{dir}level{n}_active_object.json"),
            events: format!("{dir}level{n}_events.json"),
            gaps: format!("{dir}level{n}_gaps.json"),
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
