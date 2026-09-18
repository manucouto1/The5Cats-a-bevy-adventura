use bevy::{prelude::*, window::PrimaryWindow};
use bevy_rapier2d::prelude::{
    ActiveEvents, Collider, CollisionEvent, GravityScale, RigidBody, Sleeping, Velocity,
};

use crate::{
    collectibles::components::{Collectible, ManiacBuff, SpawnCollectibleEvent},
    cursor::{
        assets::CursorAssets,
        components::{
            Crosshair, Projectile, WOOL_BALL_IMPACT_LINGER, WOOL_BALL_LIFETIME, WoolBall,
        },
    },
    enemies::components::{
        Chaser, EnemyCharacter, EnemyKilledEvent, EnemyKnockback, EnemyType, FanShot, FinalBoss,
        Patrol,
    },
    hud::PointerOverUi,
    map::zones::{BossFallArmed, FALLING_SPEED, LevelMode, LevelZones, falling_zone_center_x},
    parallax::components::MainCamera,
    physics::{AffectedByGravity, FallingMode, Mass},
    player::components::{Dead, Health, PlayerCharacter},
};

/// Descent speed for the boss while it free-falls down the shaft in phase 2.
/// Matches the player's so the two fall side by side through the gauntlet.
const BOSS_FALL_SPEED: f32 = FALLING_SPEED;

/// Pygame caps: 5 wool balls in flight normally, 8 while in maniac mode.
const MAX_WOOL_BALLS: usize = 5;
const MAX_WOOL_BALLS_MANIAC: usize = 8;
const WOOL_SPEED: f32 = 1000.0;
const MANIAC_RAYS: u32 = 16;
const MANIAC_RAY_SPEED: f32 = 520.0;

/// Wool balls spawn `32.0` px along the firing line so the player does not
/// collide with their own shot — which means an enemy closer than that gets
/// stepped over and takes nothing. Inside this radius the click becomes a
/// melee swipe instead.
const MELEE_RANGE: f32 = 46.0;
/// Shove applied by a hit. The melee swipe pushes harder horizontally than a
/// wool ball: its whole point is to get the enemy off you.
const KB_X: f32 = 220.0;
const KB_Y: f32 = 260.0;
const MELEE_KB_X: f32 = 360.0;
const MELEE_KB_Y: f32 = 240.0;
const KB_DURATION: f32 = 0.3;

pub fn update_aim_assist(
    window: Single<&Window, With<PrimaryWindow>>,
    // Must be the main camera specifically: the lighting pipeline adds a
    // `Camera2d` per off-screen pass, and a bare `With<Camera2d>` query
    // stopped resolving — which froze the crosshair in place.
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut crosshair_query: Query<&mut Transform, With<Crosshair>>,
) {
    if let Some(position) = window.cursor_position() {
        if let Ok((camera, camera_transform)) = camera_query.single() {
            if let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, position) {
                for mut crosshair_transform in crosshair_query.iter_mut() {
                    crosshair_transform.translation.x = world_position.x;
                    crosshair_transform.translation.y = world_position.y;
                }
            }
        }
    }
}

fn solve_ballistic_velocity(
    start: Vec2,
    target: Vec2,
    speed: f32,
    gravity: f32,
    prefer_high_arc: bool,
) -> Option<Vec2> {
    let dx = target.x - start.x;
    let dy = target.y - start.y;

    if dx.abs() < 1e-6 {
        let vy = if dy > 0.0 { speed } else { -speed };
        return Some(Vec2::new(0.0, vy));
    }

    let v2 = speed * speed;
    let dx2 = dx * dx;
    let g = gravity;

    let inside = v2 * v2 - g * (g * dx2 + 2.0 * dy * v2);
    if inside < 0.0 {
        return None;
    }

    let sqrt_val = inside.sqrt();
    let numerator = if prefer_high_arc {
        v2 + sqrt_val
    } else {
        v2 - sqrt_val
    };
    let tan_theta = numerator / (g * dx.abs());
    let theta = tan_theta.atan();

    let direction_x_sign = dx.signum();

    let vx = speed * theta.cos() * direction_x_sign;
    let vy = speed * theta.sin();

    Some(Vec2::new(vx, vy))
}

fn spawn_wool_ball(
    commands: &mut Commands,
    image: &Handle<Image>,
    position: Vec2,
    velocity: Vec2,
    gravity_scale: f32,
) {
    commands.spawn((
        Sprite {
            image: image.clone(),
            custom_size: Some(Vec2::splat(32.0)),
            ..default()
        },
        Transform::from_translation(position.extend(98.0)),
        WoolBall,
        Projectile {
            despawn_timer: Timer::from_seconds(WOOL_BALL_LIFETIME, TimerMode::Once),
            has_collided: false,
        },
        Collider::ball(8.0),
        RigidBody::Dynamic,
        Velocity {
            linvel: velocity,
            angvel: 6.0,
        },
        GravityScale(gravity_scale),
        ActiveEvents::COLLISION_EVENTS,
        AffectedByGravity,
        Mass { kilograms: 10.0 },
        Sleeping::default(),
    ));
}

/// One point of damage to a regular enemy, a shove away from `from_x`, and
/// the kill bookkeeping when it drops to zero. Shared by wool-ball impacts
/// and the point-blank melee so both react identically.
fn hit_enemy(
    commands: &mut Commands,
    enemy: Entity,
    enemy_tf: &Transform,
    health: &mut Health,
    kind: &EnemyType,
    velocity: &mut crate::physics::Velocity,
    from_x: Option<f32>,
    push: Vec2,
    killed: &mut EventWriter<EnemyKilledEvent>,
    sfx: &mut EventWriter<crate::audio::SfxEvent>,
) {
    health.current = health.current.saturating_sub(1);
    if health.current == 0 {
        sfx.write(crate::audio::SfxEvent::DestroyEnemy);
        killed.write(EnemyKilledEvent {
            position: enemy_tf.translation,
            kind: kind.clone(),
        });
        commands.entity(enemy).despawn();
        return;
    }
    // Pop the enemy in the direction of the impact so it visibly reacts to
    // being hit (turrets etc. used to ignore hits).
    let dir_x = from_x
        .map(|x| {
            let dx = enemy_tf.translation.x - x;
            if dx.abs() > f32::EPSILON {
                dx.signum()
            } else {
                1.0
            }
        })
        .unwrap_or(1.0);
    velocity.velocity.x = dir_x * push.x;
    velocity.velocity.y = push.y;
    commands
        .entity(enemy)
        .insert(EnemyKnockback::new(KB_DURATION));
    // A hit maniac comes after you (pygame never reset its movement once hit).
    if *kind == EnemyType::Catcifer {
        commands.entity(enemy).insert(Chaser::default());
    }
    sfx.write(crate::audio::SfxEvent::EnemyHit);
}

/// Left click throws a wool ball at the crosshair — unless an enemy is
/// already on top of the player, in which case it becomes a melee swipe
/// (see `MELEE_RANGE`). Enforces the in-flight cap, ignores clicks aimed at
/// HUD buttons, and while maniac mode is on also fires a 16-ray radial
/// burst like the original power-up.
pub fn spawn_projectile_on_click(
    mut commands: Commands,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    cursor_assets: Res<CursorAssets>,
    pointer_over_ui: Res<PointerOverUi>,
    player_query: Query<(&Transform, Has<ManiacBuff>), (With<PlayerCharacter>, Without<Dead>)>,
    crosshair_query: Query<&Transform, With<Crosshair>>,
    wool_balls: Query<(), With<WoolBall>>,
    // Bosses keep their own hit pipeline (phases + cooldown) and are big
    // enough that point-blank shots connect, so the swipe skips them.
    mut melee_targets: Query<
        (
            Entity,
            &Transform,
            &mut Health,
            &EnemyType,
            &mut crate::physics::Velocity,
        ),
        (
            With<EnemyCharacter>,
            Without<FinalBoss>,
            Without<PlayerCharacter>,
        ),
    >,
    mut killed: EventWriter<EnemyKilledEvent>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    if !mouse_button_input.just_pressed(MouseButton::Left) || pointer_over_ui.0 {
        return;
    }
    let Ok((player_transform, maniac)) = player_query.single() else {
        return;
    };
    let Ok(crosshair_transform) = crosshair_query.single() else {
        return;
    };

    let start_pos = player_transform.translation.xy();
    let target_pos = crosshair_transform.translation.xy();

    // Point-blank: whack the nearest enemy instead of throwing through it.
    // Checked before the in-flight cap — a swipe costs no wool ball.
    let closest = melee_targets
        .iter()
        .filter(|(_, tf, ..)| tf.translation.xy().distance(start_pos) <= MELEE_RANGE)
        .min_by(|(_, a, ..), (_, b, ..)| {
            a.translation
                .xy()
                .distance_squared(start_pos)
                .total_cmp(&b.translation.xy().distance_squared(start_pos))
        })
        .map(|(entity, ..)| entity);
    let mut melee_landed = false;
    if let Some(entity) = closest {
        if let Ok((entity, tf, mut health, kind, mut velocity)) = melee_targets.get_mut(entity) {
            let tf = *tf;
            // Shove away from the player, or towards the aim when the two
            // are stacked on the exact same column.
            let from_x = if (tf.translation.x - start_pos.x).abs() > f32::EPSILON {
                Some(start_pos.x)
            } else {
                Some(target_pos.x)
            };
            hit_enemy(
                &mut commands,
                entity,
                &tf,
                &mut health,
                kind,
                &mut velocity,
                from_x,
                Vec2::new(MELEE_KB_X, MELEE_KB_Y),
                &mut killed,
                &mut sfx,
            );
            melee_landed = true;
        }
    }

    // The radial burst is not aimed, so maniac mode still gets it even when
    // the click resolved as a swipe.
    let cap = if maniac {
        MAX_WOOL_BALLS_MANIAC
    } else {
        MAX_WOOL_BALLS
    };
    if wool_balls.iter().count() >= cap || (melee_landed && !maniac) {
        return;
    }

    let gravity = 9.81 * 32.0; // px/s², same as the Rapier world gravity

    let Some(velocity) =
        solve_ballistic_velocity(start_pos, target_pos, WOOL_SPEED, gravity, false)
    else {
        return;
    };
    if !melee_landed {
        sfx.write(crate::audio::SfxEvent::Shoot);
        let offset_pos = start_pos + velocity.normalize() * 32.0;
        spawn_wool_ball(
            &mut commands,
            &cursor_assets.wool_image,
            offset_pos,
            velocity,
            1.0,
        );
    }

    if maniac {
        for i in 0..MANIAC_RAYS {
            let angle = i as f32 * std::f32::consts::TAU / MANIAC_RAYS as f32;
            let dir = Vec2::new(angle.cos(), angle.sin());
            spawn_wool_ball(
                &mut commands,
                &cursor_assets.wool_image,
                start_pos + dir * 30.0,
                dir * MANIAC_RAY_SPEED,
                0.15,
            );
        }
    }
}

/// Wool-ball ↔ enemy damage. For every `CollisionEvent::Started` between a
/// wool-ball projectile and an `EnemyCharacter`, subtracts 1 HP from the
/// enemy and despawns the wool ball. If the hit drops the enemy to 0 HP,
/// emits `EnemyKilledEvent` (consumed by the collectibles plugin) and
/// despawns the enemy.
pub fn wool_ball_damage_system(
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    wool_balls: Query<(Entity, &Transform), With<WoolBall>>,
    // Bosses have their own damage pipeline (phases, hit-cooldown) — skip them here.
    mut enemies: Query<
        (
            &Transform,
            &mut Health,
            &EnemyType,
            &mut crate::physics::Velocity,
        ),
        (With<EnemyCharacter>, Without<FinalBoss>),
    >,
    mut killed: EventWriter<EnemyKilledEvent>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    for event in collision_events.read() {
        let CollisionEvent::Started(e1, e2, _) = event else {
            continue;
        };
        let (wool, enemy) = if wool_balls.get(*e1).is_ok() && enemies.get(*e2).is_ok() {
            (*e1, *e2)
        } else if wool_balls.get(*e2).is_ok() && enemies.get(*e1).is_ok() {
            (*e2, *e1)
        } else {
            continue;
        };

        let wool_pos = wool_balls
            .get(wool)
            .map(|(_, tf)| tf.translation.truncate())
            .ok();
        let Ok((tf, mut health, kind, mut velocity)) = enemies.get_mut(enemy) else {
            continue;
        };
        commands.entity(wool).despawn();
        let tf = *tf;
        hit_enemy(
            &mut commands,
            enemy,
            &tf,
            &mut health,
            kind,
            &mut velocity,
            wool_pos.map(|wp| wp.x),
            Vec2::new(KB_X, KB_Y),
            &mut killed,
            &mut sfx,
        );
    }
}

/// Boss damage pipeline. Has three differences from the regular enemy
/// version: (1) rate-limited via `FinalBoss.hit_cooldown` so the player
/// can't stun-lock with rapid fire, (2) death cycles through 3 HP phases
/// (18 → 36 → 18) before actually dying, (3) on final death emits
/// `LevelCompleteEvent` (which is the level-4 victory trigger via the
/// existing `handle_level_complete` — it sees Level4.next() is None and
/// returns to the main menu).
pub fn boss_damage_system(
    mut commands: Commands,
    time: Res<Time>,
    mut collision_events: EventReader<CollisionEvent>,
    wool_balls: Query<Entity, With<WoolBall>>,
    mut bosses: Query<(&mut Transform, &mut Health, &mut FinalBoss), With<EnemyCharacter>>,
    zones: Option<Res<LevelZones>>,
    mut armed: ResMut<BossFallArmed>,
    mut spawn_collectible: EventWriter<SpawnCollectibleEvent>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    // Tick cooldowns regardless of whether a hit happened this frame. Once
    // the boss has taken its first hit it "bleeds": every 15 s it sheds
    // three hearts the player can grab (pygame `FinalBoss.update`).
    for (tf, _, mut boss) in &mut bosses {
        boss.hit_cooldown.tick(time.delta());
        if boss.has_been_hit {
            boss.bleed_timer.tick(time.delta());
            if boss.bleed_timer.just_finished() {
                for pop in [
                    Vec2::new(-160.0, 260.0),
                    Vec2::new(0.0, 320.0),
                    Vec2::new(160.0, 260.0),
                ] {
                    spawn_collectible.write(SpawnCollectibleEvent {
                        kind: Collectible::ExtraLife,
                        position: tf.translation,
                        pop,
                    });
                }
            }
        }
    }

    for event in collision_events.read() {
        let CollisionEvent::Started(e1, e2, _) = event else {
            continue;
        };
        let (wool, boss_entity) = if wool_balls.get(*e1).is_ok() && bosses.get(*e2).is_ok() {
            (*e1, *e2)
        } else if wool_balls.get(*e2).is_ok() && bosses.get(*e1).is_ok() {
            (*e2, *e1)
        } else {
            continue;
        };

        commands.entity(wool).despawn();

        let Ok((mut tf, mut health, mut boss)) = bosses.get_mut(boss_entity) else {
            continue;
        };
        if !boss.hit_cooldown.finished() {
            continue;
        }
        boss.hit_cooldown.reset();
        boss.has_been_hit = true;
        health.current = health.current.saturating_sub(1);

        if health.current != 0 {
            sfx.write(crate::audio::SfxEvent::EnemyHit);
            continue;
        }

        sfx.write(crate::audio::SfxEvent::DestroyEnemy);
        on_boss_phase_defeated(
            &mut commands,
            boss_entity,
            &mut tf,
            &mut health,
            &mut boss,
            zones.as_deref(),
            &mut armed,
            &mut spawn_collectible,
        );
    }
}

/// What happens when a boss phase's HP reaches zero: phase 1 → it dives
/// into the shaft (and unseals it for the player), phase 2 → last stand,
/// phase 3 → it drops the foil hat that ends the game.
#[allow(clippy::too_many_arguments)]
pub fn on_boss_phase_defeated(
    commands: &mut Commands,
    boss_entity: Entity,
    tf: &mut Transform,
    health: &mut Health,
    boss: &mut FinalBoss,
    zones: Option<&LevelZones>,
    armed: &mut BossFallArmed,
    spawn_collectible: &mut EventWriter<SpawnCollectibleEvent>,
) {
    match boss.phase {
        1 => {
            boss.phase = 2;
            health.current = 36;
            health.max = 36;
            info!("Boss phase 2: running for the shaft");
            // The boss sprints to the middle of the shaft and throws itself
            // in; the seal wall comes down at the same time so the player
            // can follow. If this level has no shaft it just sinks in place.
            boss.diving = true;
            boss.dive_target_x = zones
                .and_then(falling_zone_center_x)
                .unwrap_or(tf.translation.x);
            // From here on it also throws radial bursts (pygame fired the
            // 16-ray fan whenever the hero was out of aimed range).
            commands
                .entity(boss_entity)
                .remove::<Patrol>()
                .insert(FallingMode {
                    speed: BOSS_FALL_SPEED,
                })
                .insert(FanShot {
                    range: 900.0,
                    rays: 12,
                    cooldown_timer: Timer::from_seconds(3.5, TimerMode::Repeating),
                });
            armed.0 = true;
        }
        2 => {
            // Killed mid-fall: it survives the drop and lands ready for
            // the last stand (pygame just restored gravity here).
            enter_boss_phase_3(commands, boss_entity, boss, health);
        }
        _ => {
            info!("Boss defeated! Dropping the foil hat.");
            spawn_collectible.write(SpawnCollectibleEvent {
                kind: Collectible::EndGame,
                position: tf.translation,
                pop: Vec2::new(0.0, 300.0),
            });
            commands.entity(boss_entity).despawn();
        }
    }
}

fn enter_boss_phase_3(
    commands: &mut Commands,
    boss_entity: Entity,
    boss: &mut FinalBoss,
    health: &mut Health,
) {
    boss.phase = 3;
    health.current = 18;
    health.max = 18;
    info!("Boss phase 3: final showdown");
    commands
        .entity(boss_entity)
        .remove::<FallingMode>()
        .insert(Patrol {
            speed: 70.0,
            direction: -1,
        });
}

/// When the player reaches the bottom arena (`LevelMode::Normal` after the
/// shaft) the boss stops falling and starts its last stand — pygame's
/// `normal_mode()` did the same via `enemies.on_gravity()`.
pub fn boss_landing_system(
    mut commands: Commands,
    level_mode: Res<LevelMode>,
    mut bosses: Query<(Entity, &mut Health, &mut FinalBoss), With<EnemyCharacter>>,
) {
    if *level_mode != LevelMode::Normal {
        return;
    }
    for (entity, mut health, mut boss) in &mut bosses {
        if boss.phase == 2 {
            enter_boss_phase_3(&mut commands, entity, &mut boss, &mut health);
        }
    }
}

/// Active only during boss phase 2. First the dive: the boss runs toward
/// the shaft center, ignoring the player, and drops in. Then, while both
/// fall, it tracks the player's X (slower than the player's lateral speed
/// so side-stepping works) and drifts to stay just below them, the way
/// pygame's `follow(hero)` kept it level with the hero.
pub fn boss_follow_hero_system(
    time: Res<Time>,
    zones: Option<Res<LevelZones>>,
    mut bosses: Query<
        (
            &mut Transform,
            &mut FinalBoss,
            &mut crate::physics::Velocity,
        ),
        (
            With<EnemyCharacter>,
            With<FallingMode>,
            Without<PlayerCharacter>,
        ),
    >,
    player: Query<(&Transform, Has<FallingMode>), With<PlayerCharacter>>,
) {
    let Ok((player_tf, player_falling)) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation;
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let shaft = zones
        .as_deref()
        .and_then(|z| {
            z.zones
                .iter()
                .find(|z| z.action == crate::map::zones::ZoneAction::Falling)
        })
        .map(|z| z.bounds);

    for (mut tf, mut boss, mut velocity) in &mut bosses {
        if boss.phase != 2 {
            continue;
        }
        if boss.diving {
            let dx = boss.dive_target_x - tf.translation.x;
            if dx.abs() < 8.0 {
                boss.diving = false;
                boss.hover_y = tf.translation.y - 40.0;
                velocity.velocity.x = 0.0;
            } else {
                velocity.velocity.x = dx.signum() * 260.0;
            }
            continue;
        }
        // `FallingMode` pulls it down at BOSS_FALL_SPEED every frame; the
        // vertical nudge below fights that to hover around `target_y`.
        let (target_x, target_y, track_speed) = if player_falling {
            // Shadow the player: stay a bit below, weave side to side.
            (
                player_pos.x + (t * 1.6).sin() * 150.0,
                player_pos.y - 200.0,
                140.0,
            )
        } else {
            // Waiting just under the ledge, pacing across the shaft.
            let cx = shaft.map(|b| b.center().x).unwrap_or(tf.translation.x);
            (cx + (t * 0.9).sin() * 260.0, boss.hover_y, 160.0)
        };
        let dx = target_x - tf.translation.x;
        velocity.velocity.x = if dx.abs() > 6.0 {
            dx.signum() * track_speed
        } else {
            0.0
        };
        let dy = (target_y + BOSS_FALL_SPEED / 1.5) - tf.translation.y;
        tf.translation.y += dy * (1.5 * dt).min(1.0);
    }
}

/// Phase 1 safety net: whatever happens (knockback, edge probes, the
/// player's tricks), the boss stays on the arena floor until its HP
/// milestone triggers the dive.
pub fn boss_arena_clamp_system(
    zones: Option<Res<LevelZones>>,
    mut bosses: Query<
        (
            &mut Transform,
            &FinalBoss,
            &crate::enemies::components::BodyRadius,
        ),
        With<EnemyCharacter>,
    >,
) {
    let Some(zones) = zones else { return };
    let Some(arena) = zones
        .zones
        .iter()
        .find(|z| z.action == crate::map::zones::ZoneAction::Clear)
    else {
        return;
    };
    for (mut tf, boss, radius) in &mut bosses {
        if boss.phase != 1 {
            continue;
        }
        let min_x = arena.bounds.min.x + radius.0;
        let max_x = arena.bounds.max.x - radius.0;
        tf.translation.x = tf.translation.x.clamp(min_x, max_x);
    }
}

/// Sistema para gestionar las colisiones de los proyectiles y el temporizador de desaparición.
/// Expires wool balls. Every ball lives `WOOL_BALL_LIFETIME` from the throw
/// and the clock runs whether or not it ever touches anything — a ball shot
/// at open sky used to fly forever, and each one held a slot in the
/// five-in-flight cap, so outdoors the player simply ran out of ammo for
/// good. Hitting something just cuts the remaining life short.
pub fn handle_projectile_despawn(
    mut commands: Commands,
    mut projectile_query: Query<(Entity, &mut Projectile)>,
    players: Query<Entity, With<PlayerCharacter>>,
    mut collision_events: EventReader<CollisionEvent>,
    time: Res<Time>,
) {
    let player = players.single().ok();
    for event in collision_events.read() {
        let CollisionEvent::Started(entity1, entity2, _) = event else {
            continue;
        };
        // Two balls brushing past each other is not an impact. The maniac
        // burst spawns sixteen of them on a 30 px circle around Tofe, close
        // enough that neighbours overlap on the very first frame, so
        // counting that as a hit cut every ray's life short and none of them
        // ever reached an enemy.
        if projectile_query.contains(*entity1) && projectile_query.contains(*entity2) {
            continue;
        }
        for (entity, other) in [(entity1, entity2), (entity2, entity1)] {
            // Neither is being thrown *at* the thrower.
            if player.is_some_and(|p| p == *other) {
                continue;
            }
            let Ok((_, mut projectile)) = projectile_query.get_mut(*entity) else {
                continue;
            };
            if projectile.has_collided {
                continue;
            }
            projectile.has_collided = true;
            let remaining = projectile.despawn_timer.remaining_secs();
            if remaining > WOOL_BALL_IMPACT_LINGER {
                // Rewind the timer so only the linger is left.
                let elapsed =
                    projectile.despawn_timer.duration().as_secs_f32() - WOOL_BALL_IMPACT_LINGER;
                projectile
                    .despawn_timer
                    .set_elapsed(std::time::Duration::from_secs_f32(elapsed));
            }
        }
    }

    for (entity, mut projectile) in projectile_query.iter_mut() {
        projectile.despawn_timer.tick(time.delta());
        if projectile.despawn_timer.finished() {
            commands.entity(entity).despawn();
        }
    }
}
