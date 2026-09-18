//! The five materials of the lighting pipeline, one per pass.
//!
//! Each pass is a full-screen quad drawn by its own off-screen camera
//! (see `passes.rs`), except the final one, which is drawn over the scene
//! by the main camera with a multiply blend — it is a lightmap composite,
//! exactly like the Pixi original.
//!
//! Uniforms are packed into `vec4`s by hand: WGSL uniform arrays need a
//! 16-byte stride anyway, and it keeps the bind-group layout obvious on
//! both sides.

use bevy::{
    prelude::*,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_resource::{
            AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState,
            RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
        },
    },
    sprite::{AlphaMode2d, Material2d, Material2dKey},
};

use crate::lighting::level::MAX_LIGHTS;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct LightInjectMaterial {
    /// (viewport_origin.xy, viewport_size.xy) in world space.
    #[uniform(0)]
    pub view: Vec4,
    /// (mask_origin.xy, mask_size.xy) in world space.
    #[uniform(1)]
    pub mask_rect: Vec4,
    /// (sdf_max_px, light_count, unused, unused)
    #[uniform(2)]
    pub params: Vec4,
    #[uniform(3)]
    pub lights_a: [Vec4; MAX_LIGHTS],
    #[uniform(4)]
    pub lights_b: [Vec4; MAX_LIGHTS],
    #[uniform(5)]
    pub lights_c: [Vec4; MAX_LIGHTS],
    #[texture(6)]
    #[sampler(7)]
    pub mask: Handle<Image>,
    #[texture(8)]
    #[sampler(9)]
    pub sdf: Handle<Image>,
}

impl Material2d for LightInjectMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/lighting_inject.wgsl".into()
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct LightPropagateMaterial {
    #[uniform(0)]
    pub view: Vec4,
    #[uniform(1)]
    pub mask_rect: Vec4,
    /// (sdf_max_px, energy_decay, texel.x, texel.y)
    #[uniform(2)]
    pub params: Vec4,
    #[texture(3)]
    #[sampler(4)]
    pub prev_grid: Handle<Image>,
    #[texture(5)]
    #[sampler(6)]
    pub sdf: Handle<Image>,
}

impl Material2d for LightPropagateMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/lighting_propagate.wgsl".into()
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct LightTemporalMaterial {
    /// (blend, unused, unused, unused)
    #[uniform(0)]
    pub params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub current: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    pub history: Handle<Image>,
}

impl Material2d for LightTemporalMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/lighting_temporal.wgsl".into()
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct LightBlurMaterial {
    /// (texel.x, texel.y, unused, unused)
    #[uniform(0)]
    pub params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub grid: Handle<Image>,
}

impl Material2d for LightBlurMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/lighting_blur.wgsl".into()
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct LightFinalMaterial {
    #[uniform(0)]
    pub view: Vec4,
    #[uniform(1)]
    pub mask_rect: Vec4,
    /// (ambient, humidity, ao_strength, ao_radius)
    #[uniform(2)]
    pub shading_a: Vec4,
    /// (haze_strength, tile_offset, tile_min_brightness, tile_curve)
    #[uniform(3)]
    pub shading_b: Vec4,
    /// (sdf_max_px, light_count, gi_strength, has_grid)
    #[uniform(4)]
    pub params: Vec4,
    /// (cast_shadows, shadow_softness, unused, unused)
    #[uniform(5)]
    pub shadow: Vec4,
    #[uniform(6)]
    pub lights_a: [Vec4; MAX_LIGHTS],
    #[uniform(7)]
    pub lights_b: [Vec4; MAX_LIGHTS],
    #[uniform(8)]
    pub lights_c: [Vec4; MAX_LIGHTS],
    #[texture(9)]
    #[sampler(10)]
    pub mask: Handle<Image>,
    #[texture(11)]
    #[sampler(12)]
    pub sdf: Handle<Image>,
    #[texture(13)]
    #[sampler(14)]
    pub grid: Handle<Image>,
    /// Nearest parallax layer, used as a silhouette that shadows distant
    /// lights (the skyline blocking the sun).
    #[texture(15)]
    #[sampler(16)]
    pub backdrop: Handle<Image>,
    /// (uv_scale.xy, uv_offset.xy) for `backdrop`.
    #[uniform(17)]
    pub backdrop_uv: Vec4,
}

impl Material2d for LightFinalMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/lighting_final.wgsl".into()
    }
}

/// Draws the finished lightmap over the scene. Split from the pass above so
/// the expensive shading runs at half resolution while the multiply still
/// happens at full resolution, per output pixel.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct LightCompositeMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub lightmap: Handle<Image>,
}

impl Material2d for LightCompositeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/lighting_composite.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    /// `dst * src`: the lightmap is a multiplier on what the game drew, not
    /// a colour to paint on top of it.
    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            if let Some(Some(target)) = fragment.targets.first_mut() {
                target.blend = Some(BlendState {
                    color: BlendComponent {
                        src_factor: BlendFactor::Dst,
                        dst_factor: BlendFactor::Zero,
                        operation: BlendOperation::Add,
                    },
                    alpha: BlendComponent {
                        src_factor: BlendFactor::Zero,
                        dst_factor: BlendFactor::One,
                        operation: BlendOperation::Add,
                    },
                });
            }
        }
        Ok(())
    }
}
