use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::{
    audio::SfxEvent,
    collectibles::{
        assets::CollectibleAssets,
        components::{Collectible, Magnetic, ManiacBuff, SpawnCollectibleEvent, SpawnPop},
    },
    enemies::components::{EnemyKilledEvent, EnemyType},
    game_state::{LevelCompleteEvent, PlayerStats},
    player::components::{AnimationIndices, Health, PlayerCharacter},
};

/// Distance at which a magnetic pickup starts homing onto the player.
/// Matches pygame's 128-pixel radius.
const MAGNET_RADIUS: f32 = 128.0;
const MAGNET_SPEED: f32 = 260.0;
pub const MANIAC_BUFF_DURATION_SECS: f32 = 10.0;
const POP_GRAVITY: f32 = 900.0;
/// Pickups render above tiles and below the player.
const COLLECTIBLE_Z: f32 = 4.0;

/// Maps enemy type → drop, matching pygame: Dummy→KittyPoint,
/// Fufi→ExtraLife, Catcifer→ManiacMode. The boss drops the foil hat via
/// its own death handler, and unused cats drop nothing.
fn drop_for(kind: &EnemyType) -> Option<Collectible> {
    match kind {
        EnemyType::Dummy => Some(Collectible::KittyPoint),
        EnemyType::Fufi => Some(Collectible::ExtraLife),
        EnemyType::Catcifer => Some(Collectible::ManiacMode),
        _ => None,
    }
}

pub fn drop_on_enemy_death(
    mut events: EventReader<EnemyKilledEvent>,
    mut spawn: EventWriter<SpawnCollectibleEvent>,
) {
    for ev in events.read() {
        if let Some(kind) = drop_for(&ev.kind) {
            spawn.write(SpawnCollectibleEvent {
                kind,
                position: ev.position,
                pop: Vec2::new(0.0, 220.0),
            });
        }
    }
}

fn sprite_for(kind: Collectible, assets: &CollectibleAssets) -> Sprite {
    match kind {
        Collectible::KittyPoint => Sprite {
            image: assets.kitty_point.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: assets.kitty_point_atlas.clone(),
                index: 0,
            }),
            custom_size: Some(Vec2::splat(32.0)),
            ..default()
        },
        Collectible::ExtraLife => Sprite {
            image: assets.extra_life.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: assets.extra_life_atlas.clone(),
                index: 0,
            }),
            custom_size: Some(Vec2::splat(30.0)),
            ..default()
        },
        Collectible::ManiacMode => Sprite {
            image: assets.maniac_mode.clone(),
            custom_size: Some(Vec2::splat(30.0)),
            ..default()
        },
        Collectible::EndGame => Sprite {
            image: assets.end_game.clone(),
            custom_size: Some(Vec2::splat(40.0)),
            ..default()
        },
    }
}

pub fn spawn_collectibles(
    mut commands: Commands,
    mut events: EventReader<SpawnCollectibleEvent>,
    assets: Res<CollectibleAssets>,
) {
    for ev in events.read() {
        let position = Vec3::new(ev.position.x, ev.position.y, COLLECTIBLE_Z);
        let mut entity = commands.spawn((
            sprite_for(ev.kind, &assets),
            Transform::from_translation(position),
            ev.kind,
            // Kinematic so Transform edits (magnet, pop) move the collider.
            RigidBody::KinematicPositionBased,
            Collider::ball(12.0),
            Sensor,
            ActiveEvents::COLLISION_EVENTS,
            SpawnPop {
                velocity: ev.pop,
                origin_y: position.y,
            },
        ));

        match ev.kind {
            Collectible::KittyPoint => {
                entity.insert((
                    AnimationIndices::new(0, 7, 10),
                    Magnetic {
                        trigger_radius: MAGNET_RADIUS,
                        speed: MAGNET_SPEED,
                    },
                ));
            }
            Collectible::ExtraLife => {
                entity.insert(Magnetic {
                    trigger_radius: MAGNET_RADIUS,
                    speed: MAGNET_SPEED,
                });
            }
            Collectible::ManiacMode | Collectible::EndGame => {}
        }
    }
}

/// Small arc when a pickup appears: up, then back down to where it spawned.
pub fn spawn_pop_system(
    mut commands: Commands,
    time: Res<Time>,
    mut pops: Query<(Entity, &mut Transform, &mut SpawnPop)>,
) {
    let dt = time.delta_secs();
    for (entity, mut tf, mut pop) in &mut pops {
        pop.velocity.y -= POP_GRAVITY * dt;
        tf.translation.x += pop.velocity.x * dt;
        tf.translation.y += pop.velocity.y * dt;
        if pop.velocity.y < 0.0 && tf.translation.y <= pop.origin_y {
            tf.translation.y = pop.origin_y;
            commands.entity(entity).remove::<SpawnPop>();
        }
    }
}

/// Homes magnetic collectibles toward the player when within trigger radius.
pub fn magnetic_system(
    time: Res<Time>,
    player: Query<&Transform, With<PlayerCharacter>>,
    mut magnetics: Query<
        (&mut Transform, &Magnetic),
        (
            With<Collectible>,
            Without<PlayerCharacter>,
            Without<SpawnPop>,
        ),
    >,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();
    for (mut tf, m) in &mut magnetics {
        let pos = tf.translation.truncate();
        let dist = pos.distance(player_pos);
        if dist < m.trigger_radius && dist > 1.0 {
            let dir = (player_pos - pos) / dist;
            // Accelerate as it gets closer so the last stretch snaps.
            let speed = m.speed * (1.0 + (1.0 - dist / m.trigger_radius) * 2.0);
            let step = dir * speed * time.delta_secs();
            tf.translation.x += step.x;
            tf.translation.y += step.y;
        }
    }
}

/// Grants the pickup effect when the player overlaps a collectible.
/// Despawns the collectible afterward.
pub fn pickup_system(
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(Entity, &mut Health), With<PlayerCharacter>>,
    collectibles_q: Query<&Collectible>,
    mut sfx: EventWriter<SfxEvent>,
    mut level_complete: EventWriter<LevelCompleteEvent>,
) {
    let Ok((player_entity, mut health)) = player_q.single_mut() else {
        return;
    };

    for event in collision_events.read() {
        let CollisionEvent::Started(a, b, _) = event else {
            continue;
        };
        let other = if *a == player_entity {
            *b
        } else if *b == player_entity {
            *a
        } else {
            continue;
        };
        let Ok(kind) = collectibles_q.get(other) else {
            continue;
        };

        match kind {
            Collectible::KittyPoint => {
                stats.kitty_points += 1;
                sfx.write(SfxEvent::Point);
            }
            Collectible::ExtraLife => {
                stats.hearts += 1;
                health.current = (health.current + 1).min(health.max);
                sfx.write(SfxEvent::OneUp);
            }
            Collectible::ManiacMode => {
                stats.cookies += 1;
                commands
                    .entity(player_entity)
                    .insert(ManiacBuff(Timer::from_seconds(
                        MANIAC_BUFF_DURATION_SECS,
                        TimerMode::Once,
                    )));
                sfx.write(SfxEvent::Cookie);
            }
            Collectible::EndGame => {
                sfx.write(SfxEvent::OneUp);
                level_complete.write(LevelCompleteEvent);
            }
        }
        commands.entity(other).despawn();
    }
}

/// Ticks the player's `ManiacBuff` timer and removes the component when
/// the buff expires.
pub fn maniac_buff_expiration_system(
    mut commands: Commands,
    time: Res<Time>,
    mut buffs: Query<(Entity, &mut ManiacBuff)>,
) {
    for (entity, mut buff) in &mut buffs {
        buff.0.tick(time.delta());
        if buff.0.finished() {
            commands.entity(entity).remove::<ManiacBuff>();
        }
    }
}

pub fn despawn_collectibles(mut commands: Commands, q: Query<Entity, With<Collectible>>) {
    for entity in &q {
        commands.entity(entity).despawn();
    }
}
