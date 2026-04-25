use bevy::prelude::*;
use bevy_rapier2d::prelude::{
    ActiveEvents, Collider, ColliderDisabled, CollisionEvent, QueryFilter, ReadRapierContext,
    RigidBody, RigidBodyDisabled, Sensor, Velocity as RapierVelocity,
};

use crate::{
    enemies::components::{
        Active, ContactDamage, EnemyCharacter, EnemyProjectile, EnemyState, FanShot, Patrol,
        ProjectileActive, ProjectileLifetime, TurretShot,
    },
    parallax::components::MainCamera,
    physics::Velocity,
    player::components::{
        CharacterIdleSprite, CharacterLeftSprite, CharacterRightSprite, Health, Invincibility,
        Knockback, PlayerCharacter,
    },
};

/// Distance at which a dormant enemy wakes up.
pub const ACTIVATION_RADIUS: f32 = 800.0;
/// Distance at which an active enemy goes dormant. Larger than the
/// activation radius so entities on the boundary don't toggle every frame.
pub const DEACTIVATION_RADIUS: f32 = 1000.0;

/// Maintains the `Active` marker on enemies based on camera distance, with
/// hysteresis. Active enemies run AI; dormant ones are invisible to per-frame
/// systems that filter on `With<Active>`.
pub fn activator_system(
    mut commands: Commands,
    camera: Query<&Transform, With<MainCamera>>,
    enemies: Query<(Entity, &Transform, Has<Active>), With<EnemyCharacter>>,
) {
    let Ok(cam_tf) = camera.single() else {
        return;
    };
    let cam_pos = cam_tf.translation.truncate();

    for (entity, tf, has_active) in &enemies {
        let distance = tf.translation.truncate().distance(cam_pos);
        if has_active && distance > DEACTIVATION_RADIUS {
            commands.entity(entity).remove::<Active>();
        } else if !has_active && distance < ACTIVATION_RADIUS {
            commands.entity(entity).insert(Active);
        }
    }
}

pub const PROJECTILE_SPEED: f32 = 300.0;
pub const PROJECTILE_LIFETIME_SECS: f32 = 3.0;
pub const PROJECTILE_POOL_CAPACITY: usize = 48;
const PROJECTILE_VISUAL_SIZE: f32 = 10.0;
const PROJECTILE_COLLIDER_RADIUS: f32 = 5.0;

/// Object pool for enemy projectiles. Stores entity ids of idle (hidden +
/// rigid-body-disabled) projectile entities. Firing pops from here; expiry
/// pushes back. When empty, a fresh entity is spawned and added to the pool
/// on release — capacity grows to the high-water mark and never shrinks.
#[derive(Resource, Default)]
pub struct ProjectilePool {
    pub inactive: Vec<Entity>,
}

/// Components common to every pooled projectile, active or idle.
fn projectile_base_bundle() -> impl Bundle {
    (
        Sprite {
            color: Color::srgb(0.9, 0.1, 0.1),
            custom_size: Some(Vec2::splat(PROJECTILE_VISUAL_SIZE)),
            ..default()
        },
        EnemyProjectile,
        RigidBody::KinematicVelocityBased,
        RapierVelocity::zero(),
        Collider::ball(PROJECTILE_COLLIDER_RADIUS),
        Sensor,
        ActiveEvents::COLLISION_EVENTS,
    )
}

/// Pre-allocates the pool on level entry so firing never pays the full spawn
/// cost during gameplay. Entities start idle (hidden + physics disabled).
pub fn setup_projectile_pool(mut commands: Commands, mut pool: ResMut<ProjectilePool>) {
    pool.inactive.clear();
    for _ in 0..PROJECTILE_POOL_CAPACITY {
        let entity = commands
            .spawn((
                projectile_base_bundle(),
                Transform::from_xyz(0.0, 0.0, 0.0),
                Visibility::Hidden,
                RigidBodyDisabled,
                ColliderDisabled,
            ))
            .id();
        pool.inactive.push(entity);
    }
}

/// Despawns every pooled projectile entity (both idle and active) when the
/// game state exits. Called from a system that has access to the full entity
/// set — see `teardown_projectile_pool`.
pub fn teardown_projectile_pool(
    mut commands: Commands,
    mut pool: ResMut<ProjectilePool>,
    projectiles: Query<Entity, With<EnemyProjectile>>,
) {
    for entity in &projectiles {
        commands.entity(entity).despawn();
    }
    pool.inactive.clear();
}

/// Acquire a projectile from the pool and launch it. Falls back to spawning
/// a fresh entity when the pool is empty; that entity returns to the pool on
/// release, so the pool capacity rises to the concurrent-projectile peak.
pub fn fire_projectile(
    commands: &mut Commands,
    pool: &mut ProjectilePool,
    position: Vec3,
    direction: Vec2,
) {
    let dir = direction.normalize_or_zero();
    if dir == Vec2::ZERO {
        return;
    }
    let linvel = dir * PROJECTILE_SPEED;
    let lifetime = ProjectileLifetime(Timer::from_seconds(
        PROJECTILE_LIFETIME_SECS,
        TimerMode::Once,
    ));

    if let Some(entity) = pool.inactive.pop() {
        commands
            .entity(entity)
            .insert((
                Transform::from_translation(position),
                RapierVelocity {
                    linvel,
                    angvel: 0.0,
                },
                Visibility::Visible,
                ProjectileActive,
                lifetime,
            ))
            .remove::<RigidBodyDisabled>()
            .remove::<ColliderDisabled>();
    } else {
        commands.spawn((
            projectile_base_bundle(),
            Transform::from_translation(position),
            RapierVelocity {
                linvel,
                angvel: 0.0,
            },
            Visibility::Visible,
            ProjectileActive,
            lifetime,
        ));
    }
}

/// Return a projectile entity to the pool: hide it, disable physics, and
/// queue it for reuse. Strips `ProjectileActive` and `ProjectileLifetime` so
/// per-frame systems skip it while idle.
fn release_projectile(commands: &mut Commands, pool: &mut ProjectilePool, entity: Entity) {
    pool.inactive.push(entity);
    commands
        .entity(entity)
        .insert((
            Visibility::Hidden,
            RigidBodyDisabled,
            ColliderDisabled,
            RapierVelocity::zero(),
        ))
        .remove::<ProjectileActive>()
        .remove::<ProjectileLifetime>();
}

pub fn enemy_damage_system(
    mut commands: Commands,
    mut pool: ResMut<ProjectilePool>,
    mut collision_events: EventReader<CollisionEvent>,
    mut player_query: Query<
        (
            Entity,
            &mut Health,
            Option<&Invincibility>,
            &Transform,
            &mut Velocity,
        ),
        With<PlayerCharacter>,
    >,
    contact_damage_query: Query<(&ContactDamage, &Transform)>,
    projectile_query: Query<(Entity, &Transform), (With<EnemyProjectile>, With<ProjectileActive>)>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    let Ok((player_entity, mut player_health, invincibility, player_tf, mut player_vel)) =
        player_query.single_mut()
    else {
        return;
    };

    if invincibility.is_some() {
        return;
    }

    for event in collision_events.read() {
        let CollisionEvent::Started(entity1, entity2, _) = event else {
            continue;
        };
        let other_entity = if *entity1 == player_entity {
            *entity2
        } else if *entity2 == player_entity {
            *entity1
        } else {
            continue;
        };

        if let Ok((contact_damage, enemy_tf)) = contact_damage_query.get(other_entity) {
            player_health.current = player_health.current.saturating_sub(contact_damage.amount);
            apply_knockback(
                &mut commands,
                player_entity,
                &mut player_vel,
                player_tf.translation.truncate(),
                enemy_tf.translation.truncate(),
            );
            commands
                .entity(player_entity)
                .insert(Invincibility::new(1.5));
            sfx.write(crate::audio::SfxEvent::HeroHit);
        }

        if let Ok((projectile_entity, proj_tf)) = projectile_query.get(other_entity) {
            player_health.current = player_health.current.saturating_sub(1);
            apply_knockback(
                &mut commands,
                player_entity,
                &mut player_vel,
                player_tf.translation.truncate(),
                proj_tf.translation.truncate(),
            );
            release_projectile(&mut commands, &mut pool, projectile_entity);
            commands
                .entity(player_entity)
                .insert(Invincibility::new(1.5));
            sfx.write(crate::audio::SfxEvent::HeroHit);
        }
    }
}

/// Pushes the player away from a hit source. Horizontal direction is taken
/// from the source→player vector; vertical is always positive so the hit
/// reads as a pop-up rather than depending on the source being below.
fn apply_knockback(
    commands: &mut Commands,
    player_entity: Entity,
    player_vel: &mut Velocity,
    player_pos: Vec2,
    source_pos: Vec2,
) {
    const KNOCKBACK_X: f32 = 280.0;
    const KNOCKBACK_Y: f32 = 280.0;
    const KNOCKBACK_DURATION: f32 = 0.25;

    let dx = player_pos.x - source_pos.x;
    // Fall back to a fixed sign when player is exactly above the source so
    // the push is never zero (which would feel like the hit had no effect).
    let dir_x = if dx.abs() > f32::EPSILON {
        dx.signum()
    } else {
        1.0
    };
    player_vel.velocity.x = dir_x * KNOCKBACK_X;
    player_vel.velocity.y = KNOCKBACK_Y;
    commands
        .entity(player_entity)
        .insert(Knockback::new(KNOCKBACK_DURATION));
}

/// Patrol AI: walk in a direction, flip on wall or when the floor ahead disappears.
///
/// Uses two raycasts:
/// - Wall probe: horizontal, from enemy center, just past its collider.
/// - Edge probe: vertical, one tile-step ahead of the front foot, pointing down.
///
/// If the wall probe hits or the edge probe misses, the direction flips.
pub fn patrol_system(
    rapier_ctx: ReadRapierContext,
    mut enemies: Query<
        (Entity, &Transform, &mut Patrol, &mut Velocity, &EnemyState),
        (With<EnemyCharacter>, With<Active>),
    >,
) {
    let Ok(ctx) = rapier_ctx.single() else {
        return;
    };

    // Enemy collider is Collider::ball(16.0) — see spawn_enemies_characters.
    const HALF_EXTENT: f32 = 16.0;
    const WALL_PROBE_DIST: f32 = 4.0;
    // Edge probe: cast down from just past the front foot; distance must exceed the
    // small gap between collider bottom and tile surface.
    const EDGE_LOOKAHEAD: f32 = 6.0;
    const EDGE_PROBE_DIST: f32 = 16.0;

    for (entity, transform, mut patrol, mut velocity, state) in &mut enemies {
        if *state != EnemyState::Patrolling {
            continue;
        }

        let pos = transform.translation.truncate();
        let dir_sign = patrol.direction.signum() as f32;
        let filter = QueryFilter::default().exclude_collider(entity);

        let wall_origin = pos + Vec2::new(dir_sign * HALF_EXTENT, 0.0);
        let wall_hit =
            ctx.cast_ray(wall_origin, Vec2::new(dir_sign, 0.0), WALL_PROBE_DIST, true, filter);

        let edge_origin =
            pos + Vec2::new(dir_sign * (HALF_EXTENT + EDGE_LOOKAHEAD), -HALF_EXTENT);
        let edge_hit =
            ctx.cast_ray(edge_origin, Vec2::new(0.0, -1.0), EDGE_PROBE_DIST, true, filter);

        if wall_hit.is_some() || edge_hit.is_none() {
            patrol.direction = -patrol.direction;
        }

        velocity.velocity.x = patrol.direction.signum() as f32 * patrol.speed;
    }
}

/// Syncs an enemy's left/right/idle child-sprite visibility with its current
/// horizontal movement direction. Walking sprites are swapped by direction;
/// the idle sprite only shows when horizontal velocity is effectively zero.
pub fn enemy_facing_sprite_system(
    enemies: Query<(&Velocity, &Children), (With<EnemyCharacter>, With<Active>)>,
    mut sprites: Query<(
        &mut Visibility,
        Option<&CharacterLeftSprite>,
        Option<&CharacterRightSprite>,
        Option<&CharacterIdleSprite>,
    )>,
) {
    for (velocity, children) in &enemies {
        let vx = velocity.velocity.x;
        let moving = vx.abs() > 1.0;
        for child in children.iter() {
            let Ok((mut vis, left, right, idle)) = sprites.get_mut(child) else {
                continue;
            };
            let should_show = if left.is_some() {
                moving && vx < 0.0
            } else if right.is_some() {
                moving && vx > 0.0
            } else if idle.is_some() {
                !moving
            } else {
                continue;
            };
            *vis = if should_show { Visibility::Visible } else { Visibility::Hidden };
        }
    }
}

/// Returns expired active projectiles to the pool. Idle pool entries do not
/// carry `ProjectileLifetime`, so they're automatically skipped.
pub fn projectile_lifetime_system(
    time: Res<Time>,
    mut commands: Commands,
    mut pool: ResMut<ProjectilePool>,
    mut projectiles: Query<(Entity, &mut ProjectileLifetime), With<ProjectileActive>>,
) {
    for (entity, mut lifetime) in &mut projectiles {
        lifetime.0.tick(time.delta());
        if lifetime.0.finished() {
            release_projectile(&mut commands, &mut pool, entity);
        }
    }
}

/// Fufi AI: burst of `shots_per_burst` aimed shots, then a long cooldown.
/// Only engages when the player is within `range`.
pub fn turret_shot_system(
    time: Res<Time>,
    mut commands: Commands,
    mut pool: ResMut<ProjectilePool>,
    mut turrets: Query<(&mut TurretShot, &Transform), (With<EnemyCharacter>, With<Active>)>,
    player: Query<&Transform, With<PlayerCharacter>>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (mut shot, tf) in &mut turrets {
        let pos = tf.translation.truncate();

        if pos.distance(player_pos) > shot.range {
            continue;
        }

        shot.burst_interval_timer.tick(time.delta());
        shot.cooldown_timer.tick(time.delta());

        // Start a new burst once the long cooldown has elapsed.
        if shot.shots_remaining == 0 && shot.cooldown_timer.finished() {
            shot.shots_remaining = shot.shots_per_burst;
            shot.burst_interval_timer.reset();
        }

        if shot.shots_remaining > 0 && shot.burst_interval_timer.finished() {
            let dir = (player_pos - pos).normalize_or_zero();
            if dir != Vec2::ZERO {
                // Spawn just outside the enemy's collider so the projectile
                // doesn't overlap its origin and trigger a self-collision.
                let spawn_pos = pos + dir * 20.0;
                fire_projectile(&mut commands, &mut pool, spawn_pos.extend(0.0), dir);
                shot.shots_remaining -= 1;
                shot.burst_interval_timer.reset();
                if shot.shots_remaining == 0 {
                    shot.cooldown_timer.reset();
                }
            }
        }
    }
}

/// Catcifer AI: fires `rays` projectiles radially every `cooldown`.
pub fn fan_shot_system(
    time: Res<Time>,
    mut commands: Commands,
    mut pool: ResMut<ProjectilePool>,
    mut shooters: Query<(&mut FanShot, &Transform), (With<EnemyCharacter>, With<Active>)>,
    player: Query<&Transform, With<PlayerCharacter>>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (mut shot, tf) in &mut shooters {
        shot.cooldown_timer.tick(time.delta());

        let pos = tf.translation.truncate();
        if pos.distance(player_pos) > shot.range {
            continue;
        }

        if shot.cooldown_timer.just_finished() {
            let rays = shot.rays.max(1) as f32;
            for i in 0..shot.rays {
                let angle = (i as f32) * std::f32::consts::TAU / rays;
                let dir = Vec2::new(angle.cos(), angle.sin());
                let spawn_pos = pos + dir * 20.0;
                fire_projectile(&mut commands, &mut pool, spawn_pos.extend(0.0), dir);
            }
        }
    }
}
