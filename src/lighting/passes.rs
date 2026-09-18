//! Spawns the off-screen pass chain for a level and keeps its uniforms in
//! step with the camera.
//!
//! Bevy has no "run this quad N times into these targets" primitive, so
//! each pass is a tiny 2D camera of its own: a 1x1 quad on a private
//! `RenderLayers`, an orthographic projection scaled so the quad exactly
//! fills the frame, and a `Camera.order` low enough that the whole chain
//! has finished before the main camera draws the world.
//!
//! Ping-pong without aliasing:
//!
//! ```text
//!   inject        -> A
//!   propagate 1   A -> B      propagate 2  B -> A   (…N times)
//!   temporal      (last, F) -> C     F still holds LAST frame's grid
//!   blur / copy   C -> F              …and is overwritten here
//! ```
//!
//! so the history the temporal pass reads is simply the previous frame's
//! final grid, and no pass ever samples the target it writes.

use bevy::{
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::{
        camera::{ImageRenderTarget, RenderTarget, ScalingMode},
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        view::RenderLayers,
    },
    sprite::{Material2d, MeshMaterial2d},
};

use crate::lighting::{
    level::{GiSettings, MAX_LIGHTS},
    materials::{
        LightBlurMaterial, LightCompositeMaterial, LightFinalMaterial, LightInjectMaterial,
        LightPropagateMaterial, LightTemporalMaterial,
    },
};

/// Light-grid resolution. Fixed rather than a fraction of the window: the
/// grid is addressed in UV space, so its pixel size only sets how fine the
/// indirect light is, and a fixed size keeps every texel-step uniform
/// constant across window resizes.
///
/// Deliberately coarse. Propagation moves light exactly one texel per
/// iteration, so the distance a bounce can travel is `iterations` texels —
/// a finer grid would need proportionally more passes to spread the same
/// number of world pixels, for detail the blur pass throws away anyway.
pub const GRID_WIDTH: u32 = 192;
pub const GRID_HEIGHT: u32 = 108;

/// Resolution the lightmap (the final shading pass) is rendered at. Half the
/// window's logical size: every term it computes is smooth, so the bilinear
/// upsample is invisible while the raymarching gets four times cheaper.
pub const LIGHTMAP_WIDTH: u32 = 1280;
pub const LIGHTMAP_HEIGHT: u32 = 720;

/// First render layer used by the pass chain. Layer 0 stays the game.
const FIRST_PASS_LAYER: usize = 8;
/// Every pass camera draws before the main camera (order 0).
const FIRST_PASS_ORDER: isize = -64;

/// Marks everything spawned for the lighting chain, so a level change can
/// tear it down in one query.
#[derive(Component)]
pub struct LightingPassEntity;

/// Handles the per-frame uniform update needs.
#[derive(Resource)]
pub struct LightingRuntime {
    pub inject: Option<Handle<LightInjectMaterial>>,
    pub propagate: Vec<Handle<LightPropagateMaterial>>,
    pub temporal: Option<Handle<LightTemporalMaterial>>,
    pub blur: Option<Handle<LightBlurMaterial>>,
    pub final_material: Handle<LightFinalMaterial>,
    /// Material of the quad that multiplies the lightmap over the scene.
    pub composite_material: Handle<LightCompositeMaterial>,
    /// The grid the final pass samples; `None` when GI is off.
    pub grid: Option<Handle<Image>>,
}

fn grid_image(images: &mut Assets<Image>) -> Handle<Image> {
    render_target_image(images, GRID_WIDTH, GRID_HEIGHT)
}

fn render_target_image(images: &mut Assets<Image>, width: u32, height: u32) -> Handle<Image> {
    let mut image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0u8; 8],
        // Half floats: the grid accumulates several lights per texel and
        // 8-bit quantisation shows up as banding in the indirect term.
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    images.add(image)
}

/// One off-screen pass: a camera that renders a single quad into `target`.
fn spawn_pass<M: Material2d>(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<M>,
    material: M,
    target: Handle<Image>,
    layer: usize,
    order: isize,
) -> Handle<M> {
    let handle = materials.add(material);
    let layers = RenderLayers::layer(layer);
    commands.spawn((
        LightingPassEntity,
        Camera2d,
        Camera {
            target: RenderTarget::Image(ImageRenderTarget::from(target)),
            order,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            hdr: false,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            // The quad below is 1x1, so a 1x1 view frames it exactly.
            scaling_mode: ScalingMode::Fixed {
                width: 1.0,
                height: 1.0,
            },
            ..OrthographicProjection::default_2d()
        }),
        bevy::render::view::Msaa::Off,
        layers.clone(),
    ));
    commands.spawn((
        LightingPassEntity,
        Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial2d(handle.clone()),
        Transform::default(),
        layers,
    ));
    handle
}

pub struct PassAssets<'a> {
    pub images: &'a mut Assets<Image>,
    pub meshes: &'a mut Assets<Mesh>,
    pub inject: &'a mut Assets<LightInjectMaterial>,
    pub propagate: &'a mut Assets<LightPropagateMaterial>,
    pub temporal: &'a mut Assets<LightTemporalMaterial>,
    pub blur: &'a mut Assets<LightBlurMaterial>,
    pub finals: &'a mut Assets<LightFinalMaterial>,
    pub composites: &'a mut Assets<LightCompositeMaterial>,
}

/// Builds the whole chain for a level. `mask`/`sdf` are already-uploaded
/// textures covering the map.
pub fn spawn_lighting_chain(
    commands: &mut Commands,
    assets: &mut PassAssets,
    gi: &GiSettings,
    mask: Handle<Image>,
    sdf: Handle<Image>,
    backdrop: Handle<Image>,
    mask_origin: Vec2,
    mask_size: Vec2,
    sdf_max_px: f32,
) -> LightingRuntime {
    let mask_rect = Vec4::new(mask_origin.x, mask_origin.y, mask_size.x, mask_size.y);
    let empty = [Vec4::ZERO; MAX_LIGHTS];
    let texel = Vec2::new(1.0 / GRID_WIDTH as f32, 1.0 / GRID_HEIGHT as f32);

    let mut runtime = LightingRuntime {
        inject: None,
        propagate: Vec::new(),
        temporal: None,
        blur: None,
        // Filled in below; both always exist.
        final_material: Handle::default(),
        composite_material: Handle::default(),
        grid: None,
    };

    let mut layer = FIRST_PASS_LAYER;
    let mut order = FIRST_PASS_ORDER;
    let mut next_slot = || {
        let slot = (layer, order);
        layer += 1;
        order += 1;
        slot
    };

    if gi.enabled {
        let a = grid_image(assets.images);
        let b = grid_image(assets.images);

        let (l, o) = next_slot();
        runtime.inject = Some(spawn_pass(
            commands,
            assets.meshes,
            assets.inject,
            LightInjectMaterial {
                view: Vec4::ZERO,
                mask_rect,
                params: Vec4::new(sdf_max_px, 0.0, 0.0, 0.0),
                lights_a: empty,
                lights_b: empty,
                lights_c: empty,
                mask: mask.clone(),
                sdf: sdf.clone(),
            },
            a.clone(),
            l,
            o,
        ));

        // Ping-pong: even steps read A and write B, odd ones the reverse.
        let mut last = a.clone();
        for step in 0..gi.iterations.max(1) {
            let (src, dst) = if step % 2 == 0 {
                (a.clone(), b.clone())
            } else {
                (b.clone(), a.clone())
            };
            let (l, o) = next_slot();
            runtime.propagate.push(spawn_pass(
                commands,
                assets.meshes,
                assets.propagate,
                LightPropagateMaterial {
                    view: Vec4::ZERO,
                    mask_rect,
                    params: Vec4::new(
                        sdf_max_px,
                        gi.energy_decay,
                        texel.x * gi.step_texels.max(1.0),
                        texel.y * gi.step_texels.max(1.0),
                    ),
                    prev_grid: src,
                    sdf: sdf.clone(),
                },
                dst.clone(),
                l,
                o,
            ));
            last = dst;
        }

        // `final_grid` is what the composite samples — and, one frame
        // later, the history the temporal pass blends against.
        let mut final_grid = last.clone();
        if gi.temporal {
            let blended = grid_image(assets.images);
            let history = grid_image(assets.images);
            let (l, o) = next_slot();
            runtime.temporal = Some(spawn_pass(
                commands,
                assets.meshes,
                assets.temporal,
                LightTemporalMaterial {
                    params: Vec4::new(gi.temporal_blend.clamp(0.0, 0.95), 0.0, 0.0, 0.0),
                    current: last.clone(),
                    history: history.clone(),
                },
                blended.clone(),
                l,
                o,
            ));
            // Copy/blur step writes the history image, closing the loop.
            let (l, o) = next_slot();
            let blur_texel = if gi.blur { texel } else { Vec2::ZERO };
            runtime.blur = Some(spawn_pass(
                commands,
                assets.meshes,
                assets.blur,
                LightBlurMaterial {
                    params: Vec4::new(blur_texel.x, blur_texel.y, 0.0, 0.0),
                    grid: blended,
                },
                history.clone(),
                l,
                o,
            ));
            final_grid = history;
        } else if gi.blur {
            let blurred = grid_image(assets.images);
            let (l, o) = next_slot();
            runtime.blur = Some(spawn_pass(
                commands,
                assets.meshes,
                assets.blur,
                LightBlurMaterial {
                    params: Vec4::new(texel.x, texel.y, 0.0, 0.0),
                    grid: last,
                },
                blurred.clone(),
                l,
                o,
            ));
            final_grid = blurred;
        }
        runtime.grid = Some(final_grid);
    }

    let lightmap = render_target_image(assets.images, LIGHTMAP_WIDTH, LIGHTMAP_HEIGHT);
    runtime.composite_material = assets.composites.add(LightCompositeMaterial {
        lightmap: lightmap.clone(),
    });

    // The shading pass is the last off-screen one; the quad that multiplies
    // its result over the scene rides with the main camera (layer 0) and is
    // spawned by the caller.
    let (layer, order) = next_slot();
    runtime.final_material = spawn_pass(
        commands,
        assets.meshes,
        assets.finals,
        LightFinalMaterial {
            view: Vec4::ZERO,
            mask_rect,
            shading_a: Vec4::ZERO,
            shading_b: Vec4::ZERO,
            params: Vec4::new(sdf_max_px, 0.0, 0.0, 0.0),
            shadow: Vec4::new(1.0, 1.0, 0.0, 0.0),
            lights_a: empty,
            lights_b: empty,
            lights_c: empty,
            mask,
            sdf,
            grid: runtime
                .grid
                .clone()
                .unwrap_or_else(|| grid_image(assets.images)),
            backdrop,
            backdrop_uv: Vec4::new(1.0, 1.0, 0.0, 0.0),
        },
        lightmap,
        layer,
        order,
    );
    runtime
}
