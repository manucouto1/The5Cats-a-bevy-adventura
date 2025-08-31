use bevy::prelude::*;

#[derive(Component)]
pub struct MainCamera;

#[derive(Component, Clone, Copy)]
pub struct ParallaxLayer {
    pub scroll_factor: Vec2,
    pub start_position: Vec3,
    pub layer_width: f32,      // Ancho de la capa para repetición
    pub smoothing_factor: f32, // Factor de suavizado
    pub current_offset: Vec2,  // Offset actual para interpolación suave
}

impl Default for ParallaxLayer {
    fn default() -> Self {
        ParallaxLayer {
            scroll_factor: Vec2::new(0.5, 0.5),
            start_position: Vec3::ZERO,
            layer_width: 1400.0,   // Ancho por defecto
            smoothing_factor: 8.0, // Suavizado moderado
            current_offset: Vec2::ZERO,
        }
    }
}
