use bevy::prelude::*;
use bevy_rapier2d::prelude::{KinematicCharacterController, KinematicCharacterControllerOutput};

use crate::enemies::components::{Active, EnemyCharacter};

/// Enemies only simulate while near the camera (pygame froze anything
/// farther than ~565 px). Without this, every cat placed in level 4's
/// shaft would drop to the bottom arena on load instead of raining down
/// alongside the player.
type Simulated = Or<(Without<EnemyCharacter>, With<Active>)>;

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
        (With<AffectedByGravity>, Without<FallingMode>, Simulated),
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
/// player keeps left/right control while falling; the player can also hold
/// W/S to slow down or dive (pygame allowed up/down input in free fall).
pub fn falling_mode_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<(
        &mut Velocity,
        &FallingMode,
        Has<crate::player::components::PlayerCharacter>,
    )>,
) {
    const STEER: f32 = 90.0;
    // A hit still pops the faller upward; the pop bleeds off at this rate
    // until the terminal sink speed is reached again.
    const BOUNCE_DECAY: f32 = 900.0;
    for (mut velocity, falling, is_player) in &mut query {
        let mut speed = falling.speed;
        if is_player {
            if keyboard.pressed(KeyCode::ArrowUp) || keyboard.pressed(KeyCode::KeyW) {
                speed -= STEER;
            }
            if keyboard.pressed(KeyCode::ArrowDown) || keyboard.pressed(KeyCode::KeyS) {
                speed += STEER;
            }
        }
        let target = -speed;
        velocity.velocity.y = if velocity.velocity.y > target {
            (velocity.velocity.y - BOUNCE_DECAY * time.delta_secs()).max(target)
        } else {
            target
        };
    }
}

pub fn kinematic_character_movement_system(
    time: Res<Time>,
    mut query: Query<
        (&Velocity, &mut KinematicCharacterController),
        (With<AffectedByGravity>, Simulated),
    >,
) {
    let t = (SMOOTHING_FACTOR * time.delta_secs()).min(1.0);
    for (velocity, mut controller) in &mut query {
        controller.translation = Some(velocity.velocity * t);
    }
}
