// src/player/mod.rs

pub mod assets;
pub mod bundle; // Declara el submódulo bundle.rs
pub mod components; // Declara el submódulo components.rs
pub mod systems; // Declara el submódulo systems.rs // Declara el submódulo assets.rs

use crate::game_state::{GameState, LevelState};
use crate::map::ONE_WAY_PLATFORM_GROUP;
use crate::map::assets::GameAssets;
use crate::physics::{AffectedByGravity, Mass, Velocity};
use crate::player::assets::{HeroData, load_player_assets};

use crate::player::{
    assets::PlayerAssets, // Importa PlayerAssets
    bundle::PlayerBundle, // Importa el PlayerBundle
    components::*,        // Importa todos los componentes del player
    systems::*,           // Importa todos los sistemas del player
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::{
    ActiveCollisionTypes, ActiveEvents, Collider, CollisionGroups, Group,
    KinematicCharacterController, RigidBody, Velocity as RapierVelocity,
};

// Esta función añadirá todos los sistemas del jugador a la aplicación
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // Player lifecycle is tied to LevelState: assets load on Loading,
        // character spawns on LevelLoaded, and all of it despawns on exit —
        // so changing level mid-game cleanly tears down and rebuilds.
        app.add_systems(OnEnter(LevelState::Loading), load_player_assets)
            .add_systems(
                OnEnter(LevelState::LevelLoaded),
                (
                    spawn_player_character.after(load_player_assets),
                    spawn_player_hearts.after(spawn_player_character),
                ),
            )
            .add_systems(
                Update,
                (
                    character_input_handling,
                    player_input_system,
                    execute_animations,
                    player_bounds_system,
                    reset_jumps,
                    update_player_life,
                    animate_hearts,
                    invincibility_system,
                    knockback_system,
                    check_player_death,
                    handle_gameover_timer,
                )
                    .run_if(in_state(GameState::Game)),
            )
            .add_systems(
                OnExit(LevelState::LevelLoaded),
                (despawn_player, despawn_hearts),
            );
    }
}

fn despawn_player(mut commands: Commands, player_query: Query<Entity, With<PlayerCharacter>>) {
    for entity in player_query.iter() {
        commands.entity(entity).despawn();
    }
}
fn despawn_hearts(mut commands: Commands, player_query: Query<Entity, With<PlayerHearts>>) {
    for entity in player_query.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn spawn_player_hearts(
    mut commands: Commands,
    player_assets: Res<PlayerAssets>,
    player_query: Query<&Health, With<PlayerCharacter>>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let Ok(player_health) = player_query.single() else {
        println!("No player health");
        return;
    };

    let layout = TextureAtlasLayout::from_grid(UVec2::splat(160), 21, 1, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);
    let mut x_position = 14.0;

    for idx in 0..player_health.max {
        let heart_transform = Transform {
            // translation: Vec3::new(x_position, 60.0, 10.0),
            scale: Vec3::splat(0.25),
            ..default()
        };

        commands.spawn((
            PlayerHearts::new(idx as usize),
            ImageNode {
                image: player_assets.hearts.clone(),
                texture_atlas: Some(TextureAtlas {
                    layout: texture_atlas_layout.clone(),
                    index: 0,
                }),
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(7.0),
                left: Val::Px(x_position),
                ..default()
            },
            heart_transform,
        ));

        x_position += 45.0;
    }
}

pub const PLAYER_GROUP: Group = Group::GROUP_1;

fn spawn_player_character(
    mut commands: Commands,
    player_assets: Res<PlayerAssets>, // Ahora obtenemos los assets precargados
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    hero_data: Res<HeroData>,
    game_assets: Res<GameAssets>,
) {
    let tile_size_from_json = game_assets.tile_size_px;
    let map_width_from_json = game_assets.map_width_tiles;
    let map_height_from_json = game_assets.map_height_tiles;

    let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 8, 1, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);

    let x = hero_data.x as f32;
    let y = hero_data.y as f32;

    let world_x =
        x * tile_size_from_json - (map_width_from_json as f32 * tile_size_from_json / 2.0);
    let world_y =
        -y * tile_size_from_json + (map_height_from_json as f32 * tile_size_from_json / 2.0);

    let mut transform = Transform::from_scale(Vec3::splat(0.9));

    transform.translation.x = world_x + tile_size_from_json / 2.0;
    transform.translation.y = world_y - tile_size_from_json / 2.0;

    let character_controller = KinematicCharacterController {
        filter_groups: Some(CollisionGroups {
            memberships: PLAYER_GROUP,
            filters: Group::ALL & !ONE_WAY_PLATFORM_GROUP,
        }),
        ..default()
    };

    commands
        .spawn(PlayerBundle::new(transform))
        .with_children(|parent| {
            parent.spawn((
                Sprite {
                    image: player_assets.texture_left.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: texture_atlas_layout.clone(),
                        index: 0,
                    }),
                    ..default()
                },
                Transform::from_scale(Vec3::splat(0.6)),
                CharacterLeftSprite,
                Visibility::Hidden,
                AnimationIndices::new(0, 7, ANIMATION_FPS),
            ));
            parent.spawn((
                Sprite {
                    image: player_assets.texture_right.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: texture_atlas_layout.clone(),
                        index: 0,
                    }),
                    ..default()
                },
                Transform::from_scale(Vec3::splat(0.6)),
                CharacterRightSprite,
                Visibility::Hidden,
                AnimationIndices::new(0, 7, ANIMATION_FPS),
            ));
            parent.spawn((
                Sprite {
                    image: player_assets.texture_standing.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: texture_atlas_layout.clone(),
                        index: 0,
                    }),
                    ..default()
                },
                Transform::from_scale(Vec3::splat(0.6)),
                CharacterIdleSprite,
                Visibility::Visible,
                AnimationIndices::new(0, 7, ANIMATION_FPS),
            ));
        })
        .insert(RigidBody::KinematicPositionBased)
        .insert(character_controller)
        .insert(Collider::ball(32.0 / 2.0))
        .insert(PlayerCharacter)
        .insert(AffectedByGravity)
        .insert(RapierVelocity::zero())
        .insert(Mass::default())
        .insert(Health::default())
        .insert(DoubleJump::default())
        .insert(Velocity::default())
        .insert(ActiveEvents::COLLISION_EVENTS)
        // Rapier's default ActiveCollisionTypes only reports pairs involving
        // a Dynamic body. The player is Kinematic, and so are every enemy,
        // projectile, and collectible — plus EndLevel tiles are Fixed. Without
        // this, none of those collisions fire `CollisionEvent::Started`, so
        // damage, pickups, and the level-complete trigger all go silent.
        .insert(ActiveCollisionTypes::all());
}

// Puedes definir constantes aquí o en un submódulo de constantes si tienes muchas
const ANIMATION_FPS: u8 = 10;
