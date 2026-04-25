use bevy::{prelude::*, window::PrimaryWindow};
use bevy_rapier2d::prelude::{
    ActiveEvents, Collider, CollisionEvent, RigidBody, Sleeping, Velocity,
};

use crate::{
    cursor::{
        assets::CursorAssets,
        components::{Crosshair, Projectile, WoolBall},
    },
    enemies::components::{
        EnemyCharacter, EnemyKilledEvent, EnemyType, FinalBoss, Patrol,
    },
    game_state::LevelCompleteEvent,
    map::zones::{BossFallArmed, LevelZones, falling_zone_center_x},
    physics::{AffectedByGravity, FallingMode, Mass},
    player::components::{Health, PlayerCharacter},
};

/// Descent speed for the boss while it free-falls down the shaft in phase 2.
/// Slower than the player (who falls at `FALLING_SPEED` = 500) so the player
/// catches up and stays near the boss for the fight.
const BOSS_FALL_SPEED: f32 = 320.0;

pub fn update_aim_assist(
    // Recursos para obtener la posición del ratón
    // mut evr_cursor: EventReader<CursorMoved>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
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

pub fn spawn_projectile_on_click(
    mut commands: Commands,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    cursor_assets: Res<CursorAssets>,
    player_query: Query<&Transform, With<PlayerCharacter>>,
    crosshair_query: Query<&Transform, With<Crosshair>>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        sfx.write(crate::audio::SfxEvent::Shoot);
        let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 1, 1, None, None);
        let texture_atlas_layout = texture_atlas_layouts.add(layout);

        let Ok(player_transform) = player_query.single() else {
            return;
        };
        let Ok(crosshair_transform) = crosshair_query.single() else {
            return;
        };

        // let player_position = player_transform.translation.xy();
        // let cursor_position = crosshair_transform.translation.xy();

        // let direction = (cursor_position - player_position).normalize();

        // let speed = 1500.0; // Ajusta la velocidad de la bola
        // let velocity = direction * speed;

        // let offset = 32.0; // Se ajusta a la mitad del tamaño de la bola
        // let start_position = player_position + direction * offset;
        //
        let start_pos = player_transform.translation.xy();
        let target_pos = crosshair_transform.translation.xy();

        let speed = 1000.0;
        let gravity = 9.81 * 32.0; // px/s² (ajusta según tus unidades)
        let prefer_high_arc = false;

        if let Some(velocity) =
            solve_ballistic_velocity(start_pos, target_pos, speed, gravity, prefer_high_arc)
        {
            // desplazamiento hacia adelante (por ejemplo, 32 px)
            let offset_dist = 32.0;
            let offset_pos = start_pos + velocity.normalize() * offset_dist;

            commands.spawn((
                Sprite {
                    image: cursor_assets.wool_image.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: texture_atlas_layout.clone(),
                        index: 0,
                    }),
                    ..default()
                },
                Transform::from_translation(offset_pos.extend(98.0)).with_scale(Vec3::splat(0.5)),
                WoolBall,
                Projectile {
                    despawn_timer: Timer::from_seconds(2.0, TimerMode::Once),
                    has_collided: false,
                },
                Collider::ball(8.0), // Tamaño del colisionador de la bola
                RigidBody::Dynamic,
                Velocity {
                    linvel: velocity,
                    angvel: 0.0,
                },
                ActiveEvents::COLLISION_EVENTS, // Habilita la detección de colisiones
                AffectedByGravity,
                Mass { kilograms: 10.0 },
                Sleeping::default(),
            ));
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
    wool_balls: Query<Entity, With<WoolBall>>,
    // Bosses have their own damage pipeline (phases, hit-cooldown) — skip them here.
    mut enemies: Query<
        (&Transform, &mut Health, &EnemyType),
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

        let Ok((tf, mut health, kind)) = enemies.get_mut(enemy) else {
            continue;
        };
        health.current = health.current.saturating_sub(1);
        commands.entity(wool).despawn();

        if health.current == 0 {
            sfx.write(crate::audio::SfxEvent::DestroyEnemy);
            killed.write(EnemyKilledEvent {
                position: tf.translation,
                kind: kind.clone(),
            });
            commands.entity(enemy).despawn();
        } else {
            sfx.write(crate::audio::SfxEvent::EnemyHit);
        }
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
    mut level_complete: EventWriter<LevelCompleteEvent>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
) {
    // Tick cooldowns regardless of whether a hit happened this frame.
    for (_, _, mut boss) in &mut bosses {
        boss.hit_cooldown.tick(time.delta());
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
        health.current = health.current.saturating_sub(1);

        if health.current != 0 {
            sfx.write(crate::audio::SfxEvent::EnemyHit);
            continue;
        }

        sfx.write(crate::audio::SfxEvent::DestroyEnemy);

        match boss.phase {
            1 => {
                boss.phase = 2;
                health.current = 36;
                health.max = 36;
                info!("Boss phase 2: falling down the shaft");
                // Teleport the boss to the top of the shaft so it falls INTO
                // the hole. If the shaft isn't defined for this level (only
                // level 4 has it) this is a no-op — the boss stays put and
                // just gets FallingMode, which is also fine.
                if let Some(cx) = zones.as_deref().and_then(falling_zone_center_x) {
                    tf.translation.x = cx;
                }
                commands
                    .entity(boss_entity)
                    .remove::<Patrol>()
                    .insert(FallingMode {
                        speed: BOSS_FALL_SPEED,
                    });
                armed.0 = true;
            }
            2 => {
                boss.phase = 3;
                health.current = 18;
                health.max = 18;
                info!("Boss phase 3: final showdown");
                // Drop out of free-fall, resume patrolling on the ground.
                commands
                    .entity(boss_entity)
                    .remove::<FallingMode>()
                    .insert(Patrol {
                        speed: 60.0,
                        direction: -1,
                    });
            }
            _ => {
                info!("Boss defeated!");
                level_complete.write(LevelCompleteEvent);
                commands.entity(boss_entity).despawn();
            }
        }
    }
}

/// Active only during boss phase 2. Tweens the boss's X toward the player's
/// X each frame so it "chases" while both fall down the shaft. Vertical
/// velocity is already pinned by the boss's `FallingMode`, so any Y
/// adjustment is additive to that.
pub fn boss_follow_hero_system(
    time: Res<Time>,
    mut bosses: Query<
        (&mut Transform, &FinalBoss),
        (With<EnemyCharacter>, With<FallingMode>, Without<PlayerCharacter>),
    >,
    player: Query<&Transform, With<PlayerCharacter>>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation;

    for (mut tf, boss) in &mut bosses {
        if boss.phase != 2 {
            continue;
        }
        // Smooth horizontal tracking — slower than player's lateral speed so
        // the player can dodge side-to-side.
        let dx = player_pos.x - tf.translation.x;
        let chase_speed = 180.0;
        let step = dx.signum() * chase_speed * time.delta_secs();
        // Don't overshoot.
        if step.abs() > dx.abs() {
            tf.translation.x = player_pos.x;
        } else {
            tf.translation.x += step;
        }
    }
}

/// Sistema para gestionar las colisiones de los proyectiles y el temporizador de desaparición.
pub fn handle_projectile_despawn(
    mut commands: Commands,
    mut projectile_query: Query<(Entity, &mut Projectile)>,
    mut collision_events: EventReader<CollisionEvent>,
    time: Res<Time>,
) {
    for event in collision_events.read() {
        if let CollisionEvent::Started(entity1, entity2, _) = event {
            // Revisa si alguna de las entidades es un proyectil.
            if let Ok((_, mut projectile)) = projectile_query.get_mut(*entity1) {
                projectile.has_collided = true;
            }
            if let Ok((_, mut projectile)) = projectile_query.get_mut(*entity2) {
                projectile.has_collided = true;
            }
        }
    }

    // Si un proyectil ha colisionado, su temporizador se inicia.
    for (entity, mut projectile) in projectile_query.iter_mut() {
        if projectile.has_collided {
            projectile.despawn_timer.tick(time.delta());
            if projectile.despawn_timer.finished() {
                commands.entity(entity).despawn();
            }
        }
    }
}
