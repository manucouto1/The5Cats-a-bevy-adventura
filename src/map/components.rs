// src/level_data.rs

use bevy::prelude::*;
use serde::Deserialize;

use crate::game_state::LevelPaths;

// Estructuras para deserializar el JSON del nivel
#[derive(Debug, Deserialize, Resource)] // Añadimos Resource aquí
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
    pub id: u32,
}

// Componente marcador para los tiles del nivel
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
    Bouncy,          // Plataforma que rebota al chocar con el player
    EndLevel,        // Tile que marca el final del nivel
}

// Enum para objetos pasivos coleccionables
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectibleType {
    Heart,      // Corazón que recupera vida
    YarnBall,   // Ovillo de lana que da puntos
    BoneCookie, // Galleta de hueso que aplica boost
}

// Propiedades para tiles
#[derive(Component, Debug, Clone)]
pub struct TileProperties {
    pub tile_type: TileType,
    pub damage: i32,         // Daño que causa (solo para tiles de damage)
    pub fall_delay: f32,     // Tiempo antes de caer (solo para falling tiles)
    pub shake_duration: f32, // Duración del temblor antes de caer
    pub custom_collider: Option<ColliderShape>, // Forma custom del collider
}

// Enum para formas específicas de colliders
#[derive(Debug, Clone)]
pub enum ColliderShape {
    FullTile,           // Tile completo (32x32)
    ThinHorizontal,     // Línea horizontal fina (32x4)
    HalfVertical,       // Media altura (32x16)
    QuarterBottomLeft,  // Cuarto inferior izquierdo (16x16)
    QuarterBottomRight, // Cuarto inferior derecho (16x16)
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

    fn end_level() -> TileProperties {
        TileProperties {
            tile_type: TileType::EndLevel,
            custom_collider: Some(ColliderShape::FullTile),
            ..Default::default()
        }
    }
}

// Propiedades para objetos coleccionables
#[derive(Component, Debug, Clone)]
pub struct CollectibleProperties {
    pub collectible_type: CollectibleType,
    pub health_restore: i32,   // Cantidad de vida que restaura
    pub points_value: i32,     // Puntos que otorga
    pub boost_duration: f32,   // Duración del boost en segundos
    pub boost_multiplier: f32, // Multiplicador del boost
}

impl CollectibleProperties {
    pub fn heart(health_amount: i32) -> Self {
        CollectibleProperties {
            collectible_type: CollectibleType::Heart,
            health_restore: health_amount,
            points_value: 0,
            boost_duration: 0.0,
            boost_multiplier: 1.0,
        }
    }

    pub fn yarn_ball(points: i32) -> Self {
        CollectibleProperties {
            collectible_type: CollectibleType::YarnBall,
            health_restore: 0,
            points_value: points,
            boost_duration: 0.0,
            boost_multiplier: 1.0,
        }
    }

    pub fn bone_cookie(duration: f32, multiplier: f32) -> Self {
        CollectibleProperties {
            collectible_type: CollectibleType::BoneCookie,
            health_restore: 0,
            points_value: 0,
            boost_duration: duration,
            boost_multiplier: multiplier,
        }
    }
}

// Mapeo basado en el path del JSON a propiedades específicas
pub fn get_tile_properties_from_path(path: &str) -> Option<TileProperties> {
    match path {
        "solid" | "ground" | "box" => Some(TileProperties::solid()),
        "falling" | "falling_platform" => Some(TileProperties::falling()),
        "damage" | "spikes" | "hurt" => Some(TileProperties::damage(1)),
        "pipe_left" | "pipe_bottom_left" => Some(TileProperties::pipe_bottom_left()),
        "pipe_right" | "pipe_bottom_right" => Some(TileProperties::pipe_bottom_right()),
        "bouncy" | "bouncy_platform" | "moving_platform" => Some(TileProperties::bouncy()),
        "end_level" => Some(TileProperties::end_level()),
        _ => None, // Tiles de fondo o sin propiedades especiales
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

// Componente para objetos coleccionables
#[derive(Component, Debug)]
pub struct CollectibleItem {
    pub properties: CollectibleProperties,
    pub collected: bool,
    pub bob_timer: Timer,   // Para animación de flotación
    pub bob_amplitude: f32, // Amplitud del movimiento de flotación
}

impl Default for CollectibleItem {
    fn default() -> Self {
        CollectibleItem {
            properties: CollectibleProperties::yarn_ball(10),
            collected: false,
            bob_timer: Timer::from_seconds(2.0, TimerMode::Repeating),
            bob_amplitude: 5.0,
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

// Componente marcador para tiles pipe
#[derive(Component, Debug)]
pub struct PipeTile {}

// Componente para plataformas que rebotan
#[derive(Component, Debug)]
pub struct BouncyPlatform {
    pub velocity: Vec2,
    pub bounce_force: f32,
    pub original_position: Vec3,
}

impl Default for BouncyPlatform {
    fn default() -> Self {
        BouncyPlatform {
            velocity: Vec2::ZERO,
            bounce_force: 100.0,
            original_position: Vec3::ZERO,
        }
    }
}

#[derive(Resource)]
pub struct CurrentLevelInfo {
    pub data: LevelPaths,
}
