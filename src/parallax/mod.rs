pub mod components;
pub mod systems;

use bevy::{math::Affine2, prelude::*};

use crate::{map::assets::GameAssets, parallax::components::ParallaxLayer};

// Re-exportar los sistemas para uso externo
pub use systems::infinite_parallax_system;

pub fn setup_parallax_layers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    game_assets: Res<GameAssets>, // Importa tu GameAssets
) {
    for (index, parallax_bg) in game_assets.parallax_backgrounds.iter().enumerate() {
        let width = game_assets.map_width_tiles as f32 * 32.0;
        let height = game_assets.map_height_tiles as f32 * 32.0;
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(width, height))),
            MeshMaterial2d(materials.add(ColorMaterial {
                texture: Some(parallax_bg.clone()),
                uv_transform: Affine2::from_scale(Vec2::new(2., 1.)),
                ..default()
            })),
            ParallaxLayer {
                scroll_factor: Vec2::new(
                    -0.05 * (game_assets.parallax_backgrounds.len() - index) as f32,
                    -0.05 * (game_assets.parallax_backgrounds.len() - index) as f32,
                ),
                start_position: Vec3::new(0.0, -50.0, -100.0),
                layer_width: game_assets.map_width_tiles as f32 * 32.0 * 2.0, // Ancho de la capa para repetición
                smoothing_factor: 10.0, // Factor de suavizado más alto para capas de fondo
                current_offset: Vec2::ZERO, // Inicializar offset en cero
            },
        ));
    }
}
