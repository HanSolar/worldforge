# Architecture

```
WorldConfig ──► Seed ──► Noise2D ──► Landmass ──► Coastline
                                          │
                                          ├──► Palette ──► Raster ──► PNG
                                          └──► (stats: land fraction)
```

## Stages

| Stage | Module | Input | Output |
| --- | --- | --- | --- |
| Configure | `config.rs` | user intent | validated `WorldConfig` |
| Seed | `seed.rs` | label or number | stable `u64` |
| Noise | `noise.rs` | seed + `NoiseConfig` | fBm field in `0.0..=1.0` |
| Land | `landmass.rs` | config | row-major `f32` grid |
| Coast | `coastline.rs` | grid + sea level | shoreline segments |
| Colour | `palette.rs` | elevation | `Rgba` |
| Pixels | `raster.rs` | grid + palette | RGB raster, PNG |

## Design rules

1. **Determinism is a contract.** No randomness, no wall-clock reads, no
   hash-map iteration inside the pipeline. Same config ⇒ identical bytes.
2. **Validation happens before allocation.** `WorldConfig::validate` rejects a
   4-billion-pixel request before a single `Vec` is reserved.
3. **Sampling domain is normalised.** `0.0..=1.0` on both axes regardless of
   resolution, so changing `--size` adds detail rather than rescaling the world.
4. **The bridge never panics across the ABI.** Every `extern "C"` entry point
   runs inside `catch_unwind` and reports failures via `WfStatus`.

## Extending

The natural next stages, none of which need to change existing signatures:

- **Hydrology** — rivers by steepest descent over the heightfield.
- **Biomes** — temperature/moisture fields blended into the palette lookup.
- **Export** — heightmap (`f32`) and coastline (SVG/GeoJSON) alongside PNG.
