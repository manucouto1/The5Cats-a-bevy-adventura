use bevy::{
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    math::Affine2,
    prelude::*,
    render::camera::ScalingMode,
    window::PrimaryWindow,
};

use crate::{
    game_state::{CurrentLevel, LevelState},
    map::assets::GameAssets,
    parallax::components::{MainCamera, ParallaxLayer, view_half_extents},
    player::components::PlayerCharacter,
};

/// Keeps every parallax quad centered on the camera, sized to the visible
/// area, and scrolls its texture by a depth-dependent fraction of the
/// camera position. The texture's full height always maps onto the
/// viewport height (like the 800px-tall pygame window showed the whole
/// 820px image), and the width wraps seamlessly thanks to the repeating
/// sampler.
pub fn infinite_parallax_system(
    camera_query: Query<(&Transform, &Projection), With<MainCamera>>,
    mut layers: Query<
        (
            &mut Transform,
            &ParallaxLayer,
            &MeshMaterial2d<ColorMaterial>,
        ),
        Without<MainCamera>,
    >,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Ok((camera_transform, projection)) = camera_query.single() else {
        return;
    };
    let cam = camera_transform.translation.truncate();
    let view = view_half_extents(projection) * 2.0;
    if view.x <= 0.0 || view.y <= 0.0 {
        return;
    }

    for (mut transform, layer, material) in &mut layers {
        let (uv_scale, uv_offset) = layer_uv_transform(layer, cam, view);
        if let Some(mat) = materials.get_mut(&material.0) {
            mat.uv_transform = Affine2::from_scale_angle_translation(uv_scale, 0.0, uv_offset);
        }
        transform.translation = Vec3::new(cam.x, cam.y, layer.z);
        transform.scale = Vec3::new(view.x, view.y, 1.0);
    }
}

/// UV scale and offset that map a layer's quad (whose own UVs run 0..1 over
/// the viewport) onto its texture. Shared with the lighting pass, which
/// samples the nearest layer's silhouette to shadow the sun with it — if
/// the two ever disagreed, the skyline's shadows would drift off the
/// skyline.
pub fn layer_uv_transform(layer: &ParallaxLayer, cam: Vec2, view: Vec2) -> (Vec2, Vec2) {
    // World px per texel so the texture height spans the viewport.
    let k = view.y / layer.texture_size.y;
    (
        Vec2::new(view.x / (layer.texture_size.x * k), 1.0),
        Vec2::new(
            cam.x * layer.factor.x / (layer.texture_size.x * k),
            -cam.y * layer.factor.y / (layer.texture_size.y * k),
        ),
    )
}

/// Runs while `LevelState::Loading` until every background and the tileset
/// are in memory, configures their samplers, then flips to `LevelLoaded`.
pub fn configure_parallax_textures(
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    game_assets: Res<GameAssets>,
    current_level: Res<CurrentLevel>,
    mut next_state: ResMut<NextState<LevelState>>,
) {
    let parallax_loaded = game_assets
        .parallax_backgrounds
        .iter()
        .all(|handle| asset_server.is_loaded(handle));
    let tiles_loaded = asset_server.is_loaded(&game_assets.tile_texture);

    if !parallax_loaded || !tiles_loaded {
        return;
    }

    // Horizontal wrap always. Vertical wrap only on the final level (sky
    // and clouds tile fine); elsewhere the top row is clamped so a tall
    // level never shows building roofs hanging from the sky.
    let address_mode_v = if current_level.0.is_final() {
        ImageAddressMode::Repeat
    } else {
        ImageAddressMode::ClampToEdge
    };
    for handle in &game_assets.parallax_backgrounds {
        if let Some(image) = images.get_mut(handle) {
            image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v,
                // Backgrounds are shown slightly below 1:1; bilinear keeps
                // the big flat shapes from shimmering.
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                ..default()
            })
        }
    }

    // Nearest-neighbor on the tileset prevents neighboring atlas cells
    // from bleeding into each other's edges as the camera moves.
    if let Some(image) = images.get_mut(&game_assets.tile_texture) {
        image.sampler = ImageSampler::nearest();
    }

    next_state.set(LevelState::LevelLoaded);
}

/// Set on level load so the camera jumps straight to the player instead
/// of panning across the map from wherever the previous level left it.
#[derive(Resource, Default)]
pub struct CameraSnapRequested(pub bool);

pub fn request_camera_snap(mut snap: ResMut<CameraSnapRequested>, mut mode: ResMut<CameraMode>) {
    snap.0 = true;
    *mode = CameraMode::Follow;
}

/// How the camera frames the action. Zones (level 4) switch this; every
/// other level just follows the player.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub enum CameraMode {
    #[default]
    Follow,
    /// Fixed shot centered on a world point (boss arena, bottom pit).
    /// `view_width` zooms the camera so exactly that many world px fit
    /// horizontally (None = default zoom).
    Fixed {
        point: Vec2,
        view_width: Option<f32>,
    },
    /// Horizontal position pinned (the shaft), vertical follows the player
    /// offset by `look_ahead_y` downward.
    LockX {
        x: f32,
        look_ahead_y: f32,
        view_width: Option<f32>,
    },
    /// Pygame's `CameraVerticalGap`: follows the player, but the camera's
    /// vertical travel never leaves the world-space band the player is
    /// standing in. A band shorter than the viewport pins the camera to the
    /// band's center, which is what the original 800px-tall window did for
    /// the 25-row bands levels 2 ships.
    Banded { min_y: f32, max_y: f32 },
}

impl CameraMode {
    fn view_width(&self) -> Option<f32> {
        match self {
            CameraMode::Follow | CameraMode::Banded { .. } => None,
            CameraMode::Fixed { view_width, .. } | CameraMode::LockX { view_width, .. } => {
                *view_width
            }
        }
    }

    /// Extra vertical limits this mode imposes on top of the map bounds.
    fn y_limits(&self) -> Option<(f32, f32)> {
        match self {
            CameraMode::Banded { min_y, max_y } => Some((*min_y, *max_y)),
            _ => None,
        }
    }
}

pub fn camera_follow_system(
    time: Res<Time>,
    game_assets: Res<GameAssets>,
    mode: Res<CameraMode>,
    mut snap_request: ResMut<CameraSnapRequested>,
    player_query: Query<&Transform, (With<PlayerCharacter>, Without<MainCamera>)>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok((mut camera_transform, mut projection)) = camera_query.single_mut() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };

    let player_pos = player_transform.translation.truncate();
    let current = camera_transform.translation.truncate();
    let snap = snap_request.0;
    if snap {
        snap_request.0 = false;
    }

    // Zoom first: the clamps below need the view size we are about to have.
    // Modes that pin a view width (the shaft) shrink the visible height so
    // exactly that width fits the window, keeping the window's aspect.
    let aspect = window.height().max(1.0) / window.width().max(1.0);
    let wanted_height = mode
        .view_width()
        .map(|w| w * aspect)
        .unwrap_or(crate::VIEW_HEIGHT);
    if let Projection::Orthographic(ortho) = &mut *projection {
        if let ScalingMode::FixedVertical { viewport_height } = &mut ortho.scaling_mode {
            let zoom_t = if snap {
                1.0
            } else {
                (2.5 * time.delta_secs()).min(1.0)
            };
            *viewport_height += (wanted_height - *viewport_height) * zoom_t;
            // `area` is refreshed by Bevy after this system; keep the
            // clamp below consistent with the height we just set.
            let half_h = *viewport_height / 2.0;
            ortho.area = Rect::from_center_half_size(
                ortho.area.center(),
                Vec2::new(half_h / aspect, half_h),
            );
        }
    }

    let half = view_half_extents(&projection);
    let map_w = game_assets.map_width_tiles as f32 * game_assets.tile_size_px;
    let map_h = game_assets.map_height_tiles as f32 * game_assets.tile_size_px;
    // Map is centered on the origin.
    let (map_left, map_right) = (-map_w / 2.0, map_w / 2.0);
    let (map_bottom, map_top) = (-map_h / 2.0, map_h / 2.0);
    // A vertical band (`CameraVerticalGap`) narrows the vertical range
    // further; it is always a sub-range of the map, but intersect anyway so
    // a sloppy gaps file cannot push the camera off the level.
    let (low_y, high_y) = mode
        .y_limits()
        .map(|(lo, hi)| (lo.max(map_bottom), hi.min(map_top)))
        .unwrap_or((map_bottom, map_top));

    // A view taller/wider than the range it has to stay inside cannot be
    // clamped: center it on the range and let the map-edge fade cover
    // whatever shows past the level (level 3 is 25 tiles wide against a
    // ~34-tile-wide viewport).
    let clamp_axis = |value: f32, lo: f32, hi: f32, half: f32| {
        let (min, max) = (lo + half, hi - half);
        if max <= min {
            (lo + hi) / 2.0
        } else {
            value.clamp(min, max)
        }
    };
    // Whatever the mode asks to look at, before clamping: an authored shot
    // names a point, the shaft pins X and follows the fall in Y, everything
    // else follows the player.
    let raw_target = match *mode {
        CameraMode::Fixed { point, .. } => point,
        CameraMode::LockX {
            x, look_ahead_y, ..
        } => Vec2::new(x, player_pos.y - look_ahead_y),
        _ => player_pos,
    };
    // Then keep the camera inside the level on both axes — including the
    // authored shots. Letting level 4's arena sit exactly where its gaps
    // file asks put ~160 px of off-map space on the left of the boss fight,
    // and the map-edge fade turned that into a shadow creeping over the
    // arena. A shot nudged back inside the level reads better than a
    // correctly-centred one with a dark band down its side.
    let target = Vec2::new(
        clamp_axis(raw_target.x, map_left, map_right, half.x),
        clamp_axis(raw_target.y, low_y, high_y, half.y),
    );

    // Smoothing the *clamped* target (rather than clamping the smoothed
    // position) makes a band or zone change read as a pan instead of a cut.
    // Fixed shots pan a little slower, so the move is clearly a camera move.
    let smoothing_factor = match *mode {
        CameraMode::Follow | CameraMode::Banded { .. } => 6.0,
        _ => 3.5,
    };
    let t = if snap {
        1.0
    } else {
        (smoothing_factor * time.delta_secs()).min(1.0)
    };
    let smooth = current + (target - current) * t;

    // Snap to positions that land on whole screen pixels — sub-pixel
    // camera positions on pixel-art tiles produce seams between tiles.
    let world_per_pixel = (half.y * 2.0) / window.height().max(1.0);
    camera_transform.translation.x = (smooth.x / world_per_pixel).round() * world_per_pixel;
    camera_transform.translation.y = (smooth.y / world_per_pixel).round() * world_per_pixel;
}
