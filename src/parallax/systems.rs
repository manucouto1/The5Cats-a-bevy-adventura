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

// Sistema de parallax con repetición infinita usando wrapping matemático
pub fn infinite_parallax_system(
    time: Res<Time>,
    camera_query: Query<&Transform, With<MainCamera>>,
    mut parallax_query: Query<(&mut Transform, &mut ParallaxLayer), Without<MainCamera>>,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };

    let camera_translation = camera_transform.translation;

    for (mut layer_transform, mut parallax_layer) in parallax_query.iter_mut() {
        let camera_offset = camera_translation - parallax_layer.start_position;

        // Calcular la nueva posición objetivo con parallax
        let target_x = parallax_layer.start_position.x
            + camera_offset.x * (1.0 - parallax_layer.scroll_factor.x);
        let target_y = parallax_layer.start_position.y
            + camera_offset.y * (1.0 - parallax_layer.scroll_factor.y);

        // Aplicar suavizado para evitar artefactos
        let smoothing_factor = parallax_layer.smoothing_factor;
        let t = (smoothing_factor * time.delta_secs()).min(1.0);

        let current_x = layer_transform.translation.x;
        let current_y = layer_transform.translation.y;

        let smooth_x = current_x + (target_x - current_x) * t;
        let smooth_y = current_y + (target_y - current_y) * t;

        // Implementar repetición infinita con wrapping matemático
        let layer_width = parallax_layer.layer_width;

        // Calcular la posición wrapeada usando módulo
        let wrapped_x = ((smooth_x % layer_width) + layer_width) % layer_width;

        // Ajustar para centrar el wrapping alrededor de la posición de la cámara
        let camera_x = camera_translation.x;
        let camera_relative_pos = ((camera_x % layer_width) + layer_width) % layer_width;

        // Calcular offset desde la cámara
        let mut offset_from_camera = wrapped_x - camera_relative_pos;

        // Ajustar para mantener la imagen siempre visible
        if offset_from_camera > layer_width / 2.0 {
            offset_from_camera -= layer_width;
        } else if offset_from_camera < -layer_width / 2.0 {
            offset_from_camera += layer_width;
        }

        // Posición final
        let final_x = camera_x + offset_from_camera;

        layer_transform.translation.x = final_x;
        layer_transform.translation.y = smooth_y;
        layer_transform.translation.z = parallax_layer.start_position.z;

        // Actualizar el offset actual
        parallax_layer.current_offset.x = final_x;
        parallax_layer.current_offset.y = smooth_y;
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

            // Aplicar las posiciones finales con límites horizontales
            camera_transform.translation.x = clamped_x;
            // Mantener Y fijo o seguir al jugador sin límites verticales
            // camera_transform.translation.y = current_pos.y + (target_pos.y - current_pos.y) * t;
        }
    }
}
