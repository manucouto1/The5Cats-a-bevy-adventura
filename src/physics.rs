use bevy::prelude::*;
use bevy_rapier2d::prelude::{KinematicCharacterController, KinematicCharacterControllerOutput};

pub const GRAVITY: f32 = 9.81;
pub const SMOOTHING_FACTOR: f32 = 0.9;

#[derive(Component)]
pub struct AffectedByGravity;

#[derive(Component, Debug)]
pub struct Mass {
    pub kilograms: f32,
}

impl Default for Mass {
    fn default() -> Self {
        Self {
            kilograms: 100.0, // 70kg - masa promedio de una persona
        }
    }
}

#[derive(Component, Default)]
pub struct Velocity {
    pub velocity: Vec2,
}

/// Overrides regular gravity with a constant downward velocity. Used by
/// level 4's falling zones: the player free-falls through a long shaft
/// at a steady speed while keeping horizontal control. Removed when the
/// player enters a non-falling zone.
#[derive(Component)]
pub struct FallingMode {
    pub speed: f32,
}

pub fn gravity_system(
    time: Res<Time>,
    mut query: Query<
        (&mut Velocity, &Mass, &KinematicCharacterControllerOutput),
        (With<AffectedByGravity>, Without<FallingMode>),
    >,
) {
    let t = (SMOOTHING_FACTOR * time.delta_secs()).min(1.0);
    for (mut velocity, mass, output) in &mut query {
        if !output.grounded {
            let gravity_force = GRAVITY * mass.kilograms;
            velocity.velocity.y -= gravity_force * t;
        }
    }
}

/// For every entity with `FallingMode`, pins vertical velocity to a constant
/// downward value each frame. Horizontal velocity is left alone so the
/// player keeps left/right control while falling.
pub fn falling_mode_system(mut query: Query<(&mut Velocity, &FallingMode)>) {
    for (mut velocity, falling) in &mut query {
        velocity.velocity.y = -falling.speed;
    }
}

pub fn kinematic_character_movement_system(
    time: Res<Time>,
    mut query: Query<(&Velocity, &mut KinematicCharacterController), With<AffectedByGravity>>,
) {
    let t = (SMOOTHING_FACTOR * time.delta_secs()).min(1.0);
    for (velocity, mut controller) in &mut query {
        controller.translation = Some(velocity.velocity * t);
    }
}
