use bevy::prelude::*;

use crate::{game_state::GameState, menu::components::MenuLoadingState};

#[derive(Resource, Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Menu {
    StartMenu,
    PauseMenu,
    GameOverMenu,
}
pub struct MenuPaths {
    pub background: String,
}
impl MenuPaths {
    pub fn new(background: &'static str) -> Self {
        Self {
            background: background.to_string(),
        }
    }
}
impl Menu {
    pub fn get_paths(&self) -> MenuPaths {
        match self {
            Menu::StartMenu | Menu::PauseMenu => MenuPaths::new("menu/menu_background.png"),
            Menu::GameOverMenu => MenuPaths::new("menu/game_over_background.png"),
        }
    }
}

#[derive(Resource, Default)]
pub struct MenuAssets {
    pub background: Handle<Image>,
    pub title_font: Handle<Font>,
    pub text_font: Handle<Font>,
}

pub fn load_menu_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    game_state: Res<State<GameState>>,
    mut loading: ResMut<NextState<MenuLoadingState>>,
) {
    loading.set(MenuLoadingState::Loading);

    let menu_info = match game_state.get() {
        GameState::MainMenu => Menu::StartMenu.get_paths(),
        GameState::PauseMenu => Menu::PauseMenu.get_paths(),
        GameState::GameOver => Menu::GameOverMenu.get_paths(),
        _ => panic!("Invalid game state"),
    };

    let background = asset_server.load(menu_info.background.clone());
    let title_font = asset_server.load("fonts/yukari.ttf");
    let text_font = asset_server.load("fonts/Purisa Bold.ttf");

    commands.insert_resource(MenuAssets {
        background,
        title_font,
        text_font,
    });
    println!("Menu assets loaded");
}
