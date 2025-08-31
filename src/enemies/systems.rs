use bevy::prelude::*;
use bevy_rapier2d::prelude::{
    ActiveEvents, Collider, CollisionEvent, KinematicCharacterController, RigidBody,
};
use rand::Rng;

use crate::{
    enemies::components::{ContactDamage, EnemyCharacter, EnemyProjectile, EnemyState, Teleport},
    physics::{Mass, Velocity},
    player::components::{
        AnimationIndices, CharacterIdleSprite, CharacterLeftSprite, CharacterRightSprite, GRAVITY,
        Health, Invincibility, PlayerCharacter,
    },
};

pub fn gravity_system(
    time: Res<Time>,
    mut query: Query<(&mut Velocity, &Mass), With<EnemyCharacter>>,
) {
    for (mut velocity, mass) in &mut query {
        let gravity_force = GRAVITY * mass.kilograms;
        velocity.velocity.y -= gravity_force * time.delta_secs(); // acumula
    }
}

pub fn execute_animations(time: Res<Time>, mut query: Query<(&mut AnimationIndices, &mut Sprite)>) {
    for (mut config, mut sprite) in &mut query {
        config.frame_timer.tick(time.delta());

        if config.frame_timer.just_finished() {
            if let Some(atlas) = &mut sprite.texture_atlas {
                if atlas.index == config.last {
                    atlas.index = config.first;
                } else {
                    atlas.index += 1;
                    config.frame_timer = AnimationIndices::timer_from_fps(config.fps);
                }
            }
        }
    }
}

pub fn apply_velocity_to_controller(
    time: Res<Time>,
    mut query: Query<(&Velocity, &mut KinematicCharacterController), With<EnemyCharacter>>,
) {
    // Aplicar suavizado para evitar artefactos
    let smoothing_factor = 0.9;
    let t = (smoothing_factor * time.delta_secs()).min(1.0);

    for (velocity, mut controller) in &mut query {
        let displacement = velocity.velocity * t;
        controller.translation = Some(displacement);
    }
}

pub fn character_input_handling(
    mut visibility_queries: ParamSet<(
        Query<&mut Visibility, With<CharacterLeftSprite>>,
        Query<&mut Visibility, With<CharacterRightSprite>>,
        Query<&mut Visibility, With<CharacterIdleSprite>>,
    )>,
) {
    let left = false;
    let right = false;

    match (left, right) {
        (true, false) => {
            // Moviendo a la izquierda
            if let Ok(mut v) = visibility_queries.p0().single_mut() {
                *v = Visibility::Visible;
            }
            if let Ok(mut v) = visibility_queries.p1().single_mut() {
                *v = Visibility::Hidden;
            }
            if let Ok(mut v) = visibility_queries.p2().single_mut() {
                *v = Visibility::Hidden;
            }
        }
        (false, true) => {
            // Moviendo a la derecha
            if let Ok(mut v) = visibility_queries.p0().single_mut() {
                *v = Visibility::Hidden;
            }
            if let Ok(mut v) = visibility_queries.p1().single_mut() {
                *v = Visibility::Visible;
            }
            if let Ok(mut v) = visibility_queries.p2().single_mut() {
                *v = Visibility::Hidden;
            }
        }
        _ => {
            // Sin movimiento o ambas teclas presionadas - mostrar idle
            if let Ok(mut v) = visibility_queries.p0().single_mut() {
                *v = Visibility::Hidden;
            }
            if let Ok(mut v) = visibility_queries.p1().single_mut() {
                *v = Visibility::Hidden;
            }
            if let Ok(mut v) = visibility_queries.p2().single_mut() {
                *v = Visibility::Visible;
            }
        }
    }
}

pub fn teleport_system(
    time: Res<Time>,
    mut enemy_query: Query<(&mut Transform, &mut Teleport, &EnemyState)>,
) {
    let mut rng = rand::thread_rng();
    for (mut transform, mut teleport, state) in &mut enemy_query {
        teleport.timer.tick(time.delta());
        if *state == EnemyState::Fleeing && teleport.timer.just_finished() {
            let new_x = transform.translation.x + rng.gen_range(-200.0..200.0);
            // Busca una nueva plataforma elevada (simplificado a una posición Y más alta)
            let new_y = transform.translation.y + rng.gen_range(50.0..150.0);
            transform.translation.x = new_x;
            transform.translation.y = new_y;
            teleport.timer.reset();
        }
    }
}

pub fn enemy_damage_system(
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    mut player_query: Query<(Entity, &mut Health, Option<&Invincibility>), With<PlayerCharacter>>,
    contact_damage_query: Query<&ContactDamage>,
    projectile_query: Query<Entity, With<EnemyProjectile>>,
) {
    let Ok((player_entity, mut player_health, invincibility)) = player_query.single_mut() else {
        return;
    };

    // Si el jugador es invencible, no procesar daño.
    if invincibility.is_some() {
        return;
    }

    for event in collision_events.read() {
        if let CollisionEvent::Started(entity1, entity2, _) = event {
            let (other_entity, is_player_involved) = if *entity1 == player_entity {
                (*entity2, true)
            } else if *entity2 == player_entity {
                (*entity1, true)
            } else {
                (Entity::PLACEHOLDER, false)
            };

            if is_player_involved {
                // Daño por contacto directo
                if let Ok(contact_damage) = contact_damage_query.get(other_entity) {
                    player_health.current =
                        player_health.current.saturating_sub(contact_damage.amount);
                    // Activar invencibilidad para el jugador
                    commands
                        .entity(player_entity)
                        .insert(Invincibility::new(1.5));
                }

                // Daño por proyectil
                if projectile_query.get(other_entity).is_ok() {
                    player_health.current = player_health.current.saturating_sub(1);
                    commands.entity(other_entity).despawn(); // Despawn proyectil
                    // Activar invencibilidad
                    commands
                        .entity(player_entity)
                        .insert(Invincibility::new(1.5));
                }
            }
        }
    }
}

/// Spawnea un proyectil.
fn spawn_projectile(commands: &mut Commands, position: Vec3, direction: Vec2) {
    commands.spawn((
        Sprite {
            color: Color::srgb(0.9, 0.1, 0.1),
            custom_size: Some(Vec2::splat(10.0)),
            ..default()
        },
        Transform::from_translation(position),
        EnemyProjectile,
        Velocity {
            velocity: direction * 400.0,
        }, // Velocidad del proyectil
        RigidBody::Dynamic,
        Collider::ball(5.0),
        ActiveEvents::COLLISION_EVENTS,
    ));
}
