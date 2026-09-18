// One ping-pong step of directional light propagation. Each channel reads
// only from the neighbour it flows out of:
//   R (from north) <- the cell to the north      B (from south) <- south
//   G (from east)  <- the cell to the east       A (from west)  <- west
// The half-step between the two cells is tested against the SDF, so light
// does not bleed through a wall. The cell's own previous value is added
// back (decayed) as the source term, which is what lets injected light
// survive more than one iteration and settle into a steady state.
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> view: vec4<f32>;
@group(2) @binding(1) var<uniform> mask_rect: vec4<f32>;
// (sdf_max_px, energy_decay, step.x, step.y) — `step` is the UV distance
// covered per iteration, which is a whole number of texels: reaching N
// texels with one long step instead of N one-texel passes saves N-1
// off-screen passes, and each pass costs far more in per-camera overhead
// than in shading 192x108 pixels.
@group(2) @binding(2) var<uniform> params: vec4<f32>;
@group(2) @binding(3) var prev_tex: texture_2d<f32>;
@group(2) @binding(4) var prev_sampler: sampler;
@group(2) @binding(5) var sdf_tex: texture_2d<f32>;
@group(2) @binding(6) var sdf_sampler: sampler;

fn world_from_uv(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(view.x + uv.x * view.z, view.y + (1.0 - uv.y) * view.w);
}

fn sample_sdf(wp: vec2<f32>) -> f32 {
    let uv = vec2<f32>(
        (wp.x - mask_rect.x) / mask_rect.z,
        1.0 - (wp.y - mask_rect.y) / mask_rect.w,
    );
    if (uv.x < 0.0 || uv.x >= 1.0 || uv.y < 0.0 || uv.y >= 1.0) {
        return params.x;
    }
    return textureSampleLevel(sdf_tex, sdf_sampler, uv, 0.0).r * params.x;
}

// Three probes along the step rather than one at the midpoint: a longer
// step could otherwise hop straight over a one-tile wall and leak light
// into the room behind it.
fn visibility(a: vec2<f32>, b: vec2<f32>) -> f32 {
    return smoothstep(0.0, 2.0, min(
        sample_sdf(mix(a, b, 0.25)),
        min(sample_sdf(mix(a, b, 0.5)), sample_sdf(mix(a, b, 0.75))),
    ));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let here = world_from_uv(uv);
    let step = params.zw;
    // UV v grows downward, so "north" is -v.
    let step_x = vec2<f32>(step.x, 0.0);
    let step_y = vec2<f32>(0.0, step.y);
    let world_per_step = view.zw * step;

    let wp_north = here + vec2<f32>(0.0, world_per_step.y);
    let wp_east = here + vec2<f32>(world_per_step.x, 0.0);
    let wp_south = here - vec2<f32>(0.0, world_per_step.y);
    let wp_west = here - vec2<f32>(world_per_step.x, 0.0);

    let r = textureSampleLevel(prev_tex, prev_sampler, uv - step_y, 0.0).r * visibility(here, wp_north);
    let g = textureSampleLevel(prev_tex, prev_sampler, uv + step_x, 0.0).g * visibility(here, wp_east);
    let b = textureSampleLevel(prev_tex, prev_sampler, uv + step_y, 0.0).b * visibility(here, wp_south);
    let a = textureSampleLevel(prev_tex, prev_sampler, uv - step_x, 0.0).a * visibility(here, wp_west);

    let self_light = textureSampleLevel(prev_tex, prev_sampler, uv, 0.0) * params.y;
    let out_v = vec4<f32>(r, g, b, a) * params.y + self_light;
    return clamp(out_v, vec4<f32>(0.0), vec4<f32>(1.0));
}
