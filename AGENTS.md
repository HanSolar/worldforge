# WorldForge agent notes

(c) 2026 JC Kirsten. All rights reserved.

Primary engineering standard: this file, then `.cursor/rules/`, then `docs/architecture.md`.

## What this app is

WorldForge is an **independent procedural world-generation engine** (Rust workspace) with a **PhotoCraft WebAssembly filter plug-in** as the first host adapter.

Architecture approach: **shared core + host adapters**.

**Program stack:** Rust 2024 · Cargo workspace · `worldforge-core` (deterministic field) · `worldforge-photocraft` (PhotoCraft ABI v1 `cdylib` / wasm32) · `worldforge-cli` (optional headless preview).

## Architectural boundaries (hard)

1. **WorldForge owns geography.** Generation lives in `crates/worldforge-core`.
2. **The host owns presentation.** PhotoCraft owns the document, bands, undo, and UI dialog.
3. **Chess Studio owns campaign / chess rules.** WorldForge must **not** import Chess Studio, Tauri, React, or chess logic.
4. **Adapters stay thin.** `worldforge-photocraft` only speaks ABI + params; algorithms stay in core.

```text
PhotoCraft wasm adapter ──┐
CLI adapter ──────────────┼── worldforge-core
Future VectorCraft ───────┘
```

## Layout

| Path | Role |
|------|------|
| `crates/worldforge-core` | Config, seed, noise, landmass field, coastline, palette, raster |
| `crates/worldforge-photocraft` | PhotoCraft ABI v1 (`pc_*` exports) + manifest |
| `crates/worldforge-cli` | Headless preview / manifest dump (not required for M1 ship) |
| `examples/` | Preview harnesses |
| `tests/` | Integration + wasm host simulation |
| `docs/` | Architecture and install notes |

## PhotoCraft ABI constraints (hard)

Treat upstream docs as authoritative:
https://github.com/storytold/photocraft/blob/main/docs/plugins.md

- Module is **wasm32-unknown-unknown**, **no host imports** (no WASI, clock, FS, network, randomness).
- Exports: `memory`, `pc_abi_version`, `pc_manifest`, `pc_alloc`, `pc_filter`.
- Pixels are interleaved little-endian `f32` samples, written **in place**.
- Every band runs in a **fresh instance** — no state between bands.
- Every pixel must be computable from `(seed, config, global document coordinate)`.
- Respect instruction / memory budgets; prefer cheap per-pixel work.
- Do not panic across the wasm boundary.

## Instruction budget gate (hard)

Any edit that can affect `worldforge-photocraft` runtime cost (core sampling, coastline/glow, palette, raster, manifest params, ABI glue) must include an instruction-budget check before marking work done.

- Run the wasm host/integration tests and confirm no instruction-budget failures in a fresh build.
- Keep expensive neighborhood/coast work behind explicit gates and defaults that stay budget-safe.
- If a change risks budget overruns, reduce per-pixel work first; do not ship and "hope host limits are fine."

## Determinism (hard)

Same generator version + config + seed + dimensions ⇒ same samples.

- No wall clock, OS entropy, or hash-map iteration in the pipeline.
- Use documented seeded PRNG / stream derivation (`Seed::stream`).
- Increment `GENERATOR_VERSION` when algorithm output for a given seed changes.

## Editing the code

- Name values. Limits, streams, palette hexes, and defaults live in named constants.
- Follow existing module names and object shapes.
- Comment behavior that is non-obvious (budget tradeoffs, band safety, ABI packing). Skip narration of obvious lines.
- Ask before deleting or rewriting a large existing block. Small in-place fixes do not need that question.
- Search before inventing a new approach; prefer patterns already in this repo.

## Search before you write

Use free / permissive sources as **shape** references, then write our own code. Do not paste third-party repos into `crates/`.

Useful references (read licenses first):

- PhotoCraft plug-ins: https://github.com/storytold/photocraft
- VectorCraft plug-ins: https://github.com/storytold/vectorcraft
- Red Blob Games mapgen notes: https://www.redblobgames.com
- noise-rs (ideas only unless license checked): https://github.com/Razaekel/noise-rs

Do not copy Street Fighter or other commercial map art, layouts, or assets.

## Forbidden

1. Do not put secrets in git: no real `.env`, API keys, or credentials. Placeholders only in examples.
2. Do not depend on Chess Studio, Medusa, Supabase, Tauri UI, or React from this repo.
3. Do not reintroduce a native C “return PNG bytes” ABI — PhotoCraft is wasm band filtering.
4. Do not allocate a full-canvas image inside each band.
5. Do not force-push `main`, rewrite published history, or skip hooks unless the human explicitly asks.
6. Do not mark wasm install “done” without a real build artifact and tests when the task required them.
7. Do not invent host imports or custom PhotoCraft panels (not in ABI v1).
8. Do not mark plugin-affecting work "done" until instruction-budget checks pass.

## Agent model choice

**Do not burn High-token / High-effort models on easy work.**

Easy (git, renames, one-function edits, docs, todo bookkeeping): `composer-2.5-fast`, `gpt-5-mini`, `gemini-3.8-flash-high`, or `claude-4.5-haiku-thinking` only.

Forbidden for Easy: Opus, `*-high-thinking`, Max Mode, Hard-tier slugs, and `inherit` from an expensive parent.

Medium/Hard only for real generator/ABI work (macro landmass math, wasm host bugs, instruction-budget tuning).

## Plans

Every plan must start with the mandatory header (Project, Repository, Branch, Base branch, Date, Plan scope), then:

1. **Agent usage (difficulty)** — full per-todo table
2. **Constraints**
3. **Requirements**
4. **Milestones this plan achieves**

Then the body. See `.cursor/rules/plan-agent-difficulty.mdc` and `.cursor/rules/planning-governance.mdc`.

**Branch first:** feature work for a plan goes on a dedicated branch, not directly on `main`.

## Commands

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets
cargo build -p worldforge-photocraft --release --target wasm32-unknown-unknown
cargo run -p worldforge-cli -- --seed 123456 --size 512 --png out/preview.png
cargo run -p worldforge-cli -- --manifest
```

Wasm artifact: `target/wasm32-unknown-unknown/release/worldforge_photocraft.wasm`

## Done signal for M1-style work

- Workspace builds and tests pass.
- Wasm module has **no imports** and exports the ABI surface.
- Seeded output is deterministic; banded rendering matches whole-image rendering.
- Invalid inputs fail with error codes, not traps.
- Plugin/integration tests show no instruction-budget overrun for default settings.
