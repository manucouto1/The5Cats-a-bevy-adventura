use bevy::prelude::*;

#[derive(Component)]
pub struct MainCamera;

/// One background layer. The quad itself is glued to the camera and covers
/// the visible area; the illusion of depth comes from shifting the
/// material's UV offset by a fraction of the camera position.
#[derive(Component, Clone, Copy)]
pub struct ParallaxLayer {
    /// Fraction of the camera translation applied to the texture offset.
    /// Small values = far away (barely moves), larger = closer.
    pub factor: Vec2,
    /// Texture size in texels, needed to convert world px into UV units.
    pub texture_size: Vec2,
    pub z: f32,
}

/// Half-size of the world rectangle currently visible through `projection`.
/// Source of truth for every "is this on screen" check, so window resizes
/// and fullscreen are handled in one place.
pub fn view_half_extents(projection: &Projection) -> Vec2 {
    match projection {
        Projection::Orthographic(ortho) => ortho.area.half_size(),
        _ => Vec2::new(crate::VIEW_HEIGHT * 16.0 / 9.0, crate::VIEW_HEIGHT) * 0.5,
    }
}
