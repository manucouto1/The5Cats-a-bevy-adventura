pub mod components;
pub mod systems;

use bevy::{math::Affine2, prelude::*};

use crate::{
    game_state::CurrentLevel, map::assets::GameAssets, parallax::components::ParallaxLayer,
};

pub use systems::infinite_parallax_system;

/// Pygame moved the nearest layer 1.5 px per frame for roughly 5 px of
/// camera travel and divided that by the layer's depth factor (5..1), i.e.
/// 0.3 / factor. Index 0 is the sky (deepest), the last one the nearest.
const DEPTH_FACTORS: [f32; 6] = [5.0, 4.0, 3.0, 2.0, 1.0, 1.0];
const NEAREST_SCROLL: f32 = 0.3;

pub fn setup_parallax_layers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    images: Res<Assets<Image>>,
    game_assets: Res<GameAssets>,
    current_level: Res<CurrentLevel>,
) {
    // The final level is a long vertical fall through the sky: let the
    // clouds stream past. Other levels keep the horizon steady.
    let vertical_ratio = if current_level.0.is_final() {
        1.2
    } else {
        0.35
    };

    // Only the first five images are real layers (the pygame loop ran
    // 1..=5); some level folders ship a sixth, unused one.
    for (index, parallax_bg) in game_assets.parallax_backgrounds.iter().take(5).enumerate() {
        let factor_x = NEAREST_SCROLL / DEPTH_FACTORS[index.min(DEPTH_FACTORS.len() - 1)];
        let texture_size = images
            .get(parallax_bg)
            .map(|img| img.size_f32())
            .unwrap_or(Vec2::new(1640.0, 820.0));
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
            MeshMaterial2d(materials.add(ColorMaterial {
                texture: Some(parallax_bg.clone()),
                uv_transform: Affine2::IDENTITY,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.0, -100.0 + index as f32),
            ParallaxLayer {
                factor: Vec2::new(factor_x, factor_x * vertical_ratio),
                texture_size,
                z: -100.0 + index as f32,
            },
        ));
    }
}
