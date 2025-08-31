use bevy::prelude::*;

/// Componente de marcador para la entidad del puntero (crosshair).
#[derive(Component)]
pub struct Crosshair;

/// Componente de marcador para la entidad de la línea de apuntado.
#[derive(Component)]
pub struct AimingLine;

#[derive(Component)]
pub struct WoolBall;

/// Componente para gestionar el estado de un proyectil.
#[derive(Component)]
pub struct Projectile {
    pub velocity: Vec2,
    pub despawn_timer: Timer,
    pub has_collided: bool,
}
