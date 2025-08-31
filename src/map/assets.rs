use bevy::prelude::*;

use serde_json; //
use std::fs;

use crate::map::components::{CurrentLevelInfo, LevelData}; //
// Recurso para almacenar los handles del atlas y la textura del tilemap, y el tamaño del tile
#[derive(Resource)]
pub struct GameAssets {
    pub tile_texture: Handle<Image>,
    pub parallax_backgrounds: Vec<Handle<Image>>,
    pub tile_size_px: f32, // Para guardar el tamaño del tile
    pub map_width_tiles: u32,
    pub map_height_tiles: u32,
}

// Sistema que carga los assets del juego e inserta los recursos
pub fn load_map_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>, // Usar Res<AssetServer> en lugar de AssetServer
    level_info: Res<CurrentLevelInfo>,
) {
    let tile_texture_handle = asset_server.load(level_info.data.tiles.clone());
    let level_data: LevelData = serde_json::from_str(
        &fs::read_to_string(level_info.data.config.clone()).expect("Failed to read level JSON"),
    )
    .expect("Failed to parse level JSON");

    let tile_size_from_json = level_data.tile_size as f32;
    let map_width_from_json = level_data.map_width as u32;
    let map_height_from_json = level_data.map_height as u32;

    let parallax_bg: Vec<Handle<Image>> = level_info
        .data
        .background
        .iter()
        .map(|x| asset_server.load(x))
        .collect();

    commands.insert_resource(level_data);
    commands.insert_resource(GameAssets {
        tile_texture: tile_texture_handle.clone(),
        parallax_backgrounds: parallax_bg.clone(),
        tile_size_px: tile_size_from_json,
        map_width_tiles: map_width_from_json,
        map_height_tiles: map_height_from_json,
    });
}
