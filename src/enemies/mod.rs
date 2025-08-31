use std::time::Duration;

use crate::{
    enemies::{
        assets::load_enemy_assets,
        bundle::EnemyBundle,
        components::{
            ActiveLevenData, Chase, ContactDamage, EnemyAssets, EnemyCharacter, EnemyState,
            EnemyType, Patrol, RangedAttack, RangedAttackType, Teleport,
        },
    },
    game_state::GameState,
    map::assets::GameAssets,
    physics::{AffectedByGravity, Mass, Velocity},
    player::components::{
        AnimationIndices, CharacterIdleSprite, CharacterLeftSprite, CharacterRightSprite,
    },
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::{
    ActiveEvents, Collider, KinematicCharacterController, RigidBody, Velocity as RapierVelocity,
};

pub mod assets;
pub mod bundle;
pub mod components;
pub mod systems;
pub struct EnemiesPlugin;

impl Plugin for EnemiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_enemy_assets)
            .add_systems(
                OnEnter(GameState::Game),
                spawn_enemies_characters.after(load_enemy_assets),
            )
            .add_systems(OnExit(GameState::Game), despawn_enemies);
    }
}
pub fn despawn_enemies(mut commands: Commands, query: Query<Entity, With<EnemyCharacter>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn spawn_enemies_characters(
    mut commands: Commands,
    enemies_assets: Res<EnemyAssets>,
    enemies_level_data: Res<ActiveLevenData>,
    game_assets: Res<GameAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let tile_size_from_json = game_assets.tile_size_px;
    let map_width_from_json = game_assets.map_width_tiles;
    let map_height_from_json = game_assets.map_height_tiles;

    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 8, 1, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);

    for enemy in &enemies_level_data.enemies {
        for obj in &enemy.positions {
            let enemy_type: EnemyType = enemy.name.parse().unwrap();
            let enemy_asset = &enemies_assets.map[&enemy_type];

            let x = obj.x as f32;
            let y = obj.y as f32;

            let world_x =
                x * tile_size_from_json - (map_width_from_json as f32 * tile_size_from_json / 2.0);
            let world_y = -y * tile_size_from_json
                + (map_height_from_json as f32 * tile_size_from_json / 2.0); // Invertir Y

            let mut transform = Transform::from_scale(Vec3::splat(0.6));

            transform.translation.x = world_x + tile_size_from_json / 2.0;
            transform.translation.y = world_y - tile_size_from_json / 2.0;

            let mut sprite_transform = Transform::from_scale(Vec3::splat(0.6));
            sprite_transform.translation.y += 5.0;

            let mut enemy_entity = commands.spawn(EnemyBundle::new(transform.translation));

            enemy_entity
                .with_children(|parent| {
                    parent.spawn((
                        Sprite {
                            image: enemy_asset.texture_left.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: texture_atlas_layout.clone(),
                                index: 0,
                            }),
                            ..default()
                        },
                        sprite_transform,
                        CharacterLeftSprite,
                        Visibility::Hidden,
                        AnimationIndices::new(0, 7, ANIMATION_FPS),
                    ));
                    parent.spawn((
                        Sprite {
                            image: enemy_asset.texture_right.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: texture_atlas_layout.clone(),
                                index: 0,
                            }),
                            ..default()
                        },
                        sprite_transform,
                        CharacterRightSprite,
                        Visibility::Hidden,
                        AnimationIndices::new(0, 7, ANIMATION_FPS),
                    ));
                    parent.spawn((
                        Sprite {
                            image: enemy_asset.texture_standing.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: texture_atlas_layout.clone(),
                                index: 0,
                            }),
                            ..default()
                        },
                        sprite_transform,
                        CharacterIdleSprite,
                        Visibility::Visible,
                        AnimationIndices::new(0, 7, ANIMATION_FPS),
                    ));
                })
                .insert(RigidBody::KinematicPositionBased)
                .insert(KinematicCharacterController::default())
                .insert(Collider::ball(32.0 / 2.0))
                .insert(EnemyCharacter)
                .insert(AffectedByGravity)
                .insert(RapierVelocity::zero())
                .insert(Mass::default())
                .insert(Velocity::default())
                .insert(ActiveEvents::COLLISION_EVENTS);

            // --- Aquí se añaden los componentes de IA según el tipo de enemigo ---
            match enemy_type {
                EnemyType::Dummy => {
                    enemy_entity
                        .insert(EnemyState::Patrolling)
                        .insert(Patrol {
                            speed: 50.0,
                            direction: 1,
                        })
                        .insert(ContactDamage { amount: 1 });
                }
                EnemyType::Fufi => {
                    enemy_entity
                        .insert(EnemyState::Chasing)
                        .insert(Chase {
                            speed: 80.0,
                            range: 400.0,
                        })
                        .insert(RangedAttack {
                            attack_type: RangedAttackType::SingleShot,
                            range: 350.0,
                            timer: Timer::new(Duration::from_secs(2), TimerMode::Repeating),
                        });
                }
                EnemyType::Catcifer => {
                    enemy_entity
                        .insert(EnemyState::Idle)
                        .insert(RangedAttack {
                            attack_type: RangedAttackType::FanShot,
                            range: 300.0,
                            timer: Timer::new(Duration::from_secs(3), TimerMode::Repeating),
                        })
                        .insert(Teleport {
                            trigger_distance: 100.0,
                            timer: Timer::new(Duration::from_secs(5), TimerMode::Repeating),
                        });
                }
                // Añadir casos para otros enemigos si es necesario
                _ => {
                    enemy_entity.insert(EnemyState::Idle);
                }
            }
        }
    }
}
// Puedes definir constantes aquí o en un submódulo de constantes si tienes muchas
const ANIMATION_FPS: u8 = 10;
