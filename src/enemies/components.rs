use crate::map::components::TilePosition;
use bevy::{
    asset::Handle,
    ecs::{component::Component, resource::Resource},
    image::Image,
    platform::collections::HashMap,
    time::Timer,
};
use serde::Deserialize;
use strum_macros::{Display, EnumString, VariantNames};

#[derive(Debug, Hash, PartialEq, Eq, Clone, EnumString, VariantNames, Display)]
pub enum EnemyType {
    Catcifer,
    Dummy,
    Fufi,
    KiddCat,
    Maximiliano,
    Willie,
}

pub struct EnemyAssetSet {
    pub texture_standing: Handle<Image>,
    pub texture_left: Handle<Image>,
    pub texture_right: Handle<Image>,
}

#[derive(Resource)]
pub struct EnemyAssets {
    pub map: HashMap<EnemyType, EnemyAssetSet>,
}

#[derive(Debug, Deserialize, Resource)]
pub struct ActiveObjectData {
    pub name: String,
    pub scale: u32,
    pub positions: Vec<TilePosition>,
}

#[derive(Debug, Deserialize, Resource)]
pub struct ActiveLevenData {
    pub enemies: Vec<ActiveObjectData>,
}

#[derive(Component)]
pub struct EnemyCharacter;

// --- Componentes de IA ---

/// Define el estado actual de la IA de un enemigo.
#[derive(Component, Debug, PartialEq, Eq)]
pub enum EnemyState {
    Idle,
    Patrolling,
    Chasing,
    Attacking,
    Fleeing, // Para teletransportarse
}

/// Componente para enemigos que patrullan.
#[derive(Component)]
pub struct Patrol {
    pub speed: f32,
    pub direction: i32, // 1 para derecha, -1 para izquierda
}

/// Componente para enemigos que persiguen al jugador.
#[derive(Component)]
pub struct Chase {
    pub speed: f32,
    pub range: f32,
}

/// Componente para ataques a distancia.
#[derive(Component)]
pub struct RangedAttack {
    pub attack_type: RangedAttackType,
    pub range: f32,
    pub timer: Timer,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RangedAttackType {
    SingleShot, // Para Fufi
    FanShot,    // Para Catcifer
}

/// Componente para enemigos que se teletransportan.
#[derive(Component)]
pub struct Teleport {
    /// Distancia mínima a la que el jugador debe estar para activar el teletransporte.
    pub trigger_distance: f32,
    pub timer: Timer,
}

/// Componente para enemigos que hacen daño al contacto.
#[derive(Component)]
pub struct ContactDamage {
    pub amount: u32,
}

/// Componente para marcar a los proyectiles de los enemigos.
#[derive(Component)]
pub struct EnemyProjectile;
