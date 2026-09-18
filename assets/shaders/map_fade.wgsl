// Softens whatever shows past the edges of the level. The quad is glued to
// the camera and covers the whole viewport; the fragment shader works in
// world space, so the fade is anchored to the map rather than the screen:
// fully transparent inside the level, ramping to `color` over `fade_width`
// world px outside it.
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Map AABB in world space: (min_x, min_y, max_x, max_y).
@group(2) @binding(0) var<uniform> bounds: vec4<f32>;
// (fade_width, max_alpha, inset, unused)
@group(2) @binding(1) var<uniform> params: vec4<f32>;
// Linear RGB the outside fades to.
@group(2) @binding(2) var<uniform> color: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.world_position.xy;
    let inset = params.z;
    let lo = bounds.xy + inset;
    let hi = bounds.zw - inset;
    // Distance outside the (inset) map rectangle, zero anywhere inside it.
    // Taking the length of both components rounds off the corners instead
    // of crossing two ramps there.
    let outside = max(vec2<f32>(0.0), max(lo - p, p - hi));
    let width = max(params.x, 1.0);
    let alpha = smoothstep(0.0, 1.0, length(outside) / width) * params.y;
    return vec4<f32>(color.rgb, alpha);
}
