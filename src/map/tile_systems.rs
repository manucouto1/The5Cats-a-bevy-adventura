use crate::{
    enemies::systems::HIT_INVINCIBILITY_SECS,
    game_state::LevelCompleteEvent,
    map::components::{
        BouncyPlatform, DamageTile, FallingState, FallingTile, TileProperties, TileType,
    },
    physics::Velocity as PlayerVelocity,
    player::components::{Health, Invincibility, PlayerCharacter, PlayerHitGuard},
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::{CollisionEvent, KinematicCharacterControllerOutput, RigidBody};

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

/// Bouncy tiles launch the player along the contact normal — but only when
/// they're actually moving INTO the surface. Brushing past the side of a
/// bouncy box (e.g. mid-jump) used to overwrite velocity with `normal * 100`
/// every frame, which read as a sticky wall that killed jumps. With the
/// approach-speed gate, idle / parallel contact is ignored and only real
/// impacts produce a bounce.
pub fn bouncy_platforms_system(
    bouncy_query: Query<&BouncyPlatform>,
    player_query: Query<&KinematicCharacterControllerOutput, With<PlayerCharacter>>,
    mut player_velocity_query: Query<&mut PlayerVelocity, With<PlayerCharacter>>,
) {
    let Ok(controller_output) = player_query.single() else {
        return;
    };
    let Ok(mut player_velocity) = player_velocity_query.single_mut() else {
        return;
    };

    // Ignore contacts where the player isn't really moving into the bouncy.
    // Anything below this px/s is "leaning against it"; above, it's a hit.
    const MIN_APPROACH_SPEED: f32 = 60.0;

    for collision in &controller_output.collisions {
        let Ok(bouncy) = bouncy_query.get(collision.entity) else {
            continue;
        };
        // Normal points from the bouncy surface toward the player; approach
        // speed is the component of velocity heading into the surface.
        let normal = collision
            .hit
            .details
            .map(|d| d.normal2)
            .unwrap_or(bevy::math::Vec2::Y);
        let approach = -player_velocity.velocity.dot(normal);
        if approach < MIN_APPROACH_SPEED {
            continue;
        }
        // Launch outward; faster impacts bounce harder so falls feel weighty.
        player_velocity.velocity = normal * (bouncy.bounce_force + approach * 0.5);
        break;
    }
}

pub fn damage_platforms_system(
    mut commands: Commands,
    time: Res<Time>,
    mut hit_guard: ResMut<PlayerHitGuard>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
    player_query: Query<
        (
            Entity,
            &KinematicCharacterControllerOutput,
            Option<&Invincibility>,
        ),
        (
            With<PlayerCharacter>,
            Without<crate::player::components::Dead>,
        ),
    >,
    mut health_query: Query<&mut Health, With<PlayerCharacter>>,
    mut velocity_query: Query<(&mut PlayerVelocity, &Transform), With<PlayerCharacter>>,
    damage_tile_query: Query<(&DamageTile, &Transform)>,
) {
    // Skip silently if the player isn't spawned yet (e.g., the first frame
    // of LevelLoaded before the spawn commands flush) or if they're already
    // invincible from a recent hit.
    let Ok((player_entity, controller_output, invincibility)) = player_query.single() else {
        return;
    };
    let now = time.elapsed_secs();
    if invincibility.is_some() || hit_guard.locked_until_secs > now {
        return;
    }

    for collision in &controller_output.collisions {
        let Ok((damage_tile, tile_tf)) = damage_tile_query.get(collision.entity) else {
            continue;
        };
        let Ok(mut player_health) = health_query.get_mut(player_entity) else {
            continue;
        };
        let Ok((mut player_velocity, player_tf)) = velocity_query.get_mut(player_entity) else {
            continue;
        };

        if player_health.current > 0 {
            player_health.current = player_health
                .current
                .saturating_sub(damage_tile.damage_amount as u32);
            hit_guard.locked_until_secs = now + HIT_INVINCIBILITY_SECS;
            commands
                .entity(player_entity)
                .insert(Invincibility::new(HIT_INVINCIBILITY_SECS));
            sfx.write(crate::audio::SfxEvent::HeroHit);
        }

        // Same reaction as an enemy hit: hop up and away from the spike.
        crate::enemies::systems::apply_knockback(
            &mut commands,
            player_entity,
            &mut player_velocity,
            player_tf.translation.truncate(),
            tile_tf.translation.truncate(),
        );

        // One hit per frame — touching N damage tiles in the same step (a
        // row of spikes) used to charge N hits before Invincibility could
        // be applied, dropping the player from 4 HP to 0 in one frame.
        break;
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
