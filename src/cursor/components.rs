use bevy::prelude::*;

/// Marker for the crosshair sprite.
#[derive(Component)]
pub struct Crosshair;

/// Marker for player wool-ball projectiles.
#[derive(Component)]
pub struct WoolBall;

/// State for a player wool-ball projectile. `despawn_timer` kicks in only
/// after a collision; `has_collided` gates it so the bullet stays alive
/// until it hits something, then expires after the timer.
#[derive(Component)]
pub struct Projectile {
    pub despawn_timer: Timer,
    pub has_collided: bool,
}
