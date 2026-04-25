use crate::{
    game_state::LevelCompleteEvent,
    map::components::{
        BouncyPlatform, DamageTile, FallingState, FallingTile, TileProperties, TileType,
    },
    physics::Velocity as PlayerVelocity,
    player::components::{Health, Invincibility, PlayerCharacter},
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::{CollisionEvent, KinematicCharacterControllerOutput, RigidBody};

// Sistema para manejar tiles que caen
pub fn falling_tiles_system(
    time: Res<Time>,
    mut falling_tiles: Query<(Entity, &mut FallingTile, &mut Transform), With<TileProperties>>,
    mut rigid_body_query: Query<&mut RigidBody>,
    mut commands: Commands,
) {
    for (entity, mut falling_tile, mut transform) in falling_tiles.iter_mut() {
        match falling_tile.state {
            FallingState::Triggered => {
                falling_tile.state = FallingState::Shaking;
                falling_tile.shake_timer.reset();
            }
            FallingState::Shaking => {
                falling_tile.shake_timer.tick(time.delta());

                // Aplicar efecto de sacudida
                let shake_progress = falling_tile.shake_timer.elapsed_secs()
                    / falling_tile.shake_timer.duration().as_secs_f32();
                let shake_offset = (shake_progress * 20.0).sin() * falling_tile.shake_intensity;

                transform.translation.x = falling_tile.original_position.x + shake_offset;

                // Si termina la sacudida, empezar a caer
                if falling_tile.shake_timer.just_finished() {
                    falling_tile.state = FallingState::Falling;
                    falling_tile.fall_timer.reset();
                    // Cambiar a dinámico para que caiga
                    if let Ok(mut rigid_body) = rigid_body_query.get_mut(entity) {
                        *rigid_body = RigidBody::Dynamic;
                    }
                }
            }
            FallingState::Falling => {
                falling_tile.fall_timer.tick(time.delta());

                // Si ha caído por suficiente tiempo o está muy abajo, eliminarlo completamente
                if falling_tile.fall_timer.just_finished() || transform.translation.y < -500.0 {
                    falling_tile.state = FallingState::Fallen;
                    commands.entity(entity).despawn();
                }
            }
            _ => {} // Stable y Fallen no necesitan procesamiento
        }
    }
}

pub fn trigger_falling_tiles_system(
    mut falling_tiles: Query<(Entity, &mut FallingTile, &TileProperties)>,
    player_query: Query<&KinematicCharacterControllerOutput, With<PlayerCharacter>>,
) {
    if let Ok(controller_output) = player_query.single() {
        for collision in &controller_output.collisions {
            let collided_entity = collision.entity;

            if let Ok((_, mut falling_tile, tile_properties)) =
                falling_tiles.get_mut(collided_entity)
            {
                if tile_properties.tile_type == TileType::Falling
                    && falling_tile.state == FallingState::Stable
                {
                    falling_tile.state = FallingState::Triggered;
                }
            }
        }
    }
}

pub fn bouncy_platforms_system(
    mut bouncy_query: Query<(Entity, &mut BouncyPlatform)>,
    player_query: Query<&KinematicCharacterControllerOutput, With<PlayerCharacter>>,
    player_velocity_query: Query<&PlayerVelocity, With<PlayerCharacter>>,
) {
    if let Ok(controller_output) = player_query.single() {
        if let Ok(player_velocity) = player_velocity_query.single() {
            for collision in &controller_output.collisions {
                let collided_entity = collision.entity;

                if let Ok((_, mut bouncy_platform)) = bouncy_query.get_mut(collided_entity) {
                    // Calcula la dirección del rebote
                    let direction = (bouncy_platform.velocity - player_velocity.velocity)
                        .normalize_or_zero()
                        * bouncy_platform.bounce_force;

                    // Aplica la nueva velocidad a la plataforma rebotadora
                    bouncy_platform.velocity = direction;
                }
            }
        }
    }
}

pub fn damage_platforms_system(
    mut commands: Commands,
    player_query: Query<
        (
            Entity,
            &KinematicCharacterControllerOutput,
            Option<&Invincibility>,
        ),
        With<PlayerCharacter>,
    >,
    mut health_query: Query<&mut Health, With<PlayerCharacter>>,
    mut velocity_query: Query<&mut PlayerVelocity, With<PlayerCharacter>>,
    damage_tile_query: Query<&DamageTile>,
) {
    // Skip silently if the player isn't spawned yet (e.g., the first frame
    // of LevelLoaded before the spawn commands flush) or if they're already
    // invincible from a recent hit.
    let Ok((player_entity, controller_output, invincibility)) = player_query.single() else {
        return;
    };
    if invincibility.is_some() {
        return;
    }

    for collision in &controller_output.collisions {
        let Ok(damage_tile) = damage_tile_query.get(collision.entity) else {
            continue;
        };
        let Ok(mut player_health) = health_query.get_mut(player_entity) else {
            continue;
        };
        let Ok(mut player_velocity) = velocity_query.get_mut(player_entity) else {
            continue;
        };

        if player_health.current > 0 {
            player_health.current -= damage_tile.damage_amount as u32;
            commands
                .entity(player_entity)
                .insert(Invincibility::new(1.9));
        }

        let Some(details) = collision.hit.details else {
            continue;
        };
        let n = details.normal2;
        let v = player_velocity.velocity;
        let reflected = v - 2.0 * v.dot(n) * n;
        player_velocity.velocity = reflected * 0.9;
    }
}

/// Fires a `LevelCompleteEvent` the first time the player overlaps an
/// `EndLevel` tile. The tile is a sensor, so the player walks through it
/// rather than being blocked. Only one event is emitted per collision frame,
/// even if multiple tile collisions arrive simultaneously.
pub fn end_level_trigger_system(
    mut collision_events: EventReader<CollisionEvent>,
    mut level_complete: EventWriter<LevelCompleteEvent>,
    player_query: Query<Entity, With<PlayerCharacter>>,
    tile_query: Query<&TileProperties>,
) {
    let Ok(player_entity) = player_query.single() else {
        return;
    };

    for event in collision_events.read() {
        let CollisionEvent::Started(e1, e2, _) = event else {
            continue;
        };
        let other = if *e1 == player_entity {
            *e2
        } else if *e2 == player_entity {
            *e1
        } else {
            continue;
        };

        if let Ok(props) = tile_query.get(other) {
            if props.tile_type == TileType::EndLevel {
                level_complete.write(LevelCompleteEvent);
                return;
            }
        }
    }
}
