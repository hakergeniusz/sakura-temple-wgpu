use crate::voxel::{
    add_detailed_entrance, add_tiled_roof, hash2, pal, ColorCache, MeshData, MicroVoxelVolume, Rng,
    VoxelVertex, MAT_LAMP, MAT_MATTE, MAT_METAL, MAT_FOLIAGE, MAT_WIN, MAT_WOOD,
};

pub fn build_unit_box_mesh(
    w: f32,
    h: f32,
    d: f32,
    mat_id: u32,
    hex_col: u32,
    colors: &mut ColorCache,
) -> MeshData {
    let hw = w * 0.5;
    let hh = h * 0.5;
    let hd = d * 0.5;
    let rgb = colors.get(hex_col);
    let faces: [([f32; 3], [[f32; 3]; 4], f32); 6] = [
        ([0.0, 0.0, 1.0], [[-hw, -hh, hd], [hw, -hh, hd], [hw, hh, hd], [-hw, hh, hd]], 0.96),
        ([0.0, 0.0, -1.0], [[hw, -hh, -hd], [-hw, -hh, -hd], [-hw, hh, -hd], [hw, hh, -hd]], 0.89),
        ([1.0, 0.0, 0.0], [[hw, -hh, hd], [hw, -hh, -hd], [hw, hh, -hd], [hw, hh, hd]], 0.94),
        ([-1.0, 0.0, 0.0], [[-hw, -hh, -hd], [-hw, -hh, hd], [-hw, hh, hd], [-hw, hh, -hd]], 0.91),
        ([0.0, 1.0, 0.0], [[-hw, hh, hd], [hw, hh, hd], [hw, hh, -hd], [-hw, hh, -hd]], 1.00),
        ([0.0, -1.0, 0.0], [[-hw, -hh, -hd], [hw, -hh, -hd], [hw, -hh, hd], [-hw, -hh, hd]], 0.78),
    ];
    let mut mesh = MeshData::default();
    for (norm, pts, shade) in faces {
        let bv = mesh.vertices.len() as u32;
        let col = if mat_id == MAT_LAMP as u32 {
            rgb
        } else {
            [rgb[0] * shade, rgb[1] * shade, rgb[2] * shade]
        };
        for pos in pts {
            mesh.vertices.push(VoxelVertex {
                pos,
                norm,
                color: col,
                mat_id,
            });
        }
        mesh.indices.extend_from_slice(&[bv, bv + 1, bv + 2, bv, bv + 2, bv + 3]);
    }
    mesh
}

pub fn build_seven_tier_pagoda(colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.25;
    let mut vol = MicroVoxelVolume::new(96, 224, 96, s, -48, 0, -48);
    vol.box_fn(-24, 0, -24, 24, 3, 24, MAT_MATTE, |x, y, z| {
        if (x + y + z).rem_euclid(3) == 0 { pal::STONE_MID } else { pal::STONE_LT }
    });
    vol.box_fill(-22, 4, -22, 22, 5, 22, MAT_MATTE, pal::COURTYARD);
    for step in 0..5 {
        vol.box_fill(-6, step, 24 + 5 - step, 6, step, 24 + 5 - step, MAT_MATTE, pal::STONE_LT);
        vol.box_fill(-6, step, -24 - 5 + step, 6, step, -24 - 5 + step, MAT_MATTE, pal::STONE_LT);
    }

    let tiers = [
        (14i32, 26i32, 15i32),
        (13, 24, 13),
        (12, 22, 12),
        (11, 20, 11),
        (10, 18, 10),
        (9, 16, 9),
        (8, 14, 8),
    ];
    let mut cur_y = 6;
    for (t, &(core, eave, h)) in tiers.iter().enumerate() {
        if t > 0 {
            let bal = core + 3;
            vol.box_fill(-bal, cur_y, -bal, bal, cur_y, bal, MAT_WOOD, pal::WOOD_BEAM);
            for i in -bal..=bal {
                for (bx, bz) in [(i, bal), (i, -bal), (bal, i), (-bal, i)] {
                    vol.set(bx, cur_y + 1, bz, MAT_WOOD, pal::CRIMSON);
                    vol.set(bx, cur_y + 2, bz, MAT_WOOD, pal::VERMILION);
                    if i.abs() == bal || i.rem_euclid(3) == 0 {
                        vol.set(bx, cur_y + 1, bz, MAT_WOOD, pal::CRIMSON);
                        if i.abs() == bal {
                            vol.set(bx, cur_y + 3, bz, MAT_METAL, pal::GOLD);
                        }
                    }
                }
            }
        }

        for dy in 0..h {
            for dz in -core..=core {
                for dx in -core..=core {
                    let ex = dx.abs() == core;
                    let ez = dz.abs() == core;
                    if !ex && !ez {
                        continue;
                    }
                    let is_corner = ex && ez;
                    let is_col = (ex && dz.rem_euclid(4) == 0) || (ez && dx.rem_euclid(4) == 0);
                    let mut mat = MAT_MATTE;
                    let mut col = pal::PLASTER;
                    if is_corner || is_col || dy == 0 || dy >= h - 2 {
                        mat = MAT_WOOD;
                        col = if is_corner { pal::WOOD_DARK } else { pal::CRIMSON };
                    } else if dx.abs() <= 2 || dz.abs() <= 2 {
                        if t == 0 && dy <= 9 {
                            mat = if dy % 2 == 0 || dx == 0 || dz == 0 { MAT_WOOD } else { MAT_WIN };
                            col = if mat == MAT_WOOD { pal::WOOD_DARK } else { pal::WIN_GLOW };
                        } else if dy >= 3 && dy <= h - 4 {
                            mat = MAT_WIN;
                            col = pal::WIN_GLOW;
                        }
                    }
                    vol.set(dx, cur_y + dy, dz, mat, col);
                }
            }
        }

        if t == 0 {
            for (hx, hz) in [
                (1, core + 1), (-1, core + 1),
                (1, -core - 1), (-1, -core - 1),
                (core + 1, 1), (-core - 1, 1),
            ] {
                vol.set(hx, cur_y + 4, hz, MAT_METAL, pal::GOLD);
                vol.set(hx, cur_y + 5, hz, MAT_METAL, pal::IRON);
            }
        }

        let br_y = cur_y + h - 1;
        let mut i = -core;
        while i <= core {
            for side in [-1, 1] {
                vol.box_fill(i - 1, br_y, side * (core + 1), i + 1, br_y, side * (core + 2), MAT_WOOD, pal::WOOD_WARM);
                vol.box_fill(i - 1, br_y + 1, side * (core + 2), i + 1, br_y + 1, side * (core + 3), MAT_WOOD, pal::CRIMSON);
                vol.box_fill(side * (core + 1), br_y, i - 1, side * (core + 2), br_y, i + 1, MAT_WOOD, pal::WOOD_WARM);
                vol.box_fill(side * (core + 2), br_y + 1, i - 1, side * (core + 3), br_y + 1, i + 1, MAT_WOOD, pal::CRIMSON);
            }
            i += 4;
        }

        let r_y = cur_y + h + 1;
        let slope_layers = 5;
        for layer in 0..slope_layers {
            let r = eave - ((layer as f32 / (slope_layers - 1) as f32) * (eave - core + 2) as f32).round() as i32;
            for dz in -r..=r {
                for dx in -r..=r {
                    let ax = dx.abs();
                    let az = dz.abs();
                    if ax < r - 3 && az < r - 3 && layer < slope_layers - 1 {
                        continue;
                    }
                    let is_corner = ax >= r - 2 && az >= r - 2;
                    let lift = if layer == 0 && is_corner { 2 } else if layer == 1 && is_corner { 1 } else { 0 };
                    let rib = (ax == r || az == r) || ((dx + dz).rem_euclid(2) == 0);
                    vol.set(dx, r_y + layer + lift, dz, MAT_WOOD, if rib { pal::ROOF_TRIM } else { pal::ROOF_DARK });
                    if layer == 0 && (dx.rem_euclid(2) == 0 || dz.rem_euclid(2) == 0) {
                        vol.set(dx, r_y - 1, dz, MAT_WOOD, pal::WOOD_BEAM);
                    }
                    if layer == 0 && ax == r && az == r {
                        vol.set(dx, r_y, dz, MAT_METAL, pal::GOLD);
                        vol.set(dx, r_y - 1, dz, MAT_METAL, pal::BRASS);
                    }
                }
            }
        }
        cur_y = r_y + slope_layers;
    }

    vol.box_fill(-4, cur_y, -4, 4, cur_y + 2, 4, MAT_METAL, pal::IRON);
    vol.box_fill(-3, cur_y + 3, -3, 3, cur_y + 5, 3, MAT_METAL, pal::GOLD);
    vol.cylinder(0.0, cur_y + 6, 0.0, 1.1, 36, MAT_METAL, pal::GOLD);
    for ring in 0..9 {
        let ry = cur_y + 10 + ring * 3;
        vol.box_fill(-2, ry, -2, 2, ry, 2, MAT_METAL, pal::GOLD);
    }
    vol.box_fill(-1, cur_y + 40, -1, 1, cur_y + 42, 1, MAT_METAL, pal::GOLD);
    vol.set(0, cur_y + 44, 0, MAT_LAMP, pal::WIN_GLOW);

    vol.compile(colors)
}

pub fn build_golden_hall(colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.25;
    let mut vol = MicroVoxelVolume::new(120, 96, 84, s, -60, 0, -42);
    vol.box_fn(-44, 0, -26, 44, 4, 26, MAT_MATTE, |x, _, z| {
        if (x + z).rem_euclid(4) == 0 { pal::STONE_MID } else { pal::STONE_LT }
    });
    vol.box_fill(-42, 5, -24, 42, 6, 24, MAT_WOOD, pal::WOOD_WARM);
    for st in 0..6 {
        vol.box_fill(-10, st, 25 + 5 - st, 10, st, 25 + 5 - st, MAT_MATTE, pal::COURTYARD);
    }
    for x in -42i32..=42 {
        if x.abs() > 10 {
            vol.set(x, 7, 24, MAT_WOOD, pal::CRIMSON);
            vol.set(x, 9, 24, MAT_WOOD, pal::VERMILION);
            if x.rem_euclid(6) == 0 {
                vol.box_fill(x, 7, 24, x, 10, 24, MAT_WOOD, pal::CRIMSON);
                vol.set(x, 11, 24, MAT_METAL, pal::GOLD);
            }
        }
        vol.set(x, 7, -24, MAT_WOOD, pal::CRIMSON);
        vol.set(x, 9, -24, MAT_WOOD, pal::VERMILION);
    }

    let (w_x, w_z, w_h) = (36i32, 18i32, 22i32);
    for dy in 0..w_h {
        for dz in -w_z..=w_z {
            for dx in -w_x..=w_x {
                let ex = dx.abs() == w_x;
                let ez = dz.abs() == w_z;
                if !ex && !ez {
                    continue;
                }
                let is_pillar = (ex && dz.rem_euclid(6) == 0) || (ez && dx.rem_euclid(6) == 0) || (ex && ez);
                let mut mat = MAT_MATTE;
                let mut col = pal::PLASTER;
                if is_pillar || dy <= 2 || dy >= w_h - 3 {
                    mat = MAT_WOOD;
                    col = pal::CRIMSON;
                } else if dz == w_z && dx.abs() <= 7 && dy <= 14 {
                    let is_stile = dx.abs() == 7 || dx.abs() <= 1 || dy == 14;
                    mat = MAT_WOOD;
                    col = if is_stile { pal::WOOD_DARK } else { pal::WOOD_BEAM };
                    if !is_stile && dy % 3 == 0 && dx.abs() % 2 == 1 {
                        mat = MAT_METAL;
                        col = pal::GOLD;
                    }
                } else if dy >= 5 && dy <= 14 && ((ez && dx.rem_euclid(6) != 0) || (ex && dz.abs() <= 8)) {
                    let grid = dy % 3 == 0 || dx.rem_euclid(2) == 0 || dz.rem_euclid(2) == 0;
                    mat = if grid { MAT_WOOD } else { MAT_WIN };
                    col = if grid { pal::WOOD_DARK } else { pal::WIN_GLOW };
                }
                vol.set(dx, 7 + dy, dz, mat, col);
            }
        }
    }
    vol.box_fill(-2, 13, w_z + 1, -2, 15, w_z + 1, MAT_METAL, pal::GOLD);
    vol.box_fill(2, 13, w_z + 1, 2, 15, w_z + 1, MAT_METAL, pal::GOLD);
    for lx in [-14, 14] {
        vol.cylinder(lx as f32, 26, (w_z + 3) as f32, 1.0, 3, MAT_METAL, pal::IRON);
        vol.box_fill(lx - 1, 22, w_z + 2, lx + 1, 25, w_z + 4, MAT_LAMP, pal::LAMP_GLOW);
        vol.box_fill(lx - 1, 21, w_z + 2, lx + 1, 21, w_z + 4, MAT_METAL, pal::IRON);
    }

    add_tiled_roof(&mut vol, 0, 29, 0, 40, 22, 6, 5, 'X', pal::PLASTER);
    vol.hollow_box_fn(-30, 35, -13, 30, 41, 13, MAT_WOOD, |_, y, _| {
        if y % 2 == 0 { pal::CRIMSON } else { pal::WOOD_DARK }
    });
    add_tiled_roof(&mut vol, 0, 42, 0, 36, 17, 14, 5, 'X', pal::PLASTER);

    for sx in [-24, 24] {
        vol.box_fill(sx, 57, 0, sx, 62, 0, MAT_METAL, pal::GOLD);
        vol.set(sx + if sx > 0 { 1 } else { -1 }, 61, 0, MAT_METAL, pal::GOLD);
    }
    vol.box_fill(-3, 5, 32, 3, 7, 36, MAT_MATTE, pal::STONE_MID);
    vol.cylinder(0.0, 8, 34.0, 2.2, 4, MAT_METAL, pal::IRON);
    vol.box_fill(-1, 12, 33, 1, 12, 35, MAT_LAMP, pal::LAMP_GLOW);

    vol.compile(colors)
}

pub fn build_cliff_stilt_temple(colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.25;
    let mut vol = MicroVoxelVolume::new(84, 120, 84, s, -42, -56, -42);
    let px_list = [-24, -12, 0, 12, 24];
    let pz_list = [-18, -6, 6, 18, 30];
    for &px in &px_list {
        for &pz in &pz_list {
            vol.box_fill(px - 1, -56, pz - 1, px + 1, 0, pz + 1, MAT_WOOD, pal::WOOD_DARK);
        }
    }
    for by in [-44, -30, -16, -4] {
        for &px in &px_list {
            vol.box_fill(px, by, -18, px, by + 1, 30, MAT_WOOD, pal::WOOD_BEAM);
        }
        for &pz in &pz_list {
            vol.box_fill(-24, by, pz, 24, by + 1, pz, MAT_WOOD, pal::WOOD_BEAM);
        }
    }
    for i in 0..px_list.len() - 1 {
        vol.line(px_list[i] as f32, -30.0, 30.0, px_list[i + 1] as f32, -4.0, 30.0, 0.7, MAT_WOOD, pal::WOOD_WARM);
        vol.line(px_list[i + 1] as f32, -30.0, 30.0, px_list[i] as f32, -4.0, 30.0, 0.7, MAT_WOOD, pal::WOOD_WARM);
    }
    vol.box_fill(-26, 0, -20, 26, 2, 32, MAT_WOOD, pal::WOOD_WARM);
    for x in -26..=26 {
        for rz in [32, -20] {
            vol.set(x, 3, rz, MAT_WOOD, pal::CRIMSON);
            vol.set(x, 5, rz, MAT_WOOD, pal::VERMILION);
            if x.rem_euclid(6) == 0 {
                vol.box_fill(x, 3, rz, x, 6, rz, MAT_WOOD, pal::CRIMSON);
                vol.set(x, 7, rz, MAT_METAL, pal::GOLD);
            }
        }
    }
    for z in -20..=32 {
        for rx in [-26, 26] {
            vol.set(rx, 3, z, MAT_WOOD, pal::CRIMSON);
            vol.set(rx, 5, z, MAT_WOOD, pal::VERMILION);
        }
    }
    vol.hollow_box_fn(-20, 3, -16, 20, 20, 10, MAT_MATTE, |x, y, z| {
        if y <= 5 || (x.abs() == 20 && z.rem_euclid(5) == 0) { pal::CRIMSON } else { pal::PLASTER }
    });
    add_detailed_entrance(&mut vol, 0, 3, 10, 5, 11, false, 0, true);
    add_tiled_roof(&mut vol, 0, 21, -3, 24, 17, 10, 4, 'X', pal::PLASTER);
    vol.compile(colors)
}

pub fn build_bell_tower(colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.25;
    let mut vol = MicroVoxelVolume::new(40, 56, 40, s, -20, 0, -20);
    vol.box_fn(-14, 0, -14, 14, 4, 14, MAT_MATTE, |x, _, z| {
        if (x + z).rem_euclid(3) == 0 { pal::STONE_MID } else { pal::STONE_LT }
    });
    for sx in [-1, 1] {
        for sz in [-1, 1] {
            vol.line((sx * 10) as f32, 5.0, (sz * 10) as f32, (sx * 8) as f32, 22.0, (sz * 8) as f32, 1.3, MAT_WOOD, pal::CRIMSON);
        }
    }
    vol.box_fill(-10, 18, -10, 10, 20, 10, MAT_WOOD, pal::WOOD_DARK);
    vol.cylinder(0.0, 10, 0.0, 3.2, 8, MAT_METAL, pal::IRON);
    vol.cylinder(0.0, 11, 0.0, 3.6, 1, MAT_METAL, pal::BRASS);
    vol.box_fill(-1, 18, -1, 1, 20, 1, MAT_METAL, pal::GOLD);
    vol.line(-7.0, 13.0, 0.0, 7.0, 13.0, 0.0, 0.8, MAT_WOOD, pal::WOOD_WARM);
    add_tiled_roof(&mut vol, 0, 21, 0, 14, 14, 11, 4, 'X', pal::PLASTER);
    vol.box_fill(-1, 33, -1, 1, 36, 1, MAT_METAL, pal::GOLD);
    vol.compile(colors)
}

pub fn build_nandaimon_gate(colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.25;
    let mut vol = MicroVoxelVolume::new(88, 72, 48, s, -44, 0, -24);
    vol.box_fill(-38, 0, -18, 38, 3, 18, MAT_MATTE, pal::STONE_MID);
    for px in [-32, -16, 16, 32] {
        for pz in [-12, 0, 12] {
            vol.box_fill(px - 2, 3, pz - 2, px + 2, 4, pz + 2, MAT_MATTE, pal::STONE_DARK);
            vol.cylinder(px as f32, 5, pz as f32, 1.6, 19, MAT_WOOD, pal::CRIMSON);
        }
    }
    for side in [-1, 1] {
        vol.hollow_box_fn(side * 18, 4, -10, side * 30, 18, 10, MAT_MATTE, |_, y, _| {
            if y % 4 == 0 { pal::CRIMSON } else { pal::PLASTER }
        });
        vol.box_fill(side * 22, 7, 10, side * 26, 14, 10, MAT_WOOD, pal::WOOD_DARK);
    }
    vol.cylinder(0.0, 11, 0.0, 3.2, 8, MAT_LAMP, pal::LANTERN_RED);
    vol.cylinder(0.0, 19, 0.0, 2.5, 2, MAT_METAL, pal::IRON);
    vol.cylinder(0.0, 10, 0.0, 2.5, 1, MAT_METAL, pal::IRON);
    add_tiled_roof(&mut vol, 0, 24, 0, 38, 16, 4, 4, 'X', pal::PLASTER);
    vol.hollow_box(-28, 28, -10, 28, 34, 10, MAT_WOOD, pal::CRIMSON);
    add_tiled_roof(&mut vol, 0, 35, 0, 34, 14, 11, 4, 'X', pal::PLASTER);
    vol.compile(colors)
}

pub fn build_ceremonial_staircase(span_z: f32, rise_y: f32, half_w: f32, colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.25;
    let n_z = (span_z / s).round() as i32 + 4;
    let n_y = (rise_y / s).round() as i32 + 12;
    let n_x = ((half_w * 2.0 + 4.0) / s).round() as i32;
    let mut vol = MicroVoxelVolume::new(n_x, n_y, n_z, s, -(n_x / 2), 0, -(n_z / 2));
    let steps = (rise_y / s).round() as i32;
    let z_len = (span_z / s).round() as i32;
    let hw = (half_w / s).round() as i32;
    for sz in 0..=z_len {
        let z = (z_len / 2) - sz;
        let t = sz as f32 / z_len as f32;
        let sy = (t * steps as f32).round() as i32;
        vol.box_fn(-hw, 0, z, hw, sy, z, MAT_MATTE, |x, y, _| {
            if y == sy {
                if (x + sz).rem_euclid(4) == 0 { pal::COURTYARD } else { pal::STONE_LT }
            } else {
                pal::STONE_DARK
            }
        });
        for side in [-1, 1] {
            let rx = side * (hw + 1);
            vol.box_fill(rx - 1, 0, z, rx + 1, sy + 3, z, MAT_MATTE, pal::STONE_MID);
            if sz % 12 == 0 || sz == z_len {
                vol.box_fill(rx - 1, sy + 4, z - 1, rx + 1, sy + 6, z + 1, MAT_MATTE, pal::STONE_LT);
                vol.set(rx, sy + 7, z, MAT_LAMP, pal::LAMP_GLOW);
                vol.box_fill(rx - 1, sy + 8, z - 1, rx + 1, sy + 8, z + 1, MAT_MATTE, pal::STONE_DARK);
            }
        }
    }
    vol.compile(colors)
}

pub fn build_grand_taiko_bridge(colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.2;
    let mut vol = MicroVoxelVolume::new(64, 56, 156, s, -32, -20, -78);
    let half_span = 74i32;
    let hw = 22i32;
    for z in -half_span..=half_span {
        let t = z as f32 / half_span as f32;
        let arch_y = (22.0 * (1.0 - t * t)).round() as i32;
        for x in -hw..=hw {
            let is_curb = x.abs() >= hw - 1;
            let is_plank_gap = z.rem_euclid(3) == 0;
            vol.set(
                x,
                arch_y,
                z,
                MAT_WOOD,
                if is_curb { pal::CRIMSON } else if is_plank_gap { pal::WOOD_BEAM } else { pal::WOOD_WARM },
            );
            if x == 0 || x.abs() == 10 || x.abs() == hw - 2 {
                vol.box_fill(x, arch_y - 3, z, x, arch_y - 1, z, MAT_WOOD, pal::WOOD_DARK);
            }
        }
        for rx in [-hw, hw] {
            vol.set(rx, arch_y + 3, z, MAT_WOOD, pal::VERMILION);
            vol.set(rx, arch_y + 5, z, MAT_WOOD, pal::CRIMSON);
            if z.rem_euclid(4) == 0 {
                vol.box_fill(rx, arch_y + 1, z, rx, arch_y + 5, z, MAT_WOOD, pal::VERMILION);
            }
            if z.rem_euclid(16) == 0 {
                vol.box_fill(rx - 1, arch_y + 1, z - 1, rx + 1, arch_y + 7, z + 1, MAT_WOOD, pal::CRIMSON);
                vol.box_fill(rx, arch_y + 8, z, rx, arch_y + 9, z, MAT_METAL, pal::GOLD);
            }
        }
        if z == 0 || z.abs() == 48 {
            for rx in [-hw - 1, hw + 1] {
                vol.box_fill(rx, arch_y + 1, z, rx, arch_y + 11, z, MAT_WOOD, pal::WOOD_DARK);
                vol.box_fill(rx - 1, arch_y + 12, z - 1, rx + 1, arch_y + 14, z + 1, MAT_LAMP, pal::LAMP_GLOW);
                vol.box_fill(rx - 2, arch_y + 15, z - 2, rx + 2, arch_y + 16, z + 2, MAT_WOOD, pal::ROOF_DARK);
            }
        }
        if z.abs() == 32 || z.abs() == 62 {
            for px in [-hw + 2, hw - 2] {
                vol.box_fill(px - 2, -20, z - 2, px + 2, arch_y - 2, z + 2, MAT_MATTE, pal::STONE_DARK);
            }
        }
    }
    vol.compile(colors)
}

pub fn build_covered_bridge(colors: &mut ColorCache) -> (MeshData, u64) {
    let s = 0.2;
    let mut vol = MicroVoxelVolume::new(48, 52, 120, s, -24, -18, -60);
    let half_span = 56;
    let hw = 12;
    for z in -half_span..=half_span {
        let t = z as f32 / half_span as f32;
        let ay = (12.0 * (1.0 - t * t)).round() as i32;
        vol.box_fill(-hw, ay, z, hw, ay, z, MAT_WOOD, if z.rem_euclid(3) == 0 { pal::WOOD_BEAM } else { pal::WOOD_WARM });
        for rx in [-hw, hw] {
            vol.set(rx, ay + 2, z, MAT_WOOD, pal::CRIMSON);
            vol.set(rx, ay + 4, z, MAT_WOOD, pal::WOOD_BEAM);
            if z.rem_euclid(14) == 0 {
                vol.box_fill(rx, ay + 1, z, rx, ay + 15, z, MAT_WOOD, pal::CRIMSON);
                if z.abs() == 14 || z.abs() == 42 {
                    vol.set(rx + if rx > 0 { 1 } else { -1 }, ay + 12, z, MAT_LAMP, pal::LANTERN_RED);
                }
            }
        }
        let ry = ay + 16;
        for r in 0..=5 {
            let rw = hw as f32 + 4.0 - r as f32 * 2.5;
            let bound = rw.ceil() as i32;
            let inner = rw.floor() as i32 - 1;
            for dx in -bound..=bound {
                if dx.abs() >= inner || r == 5 {
                    vol.set(dx, ry + r, z, MAT_WOOD, if z.rem_euclid(2) == 0 { pal::ROOF_TRIM } else { pal::ROOF_DARK });
                }
            }
        }
        if z.abs() == 28 {
            for px in [-hw + 1, hw - 1] {
                vol.box_fill(px - 1, -18, z - 1, px + 1, ay - 1, z + 1, MAT_MATTE, pal::STONE_DARK);
            }
        }
    }
    vol.compile(colors)
}

pub fn build_watermill_house(colors: &mut ColorCache) -> (MeshData, MeshData, u64) {
    let s = 0.16;
    let mut vol = MicroVoxelVolume::new(56, 52, 56, s, -28, -4, -28);
    vol.box_fill(-20, -4, -18, 20, 1, 18, MAT_MATTE, pal::STONE_MID);
    vol.hollow_box_fn(-18, 2, -16, 18, 22, 16, MAT_MATTE, |_, y, _| {
        if y <= 6 { pal::WOOD_BEAM } else { pal::PLASTER_W }
    });
    for cx in [-18, 0, 18] {
        for cz in [-16, 16] {
            vol.box_fill(cx, 2, cz, cx, 22, cz, MAT_WOOD, pal::WOOD_DARK);
        }
    }
    add_detailed_entrance(&mut vol, 0, 2, 16, 4, 11, true, pal::NOREN_CREAM, true);
    vol.box_fill(-10, 8, 16, -6, 14, 16, MAT_WIN, pal::WIN_GLOW);
    vol.box_fill(6, 8, 16, 10, 14, 16, MAT_WIN, pal::WIN_GLOW);
    add_tiled_roof(&mut vol, 0, 23, 0, 21, 19, 10, 3, 'X', pal::PLASTER_W);
    vol.box_fill(-26, 7, -2, -18, 9, 2, MAT_WOOD, pal::WOOD_DARK);
    let (house_mesh, c1) = vol.compile(colors);

    let ws = 0.14;
    let mut wvol = MicroVoxelVolume::new(16, 44, 44, ws, -8, -22, -22);
    wvol.box_fill(-5, -1, -1, 5, 1, 1, MAT_METAL, pal::IRON);
    let r = 17.0f32;
    for a in 0..72 {
        let ang = (a as f32 / 72.0) * std::f32::consts::TAU;
        let py = (ang.sin() * r).round() as i32;
        let pz = (ang.cos() * r).round() as i32;
        let py2 = (ang.sin() * (r - 4.0)).round() as i32;
        let pz2 = (ang.cos() * (r - 4.0)).round() as i32;
        wvol.set(-4, py, pz, MAT_WOOD, pal::WOOD_DARK);
        wvol.set(4, py, pz, MAT_WOOD, pal::WOOD_DARK);
        wvol.set(-3, py2, pz2, MAT_WOOD, pal::WOOD_WARM);
        wvol.set(3, py2, pz2, MAT_WOOD, pal::WOOD_WARM);
    }
    for sp in 0..12 {
        let ang = (sp as f32 / 12.0) * std::f32::consts::TAU;
        let sy = (ang.sin() * r).round() as f32;
        let sz = (ang.cos() * r).round() as f32;
        wvol.line(-4.0, 0.0, 0.0, -4.0, sy, sz, 0.6, MAT_WOOD, pal::WOOD_BEAM);
        wvol.line(4.0, 0.0, 0.0, 4.0, sy, sz, 0.6, MAT_WOOD, pal::WOOD_BEAM);
        let py = (ang.sin() * (r + 2.0)).round() as f32;
        let pz = (ang.cos() * (r + 2.0)).round() as f32;
        wvol.line(-4.0, sy, sz, 4.0, sy, sz, 0.7, MAT_WOOD, pal::WOOD_LT);
        wvol.line(-4.0, py, pz, 4.0, py, pz, 0.5, MAT_WOOD, pal::WOOD_WARM);
    }
    let (wheel_mesh, c2) = wvol.compile(colors);
    (house_mesh, wheel_mesh, c1 + c2)
}

pub fn build_harbor_pier(colors: &mut ColorCache) -> (MeshData, u64) {
    let ps = 0.2;
    let mut vol = MicroVoxelVolume::new(150, 32, 70, ps, 0, -16, -35);
    vol.box_fn(0, 0, -12, 130, 1, 12, MAT_WOOD, |x, _, _| {
        if x.rem_euclid(3) == 0 { pal::WOOD_BEAM } else { pal::WOOD_WARM }
    });
    vol.box_fn(100, 0, -32, 130, 1, 32, MAT_WOOD, |_, _, z| {
        if z.rem_euclid(3) == 0 { pal::WOOD_BEAM } else { pal::WOOD_WARM }
    });
    let mut px = 6;
    while px <= 126 {
        for pz in [-11, 11] {
            vol.box_fill(px - 1, -16, pz - 1, px + 1, 4, pz + 1, MAT_WOOD, pal::WOOD_DARK);
            vol.set(px, 5, pz, MAT_METAL, pal::IRON);
        }
        px += 20;
    }
    vol.box_fill(40, 2, -9, 49, 7, -3, MAT_WOOD, pal::WOOD_LT);
    vol.box_fill(52, 2, -8, 58, 5, -3, MAT_WOOD, pal::WOOD_BEAM);
    vol.cylinder(115.0, 2, -22.0, 2.5, 2, MAT_MATTE, pal::NOREN_CREAM);
    for (lx, lz) in [(16, -11), (66, 11), (126, -28), (126, 28)] {
        vol.box_fill(lx, 5, lz, lx, 12, lz, MAT_WOOD, pal::WOOD_DARK);
        vol.box_fill(lx - 1, 13, lz - 1, lx + 1, 15, lz + 1, MAT_LAMP, pal::LAMP_GLOW);
        vol.box_fill(lx - 1, 16, lz - 1, lx + 1, 16, lz + 1, MAT_WOOD, pal::ROOF_DARK);
    }
    vol.compile(colors)
}

pub fn build_island_deck(colors: &mut ColorCache) -> (MeshData, u64) {
    let mut vol = MicroVoxelVolume::new(64, 24, 64, 0.25, -32, -14, -32);
    vol.box_fn(-26, 0, -26, 26, 1, 26, MAT_WOOD, |x, _, _| {
        if x.rem_euclid(2) == 0 { pal::WOOD_BEAM } else { pal::WOOD_WARM }
    });
    for sx in [-24, 0, 24] {
        for sz in [-24, 0, 24] {
            vol.box_fill(sx - 1, -14, sz - 1, sx + 1, 0, sz + 1, MAT_WOOD, pal::WOOD_DARK);
        }
    }
    vol.compile(colors)
}

pub struct BuildingArchetypes {
    pub ryokan_blue: MeshData,
    pub ryokan_red: MeshData,
    pub machiya_cream: MeshData,
    pub machiya_dark: MeshData,
    pub teahouse: MeshData,
    pub stall_orange: MeshData,
    pub stall_blue: MeshData,
    pub watchtower: MeshData,
    pub inari: MeshData,
    pub torii_grand: MeshData,
    pub torii_small: MeshData,
    pub lantern: MeshData,
    pub total_voxels: u64,
}

pub fn compile_building_archetypes(colors: &mut ColorCache) -> BuildingArchetypes {
    let s = 0.16;
    let mut total_voxels = 0u64;

    let mk_ryokan = |noren_col: u32, colors: &mut ColorCache| -> (MeshData, u64) {
        let mut v = MicroVoxelVolume::new(72, 68, 60, s, -36, -4, -30);
        v.box_fill(-30, -4, -24, 30, 1, 24, MAT_MATTE, pal::STONE_MID);
        v.hollow_box_fn(-28, 2, -22, 28, 20, 22, MAT_MATTE, |_, y, _| {
            if y <= 5 { pal::YAKISUGI } else { pal::PLASTER_W }
        });
        let mut x = -28;
        while x <= 28 {
            for z in [-22, 22] {
                v.box_fill(x, 2, z, x, 20, z, MAT_WOOD, pal::CRIMSON);
            }
            x += 14;
        }
        add_detailed_entrance(&mut v, 0, 2, 22, 6, 13, true, noren_col, true);
        for wx in [-18, 18] {
            for dy in 6..=14 {
                for dx in -5i32..=5 {
                    let frame = dx.rem_euclid(2) == 0 || dy == 6 || dy == 14;
                    v.set(wx + dx, dy, 22, if frame { MAT_WOOD } else { MAT_WIN }, if frame { pal::WOOD_DARK } else { pal::WIN_GLOW });
                }
            }
        }
        add_tiled_roof(&mut v, 0, 21, 24, 31, 5, 3, 2, 'X', pal::PLASTER_W);
        v.hollow_box(-24, 21, -18, 24, 36, 18, MAT_MATTE, pal::PLASTER);
        v.box_fill(-25, 22, 19, 25, 22, 23, MAT_WOOD, pal::WOOD_WARM);
        for x in -25i32..=25 {
            v.set(x, 25, 23, MAT_WOOD, pal::CRIMSON);
            if x.rem_euclid(3) == 0 {
                v.box_fill(x, 23, 23, x, 25, 23, MAT_WOOD, pal::WOOD_DARK);
            }
        }
        for wx in [-12, 0, 12] {
            for dy in 24..=32 {
                for dx in -4i32..=4 {
                    let frame = dy % 3 == 0 || dx.rem_euclid(2) == 0;
                    v.set(wx + dx, dy, 18, if frame { MAT_WOOD } else { MAT_WIN }, if frame { pal::WOOD_DARK } else { pal::WIN_GLOW });
                }
            }
        }
        add_tiled_roof(&mut v, 0, 37, 0, 28, 21, 11, 3, 'X', pal::PLASTER);
        v.compile(colors)
    };

    let mk_machiya = |noren_col: u32, wall_col: u32, colors: &mut ColorCache| -> (MeshData, u64) {
        let mut v = MicroVoxelVolume::new(56, 56, 62, s, -28, -4, -31);
        v.box_fill(-20, -4, -22, 20, 1, 22, MAT_MATTE, pal::STONE_MID);
        v.hollow_box_fn(-18, 2, -20, 18, 28, 20, MAT_MATTE, |_, y, _| {
            if y <= 7 { pal::WOOD_BEAM } else { wall_col }
        });
        for cx in [-18, 0, 18] {
            for cz in [-20, 20] {
                v.box_fill(cx, 2, cz, cx, 28, cz, MAT_WOOD, pal::WOOD_DARK);
            }
        }
        add_detailed_entrance(&mut v, -6, 2, 20, 4, 12, true, noren_col, true);
        for dy in 6..=13 {
            for dx in 4..=14 {
                let frame = dx % 2 == 0 || dy == 6 || dy == 13;
                v.set(dx, dy, 20, if frame { MAT_WOOD } else { MAT_WIN }, if frame { pal::WOOD_DARK } else { pal::WIN_GLOW });
            }
        }
        v.box_fill(5, 2, 22, 13, 5, 24, MAT_WOOD, pal::WOOD_LT);
        v.cylinder(15.0, 2, 23.0, 1.8, 3, MAT_MATTE, pal::NOREN_CREAM);
        add_tiled_roof(&mut v, 0, 17, 22, 21, 4, 2, 1, 'X', wall_col);
        for dy in 20..=24 {
            for dx in -12i32..=12 {
                if dx.abs() > 2 {
                    let wall = dx.rem_euclid(2) == 0;
                    v.set(dx, dy, 20, if wall { MAT_MATTE } else { MAT_WIN }, if wall { wall_col } else { pal::WIN_GLOW });
                }
            }
        }
        add_tiled_roof(&mut v, 0, 29, 0, 22, 23, 10, 3, 'X', wall_col);
        v.compile(colors)
    };

    let (ryokan_blue, c) = mk_ryokan(pal::NOREN_BLUE, colors); total_voxels += c;
    let (ryokan_red, c) = mk_ryokan(pal::NOREN_RED, colors); total_voxels += c;
    let (machiya_cream, c) = mk_machiya(pal::NOREN_BLUE, pal::PLASTER_W, colors); total_voxels += c;
    let (machiya_dark, c) = mk_machiya(pal::NOREN_RED, pal::YAKISUGI, colors); total_voxels += c;

    let teahouse = {
        let mut v = MicroVoxelVolume::new(58, 48, 58, s, -29, -4, -29);
        let mut dummy_rng = Rng(1);
        v.box_fill(-22, -4, -20, 22, 1, 20, MAT_MATTE, pal::STONE_MID);
        v.hollow_box(-18, 2, -16, 18, 18, 12, MAT_MATTE, pal::PLASTER_W);
        add_detailed_entrance(&mut v, 0, 2, 12, 5, 11, true, pal::NOREN_RED, true);
        v.box_fill(-18, 2, 13, 18, 2, 20, MAT_WOOD, pal::WOOD_WARM);
        v.box_fill(-15, 3, 16, -7, 4, 19, MAT_MATTE, pal::CRIMSON);
        v.cylinder(12.0, 3, 17.0, 0.6, 12, MAT_WOOD, pal::WOOD_DARK);
        v.ellipsoid(12.0, 15.0, 17.0, 7.0, 2.0, 7.0, MAT_MATTE, pal::VERMILION, 0.0, &mut dummy_rng);
        add_tiled_roof(&mut v, 0, 19, -1, 23, 19, 10, 3, 'X', pal::PLASTER_W);
        let (m, cnt) = v.compile(colors);
        total_voxels += cnt;
        m
    };

    let mk_stall = |awning_col: u32, colors: &mut ColorCache| -> (MeshData, u64) {
        let mut v = MicroVoxelVolume::new(40, 34, 32, s, -20, -2, -16);
        v.box_fill(-16, -2, -10, 16, 0, 10, MAT_MATTE, pal::STONE_MID);
        for px in [-14, 14] {
            for pz in [-8, 8] {
                v.box_fill(px, 1, pz, px, 16, pz, MAT_WOOD, pal::WOOD_DARK);
            }
        }
        v.box_fill(-13, 1, 3, 13, 5, 8, MAT_WOOD, pal::WOOD_WARM);
        v.box_fill(-11, 6, 4, -6, 7, 7, MAT_MATTE, 0xd84315);
        v.box_fill(-4, 6, 4, 2, 8, 7, MAT_MATTE, pal::NOREN_CREAM);
        v.box_fill(4, 6, 4, 11, 7, 7, MAT_MATTE, pal::GRASS_RICE);
        for z in -10..=12 {
            let ay = 18 - ((z + 10) as f32 * 0.22).floor() as i32;
            v.box_fill(-17, ay, z, 17, ay, z, MAT_MATTE, if z.abs() % 4 == 0 { pal::PLASTER } else { awning_col });
        }
        v.box_fill(0, 12, 10, 0, 14, 10, MAT_LAMP, pal::LAMP_GLOW);
        v.compile(colors)
    };
    let (stall_orange, c) = mk_stall(pal::NOREN_RED, colors); total_voxels += c;
    let (stall_blue, c) = mk_stall(pal::NOREN_BLUE, colors); total_voxels += c;

    let watchtower = {
        let mut v = MicroVoxelVolume::new(42, 88, 42, s, -21, -4, -21);
        v.box_fn(-14, -4, -14, 14, 4, 14, MAT_MATTE, |x, y, z| {
            if (x + y + z).rem_euclid(3) == 0 { pal::STONE_DARK } else { pal::STONE_MID }
        });
        for sx in [-1, 1] {
            for sz in [-1, 1] {
                v.line((sx * 10) as f32, 5.0, (sz * 10) as f32, (sx * 6) as f32, 52.0, (sz * 6) as f32, 1.1, MAT_WOOD, pal::WOOD_DARK);
            }
        }
        let mut h = 14;
        while h <= 46 {
            v.hollow_box(-8, h, -8, 8, h, 8, MAT_WOOD, pal::WOOD_BEAM);
            h += 10;
        }
        v.box_fill(-9, 52, -9, 9, 53, 9, MAT_WOOD, pal::WOOD_WARM);
        v.hollow_box(-9, 54, -9, 9, 58, 9, MAT_WOOD, pal::CRIMSON);
        v.cylinder(0.0, 58, 0.0, 1.5, 4, MAT_METAL, pal::GOLD);
        add_tiled_roof(&mut v, 0, 62, 0, 12, 12, 8, 2, 'X', pal::PLASTER_W);
        let (m, cnt) = v.compile(colors);
        total_voxels += cnt;
        m
    };

    let inari = {
        let mut v = MicroVoxelVolume::new(56, 48, 56, s, -28, -4, -28);
        v.box_fill(-20, -4, -18, 20, 2, 18, MAT_MATTE, pal::STONE_MID);
        v.hollow_box_fn(-16, 3, -12, 16, 18, 12, MAT_MATTE, |_, y, _| {
            if y <= 6 { pal::CRIMSON } else { pal::PLASTER }
        });
        add_detailed_entrance(&mut v, 0, 3, 12, 5, 11, false, 0, true);
        add_tiled_roof(&mut v, 0, 19, 0, 21, 16, 10, 3, 'X', pal::PLASTER);
        for sx in [-12, 12] {
            v.box_fill(sx - 2, 3, 15, sx + 2, 6, 19, MAT_MATTE, pal::STONE_LT);
            v.box_fill(sx - 1, 7, 16, sx + 1, 11, 18, MAT_MATTE, pal::PLASTER);
            v.set(sx - 1, 12, 17, MAT_MATTE, pal::CRIMSON);
            v.set(sx + 1, 12, 17, MAT_MATTE, pal::CRIMSON);
        }
        let (m, cnt) = v.compile(colors);
        total_voxels += cnt;
        m
    };

    let mk_torii = |span: i32, h: i32, colors: &mut ColorCache| -> (MeshData, u64) {
        let ts = 0.2;
        let mut v = MicroVoxelVolume::new(span * 2 + 16, h + 14, 16, ts, -(span + 8), -4, -8);
        for sx in [-1, 1] {
            let px = sx * span;
            v.box_fill(px - 2, -4, -2, px + 2, 4, 2, MAT_MATTE, pal::TORII_BASE);
            v.line(px as f32, 5.0, 0.0, (px - sx) as f32, h as f32, 0.0, 1.4, MAT_WOOD, pal::VERMILION);
        }
        v.box_fill(-span - 4, h - 5, -1, span + 4, h - 3, 1, MAT_WOOD, pal::VERMILION);
        v.box_fill(-1, h - 2, -1, 1, h + 1, 1, MAT_METAL, pal::GOLD);
        for dx in (-span - 7)..=(span + 7) {
            let curve = if dx.abs() > span + 2 {
                ((dx.abs() - span - 2) as f32 * 0.45).round() as i32
            } else {
                0
            };
            v.box_fill(dx, h + 1 + curve, -2, dx, h + 2 + curve, 2, MAT_WOOD, pal::VERMILION);
            v.box_fill(dx, h + 3 + curve, -2, dx, h + 3 + curve, 2, MAT_WOOD, pal::ROOF_DARK);
        }
        v.compile(colors)
    };
    let (torii_grand, c) = mk_torii(18, 36, colors); total_voxels += c;
    let (torii_small, c) = mk_torii(10, 22, colors); total_voxels += c;

    let lantern = {
        let ls = 0.14;
        let mut v = MicroVoxelVolume::new(16, 28, 16, ls, -8, -2, -8);
        v.box_fill(-4, -2, -4, 4, 1, 4, MAT_MATTE, pal::STONE_DARK);
        v.box_fill(-2, 2, -2, 2, 11, 2, MAT_MATTE, pal::STONE_MID);
        v.box_fill(-4, 12, -4, 4, 13, 4, MAT_MATTE, pal::STONE_LT);
        v.box_fill(-3, 14, -3, 3, 18, 3, MAT_LAMP, pal::LAMP_GLOW);
        for (cx, cz) in [(-3, -3), (3, -3), (-3, 3), (3, 3)] {
            v.box_fill(cx, 14, cz, cx, 18, cz, MAT_MATTE, pal::STONE_MID);
        }
        v.box_fill(-5, 19, -5, 5, 20, 5, MAT_WOOD, pal::ROOF_DARK);
        v.box_fill(-3, 21, -3, 3, 21, 3, MAT_WOOD, pal::ROOF_TRIM);
        v.box_fill(-1, 22, -1, 1, 24, 1, MAT_MATTE, pal::STONE_LT);
        let (m, cnt) = v.compile(colors);
        total_voxels += cnt;
        m
    };

    BuildingArchetypes {
        ryokan_blue,
        ryokan_red,
        machiya_cream,
        machiya_dark,
        teahouse,
        stall_orange,
        stall_blue,
        watchtower,
        inari,
        torii_grand,
        torii_small,
        lantern,
        total_voxels,
    }
}

pub struct BoatArchetypes {
    pub rowboat: MeshData,
    pub yakatabune: MeshData,
    pub total_voxels: u64,
}

pub fn build_boat_archetypes(colors: &mut ColorCache) -> BoatArchetypes {
    let s = 0.08;
    let mut total_voxels = 0u64;

    let rowboat = {
        let mut vol = MicroVoxelVolume::new(92, 34, 34, s, -46, -4, -17);
        let half_l = 36i32;
        for x in -half_l..=half_l {
            let u = x as f32 / half_l as f32;
            let taper = if u > 0.65 { 1.0 - (u - 0.65) / 0.42 } else { 1.0 };
            let half_w = (11.0 * (1.0 - 0.68 * u * u) * taper).round().max(2.0) as i32;
            let sheer_y = (3.5 * u * u + if u > 0.7 { (u - 0.7) * 14.0 } else { 0.0 }).round() as i32;
            let keel_y = (-2.0 + 2.5 * u * u).round() as i32;
            for z in -half_w..=half_w {
                let zv = z.abs() as f32 / half_w.max(1) as f32;
                let hull_y = (keel_y as f32 + (sheer_y + 4 - keel_y) as f32 * zv.powf(1.9)).round() as i32;
                let is_gunwale = z.abs() == half_w;
                let col = if is_gunwale {
                    pal::WOOD_DARK
                } else if x.rem_euclid(8) == 0 {
                    pal::WOOD_BEAM
                } else {
                    pal::WOOD_WARM
                };
                vol.box_fill(x, keel_y, z, x, if is_gunwale { sheer_y + 5 } else { hull_y }, z, MAT_WOOD, col);
                if !is_gunwale && z.abs() < half_w - 1 && x > -half_l + 4 && x < half_l - 8 && z.rem_euclid(2) == 0 {
                    vol.set(x, keel_y + 2, z, MAT_WOOD, pal::WOOD_LT);
                }
            }
            if x.rem_euclid(9) == 0 && x.abs() < half_l - 5 {
                for z in (-half_w + 1)..=(half_w - 1) {
                    vol.set(x, keel_y + 2, z, MAT_WOOD, pal::WOOD_DARK);
                }
            }
        }
        for tx in [-12, 8] {
            vol.box_fill(tx - 2, 4, -8, tx + 2, 5, 8, MAT_WOOD, pal::WOOD_BEAM);
        }
        vol.box_fill(-13, 6, -4, -10, 6, 4, MAT_MATTE, pal::CRIMSON);
        vol.box_fill(half_l + 1, 8, 0, half_l + 2, 9, 0, MAT_METAL, pal::BRASS);
        vol.box_fill(-26, 6, 9, -26, 8, 9, MAT_METAL, pal::IRON);
        vol.line(-16.0, 6.0, 4.0, -44.0, 3.0, 14.0, 0.65, MAT_WOOD, pal::WOOD_LT);
        vol.box_fill(-46, 2, 13, -38, 3, 15, MAT_WOOD, pal::WOOD_WARM);
        vol.line((half_l - 4) as f32, 6.0, 0.0, (half_l - 2) as f32, 15.0, 0.0, 0.55, MAT_METAL, pal::IRON);
        vol.box_fill(half_l - 2, 11, -1, half_l, 14, 1, MAT_LAMP, pal::LAMP_GLOW);
        vol.box_fill(half_l - 2, 15, -1, half_l, 15, 1, MAT_METAL, pal::IRON);
        let (m, cnt) = vol.compile(colors);
        total_voxels += cnt;
        m
    };

    let yakatabune = {
        let mut vol = MicroVoxelVolume::new(128, 54, 48, s, -64, -5, -24);
        let half_l = 54i32;
        for x in -half_l..=half_l {
            let u = x as f32 / half_l as f32;
            let taper = if u > 0.72 { 1.0 - (u - 0.72) / 0.35 } else { 1.0 };
            let half_w = (17.0 * (1.0 - 0.55 * u * u) * taper).round().max(3.0) as i32;
            let sheer_y = (4.0 * u * u + if u > 0.75 { (u - 0.75) * 16.0 } else { 0.0 }).round() as i32;
            let keel_y = (-3.0 + 2.5 * u * u).round() as i32;
            for z in -half_w..=half_w {
                let zv = z.abs() as f32 / half_w.max(1) as f32;
                let hull_y = (keel_y as f32 + (sheer_y + 5 - keel_y) as f32 * zv.powf(2.1)).round() as i32;
                let is_gun = z.abs() == half_w;
                vol.box_fill(x, keel_y, z, x, if is_gun { sheer_y + 6 } else { hull_y }, z, MAT_WOOD, if is_gun { pal::CRIMSON } else { pal::WOOD_DARK });
                if !is_gun && z.abs() < half_w - 1 && x.abs() < half_l - 8 {
                    vol.set(x, keel_y + 2, z, MAT_WOOD, pal::WOOD_WARM);
                }
            }
        }
        for cx in [-28, -10, 10, 28] {
            for cz in [-12, 12] {
                vol.box_fill(cx, 3, cz, cx, 22, cz, MAT_WOOD, pal::CRIMSON);
            }
        }
        for x in -28..=28 {
            for cz in [-12, 12] {
                vol.box_fill(x, 3, cz, x, 7, cz, MAT_WOOD, pal::WOOD_DARK);
                for dy in 8..=18 {
                    let frame = x.rem_euclid(3) == 0 || dy % 3 == 0;
                    vol.set(x, dy, cz, if frame { MAT_WOOD } else { MAT_WIN }, if frame { pal::WOOD_BEAM } else { pal::WIN_GLOW });
                }
            }
        }
        vol.box_fill(28, 3, -4, 28, 17, 4, MAT_WOOD, pal::WOOD_DARK);
        vol.box_fill(29, 9, 1, 29, 11, 1, MAT_METAL, pal::GOLD);
        for x in -32..=32 {
            for z in -15..=15 {
                let arch = (5.0 * (1.0 - (z as f32 / 15.0).powi(2))).round() as i32;
                vol.set(x, 22 + arch, z, MAT_WOOD, if z.abs() == 15 { pal::CRIMSON } else { pal::ROOF_DARK });
            }
            if x.rem_euclid(14) == 0 {
                for lz in [-15, 15] {
                    vol.box_fill(x, 18, lz, x, 20, lz, MAT_LAMP, pal::LANTERN_RED);
                    vol.set(x, 21, lz, MAT_METAL, pal::IRON);
                }
            }
        }
        vol.box_fill(-30, 28, 0, 30, 28, 0, MAT_METAL, pal::GOLD);
        let (m, cnt) = vol.compile(colors);
        total_voxels += cnt;
        m
    };

    BoatArchetypes {
        rowboat,
        yakatabune,
        total_voxels,
    }
}

pub struct TreeArchetypes {
    pub sakuras: [MeshData; 4],
    pub pines: [MeshData; 3],
    pub bamboo: MeshData,
    pub rock: MeshData,
    pub total_voxels: u64,
}

pub fn compile_tree_archetypes(rng: &mut Rng, colors: &mut ColorCache) -> TreeArchetypes {
    let s = 0.25;
    let mut total_voxels = 0u64;

    let mk_sakura = |variant: i32, rng: &mut Rng, colors: &mut ColorCache| -> (MeshData, u64) {
        let mut v = MicroVoxelVolume::new(56, 52, 56, s, -28, -2, -28);
        let h = 14 + (variant % 3) * 3;
        let lean_x = if variant % 2 == 0 { 1 } else { -1 } * 3;
        let lean_z = if (variant >> 1) % 2 == 0 { 1 } else { -1 } * 2;
        v.line(0.0, -2.0, 0.0, lean_x as f32, (h as f32 * 0.55).round(), lean_z as f32, 1.8, MAT_WOOD, pal::WOOD_DARK);
        v.line(lean_x as f32, (h as f32 * 0.55).round(), lean_z as f32, lean_x as f32 * 1.4, h as f32, lean_z as f32 * 1.3, 1.3, MAT_WOOD, pal::WOOD_DARK);

        let col_fn = |x: i32, y: i32, z: i32| -> u32 {
            let r = hash2(x as f32 * 1.9 + y as f32 * 3.1, z as f32 * 2.3);
            if r < 0.25 {
                pal::SAKURA_DEEP
            } else if r < 0.55 {
                pal::SAKURA_2
            } else if r > 0.82 {
                pal::SAKURA_3
            } else {
                pal::SAKURA_1
            }
        };

        let top_x = (lean_x as f32 * 1.4).round() as i32;
        let top_z = (lean_z as f32 * 1.3).round() as i32;
        v.ellipsoid_fn(top_x as f32, (h + 3) as f32, top_z as f32, 10.0, 4.0, 10.0, MAT_FOLIAGE, col_fn, 0.25, rng);
        let limbs = 5;
        for b in 0..limbs {
            let ang = (b as f32 / limbs as f32) * std::f32::consts::TAU + variant as f32 * 1.1;
            let dist = 8.0 + (b % 2) as f32 * 3.0;
            let bx = (top_x as f32 + ang.cos() * dist).round() as i32;
            let bz = (top_z as f32 + ang.sin() * dist).round() as i32;
            let by = h + (b % 3) - 1;
            v.line(top_x as f32, (h - 2) as f32, top_z as f32, bx as f32, by as f32, bz as f32, 0.85, MAT_WOOD, pal::WOOD_DARK);
            v.ellipsoid_fn(bx as f32, (by + 2) as f32, bz as f32, 7.0, 3.2, 7.0, MAT_FOLIAGE, col_fn, 0.3, rng);
        }
        v.compile(colors)
    };

    let mk_pine = |variant: i32, rng: &mut Rng, colors: &mut ColorCache| -> (MeshData, u64) {
        let mut v = MicroVoxelVolume::new(48, 60, 48, s, -24, -2, -24);
        let h = 28 + variant * 4;
        let twist = if variant % 2 == 0 { 1 } else { -1 } * 3;
        v.line(0.0, -2.0, 0.0, twist as f32, (h as f32 * 0.5).round(), 1.0, 1.5, MAT_WOOD, pal::WOOD_DARK);
        v.line(twist as f32, (h as f32 * 0.5).round(), 1.0, 0.0, h as f32, -1.0, 1.1, MAT_WOOD, pal::WOOD_DARK);
        let pine_col = |x: i32, y: i32, z: i32| -> u32 {
            if hash2((x + y) as f32, z as f32) < 0.38 { pal::PINE_DARK } else { pal::PINE }
        };
        for tier in 0..4 {
            let ty = (h as f32 * (0.45 + tier as f32 * 0.17)).round() as i32;
            let rad = (11 - tier * 2) as f32;
            let ang = tier as f32 * 2.1 + variant as f32;
            let ox = if tier == 3 { 0 } else { (ang.cos() * 4.0).round() as i32 };
            let oz = if tier == 3 { -1 } else { (ang.sin() * 4.0).round() as i32 };
            v.line(0.0, (ty - 2) as f32, 0.0, ox as f32, ty as f32, oz as f32, 0.7, MAT_WOOD, pal::WOOD_DARK);
            v.ellipsoid_fn(ox as f32, (ty + 1) as f32, oz as f32, rad, 2.2, rad, MAT_FOLIAGE, pine_col, 0.22, rng);
        }
        v.compile(colors)
    };

    let (s0, c) = mk_sakura(0, rng, colors); total_voxels += c;
    let (s1, c) = mk_sakura(1, rng, colors); total_voxels += c;
    let (s2, c) = mk_sakura(2, rng, colors); total_voxels += c;
    let (s3, c) = mk_sakura(3, rng, colors); total_voxels += c;

    let (p0, c) = mk_pine(0, rng, colors); total_voxels += c;
    let (p1, c) = mk_pine(1, rng, colors); total_voxels += c;
    let (p2, c) = mk_pine(2, rng, colors); total_voxels += c;

    let bamboo = {
        let mut v = MicroVoxelVolume::new(24, 56, 24, s, -12, -2, -12);
        let stalks = [(-3, -2, 42), (2, -3, 48), (0, 3, 45), (-4, 4, 38), (4, 2, 40)];
        for (sx, sz, sh) in stalks {
            for y in -2..=sh {
                v.set(sx, y, sz, MAT_WOOD, if y.rem_euclid(5) == 0 { pal::BAMBOO_NODE } else { pal::BAMBOO });
                if y > sh - 12 && y.rem_euclid(3) == 0 {
                    let ox = sx + if y.rem_euclid(2) != 0 { 1 } else { -1 };
                    v.ellipsoid(ox as f32, y as f32, sz as f32, 2.5, 1.1, 2.5, MAT_FOLIAGE, pal::BAMBOO_LEAF, 0.35, rng);
                }
            }
        }
        let (m, cnt) = v.compile(colors);
        total_voxels += cnt;
        m
    };

    let rock = {
        let mut v = MicroVoxelVolume::new(24, 18, 24, s, -12, -2, -12);
        v.ellipsoid_fn(
            0.0,
            3.0,
            0.0,
            7.0,
            4.5,
            6.0,
            MAT_MATTE,
            |x, y, z| {
                if y > 5 {
                    pal::MOSS
                } else if hash2(x as f32, z as f32) > 0.5 {
                    pal::STONE_MID
                } else {
                    pal::STONE_DARK
                }
            },
            0.2,
            rng,
        );
        v.ellipsoid(5.0, 2.0, 4.0, 4.0, 3.0, 4.0, MAT_MATTE, pal::STONE_MID, 0.2, rng);
        let (m, cnt) = v.compile(colors);
        total_voxels += cnt;
        m
    };

    TreeArchetypes {
        sakuras: [s0, s1, s2, s3],
        pines: [p0, p1, p2],
        bamboo,
        rock,
        total_voxels,
    }
}

pub fn build_villager_archetypes(colors: &mut ColorCache) -> ([MeshData; 4], u64) {
    let s = 0.0625;
    let kim_cols = [0x223b63u32, 0xa62626, 0xeee5d2, 0x5c3a6e];
    let obi_cols = [0xe6b800u32, 0x1e2024, 0xb81d13, 0xd99b26];
    let mut total_voxels = 0u64;
    let mut dummy_rng = Rng(77);

    let mut mk = |i: usize| -> MeshData {
        let mut v = MicroVoxelVolume::new(18, 36, 18, s, -9, 0, -9);
        v.box_fill(-4, 0, -2, -1, 1, 2, MAT_WOOD, pal::WOOD_DARK);
        v.box_fill(1, 0, -2, 4, 1, 2, MAT_WOOD, pal::WOOD_DARK);
        v.box_fill(-4, 2, -2, -1, 2, 2, MAT_WOOD, pal::WOOD_WARM);
        v.box_fill(1, 2, -2, 4, 2, 2, MAT_WOOD, pal::WOOD_WARM);
        v.box_fill(-4, 3, -3, 4, 14, 3, MAT_MATTE, kim_cols[i]);
        v.box_fill(-4, 13, -3, 4, 15, 3, MAT_MATTE, obi_cols[i]);
        v.box_fill(-2, 13, -4, 2, 16, -3, MAT_MATTE, obi_cols[i]);
        v.box_fill(-5, 16, -2, 5, 21, 2, MAT_MATTE, kim_cols[i]);
        v.box_fill(-6, 13, -2, -5, 20, 2, MAT_MATTE, kim_cols[i]);
        v.box_fill(5, 13, -2, 6, 20, 2, MAT_MATTE, kim_cols[i]);
        v.box_fill(-2, 22, -2, 2, 27, 2, MAT_MATTE, 0xf3cca4);
        v.box_fill(-3, 26, -3, 3, 29, 2, MAT_MATTE, 0x1a1716);
        v.box_fill(-1, 29, -3, 1, 31, -1, MAT_MATTE, 0x1a1716);
        if i == 1 {
            v.box_fill(5, 17, 1, 5, 32, 1, MAT_WOOD, pal::WOOD_BEAM);
            v.ellipsoid(5.0, 32.0, 1.0, 6.0, 1.5, 6.0, MAT_MATTE, pal::VERMILION, 0.0, &mut dummy_rng);
        } else if i == 3 {
            v.box_fill(6, 14, 3, 6, 18, 3, MAT_LAMP, pal::LAMP_GLOW);
            v.set(6, 19, 3, MAT_METAL, pal::IRON);
        }
        let (m, cnt) = v.compile(colors);
        total_voxels += cnt;
        m
    };

    let arr = [mk(0), mk(1), mk(2), mk(3)];
    (arr, total_voxels)
}
