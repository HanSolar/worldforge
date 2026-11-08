# WorldForge

Deterministic procedural world maps, with a PhotoCraft bridge.

## Layout

| Path | Purpose |
| --- | --- |
| `crates/worldforge-core` | Pipeline: config → seed → noise → landmass → coastline → palette → raster |
| `crates/worldforge-photocraft` | `cdylib` bridge: flat C ABI + plugin manifest |
| `crates/worldforge-cli` | `worldforge` command-line tool |
| `examples/` | Runnable examples |
| `tests/` | Workspace-level integration tests |

## Quick start

```powershell
cargo run -p worldforge-cli -- --seed atlantis --size 1024x1024 --out out/atlantis.png
cargo run -p worldforge-cli -- --manifest
cargo test --workspace
```

## Determinism

One seed in, one world out. `Seed::from_label("atlantis")` is stable across
platforms, and `mix64` is the single mixing function used everywhere, so
`--seed 42` and a stored config always reproduce the same PNG.

## Docs

- [`docs/architecture.md`](docs/architecture.md) — how the pipeline stages fit together
- [`AGENTS.md`](AGENTS.md) — agent / architecture rules for this repo
- PhotoCraft plug-in ABI (upstream): https://github.com/storytold/photocraft/blob/main/docs/plugins.md
