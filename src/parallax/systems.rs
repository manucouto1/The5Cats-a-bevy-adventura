use bevy::{
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
};

use crate::{
    game_state::LevelState,
    map::assets::GameAssets,
    parallax::components::{MainCamera, ParallaxLayer},
    player::components::PlayerCharacter,
};

/// Repositions every parallax layer relative to the camera each frame.
///
/// The mesh is sized to comfortably exceed the viewport (see
/// `PARALLAX_MESH_*` constants in `parallax/mod.rs`), so we don't need any
/// smoothing or modular wrapping — a direct `mesh_x = camera_x * (1 - scroll)`
/// gives the parallax slide without the layer's edge ever entering view
/// within typical level scroll ranges.
pub fn infinite_parallax_system(
    camera_query: Query<&Transform, With<MainCamera>>,
    mut parallax_query: Query<(&mut Transform, &ParallaxLayer), Without<MainCamera>>,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };
    let cam_x = camera_transform.translation.x;
    let cam_y = camera_transform.translation.y;

    for (mut tf, layer) in parallax_query.iter_mut() {
        tf.translation.x = cam_x * (1.0 - layer.scroll_factor.x);
        tf.translation.y = cam_y;
        tf.translation.z = layer.start_position.z;
    }
}

pub fn configure_parallax_textures(
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    game_assets: Res<GameAssets>,
    mut next_state: ResMut<NextState<LevelState>>,
) {
    let all_loaded = game_assets
        .parallax_backgrounds
        .iter()
        .all(|handle| asset_server.is_loaded(handle));

    if !all_loaded {
        return;
    }

    for handle in &game_assets.parallax_backgrounds {
        if let Some(image) = images.get_mut(handle) {
            image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..default()
            })
        }
    }

    next_state.set(LevelState::LevelLoaded);
}

pub fn camera_follow_system(
    time: Res<Time>,
    game_assets: Res<GameAssets>,
    player_query: Query<&Transform, (With<PlayerCharacter>, Without<Camera>)>,
    mut camera_query: Query<&mut Transform, (With<Camera>, Without<PlayerCharacter>)>,
    windows: Query<&Window>,
) {
    if let Ok(player_transform) = player_query.single() {
        if let Ok(mut camera_transform) = camera_query.single_mut() {
            let target_pos = player_transform.translation;
            let current_pos = camera_transform.translation;

            // Suavizado mejorado para la cámara
            let smoothing_factor = 6.0;
            let t = (smoothing_factor * time.delta_secs()).min(1.0);

            // Calcular nueva posición con suavizado
            let smooth_x = current_pos.x + (target_pos.x - current_pos.x) * t;

            // Obtener dimensiones de la ventana para calcular el viewport de la cámara
            let Ok(window) = windows.single() else {
                return;
            };
            let camera_half_width = window.width() / 2.0;

            // Calcular dimensiones totales del mapa en píxeles
            let map_width_px = game_assets.map_width_tiles as f32 * game_assets.tile_size_px;

            // El mapa está centrado en el origen (0, 0), así que calculamos los bordes
            let map_left = -(map_width_px / 2.0);
            let map_right = map_width_px / 2.0;

            // Calcular límites horizontales para el centro de la cámara
            let camera_min_x = map_left + camera_half_width;
            let camera_max_x = map_right - camera_half_width;

            // Aplicar límites horizontales
            let clamped_x = if camera_max_x <= camera_min_x {
                // Si el mapa es más pequeño que el viewport de la cámara,
                // centrar la cámara horizontalmente en el mapa
                (map_left + map_right) / 2.0
            } else {
                // Caso normal: limitar la posición de la cámara a los bordes calculados
                smooth_x.clamp(camera_min_x, camera_max_x)
            };

            camera_transform.translation.x = clamped_x;

            // Vertical follow with the same clamp-to-map logic as X. Needed
            // for level 4's tall vertical scroller; on shorter levels the
            // clamp centers the camera and Y stays stable.
            let camera_half_height = window.height() / 2.0;
            let map_height_px = game_assets.map_height_tiles as f32 * game_assets.tile_size_px;
            let map_top = map_height_px / 2.0;
            let map_bottom = -map_height_px / 2.0;
            let camera_min_y = map_bottom + camera_half_height;
            let camera_max_y = map_top - camera_half_height;

            let smooth_y = current_pos.y + (target_pos.y - current_pos.y) * t;
            let clamped_y = if camera_max_y <= camera_min_y {
                (map_bottom + map_top) / 2.0
            } else {
                smooth_y.clamp(camera_min_y, camera_max_y)
            };
            camera_transform.translation.y = clamped_y;
        }
    }
}
