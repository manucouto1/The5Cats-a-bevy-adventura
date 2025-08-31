use std::time::Duration;

use bevy::prelude::*;

// Componente principal para el personaje
#[derive(Component)]
pub struct PlayerCharacter;

#[derive(Component)]
pub struct CharacterLeftSprite;
#[derive(Component)]
pub struct CharacterRightSprite;
#[derive(Component)]
pub struct CharacterIdleSprite;

#[derive(Component)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

impl Default for Health {
    fn default() -> Self {
        Self { current: 6, max: 6 }
    }
}

// Componente AnimationIndices (igual que antes)
#[derive(Component)]
pub struct AnimationIndices {
    pub first: usize,
    pub last: usize,
    pub fps: u8,
    pub frame_timer: Timer,
}

impl AnimationIndices {
    pub fn new(first: usize, last: usize, fps: u8) -> Self {
        Self {
            first,
            last,
            fps,
            frame_timer: Timer::new(
                Duration::from_secs_f32(1.0 / (fps as f32)),
                TimerMode::Repeating,
            ),
        }
    }
    pub fn timer_from_fps(fps: u8) -> Timer {
        Timer::new(
            Duration::from_secs_f32(1.0 / (fps as f32)),
            TimerMode::Repeating,
        )
    }
}

// Componente para el doble salto
#[derive(Component)]
pub struct DoubleJump {
    pub jumps_remaining: u8,
    pub max_jumps: u8,
}

impl Default for DoubleJump {
    fn default() -> Self {
        Self {
            jumps_remaining: 2,
            max_jumps: 2,
        }
    }
}

pub const GRAVITY: f32 = 9.81; // m/s² - aceleración gravitacional terrestre
pub const JUMP_FORCE: f32 = 500.0; // Newtons - fuerza de salto hacia arriba
pub const HORIZONTAL_FORCE: f32 = 200.0; // Newtons - fuerza de movimiento lateral

#[derive(Component)]
pub struct Invincibility {
    pub timer: Timer,
}

impl Invincibility {
    pub fn new(duration: f32) -> Self {
        Self {
            timer: Timer::from_seconds(duration, TimerMode::Once),
        }
    }
}

#[derive(Component)]
pub struct PlayerHearts {
    pub idx: usize,
}

impl PlayerHearts {
    pub fn new(idx: usize) -> Self {
        Self { idx: idx }
    }
}
