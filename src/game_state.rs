use bevy::prelude::*;

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
#[derive(Resource, Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Level {
    Level1,
}

pub struct LevelPaths {
    pub config: String,
    pub player: String,
    pub tiles: String,
    pub background: Vec<String>,
}

impl LevelPaths {
    pub fn new(
        config: &'static str,
        player: &'static str,
        tiles: &'static str,
        background: Vec<&'static str>,
    ) -> Self {
        Self {
            config: config.to_string(),
            player: player.to_string(),
            tiles: tiles.to_string(),
            background: background.iter().map(|&s| s.to_string()).collect(),
        }
    }
}

impl Level {
    pub fn get_path(&self) -> LevelPaths {
        match self {
            Level::Level1 => LevelPaths::new(
                "assets/levels/level1/level1.json",
                "assets/levels/level1/level1_hero.json",
                "levels/level1/level1.png",
                vec![
                    "levels/level1/background/1.png",
                    "levels/level1/background/2.png",
                    "levels/level1/background/3.png",
                    "levels/level1/background/4.png",
                    "levels/level1/background/5.png",
                    "levels/level1/background/6.png",
                ],
            ),
        }
    }
}
