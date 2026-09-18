
use bevy::prelude::*;
use serde::Deserialize;

use crate::game_state::CurrentLevel;

/// Sprite strips for one costume: standing, walking left, walking right.
#[derive(Clone)]
pub struct CostumeSheets {
    pub standing: Handle<Image>,
    pub left: Handle<Image>,
    pub right: Handle<Image>,
}

// Un Resource para contener las handles de los assets del jugador
#[derive(Resource)]
pub struct PlayerAssets {
    pub normal: CostumeSheets,
    /// Same animations wearing the foil hat — shown while maniac mode is on.
    pub hat: CostumeSheets,
}

#[derive(Debug, Deserialize, Resource, Clone)]
pub struct HeroData {
    pub x: f32,
    pub y: f32,
}

/// Root shape of `levelN_active_object.json` — extra fields (`enemies`)
/// are ignored by serde.
#[derive(Deserialize)]
struct ActiveObjectRoot {
    hero: HeroData,
}

pub fn load_player_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    current_level: Res<CurrentLevel>,
) {
    let paths = current_level.0.get_path();
    let root: ActiveObjectRoot = serde_json::from_str(
        &crate::game_state::read_level_file(&paths.active_objects)
            .expect("Failed to read level JSON"),
    )
    .expect("Failed to parse hero JSON");

    commands.insert_resource(PlayerAssets {
        normal: CostumeSheets {
            standing: asset_server.load("characters/tofe/standing/Sprite-tofe-standing-Sheet.png"),
            left: asset_server.load("characters/tofe/walking/Sprite-tofe-walking-L-Sheet.png"),
            right: asset_server.load("characters/tofe/walking/Sprite-tofe-walking-R-Sheet.png"),
        },
        hat: CostumeSheets {
            standing: asset_server
                .load("characters/tofe/standing/hat/Sprite-tofe-standing-hat-Sheet.png"),
            left: asset_server
                .load("characters/tofe/walking/hat/Sprite-tofe-walking-hat-L-Sheet.png"),
            right: asset_server
                .load("characters/tofe/walking/hat/Sprite-tofe-walking-hat-R-Sheet.png"),
        },
    });
    commands.insert_resource(root.hero);
}
