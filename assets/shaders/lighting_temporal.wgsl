// Blends the freshly propagated grid with the previous frame's result to
// take the flicker out of a low iteration count. The grid is anchored to
// the viewport, so while the camera pans the history is misaligned by
// design — keep `temporal_blend` modest.
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// (blend, unused, unused, unused)
@group(2) @binding(0) var<uniform> params: vec4<f32>;
@group(2) @binding(1) var current_tex: texture_2d<f32>;
@group(2) @binding(2) var current_sampler: sampler;
@group(2) @binding(3) var history_tex: texture_2d<f32>;
@group(2) @binding(4) var history_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let cur = textureSampleLevel(current_tex, current_sampler, in.uv, 0.0);
    let hist = textureSampleLevel(history_tex, history_sampler, in.uv, 0.0);
    return mix(cur, hist, clamp(params.x, 0.0, 1.0));
}
