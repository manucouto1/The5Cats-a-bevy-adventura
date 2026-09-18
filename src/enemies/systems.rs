use bevy::prelude::*;
use bevy_rapier2d::prelude::{
    ActiveCollisionTypes, ActiveEvents, Collider, ColliderDisabled, CollisionEvent, QueryFilter,
    ReadRapierContext, RigidBody, RigidBodyDisabled, Sensor, Velocity as RapierVelocity,
};

use crate::{
    enemies::components::{
        Active, BodyRadius, Chaser, ContactDamage, EnemyAssets, EnemyCharacter, EnemyKnockback,
        EnemyProjectile, EnemyState, EnemyType, FanShot, FinalBoss, IgnoreEdges, Patrol,
        ProjectileActive, ProjectileLifetime, TurretShot,
    },
    map::components::LevelTile,
    parallax::components::MainCamera,
    physics::FallingMode,
    physics::Velocity,
    player::components::{
        CharacterIdleSprite, CharacterLeftSprite, CharacterRightSprite, Health, Invincibility,
        Knockback, PlayerCharacter, PlayerHitGuard,
    },
};
use bevy_rapier2d::prelude::KinematicCharacterControllerOutput;

/// Margin (px) added to the visible viewport box for the wake-up check. Just
/// enough lead-in that an enemy is running its AI by the frame it slides on
/// screen, rather than appearing inert for a tick.
const ACTIVE_MARGIN: f32 = 80.0;
/// Margin for the dormant transition. The gap between this and `ACTIVE_MARGIN`
/// is the hysteresis band that keeps boundary entities from toggling each
/// frame, and ensures off-screen turrets/maniacs stop shooting from across
/// the level.
const DORMANT_MARGIN: f32 = 240.0;

/// Maintains the `Active` marker on enemies based on whether they fall inside
/// the camera's visible rectangle (extended by a margin). A circular radius
/// was wrong for our 16:9 viewport — it left enemies far above/below the
/// player active when they're nowhere near the screen.
pub fn activator_system(
    mut commands: Commands,
    camera: Query<(&Transform, &Projection), With<MainCamera>>,
    enemies: Query<(Entity, &Transform, Has<Active>, Has<FinalBoss>), With<EnemyCharacter>>,
) {
    let Ok((cam_tf, projection)) = camera.single() else {
        return;
    };
    let cam_pos = cam_tf.translation.truncate();
    let half_view = crate::parallax::components::view_half_extents(projection);
    let active_box = half_view + Vec2::splat(ACTIVE_MARGIN);
    let dormant_box = half_view + Vec2::splat(DORMANT_MARGIN);

    for (entity, tf, has_active, is_boss) in &enemies {
        // The boss is scripted (dive, hover, chase) and must keep running
        // even when the camera has panned away from it.
        if is_boss {
            if !has_active {
                commands.entity(entity).insert(Active);
            }
            continue;
        }
        let d = (tf.translation.truncate() - cam_pos).abs();
        let inside_active = d.x < active_box.x && d.y < active_box.y;
        let beyond_dormant = d.x > dormant_box.x || d.y > dormant_box.y;
        if has_active && beyond_dormant {
            commands.entity(entity).remove::<Active>();
        } else if !has_active && inside_active {
            commands.entity(entity).insert(Active);
        }
    }
}

pub const PROJECTILE_SPEED: f32 = 300.0;
/// Pygame kept at most 5 enemy bullets alive for aimed shots and 8 for
/// radial fans; that global budget is what made a screen full of turrets
/// survivable, so it's enforced here too.
pub const MAX_ENEMY_SHOTS_AIMED: usize = 5;
pub const MAX_ENEMY_SHOTS_FAN: usize = 8;
pub const PROJECTILE_LIFETIME_SECS: f32 = 3.0;
pub const PROJECTILE_POOL_CAPACITY: usize = 48;
/// Visual edge length in world px. Matches the hero's wool ball (`wool.png`
/// 64x64 rendered at scale 0.5 = 32px) so player and enemy projectiles read
/// at the same size on screen.
const PROJECTILE_VISUAL_SIZE: f32 = 32.0;
/// Collider radius in px. Matches the hero's `Collider::ball(8.0)`.
const PROJECTILE_COLLIDER_RADIUS: f32 = 8.0;
/// Render projectiles on the same plane as enemies/player so they're never
/// occluded by foreground tiles. Tile layers max out at z = 0.9 (hazards).
const PROJECTILE_Z: f32 = 5.0;

/// Object pool for enemy projectiles. Stores entity ids of idle (hidden +
/// rigid-body-disabled) projectile entities. Firing pops from here; expiry
/// pushes back. When empty, a fresh entity is spawned and added to the pool
/// on release — capacity grows to the high-water mark and never shrinks.
#[derive(Resource, Default)]
pub struct ProjectilePool {
    pub inactive: Vec<Entity>,
}

/// Components common to every pooled projectile, active or idle.
/// `RapierVelocity` is intentionally NOT included here — both spawn paths
/// (pool re-use and fresh spawn) insert it explicitly with the firing
/// linvel, and including it in the bundle caused a duplicate-component
/// panic when the fresh-spawn branch tried to set it.
fn projectile_base_bundle(image: Handle<Image>) -> impl Bundle {
    (
        Sprite {
            image,
            custom_size: Some(Vec2::splat(PROJECTILE_VISUAL_SIZE)),
            ..default()
        },
        EnemyProjectile,
        RigidBody::KinematicVelocityBased,
        Collider::ball(PROJECTILE_COLLIDER_RADIUS),
        Sensor,
        ActiveEvents::COLLISION_EVENTS,
        // Default ActiveCollisionTypes only reports pairs touching a Dynamic
        // body, so Kinematic-vs-Fixed (projectile-vs-ground) never fired and
        // shots cleared straight through walls. `all()` enables every pair.
        ActiveCollisionTypes::all(),
    )
}

/// Pre-allocates the pool on level entry so firing never pays the full spawn
/// cost during gameplay. Entities start idle (hidden + physics disabled).
pub fn setup_projectile_pool(
    mut commands: Commands,
    mut pool: ResMut<ProjectilePool>,
    enemy_assets: Res<EnemyAssets>,
) {
    pool.inactive.clear();
    for _ in 0..PROJECTILE_POOL_CAPACITY {
        let entity = commands
            .spawn((
                projectile_base_bundle(enemy_assets.projectile_texture.clone()),
                Transform::from_xyz(0.0, 0.0, 0.0),
                RapierVelocity::zero(),
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
    image: &Handle<Image>,
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
            projectile_base_bundle(image.clone()),
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
    time: Res<Time>,
    mut pool: ResMut<ProjectilePool>,
    mut hit_guard: ResMut<PlayerHitGuard>,
    mut collision_events: EventReader<CollisionEvent>,
    mut player_query: Query<
        (
            Entity,
            &mut Health,
            Option<&Invincibility>,
            &Transform,
            &mut Velocity,
        ),
        (
            With<PlayerCharacter>,
            Without<crate::player::components::Dead>,
        ),
    >,
    contact_damage_query: Query<(&ContactDamage, &Transform)>,
    projectile_query: Query<(Entity, &Transform), (With<EnemyProjectile>, With<ProjectileActive>)>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    let Ok((player_entity, mut player_health, invincibility, player_tf, mut player_vel)) =
        player_query.single_mut()
    else {
        // No player to hit (dying, or not spawned yet): drop whatever came
        // in rather than saving it for the respawn.
        collision_events.clear();
        return;
    };

    let now = time.elapsed_secs();
    if invincibility.is_some() || hit_guard.locked_until_secs > now {
        // Drain, don't skip. Returning without reading leaves every contact
        // made during the cooldown sitting in the reader's queue, and the
        // frame invincibility ends the player eats a hit from an enemy they
        // already ran away from. Damage taken while invincible is dropped,
        // exactly like the pygame hero re-testing its overlap each frame.
        collision_events.clear();
        return;
    }

    // The Invincibility insert below is deferred until commands flush, so
    // every event in this loop still observes `invincibility = None`. Without
    // bailing after the first applied hit, a 3-shot turret burst arriving in
    // one frame deducted 3 HP at once and the HUD just slowly caught up.
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
            hit_guard.locked_until_secs = now + HIT_INVINCIBILITY_SECS;
            commands
                .entity(player_entity)
                .insert(Invincibility::new(HIT_INVINCIBILITY_SECS));
            sfx.write(crate::audio::SfxEvent::HeroHit);
            return;
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
            hit_guard.locked_until_secs = now + HIT_INVINCIBILITY_SECS;
            commands
                .entity(player_entity)
                .insert(Invincibility::new(HIT_INVINCIBILITY_SECS));
            sfx.write(crate::audio::SfxEvent::HeroHit);
            return;
        }
    }
}

/// Cooldown after any hit that lands on the player. Long enough to ride out a
/// turret's 3-shot burst plus its in-burst spacing so a single engagement
/// can't drain the bar.
pub const HIT_INVINCIBILITY_SECS: f32 = 3.0;

/// Pushes the player away from a hit source. Horizontal direction is taken
/// from the source→player vector; vertical is always positive so the hit
/// reads as a pop-up rather than depending on the source being below. The
/// impulse is applied now *and* stored in `Knockback` so the input chain
/// keeps it alive for the stun duration.
pub fn apply_knockback(
    commands: &mut Commands,
    player_entity: Entity,
    player_vel: &mut Velocity,
    player_pos: Vec2,
    source_pos: Vec2,
) {
    let mut kb = Knockback::away_from(player_pos, source_pos);
    player_vel.velocity = kb.push;
    kb.applied = true;
    commands.entity(player_entity).insert(kb);
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
        (
            Entity,
            &Transform,
            &mut Patrol,
            &mut Velocity,
            &EnemyState,
            Has<IgnoreEdges>,
            &BodyRadius,
        ),
        (With<EnemyCharacter>, With<Active>, Without<EnemyKnockback>),
    >,
) {
    let Ok(ctx) = rapier_ctx.single() else {
        return;
    };

    const WALL_PROBE_DIST: f32 = 4.0;
    // Edge probe: cast down from just past the front foot; distance must exceed the
    // small gap between collider bottom and tile surface.
    const EDGE_LOOKAHEAD: f32 = 6.0;
    const EDGE_PROBE_DIST: f32 = 16.0;

    for (entity, transform, mut patrol, mut velocity, state, ignore_edges, radius) in &mut enemies {
        if *state != EnemyState::Patrolling {
            continue;
        }
        let half_extent = radius.0;

        let pos = transform.translation.truncate();
        let dir_sign = patrol.direction.signum() as f32;
        let filter = QueryFilter::default().exclude_collider(entity);

        let wall_origin = pos + Vec2::new(dir_sign * half_extent, 0.0);
        let wall_hit = ctx.cast_ray(
            wall_origin,
            Vec2::new(dir_sign, 0.0),
            WALL_PROBE_DIST,
            true,
            filter,
        );

        let edge_origin = pos + Vec2::new(dir_sign * (half_extent + EDGE_LOOKAHEAD), -half_extent);
        let edge_hit = ctx.cast_ray(
            edge_origin,
            Vec2::new(0.0, -1.0),
            EDGE_PROBE_DIST,
            true,
            filter,
        );

        // Walls always flip; edge flip only when not chasing a target below
        // (chase_system grants `IgnoreEdges` to let dummies fall off ledges).
        if wall_hit.is_some() || (edge_hit.is_none() && !ignore_edges) {
            patrol.direction = -patrol.direction;
        }

        velocity.velocity.x = patrol.direction.signum() as f32 * patrol.speed;
    }
}

/// Ticks `EnemyKnockback` timers and removes the component when they expire,
/// allowing patrol AI to resume. Guarded by `get_entity` so a knocked-back
/// enemy that was despawned the same frame doesn't trigger Bevy's "entity
/// does not exist" warning when its remove command flushes.
pub fn enemy_knockback_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut EnemyKnockback)>,
) {
    for (entity, mut kb) in query.iter_mut() {
        kb.timer.tick(time.delta());
        if kb.timer.finished() {
            if let Ok(mut entity_cmd) = commands.get_entity(entity) {
                entity_cmd.remove::<EnemyKnockback>();
            }
        }
    }
}

/// Dummy-only chase AI: when the player enters detection range, override
/// patrol direction toward the player and trigger a hop if they're elevated.
/// Does not run for non-Dummy enemies, and yields to `EnemyKnockback`.
pub fn dummy_chase_system(
    mut commands: Commands,
    rapier_ctx: ReadRapierContext,
    player_query: Query<&Transform, With<PlayerCharacter>>,
    mut dummies: Query<
        (
            Entity,
            &Transform,
            &mut Patrol,
            &mut Velocity,
            &EnemyType,
            Option<&KinematicCharacterControllerOutput>,
        ),
        (With<EnemyCharacter>, With<Active>, Without<EnemyKnockback>),
    >,
) {
    let Ok(player_tf) = player_query.single() else {
        return;
    };
    let Ok(_ctx) = rapier_ctx.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    const CHASE_RANGE: f32 = 320.0;
    const CHASE_SPEED_MULT: f32 = 1.6;
    const JUMP_VELOCITY: f32 = 380.0;
    const JUMP_TRIGGER_HEIGHT: f32 = 16.0;
    /// Player must be at least this far below the dummy before we authorise
    /// dropping off a ledge to chase. Avoids walking off platforms when the
    /// player is roughly level (e.g. on the same height across a small gap).
    const DROP_THRESHOLD: f32 = 24.0;

    for (entity, transform, mut patrol, mut velocity, kind, output) in &mut dummies {
        if *kind != EnemyType::Dummy {
            continue;
        }
        let pos = transform.translation.truncate();
        let to_player = player_pos - pos;
        if to_player.length() > CHASE_RANGE {
            // Out of range: clear the drop-off override so patrol resumes
            // normal edge-aware behaviour next frame.
            commands.entity(entity).remove::<IgnoreEdges>();
            continue;
        }

        // When the player is right under us, `to_player.x` may be ~0; keep the
        // current patrol direction in that case so the dummy keeps walking
        // toward the nearest ledge instead of freezing.
        let dir = if to_player.x.abs() > f32::EPSILON {
            to_player.x.signum()
        } else {
            patrol.direction.signum() as f32
        };
        patrol.direction = dir as i32;
        velocity.velocity.x = dir * patrol.speed * CHASE_SPEED_MULT;

        let grounded = output.is_some_and(|o| o.grounded);
        if grounded && to_player.y > JUMP_TRIGGER_HEIGHT {
            velocity.velocity.y = JUMP_VELOCITY;
        }

        // Player below: walk off ledges. Patrol's edge probe would otherwise
        // bounce us back at every drop and the dummy would pace uselessly on
        // the upper platform while the player escapes underneath.
        if to_player.y < -DROP_THRESHOLD {
            commands.entity(entity).insert(IgnoreEdges);
        } else {
            commands.entity(entity).remove::<IgnoreEdges>();
        }
    }
}

/// Enemies tagged `Chaser` walk toward the player and hop when the player
/// is above them. Skipped while knocked back or in the free-fall shaft
/// (where `enemy_falling_mode_system` steers instead).
pub fn chaser_system(
    time: Res<Time>,
    player: Query<
        &Transform,
        (
            With<PlayerCharacter>,
            Without<crate::player::components::Dead>,
        ),
    >,
    mut chasers: Query<
        (
            &Transform,
            &mut Velocity,
            &mut Chaser,
            Option<&KinematicCharacterControllerOutput>,
        ),
        (
            With<EnemyCharacter>,
            With<Active>,
            Without<EnemyKnockback>,
            Without<FallingMode>,
        ),
    >,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    for (tf, mut velocity, mut chaser, output) in &mut chasers {
        chaser.jump_cooldown.tick(time.delta());
        let pos = tf.translation.truncate();
        let dx = player_pos.x - pos.x;
        // Keep a little distance so it doesn't just sit inside the player.
        velocity.velocity.x = if dx.abs() > 28.0 {
            dx.signum() * chaser.speed
        } else {
            0.0
        };
        let grounded = output.is_some_and(|o| o.grounded);
        if grounded && player_pos.y > pos.y + 40.0 && chaser.jump_cooldown.finished() {
            velocity.velocity.y = 420.0;
            chaser.jump_cooldown.reset();
        }
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
            *vis = if should_show {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
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

/// Releases an active projectile back to the pool the moment it touches any
/// level tile. Projectiles are sensors (so they don't push dynamic platforms),
/// which means rapier won't stop them physically — without this, shots
/// travelled through walls and hit the player from across the map.
pub fn projectile_terrain_collision_system(
    mut commands: Commands,
    mut pool: ResMut<ProjectilePool>,
    mut collisions: EventReader<CollisionEvent>,
    projectiles: Query<Entity, (With<EnemyProjectile>, With<ProjectileActive>)>,
    tiles: Query<(), With<LevelTile>>,
) {
    for event in collisions.read() {
        let CollisionEvent::Started(e1, e2, _) = event else {
            continue;
        };
        let (proj, other) = if projectiles.get(*e1).is_ok() {
            (*e1, *e2)
        } else if projectiles.get(*e2).is_ok() {
            (*e2, *e1)
        } else {
            continue;
        };
        if tiles.get(other).is_ok() {
            release_projectile(&mut commands, &mut pool, proj);
        }
    }
}

/// Fufi AI: burst of `shots_per_burst` aimed shots, then a long cooldown.
/// Only engages when the player is within `range`.
pub fn turret_shot_system(
    time: Res<Time>,
    mut commands: Commands,
    mut pool: ResMut<ProjectilePool>,
    enemy_assets: Res<EnemyAssets>,
    mut turrets: Query<
        (&mut TurretShot, &Transform, &BodyRadius),
        (With<EnemyCharacter>, With<Active>),
    >,
    player: Query<
        &Transform,
        (
            With<PlayerCharacter>,
            Without<crate::player::components::Dead>,
        ),
    >,
    active_shots: Query<(), With<ProjectileActive>>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    let mut in_flight = active_shots.iter().count();

    for (mut shot, tf, radius) in &mut turrets {
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

        if shot.shots_remaining > 0
            && shot.burst_interval_timer.finished()
            && in_flight < MAX_ENEMY_SHOTS_AIMED
        {
            let dir = (player_pos - pos).normalize_or_zero();
            if dir != Vec2::ZERO {
                in_flight += 1;
                // Spawn just outside the enemy's collider so the projectile
                // doesn't overlap its origin and trigger a self-collision.
                let spawn_pos = pos + dir * (radius.0 + 6.0);
                fire_projectile(
                    &mut commands,
                    &mut pool,
                    &enemy_assets.projectile_texture,
                    spawn_pos.extend(PROJECTILE_Z),
                    dir,
                );
                sfx.write(crate::audio::SfxEvent::EnemyShoot);
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
    enemy_assets: Res<EnemyAssets>,
    mut shooters: Query<
        (&mut FanShot, &Transform, &BodyRadius),
        (With<EnemyCharacter>, With<Active>),
    >,
    player: Query<
        &Transform,
        (
            With<PlayerCharacter>,
            Without<crate::player::components::Dead>,
        ),
    >,
    active_shots: Query<(), With<ProjectileActive>>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    let mut in_flight = active_shots.iter().count();

    for (mut shot, tf, radius) in &mut shooters {
        shot.cooldown_timer.tick(time.delta());

        let pos = tf.translation.truncate();
        if pos.distance(player_pos) > shot.range {
            continue;
        }

        if shot.cooldown_timer.just_finished() && in_flight < MAX_ENEMY_SHOTS_FAN {
            in_flight += shot.rays as usize;
            sfx.write(crate::audio::SfxEvent::EnemyShoot);
            let rays = shot.rays.max(1) as f32;
            for i in 0..shot.rays {
                let angle = (i as f32) * std::f32::consts::TAU / rays;
                let dir = Vec2::new(angle.cos(), angle.sin());
                let spawn_pos = pos + dir * (radius.0 + 6.0);
                fire_projectile(
                    &mut commands,
                    &mut pool,
                    &enemy_assets.projectile_texture,
                    spawn_pos.extend(PROJECTILE_Z),
                    dir,
                );
            }
        }
    }
}
