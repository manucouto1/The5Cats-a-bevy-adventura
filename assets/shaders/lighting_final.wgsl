// Final lighting pass — the port of reptile_studio's FinalRenderPass.
// Renders the lightmap the composite pass then multiplies over the scene,
// so 1.0 means "leave this pixel as the game drew it" and 0.0 means black.
// It runs at half the window's resolution: every shadow here is soft by
// construction, so the bilinear upsample costs nothing visually and saves
// three quarters of the raymarching.
//
// Two shading paths, exactly as in the original: `air_shade` for empty
// space (ambient + direct lights + in-scattered haze + undirected GI) and
// `tile_shade` for anything inside the occluder mask (light entering the
// surface decays inward, GI is read from the air just outside the wall).
//
// Everything is in world coordinates with y pointing UP, which is where
// this differs from the GLSL original: its screen space had y down, so the
// north/south weights and the cone axis are flipped here.
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> view: vec4<f32>;
@group(2) @binding(1) var<uniform> mask_rect: vec4<f32>;
// (ambient, humidity, ao_strength, ao_radius)
@group(2) @binding(2) var<uniform> shading_a: vec4<f32>;
// (haze_strength, tile_offset, tile_min_brightness, tile_curve)
@group(2) @binding(3) var<uniform> shading_b: vec4<f32>;
// (sdf_max_px, light_count, gi_strength, has_grid)
@group(2) @binding(4) var<uniform> params: vec4<f32>;
// (cast_shadows, shadow_softness, unused, unused)
@group(2) @binding(5) var<uniform> shadow: vec4<f32>;
@group(2) @binding(6) var<uniform> lights_a: array<vec4<f32>, 8>;
@group(2) @binding(7) var<uniform> lights_b: array<vec4<f32>, 8>;
@group(2) @binding(8) var<uniform> lights_c: array<vec4<f32>, 8>;
@group(2) @binding(9) var mask_tex: texture_2d<f32>;
@group(2) @binding(10) var mask_sampler: sampler;
@group(2) @binding(11) var sdf_tex: texture_2d<f32>;
@group(2) @binding(12) var sdf_sampler: sampler;
@group(2) @binding(13) var grid_tex: texture_2d<f32>;
@group(2) @binding(14) var grid_sampler: sampler;
@group(2) @binding(15) var backdrop_tex: texture_2d<f32>;
@group(2) @binding(16) var backdrop_sampler: sampler;
// (uv_scale.xy, uv_offset.xy) of the nearest parallax layer.
@group(2) @binding(17) var<uniform> backdrop_uv: vec4<f32>;

fn mask_uv(wp: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        (wp.x - mask_rect.x) / mask_rect.z,
        1.0 - (wp.y - mask_rect.y) / mask_rect.w,
    );
}

fn sample_mask(wp: vec2<f32>) -> f32 {
    let uv = mask_uv(wp);
    if (uv.x < 0.0 || uv.x >= 1.0 || uv.y < 0.0 || uv.y >= 1.0) {
        return 0.0;
    }
    return select(0.0, 1.0, textureSampleLevel(mask_tex, mask_sampler, uv, 0.0).r > 0.5);
}

fn sample_sdf(wp: vec2<f32>) -> f32 {
    let uv = mask_uv(wp);
    if (uv.x < 0.0 || uv.x >= 1.0 || uv.y < 0.0 || uv.y >= 1.0) {
        return params.x;
    }
    return textureSampleLevel(sdf_tex, sdf_sampler, uv, 0.0).r * params.x;
}

fn inside_solid(wp: vec2<f32>) -> bool {
    return sample_mask(wp) > 0.5;
}

// Surface normal of the mask silhouette, pointing out of the solid.
fn outward_normal(wp: vec2<f32>) -> vec2<f32> {
    let l = sample_mask(wp + vec2<f32>(-2.0, 0.0));
    let r = sample_mask(wp + vec2<f32>(2.0, 0.0));
    let d = sample_mask(wp + vec2<f32>(0.0, -2.0));
    let u = sample_mask(wp + vec2<f32>(0.0, 2.0));
    let g = vec2<f32>(r - l, u - d);
    let gl = length(g);
    if (gl < 1e-3) {
        return vec2<f32>(0.0);
    }
    return -g / gl;
}

fn ambient_occlusion(wp: vec2<f32>) -> f32 {
    let t = smoothstep(0.0, max(1.0, shading_a.w), sample_sdf(wp));
    return mix(1.0 - shading_a.z, 1.0, t);
}

// Ordered 4x4 Bayer offset. The original dithered the ray start with white
// noise (`hash21`), which on a flat wall shows up as crawling speckle —
// exactly the "chaotic" look. An ordered pattern breaks the same banding
// with a fixed, screen-aligned texture the eye reads as a smooth gradient.
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

// Apparent size of the source. Bigger = wider penumbra; humid air spreads
// it further, and `shadow_softness` scales the whole level at once.
fn light_angular_size(i: i32) -> f32 {
    return (mix(0.05, 0.30, shading_a.y) + lights_b[i].w * 0.01) * max(shadow.y, 0.01);
}

// Sphere-traced soft shadow. `vis` tracks the closest the ray passed to an
// occluder relative to how far it had travelled, which is what makes the
// penumbra widen with distance from the caster.
fn soft_shadow(origin: vec2<f32>, dir: vec2<f32>, max_len: f32, angular_size: f32, frag: vec2<f32>) -> f32 {
    if (shadow.x < 0.5) {
        return 1.0;
    }
    var vis = 1.0;
    var t = 1.0 + dither(frag) * 3.0;
    let k = 1.0 / max(0.005, angular_size);
    // 96 sphere-tracing steps with a 0.75 px floor: with a 4 px SDF the
    // march converges long before that, and the floor is what stops a ray
    // grazing a wall from spending its whole budget there.
    for (var i = 0; i < 96; i = i + 1) {
        if (t >= max_len) { break; }
        let h = sample_sdf(origin + dir * t);
        if (h < 0.3) { return 0.0; }
        vis = min(vis, k * h / max(t, 0.001));
        t += max(h, 0.75);
    }
    vis = clamp(vis, 0.0, 1.0);
    // Smoothstep: takes the linear ramp off both ends of the penumbra, so
    // it reads as a gradient rather than a wedge with visible borders.
    return vis * vis * (3.0 - 2.0 * vis);
}

fn dist_to_viewport_exit(origin: vec2<f32>, dir: vec2<f32>) -> f32 {
    let inf = 1e20;
    let v_min = view.xy;
    let v_max = view.xy + view.zw;
    var tx = inf;
    if (dir.x > 0.0) {
        tx = (v_max.x - origin.x) / dir.x;
    } else if (dir.x < 0.0) {
        tx = (v_min.x - origin.x) / dir.x;
    }
    var ty = inf;
    if (dir.y > 0.0) {
        ty = (v_max.y - origin.y) / dir.y;
    } else if (dir.y < 0.0) {
        ty = (v_min.y - origin.y) / dir.y;
    }
    return max(1.0, min(tx, ty));
}

// Opacity of the painted skyline at a world position, sampled through the
// same UV transform the parallax layer is drawn with.
fn backdrop_alpha(wp: vec2<f32>) -> f32 {
    let center = view.xy + view.zw * 0.5;
    let quad_uv = vec2<f32>(
        (wp.x - center.x) / view.z + 0.5,
        0.5 - (wp.y - center.y) / view.w,
    );
    return textureSampleLevel(
        backdrop_tex,
        backdrop_sampler,
        backdrop_uv.xy * quad_uv + backdrop_uv.zw,
        0.0,
    ).a;
}

// The buildings behind the level are painted, not built, so nothing in the
// occluder mask can shadow a sunbeam that passes between them. Treat the
// nearest parallax layer as a silhouette at infinity instead: for a light
// that is itself at infinity, "is the skyline in the way?" is answered by
// sampling it a few steps along the light direction, each step further
// away than the last. Local lights are in front of the backdrop and ignore
// it (`kind` 1 = point, 2 = cone).
fn backdrop_vis(wp: vec2<f32>, dir: vec2<f32>, kind: f32) -> f32 {
    if (shadow.z <= 0.001 || kind > 0.5) {
        return 1.0;
    }
    // Four taps, far enough out that the silhouette reads as a skyline
    // rather than as the pixel's immediate surroundings, and spread so the
    // band edges come out soft instead of stamped.
    var taps = array<f32, 4>(260.0, 460.0, 720.0, 1040.0);
    var open = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        open += 1.0 - smoothstep(0.3, 0.7, backdrop_alpha(wp + dir * taps[i]));
    }
    return mix(1.0, open / 4.0, shadow.z);
}

fn light_falloff(i: i32, dir: vec2<f32>, dist: f32) -> f32 {
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
        let cone_cos = cos(radians(b.x));
        let edge = mix(cone_cos, 1.0, 0.2);
        let cone_att = smoothstep(cone_cos, edge, dot(-dir, axis));
        if (cone_att <= 0.0) { return 0.0; }
        att *= cone_att;
    }
    return att;
}

// Light scattered toward the camera by the air itself — the shafts you see
// crossing a lit gap. Only pays off with humidity.
fn in_scatter(origin: vec2<f32>, frag: vec2<f32>) -> vec3<f32> {
    let strength = shading_a.y * shading_b.x;
    if (strength <= 0.001) {
        return vec3<f32>(0.0);
    }
    var acc = vec3<f32>(0.0);
    let count = i32(params.y);
    for (var li = 0; li < 8; li = li + 1) {
        if (li >= count) { break; }
        let to_light = lights_a[li].xy - origin;
        let dist = length(to_light);
        if (dist < 1.0) { continue; }
        let dir = to_light / dist;
        let falloff_r = lights_b[li].z;
        let backdrop = backdrop_vis(origin, dir, lights_a[li].z);
        if (backdrop <= 0.001) { continue; }
        let max_ray = min(dist, dist_to_viewport_exit(origin, dir));
        let lit_edge = max(8.0, dist * light_angular_size(li));
        var t = 1.0 + dither(frag) * 6.0;
        var prev_t = t;
        var scatter = 0.0;
        for (var i = 0; i < 40; i = i + 1) {
            if (t >= max_ray) { break; }
            let h = sample_sdf(origin + dir * t);
            var att = 1.0;
            if (falloff_r > 0.0) {
                let u = clamp(1.0 - (dist - t) / falloff_r, 0.0, 1.0);
                att = u * u;
            }
            // No early exit on a hit: a ray that clips a corner used to stop
            // dead while its neighbour sailed past, and the discontinuity
            // drew hard streaks along the light direction. Occluded stretches
            // simply contribute nothing and the march carries on.
            scatter += smoothstep(0.0, lit_edge, h) * att * (t - prev_t);
            prev_t = t;
            t += clamp(h, 12.0, 40.0);
        }
        // Normalised against a fixed length rather than this ray's own
        // length: dividing by `max_ray` made the haze jump wherever the
        // viewport edge clipped the ray.
        let reference = max(64.0, view.w * 0.5);
        acc += lights_c[li].rgb * lights_b[li].y * backdrop * clamp(scatter / reference, 0.0, 1.5);
    }
    return acc * strength;
}

fn sample_light_grid(wp: vec2<f32>) -> vec4<f32> {
    if (params.w < 0.5) {
        return vec4<f32>(0.0);
    }
    let uv = clamp(
        vec2<f32>((wp.x - view.x) / view.z, 1.0 - (wp.y - view.y) / view.w),
        vec2<f32>(0.0),
        vec2<f32>(1.0),
    );
    return textureSampleLevel(grid_tex, grid_sampler, uv, 0.0);
}

// The grid holds scalar luminance per cardinal direction; a surface only
// receives what arrives from the side it faces.
fn reconstruct_gi(dir_l: vec4<f32>, n: vec2<f32>) -> vec3<f32> {
    let lum = dir_l.r * max(n.y, 0.0)
        + dir_l.g * max(n.x, 0.0)
        + dir_l.b * max(-n.y, 0.0)
        + dir_l.a * max(-n.x, 0.0);
    return vec3<f32>(lum);
}

fn air_shade(wp: vec2<f32>, ambient_base: vec3<f32>, frag: vec2<f32>) -> vec3<f32> {
    var light = ambient_base * ambient_occlusion(wp);
    let count = i32(params.y);
    for (var i = 0; i < 8; i = i + 1) {
        if (i >= count) { break; }
        let to_light = lights_a[i].xy - wp;
        let dist = length(to_light);
        if (dist < 1.0) { continue; }
        let dir = to_light / dist;
        let att = light_falloff(i, dir, dist);
        if (att <= 0.0) { continue; }
        let max_len = min(dist, dist_to_viewport_exit(wp, dir));
        let vis = soft_shadow(wp, dir, max_len, light_angular_size(i), frag)
            * backdrop_vis(wp, dir, lights_a[i].z);
        if (vis <= 0.0) { continue; }
        light += lights_c[i].rgb * lights_b[i].y * att * vis;
    }
    if (params.w > 0.5) {
        // Air has no normal: take the undirected sum of the four channels.
        let g = sample_light_grid(wp);
        light += vec3<f32>(g.r + g.g + g.b + g.a) * params.z * 0.25;
    }
    return light + in_scatter(wp, frag);
}

// How far into the solid this fragment sits along the ray to the light.
fn body_depth_along_ray(wp: vec2<f32>, dir: vec2<f32>, max_depth: f32) -> f32 {
    var t = 0.5;
    for (var i = 0; i < 40; i = i + 1) {
        if (t >= max_depth) { return max_depth; }
        if (!inside_solid(wp - dir * t)) { return t; }
        t += 2.0;
    }
    return max_depth;
}

fn tile_body_atten(body_depth: f32, interior_depth: f32) -> f32 {
    let t = max(0.0, body_depth - shading_b.y) / max(1.0, interior_depth);
    var atten: f32;
    if (shading_b.w < 0.5) {
        atten = clamp(1.0 - t, 0.0, 1.0);
    } else if (shading_b.w < 1.5) {
        atten = exp(-t);
    } else {
        atten = 0.5 + 0.5 * cos(clamp(t, 0.0, 1.0) * 3.14159265);
    }
    return mix(shading_b.z, 1.0, atten);
}

fn tile_shade(wp: vec2<f32>, ambient_base: vec3<f32>, frag: vec2<f32>) -> vec3<f32> {
    var light = ambient_base;
    let count = i32(params.y);
    for (var i = 0; i < 8; i = i + 1) {
        if (i >= count) { break; }
        let to_light = lights_a[i].xy - wp;
        let dist = length(to_light);
        if (dist < 1.0) { continue; }
        let dir = to_light / dist;
        let att = light_falloff(i, dir, dist);
        if (att <= 0.0) { continue; }

        let interior = max(1.0, lights_c[i].w);
        let body_depth = body_depth_along_ray(wp, dir, shading_b.y + interior * 6.0);
        // March the shadow from the lit face, not from inside the wall.
        let face = wp - dir * (body_depth + 1.0);
        let max_len = min(dist - body_depth, dist_to_viewport_exit(face, dir));
        let vis = soft_shadow(face, dir, max_len, light_angular_size(i), frag)
            * backdrop_vis(face, dir, lights_a[i].z);
        if (vis <= 0.0) { continue; }

        light += lights_c[i].rgb * lights_b[i].y * att * vis
            * tile_body_atten(body_depth, interior);
    }
    if (params.w > 0.5) {
        let n = outward_normal(wp);
        if (length(n) > 0.001) {
            light += reconstruct_gi(sample_light_grid(wp + n * 2.0), n) * params.z;
        }
    }
    return light;
}

fn world_from_uv(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(view.x + uv.x * view.z, view.y + (1.0 - uv.y) * view.w);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // This pass draws into the off-screen lightmap, so the fragment's world
    // position comes from its UV inside the viewport rect rather than from
    // the quad's own vertices.
    let wp = world_from_uv(in.uv);
    let ambient_tint = mix(vec3<f32>(1.0), vec3<f32>(0.78, 0.92, 1.18), shading_a.y);
    let ambient_base = vec3<f32>(shading_a.x) * ambient_tint;
    var light: vec3<f32>;
    if (inside_solid(wp)) {
        light = tile_shade(wp, ambient_base, in.position.xy);
    } else {
        light = air_shade(wp, ambient_base, in.position.xy);
    }
    return vec4<f32>(clamp(light, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
