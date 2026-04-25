use crate::map::components::TilePosition;
use bevy::prelude::*;
use bevy::platform::collections::HashMap;
use serde::Deserialize;
use strum_macros::{Display, EnumString, VariantNames};

/// Enemy types. Per-level JSON files use either the proper cat name
/// (`Catcifer`, `Fufi`, `Dummy`) or lowercase role aliases (`maniac`,
/// `turret`, `dummy`, `boss`). Strum's `serialize`/`ascii_case_insensitive`
/// attributes collapse all variants into one enum so the loader doesn't
/// care which level file it's reading.
///
/// Also used as a `Component` on live enemy entities so systems that handle
/// enemy death can read which type was killed (to spawn the right drop).
#[derive(Debug, Hash, PartialEq, Eq, Clone, Component, EnumString, VariantNames, Display)]
#[strum(ascii_case_insensitive)]
pub enum EnemyType {
    #[strum(serialize = "Catcifer", serialize = "maniac")]
    Catcifer,
    #[strum(serialize = "Dummy")]
    Dummy,
    #[strum(serialize = "Fufi", serialize = "turret")]
    Fufi,
    #[strum(serialize = "KiddCat", serialize = "boss")]
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
    pub positions: Vec<TilePosition>,
}

#[derive(Debug, Deserialize, Resource)]
pub struct ActiveLevenData {
    pub enemies: Vec<ActiveObjectData>,
}

#[derive(Component)]
pub struct EnemyCharacter;

// --- Componentes de IA ---

/// Current AI state for an enemy. Only `Idle` and `Patrolling` are used —
/// expand as more behaviors land.
#[derive(Component, Debug, PartialEq, Eq)]
pub enum EnemyState {
    Idle,
    Patrolling,
}

/// Component for enemies that patrol left/right, reversing at walls/edges.
#[derive(Component)]
pub struct Patrol {
    pub speed: f32,
    /// +1 = facing right, -1 = facing left. Flipped by `patrol_system`.
    pub direction: i32,
}

/// Fufi-style turret: fires `shots_per_burst` bullets with a short interval,
/// then waits `cooldown_timer` before the next burst. Aims at the player.
///
/// Pygame reference: `EnemyTurretShooter.shoot_hero` — 3 shots, 0.09s between,
/// 2.88s cooldown, 300px detection range.
#[derive(Component)]
pub struct TurretShot {
    pub range: f32,
    pub shots_per_burst: u8,
    pub shots_remaining: u8,
    pub burst_interval_timer: Timer,
    pub cooldown_timer: Timer,
}

/// Catcifer-style fan: fires `rays` bullets in a full circle every cooldown.
///
/// Pygame reference: `ShooterEntity.shoot_maniac` — 16 rays at pi/8 steps.
#[derive(Component)]
pub struct FanShot {
    pub range: f32,
    pub rays: u8,
    pub cooldown_timer: Timer,
}

/// Despawns a projectile after a lifetime elapses (prevents off-screen accumulation).
#[derive(Component)]
pub struct ProjectileLifetime(pub Timer);

/// Present on projectiles currently flying. Pool slots that are idle do not
/// have this marker, so per-frame systems (lifetime, damage) skip them.
#[derive(Component)]
pub struct ProjectileActive;

/// Culling marker: present only on enemies within the camera's activation
/// radius. AI systems (patrol, shoot, sprite facing) filter on this so far-
/// away enemies don't consume CPU. Maintained by `activator_system` with
/// hysteresis to avoid flicker on the boundary.
#[derive(Component)]
pub struct Active;

/// Emitted the moment an enemy's HP drops to 0, before the entity is
/// despawned. Consumers (collectibles) read `kind` to decide what to drop
/// and `position` where to spawn it.
#[derive(Event, Debug)]
pub struct EnemyKilledEvent {
    pub position: bevy::prelude::Vec3,
    pub kind: EnemyType,
}

/// Marks the final-boss entity (KiddCat). Bypasses the regular wool-ball
/// damage path and uses `boss_damage_system` instead, which cycles through
/// three HP phases (18 → 36 → 18) before emitting `LevelCompleteEvent`.
#[derive(Component)]
pub struct FinalBoss {
    pub phase: u8,
    /// Minimum time between successive hits. Matches pygame's 2s cooldown
    /// (reduced to 1s here for less grindy testing).
    pub hit_cooldown: Timer,
}

impl Default for FinalBoss {
    fn default() -> Self {
        let mut hit_cooldown = Timer::from_seconds(1.0, TimerMode::Once);
        // Boss is hittable on the very first frame.
        hit_cooldown.tick(hit_cooldown.duration());
        Self {
            phase: 1,
            hit_cooldown,
        }
    }
}

/// Componente para enemigos que hacen daño al contacto.
#[derive(Component)]
pub struct ContactDamage {
    pub amount: u32,
}

/// Componente para marcar a los proyectiles de los enemigos.
#[derive(Component)]
pub struct EnemyProjectile;
