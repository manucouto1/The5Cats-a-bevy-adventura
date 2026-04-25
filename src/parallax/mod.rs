pub mod components;
pub mod systems;

use bevy::{math::Affine2, prelude::*};

use crate::{map::assets::GameAssets, parallax::components::ParallaxLayer};

pub use systems::infinite_parallax_system;

/// Mesh dimensions for every parallax layer. Picked to comfortably exceed
/// the 1280x720 viewport even after parallax drift, so the simple
/// "layer follows camera with X parallax" code in `infinite_parallax_system`
/// never lets the mesh edge slip into view. The texture is tiled across
/// this mesh via `PARALLAX_UV_TILES_X`.
const PARALLAX_MESH_WIDTH: f32 = 4096.0;
const PARALLAX_MESH_HEIGHT: f32 = 1440.0;
const PARALLAX_UV_TILES_X: f32 = 4.0;

pub fn setup_parallax_layers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    game_assets: Res<GameAssets>,
) {
    let layer_count = game_assets.parallax_backgrounds.len();
    let mesh = meshes.add(Rectangle::new(PARALLAX_MESH_WIDTH, PARALLAX_MESH_HEIGHT));
    for (index, parallax_bg) in game_assets.parallax_backgrounds.iter().enumerate() {
        // Index 0 is the deepest layer and parallaxes the most; the front
        // layer barely moves relative to the camera.
        let depth = (layer_count - index) as f32 / layer_count as f32;
        let scroll_x = 0.6 * depth;
        commands.spawn((
            Mesh2d(mesh.clone()),
            MeshMaterial2d(materials.add(ColorMaterial {
                texture: Some(parallax_bg.clone()),
                uv_transform: Affine2::from_scale(Vec2::new(PARALLAX_UV_TILES_X, 1.0)),
                ..default()
            })),
            ParallaxLayer {
                // Y locked to camera (no vertical parallax). Vertical
                // parallax made the sky drift up the screen as the camera
                // descended in level 4-style sequences.
                scroll_factor: Vec2::new(scroll_x, 0.0),
                start_position: Vec3::new(0.0, 0.0, -100.0 - index as f32),
            },
            // Initial transform — `infinite_parallax_system` overwrites it
            // on the first frame, but spawning at z avoids a one-frame flash
            // at z=0 in front of the gameplay layer.
            Transform::from_xyz(0.0, 0.0, -100.0 - index as f32),
        ));
    }
}
