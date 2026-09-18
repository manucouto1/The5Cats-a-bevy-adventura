pub mod assets;
pub mod components;
pub mod fade;
pub mod tile_systems;
pub mod zones;

use bevy::prelude::*;

use crate::game_state::{GameState, LevelState};
use crate::map::components::TileType;
use crate::parallax::components::ParallaxLayer;
use crate::physics::Velocity as PlayerVelocity;
use crate::player::components::PlayerCharacter;
use crate::player::{ENEMY_GROUP, PLAYER_GROUP};
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
use assets::GameAssets;
use bevy_rapier2d::prelude::ActiveEvents;
use bevy_rapier2d::prelude::{
    Collider, CollisionGroups, Group, KinematicCharacterController, RigidBody, Sensor,
};
use components::LevelData;
use tile_systems::*;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<LevelState>()
            .add_plugins(bevy::sprite::Material2dPlugin::<fade::MapFadeMaterial>::default())
            .init_resource::<zones::LevelMode>()
            .init_resource::<zones::ActiveBand>()
            .init_resource::<zones::BossFallArmed>()
            .init_resource::<crate::parallax::systems::CameraSnapRequested>()
            .init_resource::<crate::parallax::systems::CameraMode>()
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
                (
                    spawn_level_tiles,
                    spawn_level_events,
                    setup_parallax_layers,
                    fade::spawn_map_edge_fade,
                    crate::parallax::systems::request_camera_snap,
                    zones::spawn_shaft_seal.after(spawn_level_tiles),
                ),
            )
            .add_systems(
                Update,
                (
                    infinite_parallax_system,
                    fade::track_camera_map_edge_fade,
                    trigger_falling_tiles_system,
                    falling_tiles_system,
                    bouncy_platforms_system,
                    damage_platforms_system,
                    one_way_platform_collision_system,
                    tile_systems::end_level_trigger_system,
                    zones::level_zone_trigger_system,
                    zones::enemy_falling_mode_system,
                    zones::unseal_shaft_system,
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

pub fn spawn_level_tiles(
    mut commands: Commands,
    level_data: Res<LevelData>,
    game_assets: Res<GameAssets>,
    images: Res<Assets<Image>>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let tile_size = game_assets.tile_size_px;
    let tile_size_u = tile_size as u32;
    let (cols, rows) = images
        .get(&game_assets.tile_texture)
        .map(|img| {
            let size = img.size();
            (size.x / tile_size_u, size.y / tile_size_u)
        })
        .unwrap_or((8, 4));
    let layout = TextureAtlasLayout::from_grid(UVec2::splat(tile_size_u), cols, rows, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);

    let map_w = game_assets.map_width_tiles as f32;
    let map_h = game_assets.map_height_tiles as f32;
    let offset_x = -(map_w * tile_size / 2.0);
    let offset_y = map_h * tile_size / 2.0;

    for layer in &level_data.layers {
        // Every level json layer carries a semantic `path` (`ground`,
        // `damage`, `falling`, `bouncy`, `pipe_left`, `pipe_right`,
        // `end_level`, `decoration`). The pygame-export `Platform` /
        // `FallingPlatform` paths have all been rewritten to those
        // semantic values. `decoration` and any unknown path → no collider.
        let properties = get_tile_properties_from_path(&layer.path);
        for tile_pos_data in &layer.positions {
            let tile_id = tile_pos_data.id;
            let x = tile_pos_data.x;
            let y = tile_pos_data.y;
            let position = Vec3::new(
                x as f32 * tile_size + offset_x + tile_size / 2.0,
                -(y as f32 * tile_size) + offset_y - tile_size / 2.0,
                layer.name as f32 * 0.1,
            );

            // Decoration / unknown path: sprite-only, no collider.
            let Some(properties) = properties.clone() else {
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
                continue;
            };

            if properties.tile_type == TileType::Solid {
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
                    RigidBody::Fixed,
                    Collider::cuboid(tile_size / 2.0, tile_size / 2.0),
                    LevelTile,
                    // Required so enemy-projectile sensors can despawn on
                    // ground impact via projectile_terrain_collision_system.
                    ActiveEvents::COLLISION_EVENTS,
                ));
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
}

pub const ONE_WAY_PLATFORM_GROUP: Group = Group::GROUP_2;

/// Clears the "shaft armed" flag and the level mode on every level load —
/// each level 4 run starts with the shaft sealed until the boss finishes
/// phase 1 again.
fn reset_boss_fall_armed(
    mut armed: ResMut<zones::BossFallArmed>,
    mut mode: ResMut<zones::LevelMode>,
) {
    armed.0 = false;
    *mode = zones::LevelMode::Normal;
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
    /// Tileset index drawn for the trigger (the goal tile).
    #[serde(default)]
    id: Option<u32>,
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
    level_data: Res<LevelData>,
    images: Res<Assets<Image>>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    // Levels whose tilemap already carries an `end_level` layer draw the
    // goal themselves; for the others the events file supplies the tile id.
    let map_has_goal_layer = level_data.layers.iter().any(|l| l.path == "end_level");
    let tile_size_u = game_assets.tile_size_px as u32;
    let (cols, rows) = images
        .get(&game_assets.tile_texture)
        .map(|img| (img.size().x / tile_size_u, img.size().y / tile_size_u))
        .unwrap_or((8, 4));
    let atlas_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(tile_size_u),
        cols,
        rows,
        None,
        None,
    ));
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

        let world_x = ev.x as f32 * tile_size - (map_w * tile_size / 2.0) + tile_size / 2.0;
        let world_y = -(ev.y as f32) * tile_size + (map_h * tile_size / 2.0) - tile_size / 2.0;

        let mut entity = commands.spawn((
            Transform::from_xyz(world_x, world_y, 0.9),
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
        if let (false, Some(id)) = (map_has_goal_layer, ev.id) {
            entity.insert(Sprite {
                image: game_assets.tile_texture.clone(),
                texture_atlas: Some(TextureAtlas {
                    layout: atlas_layout.clone(),
                    index: id as usize,
                }),
                custom_size: Some(Vec2::splat(tile_size)),
                ..default()
            });
        }
    }
}

fn spawn_special_tile(
    commands: &mut Commands,
    game_assets: &GameAssets,
    texture_atlas_layout: &Handle<TextureAtlasLayout>,
    tile_id: usize,
    position: Vec3,
    tile_size: f32,
    properties: TileProperties,
) {
    // Hazardous tiles (damage / falling / bouncy / pipe / end_level) must
    // be visible to the player — otherwise an invisible spike under a
    // decoration sprite reads as "I'm taking damage from nothing". Bump
    // their z above the decoration band (which uses layer.name * 0.1, so
    // up to ~0.5 in current levels). Player and enemies live at z >= 5.0,
    // so they still render in front.
    let position = Vec3::new(position.x, position.y, 0.9);
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
                filters: Group::ALL & !PLAYER_GROUP, // One-way: the player decides, see one_way_platform_collision_system.
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

fn create_collider_from_shape(shape: &ColliderShape, tile_size: f32) -> Collider {
    let half_size = tile_size / 2.0;
    let quarter_size = tile_size / 4.0;

    match shape {
        ColliderShape::FullTile => Collider::cuboid(half_size, half_size),
        ColliderShape::ThinHorizontal => Collider::compound(vec![(
            Vec2::new(0.0, quarter_size + 2.0),
            0.0,
            Collider::cuboid(half_size, 4.0),
        )]),
        ColliderShape::HalfVertical => Collider::compound(vec![(
            Vec2::new(0.0, -quarter_size),
            0.0,
            Collider::cuboid(half_size, quarter_size),
        )]),
        ColliderShape::QuarterBottomLeft => Collider::compound(vec![(
            Vec2::new(-quarter_size, -quarter_size),
            0.0,
            Collider::cuboid(quarter_size, quarter_size),
        )]),
        ColliderShape::QuarterBottomRight => Collider::compound(vec![(
            Vec2::new(quarter_size, -quarter_size),
            0.0,
            Collider::cuboid(quarter_size, quarter_size),
        )]),
    }
}

pub fn one_way_platform_collision_system(
    mut player_query: Query<
        (&PlayerVelocity, &mut KinematicCharacterController),
        (
            With<PlayerCharacter>,
            Without<crate::player::components::Dead>,
        ),
    >,
) {
    if let Ok((player_velocity, mut character_controller)) = player_query.single_mut() {
        // Enemies are never solid for the controller (see ENEMY_GROUP).
        if player_velocity.velocity.y < 0.0 {
            // Falling: allow collision with one-way platforms so we land on
            // them. One-way tiles filter out PLAYER_GROUP, so the controller
            // must present the full membership set here.
            character_controller.filter_groups = Some(CollisionGroups {
                memberships: Group::ALL,
                filters: Group::ALL & !ENEMY_GROUP,
            });
        } else {
            // Rising or stationary: ignore one-way platforms so we can pass up through them.
            character_controller.filter_groups = Some(CollisionGroups {
                memberships: PLAYER_GROUP,
                filters: Group::ALL & !ONE_WAY_PLATFORM_GROUP & !ENEMY_GROUP,
            });
        }
    }
}
