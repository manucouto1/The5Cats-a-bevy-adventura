pub mod assets;
pub mod components;
pub mod tile_systems;

use bevy::prelude::*;

// Importar los recursos y componentes necesarios
use crate::game_state::{GameState, Level, LevelState};
use crate::parallax::components::ParallaxLayer;
use crate::physics::Velocity as PlayerVelocity;
use crate::player::PLAYER_GROUP;
use crate::player::components::PlayerCharacter;
use assets::GameAssets;
use bevy_rapier2d::prelude::{
    Collider, CollisionGroups, Group, KinematicCharacterController, RigidBody,
};
use components::LevelData;
// Agregar componentes específicos según el tipo de tile
use crate::map::components::{CurrentLevelInfo, TileType};
use crate::{
    map::{
        assets::load_map_assets,
        components::{
            BouncyPlatform, ColliderShape, DamageTile, FallingTile, LevelTile, PipeTile,
            TileProperties, get_tile_properties_from_path,
        },
    },
    parallax::{
        infinite_parallax_system, setup_parallax_layers, systems::configure_parallax_textures,
    },
};
use bevy_rapier2d::prelude::ActiveEvents;
use tile_systems::*;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<LevelState>()
            .insert_resource(CurrentLevelInfo {
                data: Level::Level1.get_path(),
            })
            .add_systems(OnEnter(LevelState::Loading), load_map_assets)
            .add_systems(
                Update,
                configure_parallax_textures.run_if(in_state(LevelState::Loading)),
            )
            .add_systems(
                OnEnter(LevelState::LevelLoaded),
                (spawn_level_tiles, setup_parallax_layers),
            )
            .add_systems(
                Update,
                (
                    infinite_parallax_system,
                    trigger_falling_tiles_system,
                    falling_tiles_system,
                    bouncy_platforms_system,
                    damage_platforms_system,
                    one_way_platform_collision_system,
                )
                    .run_if(in_state(GameState::Game))
                    .run_if(in_state(LevelState::LevelLoaded)),
            )
            .add_systems(OnExit(GameState::Game), cleanup_level_tiles)
            .add_systems(OnExit(GameState::Game), despawn_parallax_layers);
    }
}
fn despawn_parallax_layers(mut commands: Commands, query: Query<Entity, With<ParallaxLayer>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}
pub fn cleanup_level_tiles(mut commands: Commands, level_query: Query<Entity, With<LevelTile>>) {
    for entity in level_query.iter() {
        commands.entity(entity).despawn();
    }
}

// Sistema que itera sobre los datos del nivel y spawnea los tiles
pub fn spawn_level_tiles(
    mut commands: Commands,
    // Acceder a LevelData y GameAssets como recursos
    level_data: Res<LevelData>,
    game_assets: Res<GameAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(32), 8, 4, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);

    let tile_size_from_json = game_assets.tile_size_px;
    let map_width_from_json = game_assets.map_width_tiles;
    let map_height_from_json = game_assets.map_height_tiles;

    for layer in &level_data.layers {
        for tile_pos_data in &layer.positions {
            let tile_id = tile_pos_data.id;
            let x = tile_pos_data.x as f32;
            let y = tile_pos_data.y as f32;

            let world_x =
                x * tile_size_from_json - (map_width_from_json as f32 * tile_size_from_json / 2.0);
            let world_y = -y * tile_size_from_json
                + (map_height_from_json as f32 * tile_size_from_json / 2.0); // Invertir Y

            let position = Vec3::new(
                world_x + tile_size_from_json / 2.0,
                world_y - tile_size_from_json / 2.0,
                layer.name as f32 * 0.1,
            );

            // Primero intentar mapear por path, luego por tile_id
            let tile_properties = get_tile_properties_from_path(&layer.path);

            // Verificar si es un tile con propiedades especiales
            if let Some(properties) = tile_properties {
                spawn_special_tile(
                    &mut commands,
                    &game_assets,
                    &texture_atlas_layout,
                    tile_id as usize,
                    position,
                    tile_size_from_json,
                    properties,
                );
            };
        }
    }
}

pub const ONE_WAY_PLATFORM_GROUP: Group = Group::GROUP_2;

// Función para spawnear tiles especiales
fn spawn_special_tile(
    commands: &mut Commands,
    game_assets: &GameAssets,
    texture_atlas_layout: &Handle<TextureAtlasLayout>,
    tile_id: usize,
    position: Vec3,
    tile_size: f32,
    properties: TileProperties,
) {
    let mut entity_commands = commands.spawn((
        Sprite {
            image: game_assets.tile_texture.clone(),
            texture_atlas: Some(TextureAtlas {
                layout: texture_atlas_layout.clone(),
                index: tile_id,
            }),
            custom_size: Some(Vec2::splat(tile_size)),
            ..default()
        },
        Transform::from_translation(position),
        RigidBody::Fixed,
        LevelTile,
        properties.clone(),
    ));

    // Agregar collider basado en el tipo de tile
    if let Some(collider_shape) = &properties.custom_collider {
        let collider = create_collider_from_shape(collider_shape, tile_size);
        entity_commands.insert(collider);
    }

    match properties.tile_type {
        TileType::Falling => {
            let mut falling_tile = FallingTile::default();
            falling_tile.original_position = position;
            falling_tile.shake_timer =
                Timer::from_seconds(properties.shake_duration, TimerMode::Once);
            falling_tile.fall_timer = Timer::from_seconds(properties.fall_delay, TimerMode::Once);
            entity_commands.insert(falling_tile);
            entity_commands.insert(ActiveEvents::COLLISION_EVENTS);
            entity_commands.insert(CollisionGroups {
                memberships: ONE_WAY_PLATFORM_GROUP,
                filters: Group::ALL & !PLAYER_GROUP, // La plataforma no filtra a nadie. Su colisión depende del jugador.
            });
        }
        TileType::Damage => {
            let mut damage_tile = DamageTile::default();
            damage_tile.damage_amount = properties.damage;
            entity_commands.insert(damage_tile);
            entity_commands.insert(ActiveEvents::COLLISION_EVENTS);
        }
        TileType::PipeBottomLeft => {
            entity_commands.insert(PipeTile {});
            entity_commands.insert(ActiveEvents::COLLISION_EVENTS);
        }
        TileType::PipeBottomRight => {
            entity_commands.insert(PipeTile {});
            entity_commands.insert(ActiveEvents::COLLISION_EVENTS);
        }
        TileType::Bouncy => {
            let mut bouncy_platform = BouncyPlatform::default();
            bouncy_platform.original_position = position;
            entity_commands.insert(bouncy_platform);
            entity_commands.insert(RigidBody::Dynamic);
            entity_commands.insert(ActiveEvents::COLLISION_EVENTS);
        }
        TileType::Solid => {
            entity_commands.insert(ActiveEvents::COLLISION_EVENTS);
        }
        TileType::EndLevel => {
            entity_commands.insert(ActiveEvents::COLLISION_EVENTS);
        }
    }
}

// Función para crear colliders basados en la forma con posiciones correctas
fn create_collider_from_shape(shape: &ColliderShape, tile_size: f32) -> Collider {
    let half_size = tile_size / 2.0;
    let quarter_size = tile_size / 4.0;

    match shape {
        ColliderShape::FullTile => Collider::cuboid(half_size, half_size),
        ColliderShape::ThinHorizontal => {
            // Línea fina horizontal en la parte superior del tile
            Collider::compound(vec![(
                Vec2::new(0.0, quarter_size + 2.0), // Posición superior
                0.0,                                // Sin rotación
                Collider::cuboid(half_size, 4.0),
            )])
        }
        ColliderShape::HalfVertical => {
            // Media altura en la parte inferior del tile
            Collider::compound(vec![(
                Vec2::new(0.0, -quarter_size), // Posición inferior
                0.0,                           // Sin rotación
                Collider::cuboid(half_size, quarter_size),
            )])
        }
        ColliderShape::QuarterBottomLeft => {
            // Cuarto inferior izquierdo
            Collider::compound(vec![(
                Vec2::new(-quarter_size, -quarter_size), // Posición inferior izquierda
                0.0,                                     // Sin rotación
                Collider::cuboid(quarter_size, quarter_size),
            )])
        }
        ColliderShape::QuarterBottomRight => {
            // Cuarto inferior derecho
            Collider::compound(vec![(
                Vec2::new(quarter_size, -quarter_size), // Posición inferior derecha
                0.0,                                    // Sin rotación
                Collider::cuboid(quarter_size, quarter_size),
            )])
        }
    }
}

pub fn one_way_platform_collision_system(
    mut player_query: Query<
        (&PlayerVelocity, &mut KinematicCharacterController),
        With<PlayerCharacter>,
    >,
) {
    if let Ok((player_velocity, mut character_controller)) = player_query.single_mut() {
        // Si el jugador se está moviendo hacia abajo, habilitamos la colisión con la plataforma.
        if player_velocity.velocity.y < 0.0 {
            println!("____Falling");
            // Reemplazamos el filtro con uno nuevo que incluye el grupo de la plataforma.
            character_controller.filter_groups = Some(CollisionGroups { ..default() });
        } else {
            println!("____Jumping");
            // Si no se está moviendo hacia abajo, el filtro se restablece para ignorar la plataforma.
            character_controller.filter_groups = Some(CollisionGroups {
                memberships: PLAYER_GROUP,
                filters: Group::ALL & !ONE_WAY_PLATFORM_GROUP,
            });
        }
    }
}
