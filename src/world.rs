use bytemuck::{Pod, Zeroable};

pub const SX: usize = 240;
pub const SY: usize = 96;
pub const SZ: usize = 240;
pub const HX: i32 = 120;
pub const HZ: i32 = 120;

pub const MAT_AIR: u8 = 0;
pub const MAT_MATTE: u8 = 1;
pub const MAT_WOOD: u8 = 2;
pub const MAT_LEAVES: u8 = 3;
pub const MAT_WATER: u8 = 4;
pub const MAT_GOLD: u8 = 5;
pub const MAT_WIN: u8 = 6;
pub const MAT_LAMP: u8 = 7;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct VoxelInstance {
    pub pos_yaw: [f32; 4],
    pub scale: [f32; 3],
    pub packed: u32,
}

impl VoxelInstance {
    pub fn pack(rgb: u32, mat: u8) -> u32 {
        let r = (rgb >> 16) & 0xFF;
        let g = (rgb >> 8) & 0xFF;
        let b = rgb & 0xFF;
        r | (g << 8) | (b << 16) | ((mat as u32) << 24)
    }

    pub fn new(x: f32, y: f32, z: f32, yaw: f32, sx: f32, sy: f32, sz: f32, rgb: u32, mat: u8) -> Self {
        Self { pos_yaw: [x, y, z, yaw], scale: [sx, sy, sz], packed: Self::pack(rgb, mat) }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct PetalGpu {
    pub pos_spin: [f32; 4],
    pub vel_col: [f32; 4],
}

pub struct VillagerAnim {
    pub bx: f32, pub by: f32, pub bz: f32,
    pub rot_y: f32, pub axis_x: bool, pub range: f32,
    pub phase: f32, pub speed: f32, pub color_idx: usize,
}

pub struct BoatAnim {
    pub x: f32, pub y: f32, pub z: f32, pub rot: f32, pub phase: f32,
}

pub struct CloudAnim {
    pub x: f32, pub y: f32, pub z: f32, pub speed: f32,
    pub offsets: Vec<[f32; 3]>,
}

pub struct WorldData {
    pub opaque: Vec<VoxelInstance>,
    pub shadow_opaque_count: u32, // Excludes underside bedrock from shadow pass
    pub translucent_base: Vec<VoxelInstance>,
    pub villagers: Vec<VillagerAnim>,
    pub boats: Vec<BoatAnim>,
    pub clouds: Vec<CloudAnim>,
    pub petals: Vec<PetalGpu>,
    pub point_lights: [[f32; 4]; 6],
}

struct Rng(u32);
impl Rng {
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f32) / 4294967296.0
    }
}

fn hash2(x: f32, z: f32) -> f32 {
    let n = (x * 127.1 + z * 311.7).sin() * 43758.5453123;
    n - n.floor()
}
fn noise2(x: f32, z: f32) -> f32 {
    let ix = x.floor(); let iz = z.floor();
    let fx = x - ix; let fz = z - iz;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uz = fz * fz * (3.0 - 2.0 * fz);
    let a = hash2(ix, iz); let b = hash2(ix + 1.0, iz);
    let c = hash2(ix, iz + 1.0); let d = hash2(ix + 1.0, iz + 1.0);
    a + (b - a) * ux + (c - a) * uz + (a - b - c + d) * ux * uz
}
fn fbm2(x: f32, z: f32, oct: i32) -> f32 {
    let (mut v, mut a, mut f) = (0.0, 0.5, 1.0);
    for _ in 0..oct { v += a * noise2(x * f, z * f); f *= 2.02; a *= 0.5; }
    v
}

pub fn river_z(x: f32) -> f32 { 24.0 + 9.0 * (x * 0.032).sin() + 4.0 * (x * 0.065).cos() }
pub fn river_hw(x: f32) -> f32 { 6.2 + 1.2 * (x * 0.09).sin() }
pub fn is_lotus_lake(x: f32, z: f32) -> bool { ((x - 84.0) * 0.85).hypot((z - 25.0) * 1.1) < 15.5 }
pub fn is_stream(x: f32, z: f32) -> bool { (x - (78.0 + 2.5 * (z * 0.1).sin())).abs() < 3.2 && z >= -14.0 && z <= 22.0 }
pub fn is_water_body(x: i32, z: i32) -> bool {
    let (fx, fz) = (x as f32, z as f32);
    (fz - river_z(fx)).abs() <= river_hw(fx) || is_lotus_lake(fx, fz) || is_stream(fx, fz)
}

mod col {
    pub const GRASS_1: u32 = 0x568c32;
    pub const GRASS_2: u32 = 0x487529;
    pub const GRASS_RICE: u32 = 0x67a63c;
    pub const DIRT: u32 = 0x59483a;
    pub const BEDROCK: u32 = 0x2c3034;
    pub const STONE_DARK: u32 = 0x33363b;
    pub const STONE_MID: u32 = 0x5c6063;
    pub const COURTYARD: u32 = 0x8c9296;
    pub const COURTYARD_ALT: u32 = 0x7d8388;
    pub const COURTYARD_LT: u32 = 0x9ca2a6;
    pub const MTN_BASE: u32 = 0x7a7d80;
    pub const MTN_MID: u32 = 0x8f9396;
    pub const MTN_DARK: u32 = 0x63676b;
    pub const SNOW: u32 = 0xffffff;
    pub const SNOW_SHADE: u32 = 0xedf2f7;
    pub const SAND: u32 = 0x9e9785;
    pub const WATER: u32 = 0x32a8a4;
    pub const ROOF_DARK: u32 = 0x1e2024;
    pub const ROOF_TRIM: u32 = 0x2c3036;
    pub const CRIMSON: u32 = 0x9e2a2b;
    pub const VERMILION: u32 = 0xb81d13;
    pub const TORII_BASE: u32 = 0x1b1d21;
    pub const PLASTER: u32 = 0xf4f1ea;
    pub const PLASTER_W: u32 = 0xebe4d5;
    pub const WOOD_WARM: u32 = 0xb5651d;
    pub const WOOD_DARK: u32 = 0x3b281c;
    pub const WOOD_BEAM: u32 = 0x6e3f1c;
    pub const GOLD: u32 = 0xe6b800;
    pub const WIN_GLOW: u32 = 0xfff3b0;
    pub const LAMP_GLOW: u32 = 0xff7b00;
    pub const SAKURA_1: u32 = 0xffb7c5;
    pub const SAKURA_2: u32 = 0xff9eb5;
    pub const SAKURA_3: u32 = 0xffd1dc;
    pub const SAKURA_DEEP: u32 = 0xf08ba4;
    pub const PINE: u32 = 0x2d6a1e;
    pub const PINE_DARK: u32 = 0x225216;
    pub const BAMBOO: u32 = 0x4d9430;
    pub const BAMBOO_LEAF: u32 = 0x397d25;
}

struct Grid {
    mat: Vec<u8>,
    col: Vec<u32>,
    sh: Vec<i32>,
}

impl Grid {
    fn new() -> Self {
        Self { mat: vec![0; SX * SY * SZ], col: vec![0; SX * SY * SZ], sh: vec![10; SX * SZ] }
    }
    #[inline]
    fn idx(ix: usize, iy: usize, iz: usize) -> usize { (iy * SZ + iz) * SX + ix }
    fn set(&mut self, wx: i32, wy: i32, wz: i32, m: u8, c: u32) {
        let (ix, iy, iz) = (wx + HX, wy, wz + HZ);
        if ix >= 0 && ix < SX as i32 && iy >= 0 && iy < SY as i32 && iz >= 0 && iz < SZ as i32 {
            let i = Self::idx(ix as usize, iy as usize, iz as usize);
            self.mat[i] = m;
            self.col[i] = c;
        }
    }
    fn get_mat(&self, ix: i32, iy: i32, iz: i32) -> u8 {
        if ix < 0 || ix >= SX as i32 || iy < 0 || iy >= SY as i32 || iz < 0 || iz >= SZ as i32 { return 0; }
        self.mat[Self::idx(ix as usize, iy as usize, iz as usize)]
    }
    fn fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, m: u8, c: u32) {
        for y in y0.min(y1)..=y0.max(y1) {
            for z in z0.min(z1)..=z0.max(z1) {
                for x in x0.min(x1)..=x0.max(x1) { self.set(x, y, z, m, c); }
            }
        }
    }
    fn set_sh(&mut self, x: i32, z: i32, h: i32) {
        let (ix, iz) = (x + HX, z + HZ);
        if ix >= 0 && ix < SX as i32 && iz >= 0 && iz < SZ as i32 { self.sh[iz as usize * SX + ix as usize] = h; }
    }
    fn get_sh(&self, x: i32, z: i32) -> i32 {
        let (ix, iz) = (x + HX, z + HZ);
        if ix >= 0 && ix < SX as i32 && iz >= 0 && iz < SZ as i32 { self.sh[iz as usize * SX + ix as usize] } else { 10 }
    }
}

pub fn generate_world() -> WorldData {
    let mut g = Grid::new();
    let mut rng = Rng(20261113);

    // 1. TERRAIN
    let peaks = [
        (-84f32, -92f32, 52f32, 34f32), (-44.0, -102.0, 60.0, 36.0),
        (-4.0, -106.0, 64.0, 38.0), (42.0, -100.0, 58.0, 36.0),
        (86.0, -90.0, 54.0, 34.0), (-105.0, -64.0, 42.0, 28.0),
        (105.0, -62.0, 40.0, 28.0), (-94.0, -32.0, 24.0, 22.0),
    ];

    for z in -120..120 {
        for x in -120i32..120 {
            let (fx, fz) = (x as f32, z as f32);
            let edge = (fx + 0.5).abs().max((fz + 0.5).abs());
            let bot_y = if edge > 106.0 { (2.0 + (edge - 106.0) * 0.45).floor() as i32 } else { 1 };
            let (rc, rw) = (river_z(fx), river_hw(fx));
            let driv = (fz - rc).abs();
            let in_lake = is_lotus_lake(fx, fz);
            let in_stream = is_stream(fx, fz);
            let wf = (x - 78).abs() <= 3 && z >= -66 && z <= -14;

            let (mut top_y, mut is_plat, mut is_mtn, mut is_riv, mut is_rice) = (10, false, false, false, false);

            if z <= -58 || (z <= -22 && x.abs() >= 70 && !wf) {
                is_mtn = true;
                let mut mh = 22.0f32;
                for &(px, pz, ph, pr) in &peaks {
                    let d = ((fx - px) * 0.92).hypot((fz - pz) * 1.1);
                    if d < pr { mh = mh.max(22.0 + ph * (1.0 - d / pr).powf(1.32)); }
                }
                let crag = (fbm2(fx * 0.065 + 11.0, fz * 0.065 + 7.0, 3) - 0.35) * 18.0;
                let ridge = (noise2(fx * 0.1, fz * 0.1) - 0.5).abs() * 12.0;
                top_y = (mh + crag + ridge).max(20.0).round() as i32;
                if z > -64 && x >= -68 && x <= 68 {
                    let t = (fz + 64.0) / 6.0;
                    top_y = ((top_y as f32) * (1.0 - t) + 24.0 * t).round() as i32;
                }
                if (fx + 86.0).hypot(fz + 44.0) < 12.0 { top_y = 28; }
                if x >= -88 && x <= -74 && z >= -42 && z <= -10 {
                    top_y = (12.0 + ((-10.0 - fz) / 32.0) * 16.0).round() as i32;
                }
            } else if z >= -60 && z <= -12 && x >= -68 && x <= 68 {
                is_plat = true;
                top_y = if z <= -34 && x.abs() <= 56 { 24 } else { 20 };
            } else if x >= 72 && x <= 84 && z >= -66 && z <= -14 {
                is_mtn = true;
                top_y = (10.0 + ((-14.0 - fz) / 52.0) * 28.0).round() as i32;
                if wf { top_y -= 2; }
            } else if driv <= rw + 1.5 || in_lake || in_stream {
                if driv <= rw || in_lake || in_stream {
                    is_riv = true;
                    top_y = if in_stream { 6 } else if in_lake || (1.0 - driv / rw) > 0.35 { 5 } else { 6 };
                } else { top_y = 9; }
            } else if x.abs() >= 68 && z >= -22 && fz < rc - rw - 1.0 {
                let sx = (x.abs() as f32 - 68.0) / 52.0;
                top_y = (10.0 + sx * 10.0 + fbm2(fx * 0.06, fz * 0.06, 2) * 3.0).round() as i32;
            } else if x >= 74 && x <= 112 && z >= 54 && z <= 102 {
                is_rice = true;
                top_y = 10 + (x - 74) / 10;
            } else {
                top_y = 10 + if fbm2(fx * 0.04, fz * 0.04, 2) > 0.68 { 1 } else { 0 };
                if x.abs() <= 22 && z >= -12 && z <= 112 { top_y = 10; }
                if z >= 40 && z <= 102 && x.abs() <= 98 { top_y = 10; }
                if z >= -10 && z <= 14 && x.abs() <= 72 { top_y = 10; }
            }

            g.set_sh(x, z, if is_riv { 8 } else { top_y });
            for y in bot_y..=top_y {
                let c = if (y <= bot_y + 1 && edge > 90.0) || (bot_y == 1 && y <= 2) {
                    col::BEDROCK
                } else if is_mtn {
                    let snow_y = 48.0 + fbm2(fx * 0.08, fz * 0.08, 2) * 9.0;
                    if (y as f32) >= snow_y && y >= top_y - 2 && !wf {
                        if (x + z) % 3 == 0 { col::SNOW_SHADE } else { col::SNOW }
                    } else if y == top_y && top_y <= 25 && !wf && rng.next_f32() < 0.32 {
                        col::GRASS_2
                    } else {
                        let pk = hash2(fx * 1.7 + y as f32, fz * 1.3);
                        if pk > 0.66 { col::MTN_MID } else if pk > 0.3 { col::MTN_BASE } else { col::MTN_DARK }
                    }
                } else if is_plat {
                    if y == top_y {
                        if x.abs() == 68 || z == -12 || z == -60 || (z == -34 && x.abs() <= 56) { col::STONE_MID }
                        else if (x.abs() + z.abs()) % 5 == 0 { col::COURTYARD_ALT }
                        else if (x * z) % 9 == 0 { col::COURTYARD_LT }
                        else { col::COURTYARD }
                    } else if y >= 12 {
                        if (y + x + z) % 2 == 0 { col::STONE_MID } else { col::MTN_BASE }
                    } else { col::MTN_DARK }
                } else if is_riv {
                    if y == top_y { col::SAND } else { col::MTN_DARK }
                } else if is_rice {
                    if y == top_y { if (x - 74) % 10 == 0 { col::DIRT } else { col::GRASS_RICE } } else { col::DIRT }
                } else if y == top_y {
                    if noise2(fx * 0.18, fz * 0.18) > 0.48 { col::GRASS_1 } else { col::GRASS_2 }
                } else if y >= top_y - 2 {
                    col::DIRT
                } else if (x + y + z) % 3 == 0 { col::MTN_DARK } else { col::MTN_BASE };

                g.set(x, y, z, MAT_MATTE, c);
            }
            if is_riv {
                for y in (top_y + 1)..=8 { g.set(x, y, z, MAT_WATER, col::WATER); }
            }
            if wf && z >= -64 && z <= -14 {
                for wy in (top_y + 1)..=(top_y + 3) { g.set(x, wy, z, MAT_WATER, col::WATER); }
            }
        }
    }
    for x in -68i32..=68 {
        if x.abs() > 8 && (x.abs() - 46).abs() > 3 {
            g.set(x, 21, -12, MAT_MATTE, if x % 2 == 0 { col::STONE_MID } else { col::STONE_DARK });
        }
    }
    for z in -60..=-12 {
        let c = if z % 2 == 0 { col::STONE_MID } else { col::STONE_DARK };
        g.set(-68, 21, z, MAT_MATTE, c);
        g.set(68, 21, z, MAT_MATTE, c);
    }

    // 2. PATHS, STAIRS & BRIDGES
    fn cobble(g: &mut Grid, cx: i32, cz: i32, r: i32) {
        for dz in -r..=r { for dx in -r..=r {
            let (wx, wz) = (cx + dx, cz + dz);
            if wx < -120 || wx >= 120 || wz < -120 || wz >= 120 || is_water_body(wx, wz) { continue; }
            let h = g.get_sh(wx, wz);
            g.set(wx, h, wz, MAT_MATTE, if (wx.abs() + wz.abs()) % 3 == 0 { 0x3e4248 } else { col::STONE_DARK });
        }}
    }
    for z in 39..=112 { cobble(&mut g, 0, z, 3); }
    for x in -14..=14 { for z in 62..=74 { cobble(&mut g, x, z, 0); } }
    for x in -92..=68 { cobble(&mut g, x, 68, 2); }
    for x in -80..=74 { cobble(&mut g, x, 48, 1); }
    for &bx in &[-58, -32, 32, 60] { for z in 44..=96 { cobble(&mut g, bx, z, 1); } }
    for z in 3..=16 { cobble(&mut g, 0, z, 3); }
    for x in -76..=68 { cobble(&mut g, x, 6, 1); }
    for z in 6..=21 { cobble(&mut g, 60, z, 1); }

    fn stairs(g: &mut Grid, cx: i32, hw: i32, zb: i32, yb: i32, zt: i32, yt: i32) {
        let steps = (zb - zt).abs();
        let dz = if zt < zb { -1 } else { 1 };
        for s in 0..=steps {
            let z = zb + s * dz;
            let y = (yb as f32 + (s as f32 / steps as f32) * (yt - yb) as f32).round() as i32;
            for x in (cx - hw)..=(cx + hw) {
                for fy in (yb - 1)..=y {
                    g.set(x, fy, z, MAT_MATTE, if fy == y { if x % 2 == 0 { col::STONE_MID } else { 0x676b6f } } else { col::MTN_DARK });
                }
                g.set_sh(x, z, y);
            }
            for rx in [cx - hw - 1, cx + hw + 1] {
                for fy in (yb - 1)..=(y + 1) { g.set(rx, fy, z, MAT_MATTE, col::STONE_DARK); }
            }
        }
    }
    stairs(&mut g, 0, 7, 2, 10, -12, 20);
    stairs(&mut g, -46, 3, -1, 10, -12, 20);
    stairs(&mut g, 46, 3, -1, 10, -12, 20);
    stairs(&mut g, 0, 6, -26, 20, -34, 24);
    stairs(&mut g, -36, 4, -26, 20, -34, 24);
    stairs(&mut g, 30, 5, -26, 20, -34, 24);

    for z in -33..=-13 { cobble(&mut g, 0, z, 2); }
    for x in -46..=46 { for z in -23..=-21 { g.set(x, 20, z, MAT_MATTE, if (x + z) % 2 == 0 { col::STONE_DARK } else { 0x41454b }); } }
    for z in -40..=-10 {
        let y = g.get_sh(-80, z);
        for x in -82..=-78 {
            g.set(x, y, z, MAT_MATTE, col::STONE_MID);
            for cy in (y + 1)..=(y + 9) { g.set(x, cy, z, MAT_AIR, 0); }
        }
    }

    // Bridges
    for z in 16..=40 {
        let t = (z as f32 - 28.0) / 12.0;
        let ay = (10.0 + 4.8 * (1.0 - t * t)).round() as i32;
        for x in -4i32..=4 {
            let edge = x.abs() == 4;
            g.set(x, ay, z, MAT_WOOD, if edge { col::WOOD_BEAM } else { col::WOOD_WARM });
            for cy in (ay + 1)..=(ay + 6) { g.set(x, cy, z, MAT_AIR, 0); }
            if edge {
                g.set(x, ay + 1, z, MAT_WOOD, col::VERMILION);
                if (z - 16) % 4 == 0 { g.set(x, ay + 2, z, MAT_WOOD, col::VERMILION); }
            }
        }
        if z == 21 || z == 28 || z == 35 { for px in [-4, 4] { for y in 5..ay { g.set(px, y, z, MAT_MATTE, col::STONE_DARK); } } }
    }
    for z in 20..=40 {
        let t = (z as f32 - 30.0) / 10.0;
        let ay = (10.0 + 3.2 * (1.0 - t * t)).round() as i32;
        for x in 58..=62 {
            let edge = x == 58 || x == 62;
            g.set(x, ay, z, MAT_WOOD, if edge { col::WOOD_BEAM } else { col::WOOD_WARM });
            for cy in (ay + 1)..=(ay + 5) { g.set(x, cy, z, MAT_AIR, 0); }
            if edge {
                g.set(x, ay + 1, z, MAT_WOOD, col::VERMILION);
                if (z - 20) % 5 == 0 { g.set(x, ay + 2, z, MAT_WOOD, col::VERMILION); }
            }
        }
        if z == 25 || z == 35 { for px in [58, 62] { for y in 5..ay { g.set(px, y, z, MAT_MATTE, col::STONE_DARK); } } }
    }

    // 3. TORII GATES
    fn torii(g: &mut Grid, cx: i32, by: i32, cz: i32, axis_z: bool, hspan: i32, ph: i32) {
        for s in [-hspan, hspan] {
            let (wx, wz) = if axis_z { (cx + s, cz) } else { (cx, cz + s) };
            for dy in 0..=1 { g.set(wx, by + dy, wz, MAT_MATTE, col::TORII_BASE); }
            for dy in 2..=ph { g.set(wx, by + dy, wz, MAT_WOOD, col::VERMILION); }
        }
        let nuki = by + ph - 2;
        for i in (-hspan - 1)..=(hspan + 1) {
            let (wx, wz) = if axis_z { (cx + i, cz) } else { (cx, cz + i) };
            g.set(wx, nuki, wz, MAT_WOOD, col::VERMILION);
        }
        g.set(cx, nuki + 1, cz, MAT_GOLD, col::GOLD);
        let top = by + ph + 1;
        for i in (-hspan - 2)..=(hspan + 2) {
            let (wx, wz) = if axis_z { (cx + i, cz) } else { (cx, cz + i) };
            if i.abs() != hspan + 2 {
                g.set(wx, top - 1, wz, MAT_WOOD, col::VERMILION);
                g.set(wx, top, wz, MAT_WOOD, col::ROOF_DARK);
            } else {
                g.set(wx, top, wz, MAT_WOOD, col::ROOF_DARK);
                g.set(wx, top + 1, wz, MAT_WOOD, col::ROOF_DARK);
            }
        }
    }
    torii(&mut g, 0, 10, 92, true, 4, 10);
    torii(&mut g, 0, 10, 54, true, 4, 9);
    torii(&mut g, 0, 10, 9, true, 4, 9);
    let wt_z = river_z(-48.0).round() as i32;
    torii(&mut g, -48, 6, wt_z, true, 4, 11);
    for s in [-4, 4] { g.fill(-48 + s - 1, 5, wt_z - 1, -48 + s + 1, 6, wt_z + 1, MAT_MATTE, col::STONE_MID); }
    torii(&mut g, 84, 6, 26, false, 3, 9);
    for s in [-3, 3] { g.fill(83, 5, 26 + s - 1, 85, 6, 26 + s + 1, MAT_MATTE, col::STONE_MID); }
    for z in (-36..=-12).rev().step_by(3) { let sy = g.get_sh(-80, z); torii(&mut g, -80, sy, z, true, 2, 5); }

    // 4. 5-TIERED PAGODA, MAIN TEMPLE, BELL TOWER & MOUNTAIN SHRINE
    {
        let (cx, cz, base_y) = (-36, -45, 25);
        g.fill(cx - 8, base_y, cz - 8, cx + 8, base_y, cz + 8, MAT_MATTE, col::STONE_MID);
        g.fill(cx - 3, base_y, cz + 9, cx + 3, base_y, cz + 9, MAT_MATTE, col::STONE_MID);
        let tiers: [(i32, i32, i32); 5] = [(5, 9, 5), (4, 8, 4), (4, 7, 4), (3, 6, 4), (3, 5, 3)];
        let mut cy = base_y + 1;
        for (t, &(cr, er, h)) in tiers.iter().enumerate() {
            if t > 0 {
                for dx in (-cr - 1)..=(cr + 1) { for dz in (-cr - 1)..=(cr + 1) {
                    if dx.abs() == cr + 1 || dz.abs() == cr + 1 { g.set(cx + dx, cy, cz + dz, MAT_WOOD, col::CRIMSON); }
                }}
            }
            for dy in 0..h { for dx in -cr..=cr { for dz in -cr..=cr {
                let (ex, ez) = (dx.abs() == cr, dz.abs() == cr);
                if !ex && !ez { continue; }
                let (m, c) = if ex && ez { (MAT_WOOD, col::WOOD_DARK) }
                    else if dy == 1 && (dx == 0 || dz == 0 || dx.abs() == 2 || dz.abs() == 2) { (MAT_MATTE, col::PLASTER) }
                    else if t == 0 && dz == cr && dx.abs() <= 1 && dy <= 2 { (MAT_WIN, col::WIN_GLOW) }
                    else { (MAT_WOOD, col::CRIMSON) };
                g.set(cx + dx, cy + dy, cz + dz, m, c);
            }}}
            let ry = cy + h;
            for dx in (-cr - 1)..=(cr + 1) { for dz in (-cr - 1)..=(cr + 1) {
                if dx.abs() > cr || dz.abs() > cr { g.set(cx + dx, ry, cz + dz, MAT_WOOD, col::WOOD_BEAM); }
            }}
            for dx in -er..=er { for dz in -er..=er {
                if dx.abs() <= cr - 1 && dz.abs() <= cr - 1 { continue; }
                g.set(cx + dx, ry + if dx.abs() == er && dz.abs() == er { 2 } else { 1 }, cz + dz, MAT_WOOD, col::ROOF_DARK);
            }}
            let mr = cr.max(er - 1);
            for dx in -mr..=mr { for dz in -mr..=mr {
                if dx.abs() == mr || dz.abs() == mr { g.set(cx + dx, ry + 2, cz + dz, MAT_WOOD, col::ROOF_TRIM); }
            }}
            cy += h + 2;
        }
        g.fill(cx - 2, cy, cz - 2, cx + 2, cy, cz + 2, MAT_WOOD, col::ROOF_DARK);
        g.fill(cx - 1, cy + 1, cz - 1, cx + 1, cy + 2, cz + 1, MAT_WOOD, col::ROOF_DARK);
        g.set(cx, cy + 3, cz, MAT_GOLD, col::GOLD);
        for sy in 0..11 { g.set(cx, cy + 4 + sy, cz, MAT_GOLD, col::GOLD); }
        for ry in [cy + 6, cy + 8, cy + 10, cy + 12] {
            g.set(cx - 1, ry, cz, MAT_GOLD, col::GOLD); g.set(cx + 1, ry, cz, MAT_GOLD, col::GOLD);
            g.set(cx, ry, cz - 1, MAT_GOLD, col::GOLD); g.set(cx, ry, cz + 1, MAT_GOLD, col::GOLD);
        }
    }

    // Main Temple Complex
    {
        let (cx, cz, base_y) = (30, -45, 25);
        g.fill(cx - 11, base_y, cz - 7, cx + 11, base_y, cz + 7, MAT_MATTE, col::STONE_MID);
        g.fill(cx - 10, base_y + 1, cz - 6, cx + 10, base_y + 1, cz + 6, MAT_WOOD, col::WOOD_WARM);
        g.fill(cx - 3, base_y, cz + 8, cx + 3, base_y, cz + 8, MAT_MATTE, col::COURTYARD);
        let (wx, wz, wh) = (9i32, 5i32, 6i32);
        for dy in 0..wh { for dx in -wx..=wx { for dz in -wz..=wz {
            let (ex, ez) = (dx.abs() == wx, dz.abs() == wz);
            if !ex && !ez { continue; }
            let (mut m, mut c) = (MAT_MATTE, col::PLASTER);
            if (ex && ez) || (ez && dx.abs() % 3 == 0) || (ex && dz.abs() % 2 == 1) { m = MAT_WOOD; c = col::CRIMSON; }
            if dz == wz && dx.abs() <= 2 && dy <= 3 {
                if dy == 0 || dx.abs() == 2 { m = MAT_WOOD; c = col::WOOD_BEAM; } else { m = MAT_WIN; c = col::WIN_GLOW; }
            }
            if ex && dz.abs() <= 2 && dy == 3 { m = MAT_WIN; c = col::WIN_GLOW; }
            g.set(cx + dx, base_y + 2 + dy, cz + dz, m, c);
        }}}
        let ry = base_y + 2 + wh;
        for layer in 0..=6 {
            let (rx, rz) = (wx + 3 - layer, wz + 3 - layer);
            if rz < 0 { break; }
            for dx in -rx..=rx { for dz in -rz..=rz {
                let corner = dx.abs() == rx && dz.abs() == rz;
                let gable = layer >= 4 && dx.abs() == rx && dz.abs() < rz;
                let (m, c) = if gable { (MAT_MATTE, col::PLASTER) }
                    else if layer == 0 && corner { (MAT_GOLD, col::GOLD) }
                    else { (MAT_WOOD, if layer % 2 == 0 { col::ROOF_DARK } else { col::ROOF_TRIM }) };
                g.set(cx + dx, ry + layer + if layer == 0 && corner { 1 } else { 0 }, cz + dz, m, c);
            }}
        }
        for dx in -7i32..=7 {
            let end = dx.abs() >= 6;
            g.set(cx + dx, ry + 7, cz, if end { MAT_GOLD } else { MAT_WOOD }, if end { col::GOLD } else { col::ROOF_DARK });
            if dx.abs() == 7 { g.set(cx + dx, ry + 8, cz, MAT_GOLD, col::GOLD); }
        }
        for wwx in [16, 44] {
            for z in -34..=-16 {
                g.set(wwx, 21, z, MAT_MATTE, col::STONE_MID);
                if z % 3 == 0 { for dy in 1..=3 { g.set(wwx, 21 + dy, z, MAT_WOOD, col::CRIMSON); } }
                for dx in -1..=1 { g.set(wwx + dx, 25, z, MAT_WOOD, col::ROOF_DARK); }
                g.set(wwx, 26, z, MAT_WOOD, col::ROOF_TRIM);
            }
        }
        for x in 16..=44 {
            if (x - cx).abs() > 3 {
                g.set(x, 21, -16, MAT_MATTE, col::STONE_MID);
                g.set(x, 22, -16, MAT_MATTE, col::PLASTER);
                g.set(x, 23, -16, MAT_MATTE, col::PLASTER);
                g.set(x, 24, -16, MAT_WOOD, col::ROOF_DARK);
            } else {
                if (x - cx).abs() == 3 { for dy in 0..=4 { g.set(x, 21 + dy, -16, MAT_WOOD, col::CRIMSON); } }
                for dz in -1..=1 { g.set(x, 25, -16 + dz, MAT_WOOD, col::ROOF_DARK); }
                g.set(x, 26, -16, MAT_GOLD, if (x - cx).abs() == 3 { col::GOLD } else { col::ROOF_DARK });
            }
        }
        g.fill(cx - 1, 21, -24, cx + 1, 21, -22, MAT_MATTE, col::STONE_MID);
        g.set(cx, 22, -23, MAT_MATTE, col::STONE_DARK);
        g.set(cx, 23, -23, MAT_LAMP, col::LAMP_GLOW);
    }

    // Bell Tower (Shoro)
    {
        let (bx, bz, by) = (0, -46, 25);
        g.fill(bx - 3, by, bz - 3, bx + 3, by, bz + 3, MAT_MATTE, col::STONE_MID);
        for px in [-2, 2] { for pz in [-2, 2] { for dy in 1..=4 { g.set(bx + px, by + dy, bz + pz, MAT_WOOD, col::CRIMSON); } } }
        g.fill(bx - 1, by + 2, bz - 1, bx + 1, by + 3, bz + 1, MAT_GOLD, col::GOLD);
        for l in 0..=3 {
            let r = 4 - l;
            for dx in -r..=r { for dz in -r..=r { g.set(bx + dx, by + 5 + l, bz + dz, MAT_WOOD, if l == 3 { col::ROOF_TRIM } else { col::ROOF_DARK }); } }
        }
        g.set(bx, by + 9, bz, MAT_GOLD, col::GOLD);
    }

    // 5. VILLAGE HOUSES & MOUNTAIN SHRINE
    fn build_house(g: &mut Grid, cx: i32, by: i32, cz: i32, hw: i32, hd: i32, wh: i32, ax_z: bool, inn: bool) {
        g.fill(cx - hw, by, cz - hd, cx + hw, by, cz + hd, MAT_MATTE, col::STONE_MID);
        for dy in 1..=wh { for dx in -hw..=hw { for dz in -hd..=hd {
            let (ox, oz) = (dx.abs() == hw, dz.abs() == hd);
            if !ox && !oz { continue; }
            let (mut m, mut c) = (if dy == 1 { MAT_WOOD } else { MAT_MATTE }, if dy == 1 { col::WOOD_BEAM } else { col::PLASTER_W });
            if ox && oz { m = MAT_WOOD; c = if inn { col::CRIMSON } else { col::WOOD_DARK }; }
            else if (dy == 2 || dy == 4) && ((oz && dx.abs() <= hw - 1 && dx % 2 == 0) || (ox && dz.abs() <= hd - 1 && dz % 2 == 0)) {
                m = MAT_WIN; c = col::WIN_GLOW;
            }
            g.set(cx + dx, by + dy, cz + dz, m, c);
        }}}
        let rs = by + wh + 1;
        let span = if ax_z { hd + 1 } else { hw + 1 };
        for l in 0..=span {
            if ax_z {
                let rz = hd + 1 - l;
                for dx in (-hw - 1)..=(hw + 1) { for dz in -rz..=rz {
                    if dz.abs() == rz || l == span { g.set(cx + dx, rs + l, cz + dz, MAT_WOOD, if l == span { col::ROOF_TRIM } else { col::ROOF_DARK }); }
                    else if dx.abs() == hw { g.set(cx + dx, rs + l, cz + dz, MAT_MATTE, col::PLASTER_W); }
                }}
            } else {
                let rx = hw + 1 - l;
                for dz in (-hd - 1)..=(hd + 1) { for dx in -rx..=rx {
                    if dx.abs() == rx || l == span { g.set(cx + dx, rs + l, cz + dz, MAT_WOOD, if l == span { col::ROOF_TRIM } else { col::ROOF_DARK }); }
                    else if dz.abs() == hd { g.set(cx + dx, rs + l, cz + dz, MAT_MATTE, col::PLASTER_W); }
                }}
            }
        }
    }
    build_house(&mut g, -85, 28, -45, 4, 3, 3, true, true);
    let houses = [
        (-19,60,5,4,5,false,true),(19,60,5,4,5,false,true),(-19,76,4,4,4,true,false),(19,76,4,4,4,true,false),
        (-14,92,4,3,3,true,false),(14,92,4,3,3,true,false),(-38,56,4,3,3,true,false),(-54,56,5,4,4,false,false),
        (-72,58,4,3,3,true,false),(-38,76,4,4,3,false,false),(-54,78,4,3,4,true,false),(-74,78,5,3,3,true,false),
        (-42,94,4,3,3,true,false),(-62,94,4,4,3,false,false),(-84,90,4,3,3,false,false),(38,56,4,3,3,true,false),
        (54,56,5,4,4,false,false),(38,76,4,4,3,false,false),(54,78,4,3,4,true,false),(40,94,4,3,3,true,false),
        (60,94,5,3,3,true,false),(24,0,4,3,3,true,false),(48,0,4,3,3,false,true),(74,2,4,3,3,true,false),(-28,0,4,3,3,true,false),
    ];
    for &(x, z, hw, hd, h, ax, inn) in &houses { let sy = g.get_sh(x, z); build_house(&mut g, x, sy, z, hw, hd, h, ax, inn); }

    // 6. FLORA & LANTERNS
    fn sakura_tree(g: &mut Grid, rng: &mut Rng, x: i32, by: i32, z: i32, sc: f32) {
        let th = ((4.5 + rng.next_f32() * 2.5) * sc).round() as i32;
        let (mut tx, mut tz) = (x, z);
        for dy in 1..=th {
            g.set(tx, by + dy, tz, MAT_WOOD, col::WOOD_DARK);
            if sc >= 1.1 && dy <= 2 { g.set(tx + 1, by + dy, tz, MAT_WOOD, col::WOOD_DARK); }
            if dy == 2 && rng.next_f32() < 0.6 { tx += if rng.next_f32() < 0.5 { 1 } else { -1 }; }
            if dy == 3 && rng.next_f32() < 0.6 { tz += if rng.next_f32() < 0.5 { 1 } else { -1 }; }
        }
        let cy = by + th;
        let rad = (3.6 * sc).round() as i32;
        fn blob(g: &mut Grid, rng: &mut Rng, cx: i32, ccy: i32, cz: i32, rx: i32, ry: i32, rz: i32) {
            for dy in -ry..=ry { for dz in -rz..=rz { for dx in -rx..=rx {
                let n = (dx * dx) as f32 / (rx * rx) as f32 + (dy * dy) as f32 / (ry * ry) as f32 + (dz * dz) as f32 / (rz * rz) as f32;
                if n <= 1.05 && !(n > 0.82 && rng.next_f32() < 0.35) {
                    let r = rng.next_f32();
                    let c = if dy < 0 && r < 0.45 { col::SAKURA_DEEP } else if r < 0.34 { col::SAKURA_2 } else if r > 0.72 { col::SAKURA_3 } else { col::SAKURA_1 };
                    g.set(cx + dx, ccy + dy, cz + dz, MAT_LEAVES, c);
                }
            }}}
        }
        blob(g, rng, tx, cy + 1, tz, rad + 1, 1, rad + 1);
        let br = 3 + (rng.next_f32() * 2.0) as i32;
        for b in 0..br {
            let ang = (b as f32 / br as f32) * std::f32::consts::TAU + rng.next_f32() * 0.8;
            let bx = tx + (ang.cos() * 2.4 * sc).round() as i32;
            let bz = tz + (ang.sin() * 2.4 * sc).round() as i32;
            g.set((tx + bx) / 2, cy, (tz + bz) / 2, MAT_WOOD, col::WOOD_DARK);
            let by_off = if rng.next_f32() < 0.5 { 0 } else { 1 };
            blob(g, rng, bx, cy + by_off, bz, (rad - 1).max(2), 1, (rad - 1).max(2));
        }
    }
    let hero_sakura = [
        (-10,104,1.2),(10,104,1.2),(-10,84,1.1),(10,84,1.1),(-10,46,1.15),(10,46,1.15),
        (-14,14,1.15),(14,14,1.15),(-32,40,1.2),(32,42,1.2),(-64,42,1.15),(68,42,1.2),
        (-58,-20,1.2),(58,-20,1.2),(-56,-52,1.1),(56,-52,1.1),(-16,-30,1.05),(12,-30,1.05),
        (24,-24,0.8),(36,-24,0.8),(96,8,1.2),(98,40,1.15),(-96,30,1.2),(-96,60,1.15),
    ];
    for &(sx, sz, sc) in &hero_sakura { if !is_water_body(sx, sz) { let sy = g.get_sh(sx, sz); sakura_tree(&mut g, &mut rng, sx, sy, sz, sc); } }
    for _ in 0..65 {
        let sx = ((rng.next_f32() - 0.5) * 204.0).round() as i32;
        let sz = (-54.0 + rng.next_f32() * 160.0).round() as i32;
        if is_water_body(sx, sz) || (sx.abs() < 12 && sz > -15) { continue; }
        if (sx >= -52 && sx <= 50 && sz >= -58 && sz <= -14) || (sx >= 72 && sx <= 114 && sz >= 52 && sz <= 104) || (sx < -68 && sz < 20) { continue; }
        let h = g.get_sh(sx, sz);
        if h <= 26 { let sc = 0.9 + rng.next_f32() * 0.35; sakura_tree(&mut g, &mut rng, sx, h, sz, sc); }
    }

    fn pine(g: &mut Grid, rng: &mut Rng, x: i32, by: i32, z: i32) {
        let h = 8 + (rng.next_f32() * 5.0) as i32;
        for dy in 1..=(h - 2) { g.set(x, by + dy, z, MAT_WOOD, col::WOOD_DARK); }
        for ly in 3..=(h + 1) {
            let r = (((h + 1 - ly) as f32) * 0.48).round().max(0.0) as i32;
            for dx in -r..=r { for dz in -r..=r {
                if dx.abs() + dz.abs() <= r + if (ly - 3) % 2 == 1 { 0 } else { 1 } {
                    g.set(x + dx, by + ly, z + dz, MAT_LEAVES, if rng.next_f32() < 0.35 { col::PINE_DARK } else { col::PINE });
                }
            }}
        }
    }
    fn bamboo(g: &mut Grid, rng: &mut Rng, x: i32, by: i32, z: i32) {
        let h = 8 + (rng.next_f32() * 7.0) as i32;
        for dy in 1..=h {
            g.set(x, by + dy, z, MAT_WOOD, if dy % 3 == 0 { col::PINE } else { col::BAMBOO });
            if dy >= h - 2 {
                for [ox, oz] in [[1,0],[-1,0],[0,1],[0,-1]] {
                    if rng.next_f32() < 0.55 { g.set(x + ox, by + dy, z + oz, MAT_LEAVES, col::BAMBOO_LEAF); }
                }
            }
        }
    }
    for z in (-56..=20).step_by(5) { for x in (-114..=-70).step_by(5) {
        let (px, pz) = (x + (rng.next_f32() * 3.0) as i32 - 1, z + (rng.next_f32() * 3.0) as i32 - 1);
        if is_water_body(px, pz) || (px >= -88 && px <= -74 && pz <= -8 && pz >= -50) { continue; }
        if rng.next_f32() < 0.55 { let py = g.get_sh(px, pz); pine(&mut g, &mut rng, px, py, pz); }
        else {
            let py = g.get_sh(px, pz);
            bamboo(&mut g, &mut rng, px, py, pz);
            if rng.next_f32() < 0.7 { let py2 = g.get_sh(px + 1, pz); bamboo(&mut g, &mut rng, px + 1, py2, pz); }
        }
    }}
    for z in (-54..=10).step_by(6) { for x in (86..=114).step_by(6) { if !is_water_body(x, z) { let py = g.get_sh(x, z); pine(&mut g, &mut rng, x, py, z); } } }

    let lamps = [
        (-6,102,2),(6,102,2),(-6,84,2),(6,84,2),(-10,68,2),(10,68,2),(-6,50,2),(6,50,2),
        (-6,41,2),(6,41,2),(-6,15,2),(6,15,2),(-8,3,1),(8,3,1),(-8,-11,1),(8,-11,1),
        (-24,-22,2),(24,-22,2),(-36,-36,2),(22,-36,2),(38,-36,2),(-40,68,2),(40,68,2),(60,18,2),(60,42,2),
    ];
    for &(lx, lz, t) in &lamps {
        let by = g.get_sh(lx, lz);
        g.set(lx, by + 1, lz, MAT_MATTE, col::STONE_DARK);
        if t >= 2 { g.set(lx, by + 2, lz, MAT_MATTE, col::STONE_MID); }
        g.set(lx, by + t + 1, lz, MAT_LAMP, col::LAMP_GLOW);
        g.set(lx, by + t + 2, lz, MAT_WOOD, col::ROOF_DARK);
    }
    g.set(-4, 18, 28, MAT_LAMP, col::LAMP_GLOW);
    g.set(4, 18, 28, MAT_LAMP, col::LAMP_GLOW);

    // 7. SURFACE-CULLED INSTANCE EXTRACTION
    let mut shadow_opaque = Vec::new();
    let mut noshadow_opaque = Vec::new();
    let mut translucent_base = Vec::new();
    let dirs = [[1,0,0],[-1,0,0],[0,1,0],[0,-1,0],[0,0,1],[0,0,-1]];

    for iy in 0..SY as i32 { for iz in 0..SZ as i32 { for ix in 0..SX as i32 {
        let i = Grid::idx(ix as usize, iy as usize, iz as usize);
        let m = g.mat[i];
        if m == MAT_AIR { continue; }
        let mut vis = false;
        if m == MAT_WATER {
            if g.get_mat(ix, iy + 1, iz) == MAT_AIR || ix == 0 || ix == SX as i32 - 1 || iz == 0 || iz == SZ as i32 - 1 { vis = true; }
        } else {
            for d in dirs {
                let nm = g.get_mat(ix + d[0], iy + d[1], iz + d[2]);
                if nm == MAT_AIR || nm == MAT_WATER { vis = true; break; }
            }
        }
        if !vis { continue; }
        let (wx, wy, wz, c) = ((ix - HX) as f32, iy as f32, (iz - HZ) as f32, g.col[i]);
        if m == MAT_WATER {
            translucent_base.push(VoxelInstance::new(wx, wy - 0.075, wz, 0.0, 1.0, 0.85, 1.0, c, m));
        } else if c == col::BEDROCK {
            noshadow_opaque.push(VoxelInstance::new(wx, wy, wz, 0.0, 1.0, 1.0, 1.0, c, m));
        } else {
            shadow_opaque.push(VoxelInstance::new(wx, wy, wz, 0.0, 1.0, 1.0, 1.0, c, m));
        }
    }}}

    // Static Lily Pads + Lotus Flowers
    let mut pad_count = 0usize;
    for x in (-112i32..=112).step_by(3) {
        if x.abs() < 7 || (x - 60).abs() < 5 || (x + 48).abs() < 7 { continue; }
        if rng.next_f32() < 0.6 {
            let px = x as f32 + (rng.next_f32() - 0.5) * 1.4;
            let pz = river_z(x as f32) + (rng.next_f32() - 0.5) * (river_hw(x as f32) - 1.2) * 1.4;
            let rot = rng.next_f32() * std::f32::consts::PI;
            shadow_opaque.push(VoxelInstance::new(px, 8.38, pz, rot, 0.88, 0.1, 0.88, 0x459e32, MAT_LEAVES));
            if pad_count % 3 == 0 {
                shadow_opaque.push(VoxelInstance::new(px, 8.48, pz, rot, 0.3, 0.16, 0.3, col::SAKURA_1, MAT_LEAVES));
            }
            pad_count += 1;
        }
    }

    let shadow_opaque_count = shadow_opaque.len() as u32;
    let mut opaque = shadow_opaque;
    opaque.extend(noshadow_opaque);

    // 8. DYNAMIC ENTITIES (BOATS, VILLAGERS, CLOUDS, GPU PETALS)
    let boats = vec![
        (-80.0, river_z(-80.0), 0.25), (-24.0, river_z(-24.0), -0.3), (24.0, river_z(24.0) + 1.0, 0.2),
        (44.0, river_z(44.0) - 1.0, -0.15), (76.0, 28.0, 0.6), (92.0, 22.0, -0.5),
    ].into_iter().map(|(x, z, rot)| BoatAnim { x, y: 8.35, z, rot, phase: rng.next_f32() * 6.28 }).collect();

    let mut villagers = Vec::new();
    fn add_v(villagers: &mut Vec<VillagerAnim>, rng: &mut Rng, bx: f32, by: f32, bz: f32, ci: usize, rot_y: f32, axis_x: bool, range: f32) {
        villagers.push(VillagerAnim {
            bx, by, bz, rot_y, axis_x, range,
            phase: rng.next_f32() * 6.28, speed: 0.55 + rng.next_f32() * 0.35, color_idx: ci,
        });
    }
    add_v(&mut villagers, &mut rng, -1.5, 15.5, 28.0, 0, 1.8, false, 2.8);
    add_v(&mut villagers, &mut rng, 1.6, 15.5, 26.0, 1, -1.2, false, 0.0);
    add_v(&mut villagers, &mut rng, 0.2, 15.5, 30.5, 2, 0.2, false, 0.0);
    add_v(&mut villagers, &mut rng, 60.0, 13.5, 30.0, 1, 0.0, false, 2.5);
    for &(vx, vz, ci, ax, rg) in &[
        (-4,68,0,true,5.0),(6,65,1,false,4.0),(-8,72,2,true,0.0),(10,70,0,true,0.0),
        (-2,98,1,false,5.0),(2,82,2,false,4.0),(-2,46,0,false,4.0),(-36,68,1,true,6.0),
        (-60,68,2,true,5.0),(34,68,0,true,6.0),(56,68,1,true,5.0),(-46,48,2,true,0.0),
        (44,48,0,true,0.0),(70,6,1,true,0.0),(22,6,2,true,4.0),(-24,6,0,true,4.0),
        (-6,-22,2,true,6.0),(10,-22,0,true,5.0),(-28,-22,1,true,4.0),(-30,-34,0,true,0.0),
        (28,-22,2,true,0.0),(32,-32,1,true,0.0),(-4,-42,0,true,0.0),
    ] {
        let rot = rng.next_f32() * 6.28;
        let gy = g.get_sh(vx, vz);
        add_v(&mut villagers, &mut rng, vx as f32, gy as f32 + 0.5, vz as f32, ci, rot, ax, rg);
    }

    let mut clouds = Vec::new();
    for _ in 0..22 {
        let mut offsets = Vec::new();
        let (lx, lz) = (3 + (rng.next_f32() * 5.0) as i32, 2 + (rng.next_f32() * 4.0) as i32);
        for dx in -lx..=lx { for dz in -lz..=lz {
            let d = (dx * dx) as f32 / (lx * lx) as f32 + (dz * dz) as f32 / (lz * lz) as f32;
            if d <= 1.05 {
                offsets.push([dx as f32 * 3.1, 0.0, dz as f32 * 3.1]);
                if d < 0.45 && rng.next_f32() < 0.75 { offsets.push([dx as f32 * 3.1, 1.9, dz as f32 * 3.1]); }
            }
        }}
        clouds.push(CloudAnim {
            x: (rng.next_f32() - 0.5) * 340.0, y: 82.0 + rng.next_f32() * 20.0, z: (rng.next_f32() - 0.5) * 300.0,
            speed: 1.5 + rng.next_f32() * 1.4, offsets,
        });
    }

    let petals = (0..4096).map(|i| PetalGpu {
        pos_spin: [(rng.next_f32() - 0.5) * 244.0, 6.0 + rng.next_f32() * 72.0, (rng.next_f32() - 0.5) * 244.0, rng.next_f32() * 6.28],
        vel_col: [1.4 + rng.next_f32() * 1.5, 0.8 + rng.next_f32() * 0.9, rng.next_f32() * 6.28, (i % 3) as f32],
    }).collect();

    let point_lights = [
        [0.0, 14.0, 92.0, 28.0], [0.0, 14.0, 54.0, 28.0], [0.0, 19.0, 28.0, 28.0],
        [0.0, 22.0, -10.0, 28.0], [30.0, 24.0, -23.0, 28.0], [-36.0, 27.0, -36.0, 28.0],
    ];

    WorldData { opaque, shadow_opaque_count, translucent_base, villagers, boats, clouds, petals, point_lights }
}

pub fn build_dynamic_instances(world: &mut WorldData, t: f32, dt: f32, dyn_opaque: &mut Vec<VoxelInstance>, dyn_trans: &mut Vec<VoxelInstance>) {
    dyn_opaque.clear();
    dyn_trans.clear();

    // Boats
    for b in &world.boats {
        let by = b.y + (t * 1.6 + b.phase).sin() * 0.06;
        let (c, s) = (b.rot.cos(), b.rot.sin());
        dyn_opaque.push(VoxelInstance::new(b.x, by, b.z, b.rot, 3.8, 0.45, 1.6, 0x9c5522, MAT_WOOD));
        dyn_opaque.push(VoxelInstance::new(b.x + 2.1 * c, by + 0.12, b.z - 2.1 * s, b.rot, 0.7, 0.55, 1.1, 0x5c3012, MAT_WOOD));
        dyn_opaque.push(VoxelInstance::new(b.x - 2.0 * c, by + 0.10, b.z + 2.0 * s, b.rot, 0.6, 0.52, 1.2, 0x5c3012, MAT_WOOD));
        dyn_opaque.push(VoxelInstance::new(b.x + 2.15 * c, by + 0.55, b.z - 2.15 * s, b.rot, 0.32, 0.32, 0.32, col::LAMP_GLOW, MAT_LAMP));
    }

    // 2-block-tall Kimono Villagers
    let k_cols = [0x223b63u32, 0xa62626, 0xeeeae1];
    let o_cols = [0xe6b800u32, 0x1e2024, 0xb81d13];
    for v in &world.villagers {
        let (mut px, mut pz, mut yaw) = (v.bx, v.bz, v.rot_y);
        let py = if v.range > 0.0 {
            let s = (t * v.speed + v.phase).sin();
            let ds = (t * v.speed + v.phase).cos();
            if v.axis_x { px += s * v.range; yaw = if ds >= 0.0 { 1.57 } else { -1.57 }; }
            else { pz += s * v.range; yaw = if ds >= 0.0 { 0.0 } else { 3.14 }; }
            v.by + (t * 6.0 + v.phase).sin().abs() * 0.06
        } else {
            v.by + (t * 2.0 + v.phase).sin() * 0.02
        };
        let kc = k_cols[v.color_idx % 3];
        let oc = o_cols[v.color_idx % 3];
        dyn_opaque.push(VoxelInstance::new(px, py + 0.09, pz, yaw, 0.68, 0.18, 0.45, 0x472d1c, MAT_WOOD));
        dyn_opaque.push(VoxelInstance::new(px, py + 0.52, pz, yaw, 0.74, 0.68, 0.48, kc, MAT_MATTE));
        dyn_opaque.push(VoxelInstance::new(px, py + 0.93, pz, yaw, 0.78, 0.16, 0.52, oc, MAT_MATTE));
        dyn_opaque.push(VoxelInstance::new(px, py + 1.22, pz, yaw, 0.86, 0.44, 0.46, kc, MAT_MATTE));
        dyn_opaque.push(VoxelInstance::new(px, py + 1.68, pz, yaw, 0.50, 0.48, 0.48, 0xf3cca4, MAT_MATTE));
        dyn_opaque.push(VoxelInstance::new(px, py + 1.88, pz, yaw, 0.54, 0.18, 0.52, 0x1b1817, MAT_MATTE));
    }

    // Drifting Volumetric Clouds
    for c in &mut world.clouds {
        c.x += c.speed * dt;
        if c.x > 195.0 { c.x = -195.0; }
        for &[ox, oy, oz] in &c.offsets {
            dyn_trans.push(VoxelInstance::new(c.x + ox, c.y + oy, c.z + oz, 0.0, 3.4, 2.2, 3.4, 0xf0f4f8, MAT_MATTE));
        }
    }
}
