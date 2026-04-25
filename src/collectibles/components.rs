use bevy::prelude::*;

/// Identifies what a collectible does on pickup. The entity is otherwise a
/// plain sensor sprite — the kind is read by `pickup_system` to branch on
/// the effect (score, heal, buff).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Collectible {
    KittyPoint,
    ExtraLife,
    ManiacMode,
}

/// Applied to collectibles that seek the player when close (ExtraLife, per
/// pygame). Absent on plain pickups, which stay where they dropped.
#[derive(Component)]
pub struct Magnetic {
    pub trigger_radius: f32,
    pub speed: f32,
}

/// Temporary player buff granted by picking up ManiacMode. Removed by the
/// expiration system when the timer finishes.
#[derive(Component)]
pub struct ManiacBuff(pub Timer);

/// Marker for the HUD score text node so `update_score_hud` can find and
/// rewrite its string when `Score` changes.
#[derive(Component)]
pub struct ScoreText;
