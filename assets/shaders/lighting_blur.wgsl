// 5-tap box blur; softens the grid's quantisation before the final pass
// samples it.
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// (texel.x, texel.y, unused, unused)
@group(2) @binding(0) var<uniform> params: vec4<f32>;
@group(2) @binding(1) var grid_tex: texture_2d<f32>;
@group(2) @binding(2) var grid_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = params.xy;
    var c = textureSampleLevel(grid_tex, grid_sampler, in.uv, 0.0);
    c += textureSampleLevel(grid_tex, grid_sampler, in.uv + vec2<f32>(t.x, 0.0), 0.0);
    c += textureSampleLevel(grid_tex, grid_sampler, in.uv - vec2<f32>(t.x, 0.0), 0.0);
    c += textureSampleLevel(grid_tex, grid_sampler, in.uv + vec2<f32>(0.0, t.y), 0.0);
    c += textureSampleLevel(grid_tex, grid_sampler, in.uv - vec2<f32>(0.0, t.y), 0.0);
    return c / 5.0;
}
