# WASM Plugin Capability Monitor

Living tracker for **ArtCraft** host plugin capabilities that affect WorldForge’s PhotoCraft wasm adapter.

**Authoritative for agents.** Do not implement against a **Proposed** feature until its status becomes **Released** and is verified in upstream docs.

**Source conversation:** [Track WASM Updates (ChatGPT share)](https://chatgpt.com/share/6af6bd3e-0f70-83ea-ad64-c4f4d781ff8c)  
**Agent rule:** `.cursor/rules/photocraft-abi-weekly-check.mdc`

**Last checked:** 2026-11-13 (UTC+2)

---

## Status definitions

| Status | Meaning |
|--------|---------|
| **Released** | Documented in upstream `docs/plugins.md` (or equivalent) and usable by a `.wasm` today |
| **In development** | Open PR / partial land; not yet the contract WorldForge may depend on |
| **Proposed** | Design issue / RFC only — do **not** code the adapter against it |
| **Requested** | WorldForge / community ask filed upstream; not shipped |
| **Deprecated** | Was released; upstream documents removal or replacement |

---

## Do not mix host ABIs

| Host | Contract | Exports (typical) | Primary use |
|------|----------|-------------------|-------------|
| **PhotoCraft** | WASM ABI v1 | `pc_*` | Image / canvas filters (WorldForge target) |
| **VectorCraft** | Own plugin docs | vector/exporter styles | Future geography export — separate adapter |
| **EffectCraft** | Effect plug-in WASM ABI 1/2 + scripts | `ec_*` / effect plugin API | Motion/effects — **not** PhotoCraft-compatible |

A module built for one host does **not** run in another. Track them separately; WorldForge’s first adapter is PhotoCraft only.

---

## Official sources (check these)

### PhotoCraft (primary)

| Source | URL |
|--------|-----|
| Plugin ABI docs (raw) | https://raw.githubusercontent.com/storytold/photocraft/main/docs/plugins.md |
| Plugin ABI docs (browse) | https://github.com/storytold/photocraft/blob/main/docs/plugins.md |
| Repo | https://github.com/storytold/photocraft |
| Plugin system v2 design | https://github.com/storytold/photocraft/issues/662 |
| WorldForge capability request | https://github.com/storytold/photocraft/issues/12458 |
| Related: panels / Last Filter scorecard | https://github.com/storytold/photocraft/issues/219 |
| Commits touching plugins | https://github.com/storytold/photocraft/commits/main/docs/plugins.md |

### VectorCraft (future)

| Source | URL |
|--------|-----|
| Plugin docs (raw) | https://raw.githubusercontent.com/storytold/vectorcraft/main/docs/plugins.md |
| Repo | https://github.com/storytold/vectorcraft |

### EffectCraft (do not confuse with PhotoCraft)

| Source | URL |
|--------|-----|
| Plugin docs (raw) | https://raw.githubusercontent.com/storytold/effectcraft/main/docs/plugins.md |
| Repo | https://github.com/storytold/effectcraft |

### Local ArtCraft tree (optional)

If a local checkout exists under `E:\laragon\laragon\www\artCraft_plugins\` (or sibling ArtCraft folders), compare local `docs/plugins.md` to GitHub `main` during the weekly check.

---

## Weekly inspection procedure

1. Read **Last checked** above. If &lt; 7 days and no human ask → skip fetch.
2. Fetch PhotoCraft `docs/plugins.md` (and VectorCraft when relevant).
3. Skim PhotoCraft issues/PRs: `#662`, `#12458`, `#219`, and recent PRs mentioning plugins / ABI / progress / panels.
4. Diff against the **Capability table** below: new Released rows, status promotions, deprecations.
5. Fill a **Weekly review** entry; bump **Last checked**.
6. Tell the human in one short paragraph what changed (or “no new Released features”).
7. Never invent host imports/panels; never code Proposed features into `worldforge-photocraft`.

---

## Capability table (PhotoCraft)

Append new rows; do not delete history — mark **Deprecated** instead.

| ID | Capability | Status | First noted | Evidence | WorldForge opportunity |
|----|------------|--------|-------------|----------|------------------------|
| PC-ABI-1 | Wasm filter ABI v1 (`pc_abi_version`, `pc_manifest`, `pc_alloc`, `pc_filter`, `memory`) | Released | 2026-11-13 | `docs/plugins.md` | Current adapter |
| PC-MANIFEST | Manifest `kind: filter`; params `number`/`int`/`bool`/`choice`; optional `unit: px`; `overlap`; `area` content\|canvas | Released | 2026-11-13 | `docs/plugins.md` | Dialog + canvas generators |
| PC-SANDBOX | No host imports; wasmi; fuel/memory/time limits; fresh instance per band | Released | 2026-11-13 | `docs/plugins.md` | Deterministic banded render |
| PC-DIALOG | Host-generated filter dialog + live preview | Released | 2026-11-13 | `docs/plugins.md` | Flat param UI only |
| PC-INSTALL | `plugin.install` / `list` / `run` / `remove` / `reload` | Released | 2026-11-13 | `docs/plugins.md` | Local wasm install |
| PC-SMART | Smart-filter re-run of `plugin.run` | Released | 2026-11-13 | `docs/plugins.md` | Re-apply on layer |
| PC-LAST-FILTER | Filter › Last Filter repeats plug-ins | Proposed (v1.1 milestone) | 2026-11-13 | #662 milestones | Convenience |
| PC-PROGRESS | `pc_progress` + cooperative cancel | Proposed (v1.1) | 2026-11-13 | #662 | Long worldgen UX |
| PC-KIND-COMMAND | Manifest `kind: command` | Proposed (v2) | 2026-11-13 | #662 | Randomize / menu actions |
| PC-KIND-FORMAT | Manifest `kind: format` (`pc_read`/`pc_write`) | Proposed (v2) | 2026-11-13 | #662 | Sidecar / map export |
| PC-KIND-PANEL | Declarative `kind: panel` | Proposed (v2.1) | 2026-11-13 | #662, #219 | Map browser / presets UI |
| PC-DOC-READ | Bounded layer/doc snapshot reads | Proposed (v2) | 2026-11-13 | #662 | Multi-layer maps |
| PC-DOC-WRITE | Transactional write / new layers | Proposed (v2) | 2026-11-13 | #662 | Land/ocean/overlay layers |
| PC-STATE | Per-plugin persistent KV | Proposed (v2) | 2026-11-13 | #662 | Presets / last seed |
| PC-CAP-MODEL | Opt-in capability imports or import-free event model | Proposed | 2026-11-13 | #662, #4301 | Future sandbox expansion |
| PC-REQ-BUTTONS | Action / button param type | Requested | 2026-11-09 | #12458 | Randomize / Reset |
| PC-REQ-UI-GROUPS | Grouped / conditional dialog layout | Requested | 2026-11-09 | #12458 | Continent/island/climate sections |
| PC-REQ-WIDGETS | Richer widgets (color, text, seed+randomize, %) | Requested | 2026-11-09 | #12458 | Better generator UX |
| PC-REQ-BUDGET | Generator-friendly instruction/time budgets | Requested | 2026-11-09 | #12458 | Safer full-res worldgen |
| PC-REQ-PREVIEW | Host preview vs final apply flag | Requested | 2026-11-09 | #12458 | Replace draft param hack |
| PC-REQ-SIDECAR | Structured export beyond pixels | Requested | 2026-11-09 | #12458 | JSON/SVG/heightmap |
| PC-REQ-PRESETS | Host presets API | Requested | 2026-11-09 | #12458 | Save/load map settings |
| PC-REQ-VALIDATE | Param dependency / `pc_validate` | Requested | 2026-11-09 | #12458 | Mode-aware knobs |
| PC-REQ-I18N | Manifest display names / tooltips / icons | Requested | 2026-11-09 | #12458 | Polish |

### Current Released limits (snapshot)

| Limit | Default (as of last check) |
|-------|----------------------------|
| Instruction fuel | 50 M per call + 4,000 per sample (`pc_filter`) |
| Wall clock | 60 s per run (native) |
| Module size | 32 MiB |
| Linear memory | 512 MiB per instance |
| Param types | `number`, `int`, `bool`, `choice` only |
| Plugin kinds | `filter` only |

---

## Compatibility / breaking-change log

| Date | Change | Impact on WorldForge |
|------|--------|----------------------|
| — | None observed yet | — |

When ABI version, export names, manifest schema, or fuel defaults change, add a row and bump `GENERATOR_VERSION` / adapter code only after verifying Released docs.

---

## Weekly review log

### 2026-11-13 (UTC+2) — baseline

- PhotoCraft `docs/plugins.md`: still **ABI v1**; “Not in v1” unchanged (no panels, no other-layer reads, no progress/cancel, no custom formats; Last Filter not for plugins).
- #662: v2 design still **Proposed** (command/format/panel kinds; capability model; v1.1 = Last Filter + progress).
- #12458: WorldForge request list filed; bot suggested splitting into separate tickets; related to #662 / #219.
- EffectCraft: separate effect/script plugin surface — not actionable for `worldforge-photocraft`.
- **New Released for WorldForge:** none.
- **Action:** keep adapter on v1; watch #662 v1.1 for progress/cancel and Last Filter.

---

## Agent report template (paste into chat after a check)

```text
WASM capability check (YYYY-MM-DD):
- PhotoCraft docs: …
- New Released: … (or none)
- Status promotions: …
- Proposed only (do not implement): …
- WorldForge follow-up: …
```

---

## Related WorldForge paths

- Adapter: `crates/worldforge-photocraft/`
- Architecture rule: `.cursor/rules/worldforge-architecture.mdc`
- Planning gate: `.cursor/rules/planning-governance.mdc`
- Inspiration / research: `docs/inspiration/resource.md`
