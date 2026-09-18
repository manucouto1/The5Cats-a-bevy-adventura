use bevy::prelude::*;

/// Marker for the crosshair sprite.
#[derive(Component)]
pub struct Crosshair;

/// Marker for player wool-ball projectiles.
#[derive(Component)]
pub struct WoolBall;

/// State for a player wool-ball projectile.
///
/// `despawn_timer` runs from the moment the ball is thrown, never from the
/// impact: a shot that sails off into open sky has nothing to collide with,
/// and while it was alive it counted against the five-in-flight cap — which
/// is why shooting stopped working outdoors after the first few throws, but
/// never indoors, where every ball finds a wall.
///
/// `has_collided` now only shortens what is left of that lifetime, so a
/// ball that hits something disappears promptly instead of lying there.
#[derive(Component)]
pub struct Projectile {
    pub despawn_timer: Timer,
    pub has_collided: bool,
}

/// How long a wool ball lives at most, hit or miss.
pub const WOOL_BALL_LIFETIME: f32 = 2.0;
/// What is left of that lifetime once it has hit something.
pub const WOOL_BALL_IMPACT_LINGER: f32 = 0.25;
