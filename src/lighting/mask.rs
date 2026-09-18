//! Occluder mask and signed-distance field for a level.
//!
//! reptile_studio rebuilt these every frame from the live scene, because
//! its layers move under the editor's hands. Our level geometry is fixed
//! once the tilemap is spawned, so both textures are rasterized on the CPU
//! at load time, cover the whole map, and never change again — which also
//! means the lighting shaders can sample them in plain world coordinates.

use bevy::{
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_asset::RenderAssetUsages,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use crate::map::components::{LevelData, TileType, get_tile_properties_from_path};

/// World px per mask texel. 4 px (an eighth of a tile) — the raymarch steps
/// by the sampled distance, so this is what sets how precisely a shadow can
/// hug the edge that casts it. Level 4's 50x194 tilemap still only needs
/// 400x1552 texels.
pub const MASK_TEXEL_PX: f32 = 4.0;
/// Distances are stored normalized by this, so the SDF texture only has to
/// be precise within the range shadows actually care about.
pub const SDF_MAX_PX: f32 = 320.0;

pub struct LevelMask {
    pub mask: Image,
    pub sdf: Image,
    /// World-space rectangle both textures cover (the whole map).
    pub origin: Vec2,
    pub size: Vec2,
}

/// True for tiles that should block light. Solid ground and the falling
/// platforms are real geometry; spikes, pipes, the goal and decoration are
/// either thin or see-through, and casting hard shadows from them looks
/// like a bug.
fn occludes(path: &str) -> bool {
    matches!(
        get_tile_properties_from_path(path).map(|p| p.tile_type),
        Some(TileType::Solid) | Some(TileType::Falling)
    )
}

pub fn build_level_mask(level: &LevelData) -> LevelMask {
    let tile = level.tile_size as f32;
    let map_w = level.map_width as f32 * tile;
    let map_h = level.map_height as f32 * tile;
    let width = (map_w / MASK_TEXEL_PX).ceil() as usize;
    let height = (map_h / MASK_TEXEL_PX).ceil() as usize;
    let per_tile = (tile / MASK_TEXEL_PX).round() as usize;

    let mut solid = vec![false; width * height];
    for layer in &level.layers {
        if !occludes(&layer.path) {
            continue;
        }
        for pos in &layer.positions {
            // Tile (x, y) with y growing downward, same as the texture.
            let x0 = pos.x as usize * per_tile;
            let y0 = pos.y as usize * per_tile;
            for y in y0..(y0 + per_tile).min(height) {
                for x in x0..(x0 + per_tile).min(width) {
                    solid[y * width + x] = true;
                }
            }
        }
    }

    let distance = distance_transform(&solid, width, height);

    let mut mask_bytes = vec![0u8; width * height];
    let mut sdf_bytes = vec![0u8; width * height * 2];
    for i in 0..width * height {
        mask_bytes[i] = if solid[i] { 255 } else { 0 };
        let px = (distance[i] * MASK_TEXEL_PX).min(SDF_MAX_PX);
        let half = half::f16::from_f32(px / SDF_MAX_PX);
        sdf_bytes[i * 2..i * 2 + 2].copy_from_slice(&half.to_le_bytes());
    }

    let extent = Extent3d {
        width: width as u32,
        height: height as u32,
        depth_or_array_layers: 1,
    };
    let mut mask = Image::new(
        extent,
        TextureDimension::D2,
        mask_bytes,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    // Nearest: the mask is a binary silhouette, interpolating it would
    // round off every corner of the tilemap.
    mask.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        ..default()
    });
    let mut sdf = Image::new(
        extent,
        TextureDimension::D2,
        sdf_bytes,
        TextureFormat::R16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    // Linear: the raymarch steps by the sampled distance, so a smooth
    // field means fewer, longer steps and no stair-stepping in shadows.
    sdf.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });

    LevelMask {
        mask,
        sdf,
        origin: Vec2::new(-map_w / 2.0, -map_h / 2.0),
        size: Vec2::new(map_w, map_h),
    }
}

/// Two-pass chamfer distance transform, in texels: 0 inside solids, the
/// distance to the nearest solid outside them. Exact Euclidean would need
/// a per-row parabola sweep; chamfer 3x3 is within a few percent and runs
/// once per level load.
fn distance_transform(solid: &[bool], width: usize, height: usize) -> Vec<f32> {
    const D1: f32 = 1.0;
    const D2: f32 = std::f32::consts::SQRT_2;
    let far = (width + height) as f32;
    let mut d: Vec<f32> = solid.iter().map(|&s| if s { 0.0 } else { far }).collect();

    let relax = |d: &mut Vec<f32>, x: usize, y: usize, dx: isize, dy: isize, cost: f32| {
        let (nx, ny) = (x as isize + dx, y as isize + dy);
        if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
            return;
        }
        let candidate = d[ny as usize * width + nx as usize] + cost;
        let here = &mut d[y * width + x];
        if candidate < *here {
            *here = candidate;
        }
    };

    for y in 0..height {
        for x in 0..width {
            relax(&mut d, x, y, -1, 0, D1);
            relax(&mut d, x, y, 0, -1, D1);
            relax(&mut d, x, y, -1, -1, D2);
            relax(&mut d, x, y, 1, -1, D2);
        }
    }
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            relax(&mut d, x, y, 1, 0, D1);
            relax(&mut d, x, y, 0, 1, D1);
            relax(&mut d, x, y, 1, 1, D2);
            relax(&mut d, x, y, -1, 1, D2);
        }
    }
    d
}
