// Direct light injection into the directional light grid (reptile_studio's
// InjectPass). Output channels encode "light arriving FROM direction", in
// WORLD terms (y up):
//   R = north (+y)   G = east (+x)   B = south (-y)   A = west (-x)
//
// The grid covers the camera viewport, so a fragment's world position comes
// from its UV inside the viewport rect. UV (0,0) is the top-left corner,
// which is why every world<->uv conversion flips y.
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// (viewport_origin.xy, viewport_size.xy), origin = bottom-left in world.
@group(2) @binding(0) var<uniform> view: vec4<f32>;
// (mask_origin.xy, mask_size.xy), the rect the mask/SDF cover.
@group(2) @binding(1) var<uniform> mask_rect: vec4<f32>;
// (sdf_max_px, light_count, unused, unused)
@group(2) @binding(2) var<uniform> params: vec4<f32>;
// (pos.xy, kind, angle_deg) — kind: 0 sun, 1 point, 2 cone
@group(2) @binding(3) var<uniform> lights_a: array<vec4<f32>, 8>;
// (cone_angle_deg, intensity, falloff_px, softness)
@group(2) @binding(4) var<uniform> lights_b: array<vec4<f32>, 8>;
// (color.rgb, interior_depth_px)
@group(2) @binding(5) var<uniform> lights_c: array<vec4<f32>, 8>;
@group(2) @binding(6) var mask_tex: texture_2d<f32>;
@group(2) @binding(7) var mask_sampler: sampler;
@group(2) @binding(8) var sdf_tex: texture_2d<f32>;
@group(2) @binding(9) var sdf_sampler: sampler;

fn world_from_uv(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(view.x + uv.x * view.z, view.y + (1.0 - uv.y) * view.w);
}

fn mask_uv(wp: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        (wp.x - mask_rect.x) / mask_rect.z,
        1.0 - (wp.y - mask_rect.y) / mask_rect.w,
    );
}

// Distance in px to the nearest occluder. Outside the map there is nothing
// to block light, so the field reads as "far".
fn sample_sdf(wp: vec2<f32>) -> f32 {
    let uv = mask_uv(wp);
    if (uv.x < 0.0 || uv.x >= 1.0 || uv.y < 0.0 || uv.y >= 1.0) {
        return params.x;
    }
    // textureSampleLevel, not textureSample: these are called from inside
    // raymarch loops, which is non-uniform control flow.
    return textureSampleLevel(sdf_tex, sdf_sampler, uv, 0.0).r * params.x;
}

fn inside_solid(wp: vec2<f32>) -> bool {
    let uv = mask_uv(wp);
    if (uv.x < 0.0 || uv.x >= 1.0 || uv.y < 0.0 || uv.y >= 1.0) {
        return false;
    }
    return textureSampleLevel(mask_tex, mask_sampler, uv, 0.0).r > 0.5;
}

// Ordered dither, matching the final pass — see the comment there.
fn dither(frag: vec2<f32>) -> f32 {
    let p = vec2<u32>(frag);
    var m = array<f32, 16>(
        0.0, 8.0, 2.0, 10.0,
        12.0, 4.0, 14.0, 6.0,
        3.0, 11.0, 1.0, 9.0,
        15.0, 7.0, 13.0, 5.0,
    );
    return m[(p.y & 3u) * 4u + (p.x & 3u)] / 16.0;
}

// Sphere-traced soft shadow: march the SDF toward the light, and let the
// closest approach relative to the distance travelled set the penumbra.
fn soft_shadow(origin: vec2<f32>, dir: vec2<f32>, max_len: f32, k: f32, frag: vec2<f32>) -> f32 {
    var vis = 1.0;
    var t = 1.0 + dither(frag) * 3.0;
    for (var i = 0; i < 96; i = i + 1) {
        if (t >= max_len) { break; }
        let h = sample_sdf(origin + dir * t);
        if (h < 0.5) { return 0.0; }
        vis = min(vis, k * h / max(t, 0.001));
        t += max(h, 0.5);
    }
    return clamp(vis, 0.0, 1.0);
}

fn falloff_att(i: i32, dir: vec2<f32>, dist: f32) -> f32 {
    let b = lights_b[i];
    var att = 1.0;
    if (b.z > 0.0) {
        let t = clamp(1.0 - dist / b.z, 0.0, 1.0);
        att = t * t;
        if (att <= 0.0) { return 0.0; }
    }
    if (lights_a[i].z > 1.5) {
        let rad = radians(lights_a[i].w);
        let axis = vec2<f32>(cos(rad), sin(rad));
        let cos_a = dot(-dir, axis);
        let cone_cos = cos(radians(b.x));
        let edge = mix(cone_cos, 1.0, 0.2);
        let cone_att = smoothstep(cone_cos, edge, cos_a);
        if (cone_att <= 0.0) { return 0.0; }
        att *= cone_att;
    }
    return att;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let wp = world_from_uv(in.uv);

    // A wall injects nothing: light does not travel inside it. The final
    // pass reads the grid just outside a surface instead.
    if (inside_solid(wp)) {
        return vec4<f32>(0.0);
    }

    var acc = vec4<f32>(0.0);
    let count = i32(params.y);
    for (var i = 0; i < 8; i = i + 1) {
        if (i >= count) { break; }
        let to_light = lights_a[i].xy - wp;
        let dist = length(to_light);
        if (dist < 1.0) { continue; }
        let dir = to_light / dist;

        let att = falloff_att(i, dir, dist);
        if (att <= 0.0) { continue; }

        let ang_size = max(0.005, 0.06 + lights_b[i].w * 0.01);
        let vis = soft_shadow(wp, dir, dist, 1.0 / ang_size, in.position.xy);
        if (vis <= 0.0) { continue; }

        // The grid stores scalar luminance per direction — four floats is
        // all an RGBA texel has. Colour comes back in the final pass from
        // the direct lights; the indirect term stays neutral.
        let col = lights_c[i].rgb * lights_b[i].y * att * vis;
        let lum = dot(col, vec3<f32>(0.2126, 0.7152, 0.0722));

        acc.r += lum * max(dir.y, 0.0);
        acc.g += lum * max(dir.x, 0.0);
        acc.b += lum * max(-dir.y, 0.0);
        acc.a += lum * max(-dir.x, 0.0);
    }

    return clamp(acc, vec4<f32>(0.0), vec4<f32>(1.0));
}
