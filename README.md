# 桜の帝都村 — Massive Micro-Voxel Sakura Empire

Native Rust + wgpu 30.0 + WGSL rewrite of the `桜の帝都村` Massive Micro-Voxel Sakura Empire diorama.

## Highlights

- **896×896 Micro-Terrain (`448×448` world units at `0.5` unit cell resolution)** with greedy-meshed `64` chunks, stratified mountain cliffs, ishigaki stone retaining walls, stepped rice paddies, alpine waterfalls, meandering river, harbor cove, lotus lagoon, east mountain stream, and west watermill canal.
- **Greedy-Meshed `MicroVoxelVolume` with Baked 3D Ambient Occlusion** across all hero landmarks, village buildings, boats, flora, and animated villagers:
  - 7-Tier Pagoda (`Gojunoto`), Golden Hall (`Kondo`), twin Bell/Drum Towers, `Nandaimon` Gate with giant red `chochin` lantern, Kiyomizu-style Cliff Stilt Temple, Grand Taiko Bridge, Covered Bridges, Harbor Pier, Island Deck, and Watermill House with rotating wheel.
  - Instanced Machiya townhouses, Ryokan inns with fabric *noren* curtains, Teahouses with red parasols, street food stalls, watchtowers, Inari Shrine, 6 Grand/Water Torii + 42 winding `Senbon Torii` gates, and stone lanterns.
  - Moored & gliding `Yakatabune` pleasure boats and wooden rowboats, 37 animated kimono villagers, 26 drifting volumetric cloud clusters, 48 floating `Toro Nagashi` river lanterns, 320 waterfall mist/spray particles, and 2,800 GPU compute-simulated sakura petals.
- **2048×2048 PCF Shadow Mapping, 12 Lantern Point Lights, Material Shading & ACES Filmic Tone Mapping** with **Day**, **Sunset**, and **Lantern Night** modes.

## Run

```
cargo run --release
```

## Controls

- **Left-Drag**: Orbit camera
- **Right-Drag**: Pan across the empire
- **Scroll Wheel**: Zoom
- **`T` or `Space`**: Toggle the 75-second `CatmullRomCurve3` Cinematic Tour
- **`N` or `L`**: Cycle Time of Day (`Day` → `Sunset` → `Lantern Night`)
- **`P`**: Pause / resume animations & petal simulation
- **`1`–`9`**: Camera presets (`1` Overview, `2` Town & Doors, `3` Harbor & Boats, `4` Grand Bridge, `5` Watermill, `6` Cliff Temple, `7` Senbon Torii, `8` Zen Courtyard, `9` 7-Tier Pagoda)
- **`R`**: Reset to Overview
- **`Esc`**: Exit

## License

- **Code** (Rust, WGSL, everything in this repo): MIT — see [LICENSE](LICENSE).
- **Visuals**: procedurally generated at runtime; the rendered output and diorama design are dedicated to the public domain via [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).
