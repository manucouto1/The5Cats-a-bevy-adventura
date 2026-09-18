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

pub const JUMP_FORCE: f32 = 500.0;
pub const HORIZONTAL_FORCE: f32 = 200.0;

/// The player has run out of health. Input is ignored, the collider stops
/// interacting with the level and the sprite tumbles off screen while the
/// game-over timer runs.
#[derive(Component)]
pub struct Dead;

/// Brief invulnerability on spawn so an enemy standing on the start tile
/// can't land a hit before the player has even seen the level.
pub const SPAWN_GRACE_SECS: f32 = 1.5;

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

/// Synchronous gate shared by every damage source so two systems firing in
/// the same frame can't both bypass `Invincibility` (commands inserting that
/// component are deferred until the schedule's next sync, so two damage
/// systems would both observe it as absent and stack -1 HP each). Damage
/// systems must consult and update this resource directly — Resource writes
/// are immediate, no deferral.
#[derive(Resource, Default, Debug)]
pub struct PlayerHitGuard {
    pub locked_until_secs: f32,
}

/// Brief stun applied when an enemy, projectile or hazard hits the player.
/// While present, `player_input_system` replaces horizontal input with the
/// stored push so the shove lasts the whole stun (pygame kept the hero
/// moving away for 0.5 s), and the vertical pop is applied on the first
/// frame it is seen.
#[derive(Component)]
pub struct Knockback {
    pub timer: Timer,
    pub push: Vec2,
    pub applied: bool,
}

pub const KNOCKBACK_DURATION: f32 = 0.4;
pub const KNOCKBACK_PUSH_X: f32 = 260.0;
pub const KNOCKBACK_PUSH_Y: f32 = 330.0;

impl Knockback {
    pub fn with_push(duration: f32, push: Vec2) -> Self {
        Self {
            timer: Timer::from_seconds(duration, TimerMode::Once),
            push,
            applied: false,
        }
    }

    /// Standard hit reaction: pop up and away from `source`.
    pub fn away_from(player_pos: Vec2, source_pos: Vec2) -> Self {
        let dx = player_pos.x - source_pos.x;
        let dir_x = if dx.abs() > f32::EPSILON {
            dx.signum()
        } else {
            1.0
        };
        Self::with_push(
            KNOCKBACK_DURATION,
            Vec2::new(dir_x * KNOCKBACK_PUSH_X, KNOCKBACK_PUSH_Y),
        )
    }
}
