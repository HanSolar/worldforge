# Grok bot — hard rules (`worldforge`)

(c) 2026 JC Kirsten. All rights reserved.

**Audience:** Grok / content bots helping on WorldForge via the Grokbot app.  
**Read this first**, then any queued task file the human names.

Adapted from Chess Latte Store / Chess Studio bot rules for this product.

## Who you are

You assist with **allowed WorldForge tasks**: docs, README prose, install/build notes, fixture descriptions, and explicit steps the queue names. You do **not** change Chess Studio (`chessmaestro_v3`) or Chess Latte Store (`chess_latte_store`) unless the human orders that in the same task.

## Forbidden

1. Do not put secrets in git: no real `.env`, API keys, or credentials. Only placeholder names in examples.
2. Do not edit Chess Studio or Chess Latte Store unless the human orders that in the same task.
3. Do not invent PhotoCraft host imports, custom panels, or ABI features not in upstream docs. On new branches, if the weekly ABI check is due, update `docs/development/wasm_plugin_capability_monitor.md` from upstream docs only (see `.cursor/rules/photocraft-abi-weekly-check.mdc`).
4. Do not force-push or delete remotes.
5. Do not mark a wasm build / install task `done` if the `.wasm` was not actually built when the task required a smoke check — mark `blocked` and explain.
6. Do not add Chess Studio campaign, tournament, or entitlement logic to WorldForge.

## Allowed (when queued)

- Draft README / docs prose and installation steps for PhotoCraft.
- Update parameter documentation to match the current manifest.
- Run `cargo test` / `cargo build --target wasm32-unknown-unknown` when the task says so.
- Update a task queue file when finished (if one exists for this job).

## Done signal

If a queue file is in use: set the task `status` to `done`, fill `completed_at` (ISO datetime), `notes`, and `files_touched`. Cursor polls that file and continues.
