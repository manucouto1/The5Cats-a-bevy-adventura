use bevy::prelude::*;

/// Identifies what a collectible does on pickup. The entity is otherwise a
/// plain sensor sprite — the kind is read by `pickup_system` to branch on
/// the effect (score, heal, buff, end of the game).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Collectible {
    /// Rolling-ball point dropped by dummies.
    KittyPoint,
    /// Heart dropped by turrets (and bled by the boss): +1 half-heart.
    ExtraLife,
    /// Treat dropped by maniacs: 10 s of radial fire.
    ManiacMode,
    /// Foil hat dropped by the final boss: touching it wins the game.
    EndGame,
}

/// Applied to collectibles that seek the player when close. Absent on
/// plain pickups, which stay where they dropped.
#[derive(Component)]
pub struct Magnetic {
    pub trigger_radius: f32,
    pub speed: f32,
}

/// Temporary player buff granted by picking up ManiacMode. Removed by the
/// expiration system when the timer finishes.
#[derive(Component)]
pub struct ManiacBuff(pub Timer);

/// Little hop played when a pickup spawns so it visibly pops out of the
/// enemy instead of appearing in place.
#[derive(Component)]
pub struct SpawnPop {
    pub velocity: Vec2,
    pub origin_y: f32,
}

/// Request to place a collectible in the world. Emitted by enemy death and
/// by the boss when it bleeds hearts.
#[derive(Event, Debug)]
pub struct SpawnCollectibleEvent {
    pub kind: Collectible,
    pub position: Vec3,
    /// Initial hop velocity (world px/s).
    pub pop: Vec2,
}
