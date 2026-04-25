//! Level-4 style "gap zones": rectangular regions (in tile coords) that,
//! when the player is inside them, change the active level mode. The big
//! one is `falling_mode` — gravity is replaced by a constant downward
//! velocity so the player free-falls through a long vertical shaft.
//!
//! Level 1 and 3 ship an empty `gaps` array, and level 2 uses the structure
//! only for camera Y-range hints without actions. We accept either: zones
//! without a recognized `action` are ignored.

use bevy::prelude::*;
use serde::Deserialize;

use crate::{
    game_state::CurrentLevel,
    map::assets::GameAssets,
    physics::FallingMode,
    player::components::PlayerCharacter,
};

/// Current high-level gameplay mode dictated by the zone the player is in.
/// Defaults to `Normal`; gets flipped to `Falling` in level-4 shafts.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LevelMode {
    #[default]
    Normal,
    Clear,
    Falling,
}

/// Gate for level 4's falling shaft. The player only enters `FallingMode`
/// in a falling-type zone AFTER this flag is armed (set by the boss when
/// phase 1 ends). This enforces the narrative: the boss falls first, then
/// the player jumps in after.
#[derive(Resource, Default, Debug)]
pub struct BossFallArmed(pub bool);

/// Falling shaft speed — player descends at this rate while `LevelMode`
/// is `Falling`. Tuned so the ~6000 px shaft clears in ~12s.
pub const FALLING_SPEED: f32 = 500.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneAction {
    Normal,
    Clear,
    Falling,
}

pub struct Zone {
    pub action: ZoneAction,
    /// AABB in WORLD coordinates (not tiles).
    pub bounds: Rect,
}

#[derive(Resource, Default)]
pub struct LevelZones {
    pub zones: Vec<Zone>,
}

#[derive(Deserialize)]
struct GapsFile {
    gaps: Vec<GapEntry>,
}

#[derive(Deserialize)]
struct GapEntry {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    x_init: Option<u32>,
    #[serde(default)]
    x_end: Option<u32>,
    #[serde(default)]
    y_init: Option<u32>,
    #[serde(default)]
    y_end: Option<u32>,
}

/// Parses `levelN_gaps.json` and materializes zones as world-space rectangles
/// in the `LevelZones` resource. Entries without x/y bounds or with an
/// unrecognized action are skipped (level 2 uses this file for Y-only camera
/// hints, not for action zones).
pub fn load_level_zones(
    mut commands: Commands,
    current_level: Res<CurrentLevel>,
    game_assets: Res<GameAssets>,
) {
    let paths = current_level.0.get_path();
    let Ok(raw) = std::fs::read_to_string(&paths.gaps) else {
        commands.insert_resource(LevelZones::default());
        return;
    };
    let parsed: GapsFile = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to parse gaps file {}: {e}", paths.gaps);
            commands.insert_resource(LevelZones::default());
            return;
        }
    };

    let tile_size = game_assets.tile_size_px;
    let map_w = game_assets.map_width_tiles as f32;
    let map_h = game_assets.map_height_tiles as f32;
    let offset_x = -(map_w * tile_size / 2.0);
    let offset_y = map_h * tile_size / 2.0;

    let mut zones = Vec::new();
    for g in parsed.gaps {
        let action = match g.action.as_deref() {
            Some("falling_mode") => ZoneAction::Falling,
            Some("clear_mode") => ZoneAction::Clear,
            Some("normal_mode") => ZoneAction::Normal,
            _ => continue,
        };
        let (Some(xi), Some(xe), Some(yi), Some(ye)) = (g.x_init, g.x_end, g.y_init, g.y_end)
        else {
            continue;
        };
        // Convert tile-space AABB to world-space. The level 4 mapping:
        // tile y grows downward, world y grows upward, so y_init (smaller
        // tile y) maps to the HIGHER world y (max), and vice versa.
        let min_x = xi as f32 * tile_size + offset_x;
        let max_x = xe as f32 * tile_size + offset_x;
        let max_y = -(yi as f32 * tile_size) + offset_y;
        let min_y = -(ye as f32 * tile_size) + offset_y;
        zones.push(Zone {
            action,
            bounds: Rect::new(min_x, min_y, max_x, max_y),
        });
    }

    info!("Loaded {} level zones", zones.len());
    commands.insert_resource(LevelZones { zones });
}

/// Each frame, finds which zone the player is in and updates `LevelMode`
/// accordingly. The player's `FallingMode` component is added/removed to
/// match — but only when `BossFallArmed` is set, which the boss triggers
/// when it finishes phase 1. Before the shaft is armed, the player can
/// still wander into the falling zone, they just don't enter free-fall.
pub fn level_zone_trigger_system(
    mut commands: Commands,
    zones: Option<Res<LevelZones>>,
    mut level_mode: ResMut<LevelMode>,
    armed: Res<BossFallArmed>,
    player: Query<(Entity, &Transform, Has<FallingMode>), With<PlayerCharacter>>,
) {
    let Some(zones) = zones else { return };
    if zones.zones.is_empty() {
        if *level_mode != LevelMode::Normal {
            *level_mode = LevelMode::Normal;
        }
        return;
    }
    let Ok((player_entity, tf, has_falling)) = player.single() else {
        return;
    };
    let pos = tf.translation.truncate();

    let new_mode = zones
        .zones
        .iter()
        .find(|z| z.bounds.contains(pos))
        .map(|z| match z.action {
            ZoneAction::Falling => LevelMode::Falling,
            ZoneAction::Clear => LevelMode::Clear,
            ZoneAction::Normal => LevelMode::Normal,
        })
        .unwrap_or(*level_mode);

    if new_mode != *level_mode {
        *level_mode = new_mode;
    }

    // Falling mode only applies to the player after the boss has armed the
    // shaft (phase 1 complete). Otherwise the player falls through with
    // regular gravity — not the stylized constant-speed shaft descent.
    let should_fall = matches!(*level_mode, LevelMode::Falling) && armed.0;
    match (should_fall, has_falling) {
        (true, false) => {
            commands
                .entity(player_entity)
                .insert(FallingMode { speed: FALLING_SPEED });
        }
        (false, true) => {
            commands.entity(player_entity).remove::<FallingMode>();
        }
        _ => {}
    }
}

/// Returns the horizontal center of the falling-zone rectangle in world
/// coordinates (or `None` if no falling zone is defined). Used by the
/// boss to teleport to the top of the shaft when it starts falling.
pub fn falling_zone_center_x(zones: &LevelZones) -> Option<f32> {
    zones
        .zones
        .iter()
        .find(|z| z.action == ZoneAction::Falling)
        .map(|z| (z.bounds.min.x + z.bounds.max.x) / 2.0)
}
