//! 2D lighting: the port of the reptile_studio shader pipeline.
//!
//! Strategy `gi` from the original, end to end — Inject → Propagate(N) →
//! Temporal → Blur feeding a final composite that does raymarched soft
//! shadows, ambient occlusion, in-scattered haze and inward tile falloff.
//! `passes.rs` wires the off-screen chain, `materials.rs` holds the bind
//! groups, `mask.rs` rasterizes the occluders, and `level.rs` parses the
//! per-level `levelN_lights.json`.
//!
//! The one structural difference from the original: our level geometry
//! never moves, so the occluder mask and its SDF are built once on the CPU
//! at load and cover the whole map, instead of being re-rendered per frame
//! for the viewport.

pub mod level;
pub mod mask;
pub mod materials;
pub mod passes;

use bevy::{
    prelude::*,
    sprite::{Material2dPlugin, MeshMaterial2d},
};

use crate::{
    game_state::{CurrentLevel, LevelState},
    lighting::{
        level::{LevelLighting, LightEntry, LightKind, MAX_LIGHTS, load_level_lighting},
        mask::{SDF_MAX_PX, build_level_mask},
        materials::{
            LightBlurMaterial, LightCompositeMaterial, LightFinalMaterial, LightInjectMaterial,
            LightPropagateMaterial, LightTemporalMaterial,
        },
        passes::{LightingPassEntity, LightingRuntime, PassAssets, spawn_lighting_chain},
    },
    map::components::LevelData,
    parallax::components::{MainCamera, view_half_extents},
    player::components::PlayerCharacter,
};

/// Drawn above the level and the characters but below the map-edge fade,
/// so the fade still swallows whatever lies outside the level.
const COMPOSITE_Z: f32 = 45.0;

/// A `sun` is a point light parked this far outside the view.
const SUN_DISTANCE: f32 = 6000.0;

/// Marker on the full-screen composite quad.
#[derive(Component)]
pub struct LightingComposite;

/// Off by default: the levels are hand-painted with their own light baked
/// into the art, and a physically-shaded pass on top fights it instead of
/// adding to it. The pipeline stays available for experiments —
/// `THE5CATS_LIGHTING=1` builds it at level load, F10 toggles it from
/// there — but nothing about the default build touches it.
#[derive(Resource)]
pub struct LightingEnabled(pub bool);

impl Default for LightingEnabled {
    fn default() -> Self {
        Self(
            std::env::var("THE5CATS_LIGHTING")
                .map(|v| v == "1")
                .unwrap_or(false),
        )
    }
}

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            Material2dPlugin::<LightInjectMaterial>::default(),
            Material2dPlugin::<LightPropagateMaterial>::default(),
            Material2dPlugin::<LightTemporalMaterial>::default(),
            Material2dPlugin::<LightBlurMaterial>::default(),
            Material2dPlugin::<LightFinalMaterial>::default(),
            Material2dPlugin::<LightCompositeMaterial>::default(),
        ))
        .init_resource::<LightingEnabled>()
        .add_systems(OnEnter(LevelState::LevelLoaded), setup_level_lighting)
        .add_systems(OnExit(LevelState::LevelLoaded), teardown_level_lighting)
        .add_systems(
            Update,
            (toggle_lighting, update_lighting_uniforms)
                .chain()
                .run_if(in_state(LevelState::LevelLoaded)),
        );
    }
}

fn setup_level_lighting(
    mut commands: Commands,
    current_level: Res<CurrentLevel>,
    level_data: Res<LevelData>,
    game_assets: Res<crate::map::assets::GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut inject: ResMut<Assets<LightInjectMaterial>>,
    mut propagate: ResMut<Assets<LightPropagateMaterial>>,
    mut temporal: ResMut<Assets<LightTemporalMaterial>>,
    mut blur: ResMut<Assets<LightBlurMaterial>>,
    mut finals: ResMut<Assets<LightFinalMaterial>>,
    mut composite_materials: ResMut<Assets<LightCompositeMaterial>>,
    enabled: Res<LightingEnabled>,
) {
    if !enabled.0 {
        // Nothing spawned at all: no mask, no SDF, no off-screen cameras,
        // no per-frame cost.
        return;
    }
    let path = current_level.0.get_path().lights;
    let Some(config) = load_level_lighting(&path) else {
        info!("No lighting file for this level ({path}) — running unlit");
        return;
    };
    if !config.is_enabled() {
        return;
    }

    // Nearest parallax layer: the silhouette the sun gets shadowed by.
    let backdrop = game_assets
        .parallax_backgrounds
        .iter()
        .take(5)
        .last()
        .cloned()
        .unwrap_or_default();

    let built = build_level_mask(&level_data);
    let (origin, size) = (built.origin, built.size);
    let mask = images.add(built.mask);
    let sdf = images.add(built.sdf);

    let mut assets = PassAssets {
        images: &mut images,
        meshes: &mut meshes,
        inject: &mut inject,
        propagate: &mut propagate,
        temporal: &mut temporal,
        blur: &mut blur,
        finals: &mut finals,
        composites: &mut composite_materials,
    };
    let runtime = spawn_lighting_chain(
        &mut commands,
        &mut assets,
        &config.gi,
        mask,
        sdf,
        backdrop,
        origin,
        size,
        SDF_MAX_PX,
    );

    commands.spawn((
        LightingComposite,
        LightingPassEntity,
        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial2d(runtime.composite_material.clone()),
        Transform::from_xyz(0.0, 0.0, COMPOSITE_Z),
        if enabled.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
    ));

    info!(
        "Lighting: {} authored light(s){}, GI {}",
        config.lights.len(),
        if config.player_light.is_some() {
            " + player"
        } else {
            ""
        },
        if runtime.grid.is_some() { "on" } else { "off" },
    );
    commands.insert_resource(config);
    commands.insert_resource(runtime);
}

fn teardown_level_lighting(
    mut commands: Commands,
    entities: Query<Entity, With<LightingPassEntity>>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<LightingRuntime>();
    commands.remove_resource::<LevelLighting>();
}

/// F10 hides the composite *and* stops the off-screen chain, so turning
/// the lighting off actually gives the frame time back.
fn toggle_lighting(
    keys: Res<ButtonInput<KeyCode>>,
    mut enabled: ResMut<LightingEnabled>,
    mut composites: Query<&mut Visibility, With<LightingComposite>>,
    mut pass_cameras: Query<&mut Camera, With<LightingPassEntity>>,
) {
    if keys.just_pressed(KeyCode::F10) {
        if composites.is_empty() {
            info!("Lighting is not built this run — start with THE5CATS_LIGHTING=1");
            return;
        }
        enabled.0 = !enabled.0;
        info!("Lighting {}", if enabled.0 { "on" } else { "off" });
    }
    // Re-applied every frame rather than only on the keypress: it is a
    // handful of entities, and it also catches the chain a level load
    // spawned after the last toggle.
    for mut visibility in &mut composites {
        *visibility = if enabled.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut camera in &mut pass_cameras {
        camera.is_active = enabled.0;
    }
}

/// One light packed the way the shaders read it.
struct Packed {
    a: Vec4,
    b: Vec4,
    c: Vec4,
    /// Distance to the view centre, used to pick the closest 8.
    distance: f32,
}

fn pack_light(entry: &LightEntry, position: Vec2, tile_size: f32, view_center: Vec2) -> Packed {
    let falloff_px = entry.falloff * tile_size;
    Packed {
        a: Vec4::new(position.x, position.y, entry.kind.shader_id(), entry.angle),
        b: Vec4::new(
            entry.cone_angle,
            entry.intensity,
            if entry.kind == LightKind::Sun {
                0.0
            } else {
                falloff_px
            },
            entry.softness,
        ),
        c: Vec4::new(
            entry.color[0],
            entry.color[1],
            entry.color[2],
            entry.interior_depth,
        ),
        distance: position.distance(view_center),
    }
}

/// Tile coordinates → world, the same mapping the tilemap spawner uses.
fn tile_to_world(x: f32, y: f32, level: &LevelData) -> Vec2 {
    let tile = level.tile_size as f32;
    Vec2::new(
        x * tile - (level.map_width as f32 * tile / 2.0) + tile / 2.0,
        -y * tile + (level.map_height as f32 * tile / 2.0) - tile / 2.0,
    )
}

/// Feeds every pass the camera's world rect and the lights that matter
/// this frame. Runs after `camera_follow_system` so the rect is the one
/// the frame will actually draw.
fn update_lighting_uniforms(
    runtime: Option<Res<LightingRuntime>>,
    config: Option<Res<LevelLighting>>,
    level_data: Option<Res<LevelData>>,
    enabled: Res<LightingEnabled>,
    camera: Query<(&Transform, &Projection), With<MainCamera>>,
    player: Query<
        &Transform,
        (
            With<PlayerCharacter>,
            Without<MainCamera>,
            Without<LightingComposite>,
        ),
    >,
    mut inject: ResMut<Assets<LightInjectMaterial>>,
    mut propagate: ResMut<Assets<LightPropagateMaterial>>,
    mut finals: ResMut<Assets<LightFinalMaterial>>,
    mut composites: Query<
        &mut Transform,
        (
            With<LightingComposite>,
            Without<MainCamera>,
            Without<PlayerCharacter>,
        ),
    >,
    backdrop_layer: Query<&crate::parallax::components::ParallaxLayer>,
) {
    let (Some(runtime), Some(config), Some(level_data)) = (runtime, config, level_data) else {
        return;
    };
    if !enabled.0 {
        return;
    }
    let Ok((camera_transform, projection)) = camera.single() else {
        return;
    };
    let center = camera_transform.translation.truncate();
    let half = view_half_extents(projection);
    let view = Vec4::new(
        center.x - half.x,
        center.y - half.y,
        half.x * 2.0,
        half.y * 2.0,
    );
    let tile = level_data.tile_size as f32;

    // The composite covers exactly what the camera sees, like the
    // map-edge fade quad.
    for mut transform in &mut composites {
        transform.translation = Vec3::new(center.x, center.y, COMPOSITE_Z);
        transform.scale = Vec3::new(half.x * 2.0, half.y * 2.0, 1.0);
    }

    // The player's light first (it is the one that must never be culled),
    // then the authored lights nearest the view.
    let mut packed: Vec<Packed> = Vec::new();
    if let (Some(entry), Ok(player_tf)) = (config.player_light.as_ref(), player.single()) {
        let mut light = pack_light(entry, player_tf.translation.truncate(), tile, center);
        light.distance = f32::NEG_INFINITY;
        packed.push(light);
    }
    for entry in &config.lights {
        let position = match entry.kind {
            LightKind::Sun => {
                let rad = entry.angle.to_radians();
                center + Vec2::new(rad.cos(), rad.sin()) * SUN_DISTANCE
            }
            _ => tile_to_world(entry.x, entry.y, &level_data),
        };
        packed.push(pack_light(entry, position, tile, center));
    }
    packed.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    packed.truncate(MAX_LIGHTS);

    let mut lights_a = [Vec4::ZERO; MAX_LIGHTS];
    let mut lights_b = [Vec4::ZERO; MAX_LIGHTS];
    let mut lights_c = [Vec4::ZERO; MAX_LIGHTS];
    for (i, light) in packed.iter().enumerate() {
        lights_a[i] = light.a;
        lights_b[i] = light.b;
        lights_c[i] = light.c;
    }
    let count = packed.len() as f32;

    if let Some(handle) = &runtime.inject {
        if let Some(material) = inject.get_mut(handle) {
            material.view = view;
            material.params.y = count;
            material.lights_a = lights_a;
            material.lights_b = lights_b;
            material.lights_c = lights_c;
        }
    }
    for handle in &runtime.propagate {
        if let Some(material) = propagate.get_mut(handle) {
            material.view = view;
        }
    }
    if let Some(material) = finals.get_mut(&runtime.final_material) {
        let shading = &config.shading;
        material.view = view;
        material.shading_a = Vec4::new(
            shading.ambient,
            shading.humidity,
            shading.ao_strength,
            shading.ao_radius,
        );
        material.shading_b = Vec4::new(
            shading.haze_strength,
            shading.tile_offset,
            shading.tile_min_brightness,
            shading.curve_id(),
        );
        material.shadow = Vec4::new(
            if shading.cast_shadows { 1.0 } else { 0.0 },
            shading.shadow_softness,
            shading.backdrop_shadows.clamp(0.0, 1.0),
            0.0,
        );
        // Mirror the transform the nearest parallax layer is drawn with.
        if let Some(layer) = backdrop_layer.iter().max_by(|a, b| a.z.total_cmp(&b.z)) {
            let (scale, offset) =
                crate::parallax::systems::layer_uv_transform(layer, center, half * 2.0);
            material.backdrop_uv = Vec4::new(scale.x, scale.y, offset.x, offset.y);
        }
        material.params = Vec4::new(
            SDF_MAX_PX,
            count,
            config.gi.strength,
            if runtime.grid.is_some() { 1.0 } else { 0.0 },
        );
        material.lights_a = lights_a;
        material.lights_b = lights_b;
        material.lights_c = lights_c;
    }
}
