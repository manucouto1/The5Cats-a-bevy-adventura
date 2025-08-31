use std::fs;

use bevy::prelude::*;
use serde::Deserialize;

use crate::map::components::CurrentLevelInfo;

// Un Resource para contener las handles de los assets del jugador
#[derive(Resource)]
pub struct PlayerAssets {
    pub hearts: Handle<Image>,
    pub texture_standing: Handle<Image>,
    pub texture_left: Handle<Image>,
    pub texture_right: Handle<Image>,
    // Puedes añadir más assets si los necesitas, como sonidos, otras animaciones, etc.
}

#[derive(Debug, Deserialize, Resource, Clone)] // Añadimos Resource aquí
pub struct HeroData {
    pub x: f32,
    pub y: f32,
}

pub fn load_player_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    level_info: Res<CurrentLevelInfo>,
) {
    let hero_data: HeroData = serde_json::from_str(
        &fs::read_to_string(level_info.data.player.clone()).expect("Failed to read level JSON"),
    )
    .expect("Failed to parse hero JSON");

    commands.insert_resource(PlayerAssets {
        hearts: asset_server.load("player/Corazon-Sheet.png"),
        texture_standing: asset_server
            .load("characters/tofe/standing/Sprite-tofe-standing-Sheet.png"),
        texture_left: asset_server.load("characters/tofe/walking/Sprite-tofe-walking-L-Sheet.png"),
        texture_right: asset_server.load("characters/tofe/walking/Sprite-tofe-walking-R-Sheet.png"),
    });
    commands.insert_resource(hero_data);
}
