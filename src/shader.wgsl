struct Uniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    cam_pos: vec4<f32>,      // xyz = cam, w = time
    sun_dir: vec4<f32>,      // xyz = sun dir, w = fog_density
    point_lights: array<vec4<f32>, 6>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var shadow_tex: texture_depth_2d;
@group(0) @binding(2) var shadow_samp: sampler_comparison;

// -------------------- SKY DOME FULLSCREEN PASS --------------------
struct SkyOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) vid: u32) -> SkyOut {
    let x = f32(i32(vid & 1u) * 4 - 1);
    let y = f32(i32(vid >> 1u) * 4 - 1);
    var out: SkyOut;
    out.pos = vec4<f32>(x, y, 0.9999, 1.0);
    out.ndc = vec2<f32>(x, y);
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    let wp = u.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let dir = normalize(wp.xyz / wp.w - u.cam_pos.xyz);
    let h = dir.y;
    let bot = srgb_to_linear(vec3<f32>(0.910, 0.929, 0.949));
    let mid = srgb_to_linear(vec3<f32>(0.835, 0.894, 0.941));
    let top = srgb_to_linear(vec3<f32>(0.596, 0.761, 0.902));
    var col = mix(bot, mid, smoothstep(-0.04, 0.24, h));
    col = mix(col, top, smoothstep(0.22, 0.85, h));
    return vec4<f32>(col, 1.0);
}

// -------------------- VOXEL SHADOW & LIT PASS --------------------
struct VIn {
    @location(0) v_pos: vec3<f32>,
    @location(1) v_norm: vec3<f32>,
    @location(2) inst_pos_yaw: vec4<f32>, // xyz = center, w = yaw
    @location(3) inst_scale: vec3<f32>,
    @location(4) inst_packed: u32,        // rgb8 + mat_id<<24
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) @interpolate(flat) mat_id: u32,
};

fn world_xform(in: VIn) -> array<vec3<f32>, 2> {
    let scaled = in.v_pos * in.inst_scale;
    let c = cos(in.inst_pos_yaw.w);
    let s = sin(in.inst_pos_yaw.w);
    let rpos = vec3<f32>(scaled.x * c + scaled.z * s, scaled.y, -scaled.x * s + scaled.z * c);
    let rnorm = vec3<f32>(in.v_norm.x * c + in.v_norm.z * s, in.v_norm.y, -in.v_norm.x * s + in.v_norm.z * c);
    return array<vec3<f32>, 2>(rpos + in.inst_pos_yaw.xyz, normalize(rnorm));
}

@vertex
fn vs_shadow(in: VIn) -> @builtin(position) vec4<f32> {
    let xform = world_xform(in);
    return u.light_vp * vec4<f32>(xform[0], 1.0);
}

@vertex
fn vs_main(in: VIn) -> VOut {
    let xform = world_xform(in);
    var out: VOut;
    out.world_pos = xform[0];
    out.normal = xform[1];
    out.clip = u.view_proj * vec4<f32>(xform[0], 1.0);
    let r = f32(in.inst_packed & 0xFFu) / 255.0;
    let g = f32((in.inst_packed >> 8u) & 0xFFu) / 255.0;
    let b = f32((in.inst_packed >> 16u) & 0xFFu) / 255.0;
    out.color = srgb_to_linear(vec3<f32>(r, g, b));
    out.mat_id = (in.inst_packed >> 24u) & 0xFFu;
    return out;
}

fn sample_shadow(wpos: vec3<f32>, n: vec3<f32>) -> f32 {
    let lclip = u.light_vp * vec4<f32>(wpos + n * 0.12, 1.0);
    let ndc = lclip.xyz / lclip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }
    let texel = 1.0 / 2048.0;
    var sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            sum += textureSampleCompareLevel(
                shadow_tex, shadow_samp,
                uv + vec2<f32>(f32(x), f32(y)) * texel,
                ndc.z - 0.0008
            );
        }
    }
    return sum / 9.0;
}

fn shade_common(in: VOut, alpha: f32) -> vec4<f32> {
    let n = normalize(in.normal);
    let l = normalize(u.sun_dir.xyz);
    let v = normalize(u.cam_pos.xyz - in.world_pos);
    let shadow = sample_shadow(in.world_pos, n);

    // Hemisphere ambient
    let hemi_t = n.y * 0.5 + 0.5;
    let sky_amb = srgb_to_linear(vec3<f32>(0.875, 0.941, 1.0));
    let gnd_amb = srgb_to_linear(vec3<f32>(0.435, 0.557, 0.345));
    var lit = in.color * mix(gnd_amb, sky_amb, hemi_t) * 0.44;

    // Directional afternoon sun + bounce fill
    let sun_col = vec3<f32>(1.0, 0.965, 0.90) * 1.45;
    let ndotl = max(dot(n, l), 0.0);
    lit += in.color * sun_col * ndotl * shadow;

    let fill_l = normalize(vec3<f32>(-0.6, 0.4, -0.5));
    lit += in.color * vec3<f32>(0.18, 0.22, 0.28) * max(dot(n, fill_l), 0.0);

    // Specular for Gold (mat=5) and Water (mat=4)
    let hvec = normalize(l + v);
    if (in.mat_id == 5u) {
        let spec = pow(max(dot(n, hvec), 0.0), 40.0) * shadow;
        lit += vec3<f32>(1.0, 0.85, 0.35) * spec * 1.3;
    } else if (in.mat_id == 4u) {
        let wave = sin(in.world_pos.x * 0.65 + u.cam_pos.w * 2.2) * cos(in.world_pos.z * 0.65 - u.cam_pos.w * 1.7);
        let wn = normalize(n + vec3<f32>(wave * 0.08, 0.0, wave * 0.08));
        let wh = normalize(l + v);
        let spec = pow(max(dot(wn, wh), 0.0), 64.0) * shadow;
        lit += vec3<f32>(0.85, 0.98, 1.0) * spec * 0.9;
    }

    // Warm Lantern Point Lights
    for (var i = 0u; i < 6u; i++) {
        let pl = u.point_lights[i];
        let diff = pl.xyz - in.world_pos;
        let d2 = dot(diff, diff);
        let r2 = pl.w * pl.w;
        if (d2 < r2) {
            let dist = sqrt(d2);
            let att = pow(1.0 - dist / pl.w, 2.0);
            let p_ndotl = max(dot(n, diff / max(dist, 0.001)), 0.15);
            lit += in.color * vec3<f32>(1.0, 0.45, 0.08) * (3.2 * att * p_ndotl);
        }
    }

    // Emissive Windows (mat=6) & Lanterns (mat=7)
    if (in.mat_id == 6u) {
        lit += in.color * 0.95;
    } else if (in.mat_id == 7u) {
        lit += in.color * 1.75;
    }

    // Subtle ACES Filmic Tone Map + FogExp2
    let x = lit * 0.95;
    let mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
    let dist_cam = length(u.cam_pos.xyz - in.world_pos);
    let fog_d = u.sun_dir.w;
    let fog_f = 1.0 - exp(-pow(dist_cam * fog_d, 2.0));
    let fog_col = srgb_to_linear(vec3<f32>(0.910, 0.929, 0.949));
    return vec4<f32>(mix(mapped, fog_col, clamp(fog_f, 0.0, 1.0)), alpha);
}

@fragment
fn fs_opaque(in: VOut) -> @location(0) vec4<f32> {
    return shade_common(in, 1.0);
}

@fragment
fn fs_translucent(in: VOut) -> @location(0) vec4<f32> {
    return shade_common(in, 0.78);
}
