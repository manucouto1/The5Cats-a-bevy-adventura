//! Map-edge fade.
//!
//! The pygame build ran an 800x800 window and clamped the camera to the
//! map, so the level always filled the screen. Our viewport is wider than
//! some levels are (level 3 is 25 tiles = 800 px across against a ~1088 px
//! view) and taller than the shots level 4 frames, so the camera cannot
//! always stay inside the map and raw parallax shows past its edges.
//!
//! Instead of cropping the view, a full-screen quad glued to the camera
//! darkens everything outside the map bounds with a soft ramp, so the level
//! dissolves into the background rather than ending on a hard tile edge.

use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef},
    sprite::{AlphaMode2d, Material2d, MeshMaterial2d},
};

use crate::{
    map::{assets::GameAssets, components::LevelTile},
    parallax::components::{MainCamera, view_half_extents},
};

const SHADER_PATH: &str = "shaders/map_fade.wgsl";

/// How far past the map edge the fade takes to reach full strength.
const FADE_WIDTH: f32 = 140.0;
/// How far *inside* the map the ramp starts, so the outermost tile row
/// blends out instead of ending abruptly.
const FADE_INSET: f32 = 12.0;
/// Never fully opaque — a hint of the parallax stays visible, which reads
/// as depth rather than as a black border.
const MAX_ALPHA: f32 = 0.96;
/// What the outside fades to: near-black with a touch of blue, so it sits
/// with the night backgrounds instead of looking like a dead pixel region.
/// Declared in sRGB and converted below — the shader writes straight to a
/// linear render target, and feeding it sRGB numbers makes the "dark" edge
/// come out lighter than the level it is supposed to swallow.
const FADE_COLOR: Color = Color::srgb(0.05, 0.05, 0.09);
/// Above tiles (z <= 0.9), player and enemies (z 5..10), below the cursor
/// (z 100).
const FADE_Z: f32 = 50.0;

#[derive(Component)]
pub struct MapEdgeFade;

/// Three plain `vec4` uniforms rather than one struct: `ShaderType`'s derive
/// would pull in a size-check function per field that nothing ever calls.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct MapFadeMaterial {
    /// (min_x, min_y, max_x, max_y) of the map in world space.
    #[uniform(0)]
    pub bounds: Vec4,
    /// (fade_width, max_alpha, inset, unused)
    #[uniform(1)]
    pub params: Vec4,
    /// Linear RGB the outside fades to.
    #[uniform(2)]
    pub color: Vec4,
}

impl Material2d for MapFadeMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

pub fn spawn_map_edge_fade(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MapFadeMaterial>>,
    game_assets: Res<GameAssets>,
) {
    let map_w = game_assets.map_width_tiles as f32 * game_assets.tile_size_px;
    let map_h = game_assets.map_height_tiles as f32 * game_assets.tile_size_px;
    commands.spawn((
        MapEdgeFade,
        LevelTile,
        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
        // Camera-sized quad: same reason as the parallax layers.
        bevy::render::view::NoFrustumCulling,
        MeshMaterial2d(materials.add(MapFadeMaterial {
            // Map is centered on the origin.
            bounds: Vec4::new(-map_w / 2.0, -map_h / 2.0, map_w / 2.0, map_h / 2.0),
            params: Vec4::new(FADE_WIDTH, MAX_ALPHA, FADE_INSET, 0.0),
            color: {
                let c = LinearRgba::from(FADE_COLOR);
                Vec4::new(c.red, c.green, c.blue, 1.0)
            },
        })),
        Transform::from_xyz(0.0, 0.0, FADE_Z),
    ));
}

/// Keeps the quad covering exactly what the camera sees. The fade itself is
/// computed in world space by the shader, so only the quad's placement has
/// to follow the camera.
pub fn track_camera_map_edge_fade(
    camera: Query<(&Transform, &Projection), With<MainCamera>>,
    mut fades: Query<&mut Transform, (With<MapEdgeFade>, Without<MainCamera>)>,
) {
    let Ok((camera_transform, projection)) = camera.single() else {
        return;
    };
    let cam = camera_transform.translation.truncate();
    // A couple of px of slack so rounding never leaves an unpainted seam at
    // the viewport border.
    let view = view_half_extents(projection) * 2.0 + 4.0;
    if view.x <= 0.0 || view.y <= 0.0 {
        return;
    }
    for mut transform in &mut fades {
        transform.translation = Vec3::new(cam.x, cam.y, FADE_Z);
        transform.scale = Vec3::new(view.x, view.y, 1.0);
    }
}
