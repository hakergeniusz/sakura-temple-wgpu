# 桜の寺村 — Grand Sakura Valley

Native Rust + wgpu 30.0 + WGSL rewrite of the 240 × 96 × 240 Grand Sakura Valley voxel diorama.

- Single instanced draw call for ~140k static voxels (32-byte `VoxelInstance` buffer) + one depth-only shadow pass
- 4096 falling sakura petals simulated entirely in a WGSL compute shader (`STORAGE | VERTEX` buffer, zero per-frame CPU uploads)
- 2048×2048 depth shadow map with 3×3 PCF comparison filtering, hemisphere + directional sun lighting, 6 warm lantern point lights, metallic gold speculars, emissive windows, animated translucent water, procedural sky dome, exponential fog

## Run

```
cargo run --release
```

## Controls

- Left-Drag: Orbit camera
- Right-Drag: Pan across the valley
- Scroll Wheel: Zoom
- T or Space: Toggle the CatmullRom Cinematic Tour
- 1–7: Camera presets (1 Overview, 2 Grand Bridge, 3 Water Torii, 4 Waterfall Lake, 5 Senbon Torii, 6 Courtyard, 7 5-Tier Pagoda)

## License

- **Code** (Rust, WGSL, everything in this repo): MIT — see [LICENSE](LICENSE).
- **Visuals**: there are no standalone visual assets here — every image is procedurally
  generated at runtime from the code. The rendered output and the diorama's artistic
  design (scene layout, color palettes, the shaders' visual content) are dedicated to
  the public domain via [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/):
  screenshots, recordings and derivative artwork are free to use for any purpose,
  no attribution required.
