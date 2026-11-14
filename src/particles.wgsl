struct SimParams {
    dt: f32,
    time: f32,
    count: u32,
    _pad: u32,
};

struct Petal {
    pos_spin: vec4<f32>,   // xyz = pos, w = spin angle
    vel_col: vec4<f32>,    // x = vy, y = vx, z = phase, w = color_idx
};

@group(0) @binding(0) var<uniform> sim: SimParams;
@group(0) @binding(1) var<storage, read_write> petals: array<Petal>;

@compute @workgroup_size(64)
fn cs_petals(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= sim.count) { return; }

    var p = petals[idx];
    let phase = p.vel_col.z;
    let wind_x = sin(sim.time * 0.45) * 0.65 + 0.85;
    let wind_z = cos(sim.time * 0.35) * 0.45;

    p.pos_spin.y -= p.vel_col.x * sim.dt;
    p.pos_spin.x += (p.vel_col.y * wind_x + sin(sim.time * 2.0 + phase) * 0.65) * sim.dt;
    p.pos_spin.z += (wind_z + cos(sim.time * 1.7 + phase) * 0.55) * sim.dt;
    p.pos_spin.w += (1.2 + sin(phase) * 1.6) * sim.dt;

    if (p.pos_spin.y < 3.0) { p.pos_spin.y = 74.0 + f32(idx % 9u); }
    if (p.pos_spin.x > 122.0) { p.pos_spin.x -= 244.0; }
    if (p.pos_spin.x < -122.0) { p.pos_spin.x += 244.0; }
    if (p.pos_spin.z > 122.0) { p.pos_spin.z -= 244.0; }
    if (p.pos_spin.z < -122.0) { p.pos_spin.z += 244.0; }

    petals[idx] = p;
}

// Petal Render Shader
struct CamUniform {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
};
@group(0) @binding(0) var<uniform> cam: CamUniform;

struct PIn {
    @location(0) v_pos: vec3<f32>,
    @location(1) p_pos_spin: vec4<f32>,
    @location(2) p_vel_col: vec4<f32>,
};

struct POut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
};

@vertex
fn vs_petal(in: PIn) -> POut {
    let s = sin(in.p_pos_spin.w);
    let c = cos(in.p_pos_spin.w);
    let p = in.v_pos * vec3<f32>(0.26, 0.05, 0.26);
    let rx = vec3<f32>(p.x * c + p.z * s, p.y * c - p.z * s * 0.4, -p.x * s + p.z * c);
    let wpos = rx + in.p_pos_spin.xyz;

    let ci = u32(in.p_vel_col.w);
    var col = vec3<f32>(1.0, 0.718, 0.773);
    if (ci == 1u) { col = vec3<f32>(1.0, 0.620, 0.710); }
    else if (ci == 2u) { col = vec3<f32>(1.0, 0.820, 0.863); }

    var out: POut;
    out.clip = cam.view_proj * vec4<f32>(wpos, 1.0);
    out.color = pow(col, vec3<f32>(2.2));
    out.world_pos = wpos;
    return out;
}

@fragment
fn fs_petal(in: POut) -> @location(0) vec4<f32> {
    let d = length(cam.cam_pos.xyz - in.world_pos);
    let fog_f = 1.0 - exp(-pow(d * 0.0021, 2.0));
    let fog_col = pow(vec3<f32>(0.910, 0.929, 0.949), vec3<f32>(2.2));
    return vec4<f32>(mix(in.color, fog_col, clamp(fog_f, 0.0, 1.0)), 1.0);
}
