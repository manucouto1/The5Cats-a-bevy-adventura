// Multiplies the half-resolution lightmap over the rendered scene. The quad
// is glued to the camera and its UVs map 1:1 onto the lightmap, which was
// rendered for exactly this viewport.
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var lightmap_tex: texture_2d<f32>;
@group(2) @binding(1) var lightmap_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(lightmap_tex, lightmap_sampler, in.uv).rgb, 1.0);
}
