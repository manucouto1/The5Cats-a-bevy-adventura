
use bevy::{platform::collections::HashMap, prelude::*};

use crate::enemies::components::{ActiveLevenData, EnemyAssetSet, EnemyAssets, EnemyType};
use crate::game_state::CurrentLevel;

pub fn load_enemy_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    current_level: Res<CurrentLevel>,
) {
    let mut map = HashMap::new();

    let paths = current_level.0.get_path();
    let json_string = crate::game_state::read_level_file(&paths.active_objects)
        .expect("Failed to read level JSON");
    let enemies_level_data: ActiveLevenData =
        serde_json::from_str(&json_string).expect("Failed to parse level JSON");
    map.insert(
        EnemyType::Catcifer,
        EnemyAssetSet {
            texture_standing: asset_server
                .load("characters/catcifer/standing/Sprite-catcifer-standing-Sheet.png"),
            texture_left: asset_server
                .load("characters/catcifer/walking/Sprite-catcifer-walking-L-Sheet.png"),
            texture_right: asset_server
                .load("characters/catcifer/walking/Sprite-catcifer-walking-R-Sheet.png"),
        },
    );

    map.insert(
        EnemyType::Dummy,
        EnemyAssetSet {
            texture_standing: asset_server
                .load("characters/enemy/standing/enemy-standing-Sheet.png"),
            texture_left: asset_server.load("characters/enemy/walking/enemy-walking-Sheet.png"),
            texture_right: asset_server.load("characters/enemy/walking/enemy-walking-Sheet.png"),
        },
    );

    map.insert(
        EnemyType::Fufi,
        EnemyAssetSet {
            texture_standing: asset_server
                .load("characters/fufi/standing/Sprite-fufi-standing-Sheet.png"),
            texture_left: asset_server
                .load("characters/fufi/walking/Sprite-fufi-walking-L-Sheet.png"),
            texture_right: asset_server
                .load("characters/fufi/walking/Sprite-fufi-walking-R-Sheet.png"),
        },
    );

    map.insert(
        EnemyType::KiddCat,
        EnemyAssetSet {
            texture_standing: asset_server
                .load("characters/kidd_cat/standing/Sprite-kidd-standing-Sheet.png"),
            texture_left: asset_server
                .load("characters/kidd_cat/walking/Sprite-kidd-walking-L-Sheet.png"),
            texture_right: asset_server
                .load("characters/kidd_cat/walking/Sprite-kidd-walking-R-Sheet.png"),
        },
    );

    map.insert(
        EnemyType::Maximiliano,
        EnemyAssetSet {
            texture_standing: asset_server
                .load("characters/maximiliano/standing/Sprite-tom-standing-Sheet.png"),
            texture_left: asset_server
                .load("characters/maximiliano/walking/Sprite-tom-walking-L-Sheet.png"),
            texture_right: asset_server
                .load("characters/maximiliano/walking/Sprite-tom-walking-R-Sheet.png"),
        },
    );

    map.insert(
        EnemyType::Willie,
        EnemyAssetSet {
            texture_standing: asset_server
                .load("characters/willie/standing/Sprite-willie-standing-Sheet.png"),
            texture_left: asset_server
                .load("characters/willie/walking/Sprite-willie-walking-L-Sheet.png"),
            texture_right: asset_server
                .load("characters/willie/walking/Sprite-willie-walking-R-Sheet.png"),
        },
    );
    // Repite para los demás tipos...
    commands.insert_resource(enemies_level_data);
    commands.insert_resource(EnemyAssets {
        map,
        projectile_texture: asset_server.load("projectiles/wool.png"),
    });
}
