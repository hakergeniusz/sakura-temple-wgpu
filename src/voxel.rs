use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::collections::HashMap;

#[allow(dead_code)]
pub const MAT_AIR: u8 = 0;
pub const MAT_MATTE: u8 = 1;
pub const MAT_WOOD: u8 = 2;
pub const MAT_METAL: u8 = 3;
pub const MAT_FOLIAGE: u8 = 4;
pub const MAT_WIN: u8 = 5;
pub const MAT_LAMP: u8 = 6;
pub const MAT_WATER: u8 = 7;
pub const MAT_CLOUD: u8 = 8;
pub const MAT_MIST: u8 = 9;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct VoxelVertex {
    pub pos: [f32; 3],
    pub norm: [f32; 3],
    pub color: [f32; 3],
    pub mat_id: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct ModelInstance {
    pub model: [[f32; 4]; 4],
}

impl ModelInstance {
    pub fn identity() -> Self {
        Self {
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn from_mat4(m: glam::Mat4) -> Self {
        Self {
            model: m.to_cols_array_2d(),
        }
    }

    pub fn from_trs(pos: Vec3, rot_y: f32, scale: Vec3) -> Self {
        let m = glam::Mat4::from_translation(pos)
            * glam::Mat4::from_rotation_y(rot_y)
            * glam::Mat4::from_scale(scale);
        Self::from_mat4(m)
    }
}

#[derive(Default, Clone)]
pub struct MeshData {
    pub vertices: Vec<VoxelVertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn tri_count(&self) -> usize {
        self.indices.len() / 3
    }
}

pub struct Rng(pub u32);

impl Rng {
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f32) / 4294967296.0
    }
}

#[inline]
pub fn hash2(x: f32, z: f32) -> f32 {
    let n = (x * 127.1 + z * 311.7).sin() * 43758.5453123;
    n - n.floor()
}

#[inline]
pub fn noise2(x: f32, z: f32) -> f32 {
    let ix = x.floor();
    let iz = z.floor();
    let fx = x - ix;
    let fz = z - iz;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uz = fz * fz * (3.0 - 2.0 * fz);
    let a = hash2(ix, iz);
    let b = hash2(ix + 1.0, iz);
    let c = hash2(ix, iz + 1.0);
    let d = hash2(ix + 1.0, iz + 1.0);
    a + (b - a) * ux + (c - a) * uz + (a - b - c + d) * ux * uz
}

pub fn fbm2(x: f32, z: f32, oct: i32) -> f32 {
    let mut v = 0.0;
    let mut a = 0.5;
    let mut f = 1.0;
    for _ in 0..oct {
        v += a * noise2(x * f, z * f);
        f *= 2.03;
        a *= 0.5;
    }
    v
}

pub fn ridged2(x: f32, z: f32, oct: i32) -> f32 {
    let mut v = 0.0;
    let mut a = 0.5;
    let mut f = 1.0;
    for _ in 0..oct {
        let n = 1.0 - (noise2(x * f, z * f) * 2.0 - 1.0).abs();
        v += a * n * n;
        f *= 2.07;
        a *= 0.5;
    }
    v
}

#[inline]
fn srgb_chan_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
pub fn hex_to_linear_rgb(hex: u32) -> [f32; 3] {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    [
        srgb_chan_to_linear(r),
        srgb_chan_to_linear(g),
        srgb_chan_to_linear(b),
    ]
}

pub struct ColorCache {
    map: HashMap<u32, [f32; 3]>,
}

impl ColorCache {
    pub fn new() -> Self {
        Self {
            map: HashMap::with_capacity(64),
        }
    }

    #[inline]
    pub fn get(&mut self, hex: u32) -> [f32; 3] {
        *self.map.entry(hex).or_insert_with(|| hex_to_linear_rgb(hex))
    }
}

pub mod pal {
    pub const GRASS_1: u32 = 0x578e34;
    pub const GRASS_2: u32 = 0x4b7d2c;
    pub const GRASS_3: u32 = 0x679e40;
    pub const GRASS_RICE: u32 = 0x72b444;
    pub const MOSS: u32 = 0x3f6b2c;
    pub const DIRT: u32 = 0x5d4a3a;
    pub const BEDROCK: u32 = 0x262a2f;
    pub const STONE_DARK: u32 = 0x34383e;
    pub const STONE_MID: u32 = 0x62676d;
    pub const STONE_LT: u32 = 0x878d93;
    pub const COURTYARD: u32 = 0x90969a;
    pub const COURTYARD_ALT: u32 = 0x80868b;
    pub const COURTYARD_LT: u32 = 0xa3a8ad;
    pub const ZEN_GRAVEL: u32 = 0xd4d6d2;
    pub const ZEN_GRAVEL_DK: u32 = 0xb7bab4;
    pub const MTN_BASE: u32 = 0x73777c;
    pub const MTN_MID: u32 = 0x8a8e94;
    pub const MTN_DARK: u32 = 0x5a5e64;
    pub const SNOW: u32 = 0xffffff;
    pub const SNOW_SHADE: u32 = 0xe6edf5;
    pub const SAND: u32 = 0xa49b87;
    pub const WATER: u32 = 0x2ba4a0;
    pub const FOAM: u32 = 0xdff7f9;
    pub const ROOF_DARK: u32 = 0x1d2025;
    pub const ROOF_TRIM: u32 = 0x2f343c;
    pub const ROOF_RIDGE: u32 = 0x3b404a;
    pub const CRIMSON: u32 = 0x9f2726;
    pub const VERMILION: u32 = 0xc21e14;
    pub const TORII_BASE: u32 = 0x181a1e;
    pub const PLASTER: u32 = 0xf4f0e7;
    pub const PLASTER_W: u32 = 0xe9e1d2;
    pub const YAKISUGI: u32 = 0x2e2521;
    pub const WOOD_WARM: u32 = 0xb56926;
    pub const WOOD_LT: u32 = 0xcf873f;
    pub const WOOD_DARK: u32 = 0x3d281a;
    pub const WOOD_BEAM: u32 = 0x6b3d1b;
    pub const GOLD: u32 = 0xe6b800;
    pub const BRASS: u32 = 0xd69f22;
    pub const IRON: u32 = 0x24262b;
    pub const WIN_GLOW: u32 = 0xffe89c;
    pub const LAMP_GLOW: u32 = 0xff841c;
    pub const LANTERN_RED: u32 = 0xff5a2c;
    pub const SAKURA_1: u32 = 0xffb7c5;
    pub const SAKURA_2: u32 = 0xff9bb3;
    pub const SAKURA_3: u32 = 0xffd2de;
    pub const SAKURA_DEEP: u32 = 0xe97b97;
    pub const PINE: u32 = 0x28641c;
    pub const PINE_DARK: u32 = 0x1e4d14;
    pub const BAMBOO: u32 = 0x529b32;
    pub const BAMBOO_NODE: u32 = 0x336e22;
    pub const BAMBOO_LEAF: u32 = 0x3d8428;
    pub const NOREN_BLUE: u32 = 0x223b63;
    pub const NOREN_RED: u32 = 0x9a2626;
    pub const NOREN_CREAM: u32 = 0xeee5d2;
}

pub struct CatmullRomCurve3 {
    points: Vec<Vec3>,
    closed: bool,
    tension: f32,
    arc_lengths: Vec<f32>,
}

impl CatmullRomCurve3 {
    pub fn new(points: Vec<Vec3>, closed: bool, tension: f32) -> Self {
        let mut c = Self {
            points,
            closed,
            tension,
            arc_lengths: Vec::new(),
        };
        c.compute_arc_lengths(200);
        c
    }

    pub fn get_point(&self, t: f32) -> Vec3 {
        let l = self.points.len() as i32;
        let segs = if self.closed { l } else { l - 1 };
        let p = segs as f32 * t;
        let mut int_pt = p.floor() as i32;
        let mut w = p - int_pt as f32;

        if self.closed {
            int_pt = int_pt.rem_euclid(l);
        } else if w == 0.0 && int_pt >= l - 1 {
            int_pt = l - 2;
            w = 1.0;
        } else if int_pt < 0 {
            int_pt = 0;
            w = 0.0;
        } else if int_pt >= l - 1 {
            int_pt = l - 2;
            w = 1.0;
        }

        let idx = |i: i32| -> usize { i.rem_euclid(l) as usize };

        let p0 = if self.closed || int_pt > 0 {
            self.points[idx(int_pt - 1)]
        } else {
            2.0 * self.points[0] - self.points[1]
        };
        let p1 = self.points[idx(int_pt)];
        let p2 = self.points[idx(int_pt + 1)];
        let p3 = if self.closed || int_pt + 2 < l {
            self.points[idx(int_pt + 2)]
        } else {
            2.0 * self.points[(l - 1) as usize] - self.points[(l - 2) as usize]
        };

        let v0 = (p2 - p0) * self.tension;
        let v1 = (p3 - p1) * self.tension;
        let c0 = p1;
        let c1 = v0;
        let c2 = -3.0 * p1 + 3.0 * p2 - 2.0 * v0 - v1;
        let c3 = 2.0 * p1 - 2.0 * p2 + v0 + v1;
        ((c3 * w + c2) * w + c1) * w + c0
    }

    fn compute_arc_lengths(&mut self, divisions: usize) {
        self.arc_lengths.clear();
        self.arc_lengths.reserve(divisions + 1);
        self.arc_lengths.push(0.0);
        let mut last = self.get_point(0.0);
        let mut sum = 0.0;
        for p in 1..=divisions {
            let cur = self.get_point(p as f32 / divisions as f32);
            sum += cur.distance(last);
            self.arc_lengths.push(sum);
            last = cur;
        }
    }

    pub fn u_to_t(&self, u: f32) -> f32 {
        let u = u.clamp(0.0, 1.0);
        let total = *self.arc_lengths.last().unwrap_or(&1.0);
        let target = u * total;
        let il = self.arc_lengths.len() - 1;
        let mut low = 0usize;
        let mut high = il;
        while low <= high {
            let mid = (low + high) / 2;
            let diff = self.arc_lengths[mid] - target;
            if diff < 0.0 {
                low = mid + 1;
            } else if diff > 0.0 {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            } else {
                high = mid;
                break;
            }
        }
        let i = high.min(il - 1);
        let len_before = self.arc_lengths[i];
        let len_after = self.arc_lengths[i + 1];
        let seg_len = (len_after - len_before).max(1e-6);
        let frac = (target - len_before) / seg_len;
        (i as f32 + frac) / il as f32
    }

    pub fn get_point_at(&self, u: f32) -> Vec3 {
        self.get_point(self.u_to_t(u))
    }

    pub fn get_tangent_at(&self, u: f32) -> Vec3 {
        let t = self.u_to_t(u);
        let delta = 0.0001;
        let t1 = (t - delta).max(0.0);
        let t2 = (t + delta).min(1.0);
        (self.get_point(t2) - self.get_point(t1)).normalize_or_zero()
    }
}

struct FaceDef {
    axis: usize,
    sign: i32,
    u_axis: usize,
    v_axis: usize,
    shade: f32,
}

const FACES: [FaceDef; 6] = [
    FaceDef { axis: 0, sign:  1, u_axis: 1, v_axis: 2, shade: 0.94 }, // +X
    FaceDef { axis: 0, sign: -1, u_axis: 2, v_axis: 1, shade: 0.91 }, // -X
    FaceDef { axis: 1, sign:  1, u_axis: 2, v_axis: 0, shade: 1.00 }, // +Y
    FaceDef { axis: 1, sign: -1, u_axis: 0, v_axis: 2, shade: 0.78 }, // -Y
    FaceDef { axis: 2, sign:  1, u_axis: 0, v_axis: 1, shade: 0.96 }, // +Z
    FaceDef { axis: 2, sign: -1, u_axis: 1, v_axis: 0, shade: 0.89 }, // -Z
];

const AO_FACTORS: [f32; 4] = [0.56, 0.72, 0.86, 1.00];

pub struct MicroVoxelVolume {
    pub nx: i32,
    pub ny: i32,
    pub nz: i32,
    pub scale: f32,
    pub ox: i32,
    pub oy: i32,
    pub oz: i32,
    mat: Vec<u8>,
    col: Vec<u32>,
    pub placed_count: u64,
}

impl MicroVoxelVolume {
    pub fn new(nx: i32, ny: i32, nz: i32, scale: f32, ox: i32, oy: i32, oz: i32) -> Self {
        let n = (nx * ny * nz) as usize;
        Self {
            nx,
            ny,
            nz,
            scale,
            ox,
            oy,
            oz,
            mat: vec![0; n],
            col: vec![0; n],
            placed_count: 0,
        }
    }

    #[inline]
    pub fn set(&mut self, wx: i32, wy: i32, wz: i32, mat: u8, col: u32) {
        let ix = wx - self.ox;
        let iy = wy - self.oy;
        let iz = wz - self.oz;
        if ix < 0 || ix >= self.nx || iy < 0 || iy >= self.ny || iz < 0 || iz >= self.nz {
            return;
        }
        let i = ((iy * self.nz + iz) * self.nx + ix) as usize;
        if self.mat[i] == 0 && mat != 0 {
            self.placed_count += 1;
        }
        self.mat[i] = mat;
        self.col[i] = col;
    }

    #[inline]
    fn get_mat_raw(&self, ix: i32, iy: i32, iz: i32) -> u8 {
        if ix < 0 || ix >= self.nx || iy < 0 || iy >= self.ny || iz < 0 || iz >= self.nz {
            return 0;
        }
        self.mat[((iy * self.nz + iz) * self.nx + ix) as usize]
    }

    pub fn box_fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, mat: u8, col: u32) {
        let (ax, bx) = (x0.min(x1), x0.max(x1));
        let (ay, by) = (y0.min(y1), y0.max(y1));
        let (az, bz) = (z0.min(z1), z0.max(z1));
        for y in ay..=by {
            for z in az..=bz {
                for x in ax..=bx {
                    self.set(x, y, z, mat, col);
                }
            }
        }
    }

    pub fn box_fn<F>(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, mat: u8, mut col_fn: F)
    where
        F: FnMut(i32, i32, i32) -> u32,
    {
        let (ax, bx) = (x0.min(x1), x0.max(x1));
        let (ay, by) = (y0.min(y1), y0.max(y1));
        let (az, bz) = (z0.min(z1), z0.max(z1));
        for y in ay..=by {
            for z in az..=bz {
                for x in ax..=bx {
                    self.set(x, y, z, mat, col_fn(x, y, z));
                }
            }
        }
    }

    pub fn hollow_box(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, mat: u8, col: u32) {
        let (ax, bx) = (x0.min(x1), x0.max(x1));
        let (ay, by) = (y0.min(y1), y0.max(y1));
        let (az, bz) = (z0.min(z1), z0.max(z1));
        for y in ay..=by {
            for z in az..=bz {
                for x in ax..=bx {
                    if x == ax || x == bx || y == ay || y == by || z == az || z == bz {
                        self.set(x, y, z, mat, col);
                    }
                }
            }
        }
    }

    pub fn hollow_box_fn<F>(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, mat: u8, mut col_fn: F)
    where
        F: FnMut(i32, i32, i32) -> u32,
    {
        let (ax, bx) = (x0.min(x1), x0.max(x1));
        let (ay, by) = (y0.min(y1), y0.max(y1));
        let (az, bz) = (z0.min(z1), z0.max(z1));
        for y in ay..=by {
            for z in az..=bz {
                for x in ax..=bx {
                    if x == ax || x == bx || y == ay || y == by || z == az || z == bz {
                        self.set(x, y, z, mat, col_fn(x, y, z));
                    }
                }
            }
        }
    }

    pub fn ellipsoid_fn<F>(
        &mut self,
        cx: f32,
        cy: f32,
        cz: f32,
        rx: f32,
        ry: f32,
        rz: f32,
        mat: u8,
        mut col_fn: F,
        jitter: f32,
        rng: &mut Rng,
    ) where
        F: FnMut(i32, i32, i32) -> u32,
    {
        let (ix0, ix1) = ((cx - rx).floor() as i32, (cx + rx).ceil() as i32);
        let (iy0, iy1) = ((cy - ry).floor() as i32, (cy + ry).ceil() as i32);
        let (iz0, iz1) = ((cz - rz).floor() as i32, (cz + rz).ceil() as i32);
        for y in iy0..=iy1 {
            for z in iz0..=iz1 {
                for x in ix0..=ix1 {
                    let dx = (x as f32 - cx) / (rx + 0.01);
                    let dy = (y as f32 - cy) / (ry + 0.01);
                    let dz = (z as f32 - cz) / (rz + 0.01);
                    let d = dx * dx + dy * dy + dz * dz;
                    if d <= 1.0 && !(jitter > 0.0 && d > 1.0 - jitter && rng.next_f32() < 0.4) {
                        self.set(x, y, z, mat, col_fn(x, y, z));
                    }
                }
            }
        }
    }

    pub fn ellipsoid(&mut self, cx: f32, cy: f32, cz: f32, rx: f32, ry: f32, rz: f32, mat: u8, col: u32, jitter: f32, rng: &mut Rng) {
        self.ellipsoid_fn(cx, cy, cz, rx, ry, rz, mat, |_, _, _| col, jitter, rng);
    }

    pub fn cylinder(&mut self, cx: f32, y0: i32, cz: f32, r: f32, h: i32, mat: u8, col: u32) {
        let rr = r * r + 0.15;
        let (iz0, iz1) = ((cz - r).floor() as i32, (cz + r).ceil() as i32);
        let (ix0, ix1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
        for y in y0..=(y0 + h) {
            for z in iz0..=iz1 {
                for x in ix0..=ix1 {
                    let dx = x as f32 - cx;
                    let dz = z as f32 - cz;
                    if dx * dx + dz * dz <= rr {
                        self.set(x, y, z, mat, col);
                    }
                }
            }
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32, radius: f32, mat: u8, col: u32) {
        let dist = ((x1 - x0).powi(2) + (y1 - y0).powi(2) + (z1 - z0).powi(2)).sqrt();
        let steps = (dist * 1.5).ceil().max(1.0) as i32;
        let ir = radius.ceil() as i32;
        let rr = radius * radius + 0.15;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let cx = x0 + (x1 - x0) * t;
            let cy = y0 + (y1 - y0) * t;
            let cz = z0 + (z1 - z0) * t;
            if radius <= 0.55 {
                self.set(cx.round() as i32, cy.round() as i32, cz.round() as i32, mat, col);
            } else {
                for dy in -ir..=ir {
                    for dz in -ir..=ir {
                        for dx in -ir..=ir {
                            if (dx * dx + dy * dy + dz * dz) as f32 <= rr {
                                self.set(
                                    (cx + dx as f32).round() as i32,
                                    (cy + dy as f32).round() as i32,
                                    (cz + dz as f32).round() as i32,
                                    mat,
                                    col,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn compile(&self, colors: &mut ColorCache) -> (MeshData, u64) {
        let dims = [self.nx, self.ny, self.nz];
        let offs = [self.ox, self.oy, self.oz];
        let s = self.scale;
        let max_slice = (self.nx * self.ny).max(self.ny * self.nz).max(self.nx * self.nz) as usize;
        let mut mask_mat = vec![0u8; max_slice];
        let mut mask_col = vec![0u32; max_slice];
        let mut mask_ao = vec![0u8; max_slice];
        let mut mesh = MeshData::default();

        for f in &FACES {
            let axis = f.axis;
            let sign = f.sign;
            let u_axis = f.u_axis;
            let v_axis = f.v_axis;
            let shade = f.shade;

            let n_slices = dims[axis];
            let n_u = dims[u_axis];
            let n_v = dims[v_axis];
            let mut n_vec = [0.0f32; 3];
            n_vec[axis] = sign as f32;

            let stride = |ax: usize| -> i32 {
                match ax {
                    0 => 1,
                    1 => self.nx * self.nz,
                    _ => self.nx,
                }
            };
            let stride_a = stride(axis);
            let stride_u = stride(u_axis);
            let stride_v = stride(v_axis);

            for slice in 0..n_slices {
                let adj_slice = slice + sign;
                let out_of_slice = adj_slice < 0 || adj_slice >= n_slices;
                let mut any_visible = false;

                for v in 0..n_v {
                    for u in 0..n_u {
                        let k = (v * n_u + u) as usize;
                        let v_idx = (slice * stride_a + v * stride_v + u * stride_u) as usize;
                        let m = self.mat[v_idx];
                        if m == 0 {
                            mask_mat[k] = 0;
                            continue;
                        }
                        let nm = if out_of_slice {
                            0
                        } else {
                            self.mat[(adj_slice * stride_a + v * stride_v + u * stride_u) as usize]
                        };
                        if nm != 0 {
                            mask_mat[k] = 0;
                            continue;
                        }

                        any_visible = true;
                        mask_mat[k] = m;
                        mask_col[k] = self.col[v_idx];

                        if out_of_slice || m == MAT_WIN || m == MAT_LAMP {
                            mask_ao[k] = 255;
                        } else {
                            let mut coord = [0i32; 3];
                            coord[axis] = adj_slice;
                            coord[u_axis] = u;
                            coord[v_axis] = v;
                            let g = |du: i32, dv: i32| -> u8 {
                                let mut nc = coord;
                                nc[u_axis] += du;
                                nc[v_axis] += dv;
                                if self.get_mat_raw(nc[0], nc[1], nc[2]) != 0 { 1 } else { 0 }
                            };
                            let ao_val = |s1: u8, s2: u8, cn: u8| -> u8 {
                                if s1 != 0 && s2 != 0 { 0 } else { 3 - (s1 + s2 + cn) }
                            };
                            let um = g(-1, 0);
                            let up = g(1, 0);
                            let vm = g(0, -1);
                            let vp = g(0, 1);
                            let a00 = ao_val(um, vm, g(-1, -1));
                            let a10 = ao_val(up, vm, g(1, -1));
                            let a11 = ao_val(up, vp, g(1, 1));
                            let a01 = ao_val(um, vp, g(-1, 1));
                            mask_ao[k] = a00 | (a10 << 2) | (a11 << 4) | (a01 << 6);
                        }
                    }
                }
                if !any_visible {
                    continue;
                }

                let face_plane = (slice + if sign > 0 { 1 } else { 0 } + offs[axis]) as f32 * s;
                for v in 0..n_v {
                    let mut u = 0;
                    while u < n_u {
                        let k = (v * n_u + u) as usize;
                        let m = mask_mat[k];
                        if m == 0 {
                            u += 1;
                            continue;
                        }
                        let c = mask_col[k];
                        let ao = mask_ao[k];
                        let mut w = 1;
                        while u + w < n_u {
                            let kw = k + w as usize;
                            if mask_mat[kw] == m && mask_col[kw] == c && mask_ao[kw] == ao {
                                w += 1;
                            } else {
                                break;
                            }
                        }
                        let mut h = 1;
                        'outer: while v + h < n_v {
                            let row_base = ((v + h) * n_u + u) as usize;
                            for du in 0..w as usize {
                                let kr = row_base + du;
                                if mask_mat[kr] != m || mask_col[kr] != c || mask_ao[kr] != ao {
                                    break 'outer;
                                }
                            }
                            h += 1;
                        }
                        for dv in 0..h {
                            let row_base = ((v + dv) * n_u + u) as usize;
                            for du in 0..w as usize {
                                mask_mat[row_base + du] = 0;
                            }
                        }

                        let base_v = mesh.vertices.len() as u32;
                        let u0 = (u + offs[u_axis]) as f32 * s;
                        let u1 = (u + w + offs[u_axis]) as f32 * s;
                        let v0 = (v + offs[v_axis]) as f32 * s;
                        let v1 = (v + h + offs[v_axis]) as f32 * s;
                        let rgb = colors.get(c);

                        let mut push_pt = |pu: f32, pv: f32, ao_bits: u8| {
                            let mut p = [0.0f32; 3];
                            p[axis] = face_plane;
                            p[u_axis] = pu;
                            p[v_axis] = pv;
                            let mult = shade * AO_FACTORS[(ao_bits & 3) as usize];
                            mesh.vertices.push(VoxelVertex {
                                pos: p,
                                norm: n_vec,
                                color: [rgb[0] * mult, rgb[1] * mult, rgb[2] * mult],
                                mat_id: m as u32,
                            });
                        };

                        let a00 = ao & 3;
                        let a10 = (ao >> 2) & 3;
                        let a11 = (ao >> 4) & 3;
                        let a01 = (ao >> 6) & 3;
                        push_pt(u0, v0, a00);
                        push_pt(u1, v0, a10);
                        push_pt(u1, v1, a11);
                        push_pt(u0, v1, a01);

                        if a00 + a11 > a10 + a01 {
                            mesh.indices.extend_from_slice(&[
                                base_v, base_v + 1, base_v + 2,
                                base_v, base_v + 2, base_v + 3,
                            ]);
                        } else {
                            mesh.indices.extend_from_slice(&[
                                base_v + 1, base_v + 2, base_v + 3,
                                base_v + 1, base_v + 3, base_v,
                            ]);
                        }
                        u += w;
                    }
                }
            }
        }

        (mesh, self.placed_count)
    }
}

pub fn add_detailed_entrance(
    vol: &mut MicroVoxelVolume,
    cx: i32,
    y0: i32,
    front_z: i32,
    door_half_w: i32,
    door_h: i32,
    has_noren: bool,
    noren_col: u32,
    has_lantern: bool,
) {
    vol.box_fill(cx - door_half_w - 2, y0 - 1, front_z, cx + door_half_w + 2, y0, front_z + 2, MAT_MATTE, pal::STONE_LT);
    vol.box_fill(cx - door_half_w - 1, y0 + 1, front_z, cx + door_half_w + 1, y0 + 1, front_z, MAT_WOOD, pal::WOOD_DARK);
    vol.box_fill(cx - door_half_w - 1, y0 + 1, front_z, cx - door_half_w - 1, y0 + door_h + 1, front_z + 1, MAT_WOOD, pal::WOOD_BEAM);
    vol.box_fill(cx + door_half_w + 1, y0 + 1, front_z, cx + door_half_w + 1, y0 + door_h + 1, front_z + 1, MAT_WOOD, pal::WOOD_BEAM);
    vol.box_fill(cx - door_half_w - 2, y0 + door_h + 1, front_z, cx + door_half_w + 2, y0 + door_h + 2, front_z + 1, MAT_WOOD, pal::WOOD_DARK);

    let door_z = front_z - 1;
    for dy in 2..=door_h {
        for dx in -door_half_w..=door_half_w {
            let is_center_stile = dx == 0 || (dx == -1 && door_half_w > 3);
            let is_bottom_kick = dy <= 4;
            let is_grid = dy % 3 == 0 || dx.abs() % 2 == 0;
            if is_center_stile || is_bottom_kick || is_grid {
                vol.set(cx + dx, y0 + dy, door_z, MAT_WOOD, if is_center_stile { pal::WOOD_DARK } else { pal::WOOD_WARM });
            } else {
                vol.set(cx + dx, y0 + dy, door_z, MAT_WIN, pal::WIN_GLOW);
            }
        }
    }

    let h_y = y0 + (door_h as f32 * 0.48).round() as i32;
    let handle_x = cx + 1;
    vol.box_fill(handle_x, h_y - 1, door_z, handle_x, h_y + 1, door_z, MAT_METAL, pal::IRON);
    vol.box_fill(handle_x, h_y, door_z + 1, handle_x, h_y + 1, door_z + 1, MAT_METAL, pal::BRASS);
    vol.set(cx - 1, h_y, door_z, MAT_METAL, pal::IRON);
    vol.set(cx - 1, h_y, door_z + 1, MAT_METAL, pal::BRASS);

    if has_noren {
        let rod_y = y0 + door_h + 3;
        vol.box_fill(cx - door_half_w - 2, rod_y, front_z + 2, cx + door_half_w + 2, rod_y, front_z + 2, MAT_WOOD, pal::WOOD_LT);
        let panels = 3;
        let span = door_half_w * 2 + 1;
        let slit = (span / panels).max(2);
        for dx in -door_half_w..=door_half_w {
            let rel = dx + door_half_w;
            if rel > 0 && rel % slit == 0 {
                continue;
            }
            vol.box_fill(cx + dx, rod_y - 4, front_z + 2, cx + dx, rod_y - 1, front_z + 2, MAT_MATTE, noren_col);
            if dx == 0 && rod_y - 2 >= 0 {
                vol.set(cx + dx, rod_y - 2, front_z + 2, MAT_MATTE, pal::PLASTER);
            }
        }
    }

    if has_lantern {
        let lx = cx - door_half_w - 3;
        let ly = y0 + door_h;
        let lz = front_z + 2;
        vol.box_fill(lx, ly + 1, front_z + 1, lx, ly + 1, lz, MAT_METAL, pal::IRON);
        vol.set(lx, ly, lz, MAT_WOOD, pal::WOOD_DARK);
        vol.box_fill(lx, ly - 3, lz, lx, ly - 1, lz, MAT_LAMP, pal::LANTERN_RED);
        vol.set(lx, ly - 4, lz, MAT_WOOD, pal::WOOD_DARK);
    }
}

pub fn add_tiled_roof(
    vol: &mut MicroVoxelVolume,
    cx: i32,
    base_y: i32,
    cz: i32,
    hw: i32,
    hd: i32,
    ridge_height: i32,
    eave_flare: i32,
    axis: char,
    gable_wall_col: u32,
) {
    for layer in 0..=ridge_height {
        let t = layer as f32 / ridge_height.max(1) as f32;
        let flare = (eave_flare as f32 * (1.0 - t) * (1.0 - t)).round() as i32;
        let rx = if axis == 'X' {
            hw + flare - (layer as f32 * 0.35).floor() as i32
        } else {
            hw + flare - layer
        };
        let rz = if axis == 'Z' {
            hd + flare - (layer as f32 * 0.35).floor() as i32
        } else {
            hd + flare - layer
        };
        if rx < 1 || rz < 1 {
            break;
        }
        for dz in -rz..=rz {
            for dx in -rx..=rx {
                let on_edge = dx.abs() == rx || dz.abs() == rz;
                let is_corner = dx.abs() >= rx - 1 && dz.abs() >= rz - 1;
                let corner_lift = if layer <= 1 && is_corner { 1 } else { 0 };
                if axis == 'X' && layer >= 2 && dx.abs() == rx && dz.abs() < rz - 1 {
                    vol.set(cx + dx, base_y + layer, cz + dz, MAT_MATTE, gable_wall_col);
                } else if axis == 'Z' && layer >= 2 && dz.abs() == rz && dx.abs() < rx - 1 {
                    vol.set(cx + dx, base_y + layer, cz + dz, MAT_MATTE, gable_wall_col);
                } else if on_edge || layer == ridge_height {
                    let is_rib = (if axis == 'X' { dx } else { dz }).rem_euclid(2) == 0;
                    let col = if is_rib { pal::ROOF_TRIM } else { pal::ROOF_DARK };
                    vol.set(cx + dx, base_y + layer + corner_lift, cz + dz, MAT_WOOD, col);
                    if layer == 0 && is_rib && !is_corner {
                        vol.set(cx + dx, base_y - 1, cz + dz, MAT_WOOD, pal::WOOD_BEAM);
                    }
                }
            }
        }
    }
    let top_y = base_y + ridge_height + 1;
    let r_span = if axis == 'X' {
        (hw - (ridge_height as f32 * 0.35).floor() as i32).max(2)
    } else {
        (hd - (ridge_height as f32 * 0.35).floor() as i32).max(2)
    };
    for i in -r_span..=r_span {
        let wx = if axis == 'X' { cx + i } else { cx };
        let wz = if axis == 'Z' { cz + i } else { cz };
        let is_end = i.abs() >= r_span - 1;
        vol.set(
            wx,
            top_y,
            wz,
            if is_end { MAT_METAL } else { MAT_WOOD },
            if is_end { pal::GOLD } else { pal::ROOF_RIDGE },
        );
        if i.abs() == r_span {
            vol.set(wx, top_y + 1, wz, MAT_METAL, pal::GOLD);
        }
    }
}
