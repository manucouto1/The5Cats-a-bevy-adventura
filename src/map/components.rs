// src/level_data.rs

use bevy::prelude::*;
use serde::Deserialize;

#[derive(Debug, Deserialize, Resource)]
pub struct LevelData {
    pub tile_size: u32,
    pub map_width: u32,
    pub map_height: u32,
    pub layers: Vec<LayerData>,
}

#[derive(Debug, Deserialize)]
pub struct LayerData {
    pub name: u32,
    pub path: String,
    pub positions: Vec<TilePosition>,
}

#[derive(Debug, Deserialize)]
pub struct TilePosition {
    pub x: u32,
    pub y: u32,
    /// Tile atlas index. Required for tilemap layers (drives the sprite
    /// rendered for each cell). Optional for active-object positions
    /// (`level{N}_active_object.json` enemies/hero), which only consume
    /// `x`/`y` — older exports often omit it for those entries.
    #[serde(default)]
    pub id: u32,
}

#[derive(Component)]
pub struct LevelTile;

// Enum para los diferentes tipos de tiles específicos del juego
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileType {
    Solid,           // Tile sólido estándar (cajas, suelo)
    Falling,         // Tile que cae cuando el player lo pisa
    Damage,          // Tile que causa daño al player
    PipeBottomLeft,  // Pipe en esquina inferior izquierda
    PipeBottomRight, // Pipe en esquina inferior derecha
    Bouncy,          // Springboard: bounces the player on contact
    EndLevel,        // The goal
}

#[derive(Component, Debug, Clone)]
pub struct TileProperties {
    pub tile_type: TileType,
    pub damage: i32,         // Daño que causa (solo para tiles de damage)
    pub fall_delay: f32,     // Tiempo antes de caer (solo para falling tiles)
    pub shake_duration: f32, // Duración del temblor antes de caer
    pub custom_collider: Option<ColliderShape>, // Collider shape, if not the full tile
}

#[derive(Debug, Clone)]
pub enum ColliderShape {
    FullTile,           // Tile completo (32x32)
    ThinHorizontal,     // Línea horizontal fina (32x4)
    HalfVertical,       // Bottom half only (32x16)
    QuarterBottomLeft,  // Bottom-left quarter (16x16)
    QuarterBottomRight, // Bottom-right quarter (16x16)
}

impl Default for TileProperties {
    fn default() -> Self {
        TileProperties {
            tile_type: TileType::Solid,
            damage: 0,
            fall_delay: 1.0,
            shake_duration: 0.5,
            custom_collider: Some(ColliderShape::FullTile),
        }
    }
}

impl TileProperties {
    pub fn solid() -> Self {
        TileProperties {
            tile_type: TileType::Solid,
            custom_collider: Some(ColliderShape::FullTile),
            ..Default::default()
        }
    }

    pub fn falling() -> Self {
        TileProperties {
            tile_type: TileType::Falling,
            fall_delay: 1.5,
            shake_duration: 0.8,
            custom_collider: Some(ColliderShape::ThinHorizontal),
            ..Default::default()
        }
    }

    pub fn damage(damage_amount: i32) -> Self {
        TileProperties {
            tile_type: TileType::Damage,
            damage: damage_amount,
            custom_collider: Some(ColliderShape::HalfVertical),
            ..Default::default()
        }
    }

    pub fn pipe_bottom_left() -> Self {
        TileProperties {
            tile_type: TileType::PipeBottomLeft,
            custom_collider: Some(ColliderShape::QuarterBottomLeft),
            ..Default::default()
        }
    }

    pub fn pipe_bottom_right() -> Self {
        TileProperties {
            tile_type: TileType::PipeBottomRight,
            custom_collider: Some(ColliderShape::QuarterBottomRight),
            ..Default::default()
        }
    }

    pub fn bouncy() -> Self {
        TileProperties {
            tile_type: TileType::Bouncy,
            custom_collider: Some(ColliderShape::FullTile),
            ..Default::default()
        }
    }

    pub fn end_level() -> TileProperties {
        TileProperties {
            tile_type: TileType::EndLevel,
            custom_collider: Some(ColliderShape::FullTile),
            ..Default::default()
        }
    }
}

/// Maps the semantic `path` string in each level json layer to the per-tile
/// properties that drive collision and behavior. Paths are normalized by
/// `scripts/rebuild_level_paths.py` so a single match is enough — no spatial
/// heuristics, no per-level overrides. `decoration` and any unknown path
/// returns `None` (sprite-only render, no collider).
pub fn get_tile_properties_from_path(path: &str) -> Option<TileProperties> {
    match path {
        "ground" => Some(TileProperties::solid()),
        "falling" => Some(TileProperties::falling()),
        "damage" => Some(TileProperties::damage(1)),
        "bouncy" => Some(TileProperties::bouncy()),
        "pipe_left" => Some(TileProperties::pipe_bottom_left()),
        "pipe_right" => Some(TileProperties::pipe_bottom_right()),
        "end_level" => Some(TileProperties::end_level()),
        _ => None, // "decoration" and anything else: sprite-only, no collider.
    }
}

// Componente para tiles que caen
#[derive(Component, Debug)]
pub struct FallingTile {
    pub state: FallingState,
    pub shake_timer: Timer,
    pub fall_timer: Timer,
    pub original_position: Vec3,
    pub shake_intensity: f32,
}

#[derive(Debug, PartialEq)]
pub enum FallingState {
    Stable,    // Estado normal
    Triggered, // El player lo ha pisado
    Shaking,   // Temblando antes de caer
    Falling,   // Cayendo
    Fallen,    // Ya ha caído
}

impl Default for FallingTile {
    fn default() -> Self {
        FallingTile {
            state: FallingState::Stable,
            shake_timer: Timer::from_seconds(0.8, TimerMode::Once),
            fall_timer: Timer::from_seconds(1.5, TimerMode::Once),
            original_position: Vec3::ZERO,
            shake_intensity: 2.0,
        }
    }
}

// Componente para tiles que causan daño
#[derive(Component, Debug)]
pub struct DamageTile {
    pub damage_amount: i32,
}

impl Default for DamageTile {
    fn default() -> Self {
        DamageTile { damage_amount: 1 }
    }
}

#[derive(Component, Debug)]
pub struct PipeTile {}

#[derive(Component, Debug)]
pub struct BouncyPlatform {
    pub bounce_force: f32,
    pub original_position: Vec3,
}

impl Default for BouncyPlatform {
    fn default() -> Self {
        BouncyPlatform {
            bounce_force: 100.0,
            original_position: Vec3::ZERO,
        }
    }
}
