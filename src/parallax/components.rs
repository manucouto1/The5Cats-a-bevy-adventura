use bevy::prelude::*;

#[derive(Component)]
pub struct MainCamera;

#[derive(Component, Clone, Copy)]
pub struct ParallaxLayer {
    /// Per-axis parallax weight in [0, 1]. The system positions the layer
    /// at `camera * (1 - scroll_factor)`, so 0 locks the layer to the
    /// camera and 1 locks it to the world.
    pub scroll_factor: Vec2,
    /// World position used as the layer's base; only `z` is currently
    /// honored, used to stack layers back-to-front.
    pub start_position: Vec3,
}

impl Default for ParallaxLayer {
    fn default() -> Self {
        ParallaxLayer {
            scroll_factor: Vec2::new(0.5, 0.0),
            start_position: Vec3::ZERO,
        }
    }
}
