use bevy::prelude::*;

use serde_json;
use std::fs;

use crate::game_state::CurrentLevel;
use crate::map::components::LevelData;

// Recurso para almacenar los handles del atlas y la textura del tilemap, y el tamaño del tile
#[derive(Resource)]
pub struct GameAssets {
    pub tile_texture: Handle<Image>,
    pub parallax_backgrounds: Vec<Handle<Image>>,
    pub tile_size_px: f32,
    pub map_width_tiles: u32,
    pub map_height_tiles: u32,
}

pub fn load_map_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    current_level: Res<CurrentLevel>,
) {
    let paths = current_level.0.get_path();
    let tile_texture_handle = asset_server.load(&paths.tiles);
    let level_data: LevelData = serde_json::from_str(
        &fs::read_to_string(&paths.config).expect("Failed to read level JSON"),
    )
    .expect("Failed to parse level JSON");

    let tile_size_from_json = level_data.tile_size as f32;
    let map_width_from_json = level_data.map_width as u32;
    let map_height_from_json = level_data.map_height as u32;

    let parallax_bg: Vec<Handle<Image>> = paths
        .background
        .iter()
        .map(|x| asset_server.load(x))
        .collect();

    commands.insert_resource(level_data);
    commands.insert_resource(GameAssets {
        tile_texture: tile_texture_handle,
        parallax_backgrounds: parallax_bg,
        tile_size_px: tile_size_from_json,
        map_width_tiles: map_width_from_json,
        map_height_tiles: map_height_from_json,
    });
}
