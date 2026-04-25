pub mod assets;
pub mod components;
pub mod tile_systems;
pub mod zones;

use bevy::prelude::*;

use crate::game_state::{GameState, LevelState};
use crate::parallax::components::ParallaxLayer;
use crate::physics::Velocity as PlayerVelocity;
use crate::player::PLAYER_GROUP;
use crate::player::components::PlayerCharacter;
use assets::GameAssets;
use bevy_rapier2d::prelude::{
    Collider, CollisionGroups, Group, KinematicCharacterController, RigidBody, Sensor,
};
use components::LevelData;
use crate::map::components::TileType;
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
            .init_resource::<zones::LevelMode>()
            .init_resource::<zones::BossFallArmed>()
            .add_event::<crate::game_state::LevelCompleteEvent>()
            .add_systems(
                OnEnter(LevelState::Loading),
                (
                    load_map_assets,
                    zones::load_level_zones.after(load_map_assets),
                    reset_boss_fall_armed,
                ),
            )
            .add_systems(
                Update,
                configure_parallax_textures.run_if(in_state(LevelState::Loading)),
            )
            .add_systems(
                OnEnter(LevelState::LevelLoaded),
                (spawn_level_tiles, spawn_level_events, setup_parallax_layers),
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
                    tile_systems::end_level_trigger_system,
                    zones::level_zone_trigger_system,
                )
                    .run_if(in_state(GameState::Game))
                    .run_if(in_state(LevelState::LevelLoaded)),
            )
            .add_systems(
                OnExit(LevelState::LevelLoaded),
                (cleanup_level_tiles, despawn_parallax_layers),
            );
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
    level_data: Res<LevelData>,
    game_assets: Res<GameAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(32), 8, 4, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);

    let tile_size = game_assets.tile_size_px;
    let map_w = game_assets.map_width_tiles as f32;
    let map_h = game_assets.map_height_tiles as f32;
    let offset_x = -(map_w * tile_size / 2.0);
    let offset_y = map_h * tile_size / 2.0;

    // Collect solid-tile grid positions across all layers so we can merge
    // them into fewer, larger colliders in a single pass below.
    let mut solid_cells: std::collections::HashSet<(u32, u32)> =
        std::collections::HashSet::new();

    for layer in &level_data.layers {
        for tile_pos_data in &layer.positions {
            let Some(properties) = get_tile_properties_from_path(&layer.path) else {
                continue;
            };

            let tile_id = tile_pos_data.id;
            let x = tile_pos_data.x;
            let y = tile_pos_data.y;
            let position = Vec3::new(
                x as f32 * tile_size + offset_x + tile_size / 2.0,
                -(y as f32 * tile_size) + offset_y - tile_size / 2.0,
                layer.name as f32 * 0.1,
            );

            if properties.tile_type == TileType::Solid {
                // Sprite only — the collider is produced by the merge pass.
                commands.spawn((
                    Sprite {
                        image: game_assets.tile_texture.clone(),
                        texture_atlas: Some(TextureAtlas {
                            layout: texture_atlas_layout.clone(),
                            index: tile_id as usize,
                        }),
                        custom_size: Some(Vec2::splat(tile_size)),
                        ..default()
                    },
                    Transform::from_translation(position),
                    LevelTile,
                ));
                solid_cells.insert((x, y));
            } else {
                spawn_special_tile(
                    &mut commands,
                    &game_assets,
                    &texture_atlas_layout,
                    tile_id as usize,
                    position,
                    tile_size,
                    properties,
                );
            }
        }
    }

    spawn_merged_solid_colliders(
        &mut commands,
        &solid_cells,
        tile_size,
        offset_x,
        offset_y,
    );
}

/// Merges a grid of solid tile positions into the smallest number of
/// axis-aligned rectangles, then spawns one Rapier `Fixed` cuboid collider
/// per rectangle. Greedy: for each unconsumed cell, extend right as far as
/// possible, then extend down while every row below has the same extent.
/// Good but not optimal — usually collapses a few hundred tile-sized
/// colliders into a few dozen larger ones, which is the win for Rapier's
/// broadphase.
fn spawn_merged_solid_colliders(
    commands: &mut Commands,
    solid: &std::collections::HashSet<(u32, u32)>,
    tile_size: f32,
    offset_x: f32,
    offset_y: f32,
) {
    let mut cells: Vec<(u32, u32)> = solid.iter().copied().collect();
    cells.sort_by_key(|&(x, y)| (y, x));

    let mut consumed: std::collections::HashSet<(u32, u32)> =
        std::collections::HashSet::new();
    let mut rect_count = 0usize;

    for &(x, y) in &cells {
        if consumed.contains(&(x, y)) {
            continue;
        }

        let mut w: u32 = 1;
        while solid.contains(&(x + w, y)) && !consumed.contains(&(x + w, y)) {
            w += 1;
        }

        let mut h: u32 = 1;
        'grow: loop {
            let ny = y + h;
            for dx in 0..w {
                if !solid.contains(&(x + dx, ny)) || consumed.contains(&(x + dx, ny)) {
                    break 'grow;
                }
            }
            h += 1;
        }

        for dy in 0..h {
            for dx in 0..w {
                consumed.insert((x + dx, y + dy));
            }
        }

        let left = x as f32 * tile_size + offset_x;
        let top = -(y as f32 * tile_size) + offset_y;
        let half_w = w as f32 * tile_size / 2.0;
        let half_h = h as f32 * tile_size / 2.0;
        let center = Vec3::new(left + half_w, top - half_h, 0.0);

        commands.spawn((
            Transform::from_translation(center),
            RigidBody::Fixed,
            Collider::cuboid(half_w, half_h),
            LevelTile,
        ));
        rect_count += 1;
    }

    info!(
        "Merged {} solid tiles into {} collider rectangles",
        solid.len(),
        rect_count
    );
}

pub const ONE_WAY_PLATFORM_GROUP: Group = Group::GROUP_2;

/// Clears the "shaft armed" flag on every level load — each level 4 run
/// starts with the shaft sealed until the boss finishes phase 1 again.
fn reset_boss_fall_armed(mut armed: ResMut<zones::BossFallArmed>) {
    armed.0 = false;
}

/// Shape of a `levelN_events.json` file. We only care about `path`, `x`, `y`;
/// other fields (`scale`, `id`) are ignored.
#[derive(serde::Deserialize)]
struct EventsFile {
    events: Vec<EventEntry>,
}

#[derive(serde::Deserialize)]
struct EventEntry {
    path: String,
    x: u32,
    y: u32,
}

/// Spawns event triggers declared in `levelN_events.json` as invisible
/// sensor tiles. The only currently-handled event is `EndLevel` (the goal
/// tile used by levels 2+, where the tilemap itself has no `end_level`
/// layer). Level 1 also has this file but its tilemap already contains
/// the goal tile, so both spawn at the same spot — harmless overlap.
pub fn spawn_level_events(
    mut commands: Commands,
    current_level: Res<crate::game_state::CurrentLevel>,
    game_assets: Res<GameAssets>,
) {
    let paths = current_level.0.get_path();
    let Ok(raw) = std::fs::read_to_string(&paths.events) else {
        return; // missing events file is not fatal
    };
    let Ok(parsed): Result<EventsFile, _> = serde_json::from_str(&raw) else {
        warn!("Failed to parse events file {}", paths.events);
        return;
    };

    let tile_size = game_assets.tile_size_px;
    let map_w = game_assets.map_width_tiles as f32;
    let map_h = game_assets.map_height_tiles as f32;

    for ev in parsed.events {
        // Accept both the short name ("EndLevel") and the pygame-qualified
        // path ("src.sprites.passive.event.EndLevel").
        let is_end_level = ev.path == "EndLevel" || ev.path.ends_with(".EndLevel");
        if !is_end_level {
            continue;
        }

        let world_x =
            ev.x as f32 * tile_size - (map_w * tile_size / 2.0) + tile_size / 2.0;
        let world_y =
            -(ev.y as f32) * tile_size + (map_h * tile_size / 2.0) - tile_size / 2.0;

        commands.spawn((
            Transform::from_xyz(world_x, world_y, 0.0),
            RigidBody::Fixed,
            Collider::cuboid(tile_size / 2.0, tile_size / 2.0),
            Sensor,
            ActiveEvents::COLLISION_EVENTS,
            LevelTile,
            TileProperties {
                tile_type: TileType::EndLevel,
                ..Default::default()
            },
        ));
    }
}

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
            // Sensor so the player walks through it rather than being blocked;
            // collision events still fire and drive the level-complete trigger.
            entity_commands.insert(Sensor);
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
        if player_velocity.velocity.y < 0.0 {
            // Falling: allow collision with one-way platforms so we land on them.
            character_controller.filter_groups = Some(CollisionGroups { ..default() });
        } else {
            // Rising or stationary: ignore one-way platforms so we can pass up through them.
            character_controller.filter_groups = Some(CollisionGroups {
                memberships: PLAYER_GROUP,
                filters: Group::ALL & !ONE_WAY_PLATFORM_GROUP,
            });
        }
    }
}
