use std::time::Duration;

use crate::{
    enemies::{
        assets::load_enemy_assets,
        bundle::EnemyBundle,
        components::{
            ActiveLevenData, ContactDamage, EnemyAssets, EnemyCharacter, EnemyState, EnemyType,
            FanShot, FinalBoss, Patrol, TurretShot,
        },
        systems::{
            ProjectilePool, activator_system, enemy_damage_system, enemy_facing_sprite_system,
            fan_shot_system, patrol_system, projectile_lifetime_system, setup_projectile_pool,
            teardown_projectile_pool, turret_shot_system,
        },
    },
    game_state::{GameState, LevelState},
    map::assets::GameAssets,
    physics::{AffectedByGravity, Mass, Velocity},
    player::components::{
        AnimationIndices, CharacterIdleSprite, CharacterLeftSprite, CharacterRightSprite, Health,
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
        app.init_resource::<ProjectilePool>()
            .add_systems(OnEnter(LevelState::Loading), load_enemy_assets)
            .add_systems(
                OnEnter(LevelState::LevelLoaded),
                (
                    spawn_enemies_characters.after(load_enemy_assets),
                    setup_projectile_pool,
                ),
            )
            .add_systems(
                Update,
                (
                    // activator runs first so the `With<Active>` filters on
                    // the other systems see the up-to-date marker set.
                    activator_system,
                    patrol_system,
                    turret_shot_system,
                    fan_shot_system,
                    projectile_lifetime_system,
                    enemy_damage_system,
                    enemy_facing_sprite_system,
                )
                    .chain()
                    .run_if(in_state(GameState::Game))
                    .run_if(in_state(LevelState::LevelLoaded)),
            )
            .add_systems(
                OnExit(LevelState::LevelLoaded),
                (despawn_enemies, teardown_projectile_pool),
            );
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

            // Per-type starting HP (pygame `self.life`): Dummy=1, Fufi=2,
            // Catcifer=1, boss=18 (first phase). Other variants default to 1.
            let hp = match enemy_type {
                EnemyType::Dummy => 1,
                EnemyType::Fufi => 2,
                EnemyType::Catcifer => 1,
                EnemyType::KiddCat => 18,
                _ => 1,
            };

            // Rolling enemies (Dummy) ship a single walking sheet for both
            // directions — see enemies/assets.rs. Animating it the same way
            // for left and right makes the cat appear to roll in only one
            // direction. When the L and R handles are the same, mirror the
            // left sprite so the rotation reads as moving with the body.
            let mirror_left = enemy_asset.texture_left == enemy_asset.texture_right;

            enemy_entity
                .insert(enemy_type.clone())
                .insert(Health { current: hp, max: hp })
                .with_children(|parent| {
                    parent.spawn((
                        Sprite {
                            image: enemy_asset.texture_left.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: texture_atlas_layout.clone(),
                                index: 0,
                            }),
                            flip_x: mirror_left,
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
                            direction: -1, // pygame EnemyDummy starts facing LEFT
                        })
                        .insert(ContactDamage { amount: 1 });
                }
                EnemyType::Fufi => {
                    // Pygame EnemyTurretShooter: 3 aimed shots ~0.09s apart, 2.88s cooldown, 300px.
                    let mut burst_interval =
                        Timer::new(Duration::from_millis(90), TimerMode::Once);
                    burst_interval.tick(burst_interval.duration()); // ready on first frame in range
                    let mut cooldown = Timer::new(Duration::from_millis(2880), TimerMode::Once);
                    cooldown.tick(cooldown.duration());
                    enemy_entity
                        .insert(EnemyState::Idle)
                        .insert(TurretShot {
                            range: 300.0,
                            shots_per_burst: 3,
                            shots_remaining: 0,
                            burst_interval_timer: burst_interval,
                            cooldown_timer: cooldown,
                        });
                }
                EnemyType::Catcifer => {
                    // Pygame Maniac: fires 16-ray fan every ~2s while hero within 300px.
                    enemy_entity
                        .insert(EnemyState::Idle)
                        .insert(FanShot {
                            range: 300.0,
                            rays: 16,
                            cooldown_timer: Timer::new(
                                Duration::from_secs(2),
                                TimerMode::Repeating,
                            ),
                        });
                }
                EnemyType::KiddCat => {
                    // Final boss phase 1: patrols its platform and fires
                    // Fufi-style aimed bursts. `boss_damage_system` will
                    // swap behavior when phases advance.
                    let mut burst = Timer::new(Duration::from_millis(120), TimerMode::Once);
                    burst.tick(burst.duration());
                    let mut cooldown = Timer::new(Duration::from_millis(2500), TimerMode::Once);
                    cooldown.tick(cooldown.duration());
                    enemy_entity
                        .insert(EnemyState::Patrolling)
                        .insert(Patrol {
                            speed: 40.0,
                            direction: -1,
                        })
                        .insert(FinalBoss::default())
                        .insert(TurretShot {
                            range: 500.0,
                            shots_per_burst: 3,
                            shots_remaining: 0,
                            burst_interval_timer: burst,
                            cooldown_timer: cooldown,
                        });
                }
                _ => {
                    enemy_entity.insert(EnemyState::Idle);
                }
            }
        }
    }
}
// Puedes definir constantes aquí o en un submódulo de constantes si tienes muchas
const ANIMATION_FPS: u8 = 10;
