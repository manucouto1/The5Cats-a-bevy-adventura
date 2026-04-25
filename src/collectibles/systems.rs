use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::{
    collectibles::{
        Score,
        assets::CollectibleAssets,
        components::{Collectible, Magnetic, ManiacBuff, ScoreText},
    },
    enemies::components::{EnemyKilledEvent, EnemyType},
    player::components::{Health, PlayerCharacter},
};

/// Distance at which an ExtraLife starts homing onto the player.
/// Matches pygame's 128-pixel radius.
const EXTRA_LIFE_MAGNET_RADIUS: f32 = 128.0;
const EXTRA_LIFE_MAGNET_SPEED: f32 = 250.0;
const MANIAC_BUFF_DURATION_SECS: f32 = 10.0;

/// Maps enemy type → collectible drop, matching pygame drops:
/// Dummy→KittyPoint, Fufi→ExtraLife, Catcifer→ManiacMode. Unmapped types
/// (Maximiliano, Willie) drop nothing.
fn drop_for(kind: &EnemyType) -> Option<Collectible> {
    match kind {
        EnemyType::Dummy => Some(Collectible::KittyPoint),
        EnemyType::Fufi => Some(Collectible::ExtraLife),
        EnemyType::Catcifer => Some(Collectible::ManiacMode),
        EnemyType::KiddCat => Some(Collectible::ExtraLife),
        _ => None,
    }
}

fn sprite_for(kind: Collectible, assets: &CollectibleAssets) -> Sprite {
    match kind {
        Collectible::KittyPoint => Sprite {
            image: assets.kitty_point.clone(),
            custom_size: Some(Vec2::splat(24.0)),
            ..default()
        },
        Collectible::ExtraLife => Sprite {
            image: assets.extra_life.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: assets.extra_life_atlas.clone(),
                index: 0,
            }),
            custom_size: Some(Vec2::splat(32.0)),
            ..default()
        },
        Collectible::ManiacMode => Sprite {
            image: assets.maniac_mode.clone(),
            custom_size: Some(Vec2::splat(28.0)),
            ..default()
        },
    }
}

pub fn spawn_collectible_on_death(
    mut commands: Commands,
    mut events: EventReader<EnemyKilledEvent>,
    assets: Res<CollectibleAssets>,
) {
    for ev in events.read() {
        let Some(kind) = drop_for(&ev.kind) else {
            continue;
        };

        let mut entity = commands.spawn((
            sprite_for(kind, &assets),
            Transform::from_translation(ev.position),
            kind,
            // Kinematic so Transform edits in `magnetic_system` actually
            // move the collider (Fixed would anchor it at spawn).
            RigidBody::KinematicPositionBased,
            Collider::ball(10.0),
            Sensor,
            ActiveEvents::COLLISION_EVENTS,
        ));

        if matches!(kind, Collectible::ExtraLife) {
            entity.insert(Magnetic {
                trigger_radius: EXTRA_LIFE_MAGNET_RADIUS,
                speed: EXTRA_LIFE_MAGNET_SPEED,
            });
        }
    }
}

/// Homes magnetic collectibles toward the player when within trigger radius.
pub fn magnetic_system(
    time: Res<Time>,
    player: Query<&Transform, With<PlayerCharacter>>,
    mut magnetics: Query<
        (&mut Transform, &Magnetic),
        (With<Collectible>, Without<PlayerCharacter>),
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
            let step = dir * m.speed * time.delta_secs();
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
    mut score: ResMut<Score>,
    mut player_q: Query<(Entity, &mut Health), With<PlayerCharacter>>,
    collectibles_q: Query<&Collectible>,
    mut sfx: EventWriter<crate::audio::SfxEvent>,
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
                score.kitty_points += 1;
                sfx.write(crate::audio::SfxEvent::Point);
                info!("+1 KittyPoint (total: {})", score.kitty_points);
            }
            Collectible::ExtraLife => {
                health.current = (health.current + 2).min(health.max);
                sfx.write(crate::audio::SfxEvent::OneUp);
                info!("+ExtraLife (hp {}/{})", health.current, health.max);
            }
            Collectible::ManiacMode => {
                commands.entity(player_entity).insert(ManiacBuff(
                    Timer::from_seconds(MANIAC_BUFF_DURATION_SECS, TimerMode::Once),
                ));
                sfx.write(crate::audio::SfxEvent::Cookie);
                info!("ManiacMode activated for {}s", MANIAC_BUFF_DURATION_SECS);
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

/// Spawns the score counter in the top-right corner of the screen as a
/// Bevy UI text node. Resets the score resource on level entry so each
/// run/level starts from 0 (pygame behavior).
pub fn spawn_score_hud(mut commands: Commands, mut score: ResMut<Score>) {
    score.kitty_points = 0;
    commands.spawn((
        Text::new("Kitties: 0"),
        TextFont {
            font_size: 28.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.95, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(20.0),
            ..default()
        },
        ScoreText,
    ));
}

/// Rewrites the score text only when `Score` actually changes — avoids
/// touching the Text component every frame.
pub fn update_score_hud(
    score: Res<Score>,
    mut score_text: Query<&mut Text, With<ScoreText>>,
) {
    if !score.is_changed() {
        return;
    }
    for mut text in &mut score_text {
        **text = format!("Kitties: {}", score.kitty_points);
    }
}

pub fn despawn_score_hud(mut commands: Commands, q: Query<Entity, With<ScoreText>>) {
    for entity in &q {
        commands.entity(entity).despawn();
    }
}
