# WorldForge inspiration resources

Curated research links for procedural map / world generation.

**Source conversation:** [Procedural Map Design (ChatGPT share)](https://chatgpt.com/share/6af17fb1-e86c-83e9-af03-739c20c43197)

**Usage rule (same as `AGENTS.md`):** treat these as *shape* references. Study algorithms and pipelines, then write our own code. Do **not** paste third-party repos into `crates/`. Read each project's license before any reuse. Do **not** copy Street Fighter or other commercial map art, layouts, or assets — arcade style is thematic inspiration only.

---

## Primary focus — Red Blob Games / Mapgen4

Interactive demo and deep write-ups on hand-drawn-inspired procedural wilderness maps (Voronoi dual mesh, paint-then-simulate rainfall/rivers, oblique rendering).

| Resource | URL | Notes |
| --- | --- | --- |
| Mapgen4 interactive | https://www.redblobgames.com/maps/mapgen4/ | Paint mountains/valleys/oceans; rainfall → biomes/rivers |
| Mapgen4 source | https://github.com/redblobgames/mapgen4 | TypeScript; dual-mesh, map, painting, render workers |
| Mapgen4 blog index | https://simblob.blogspot.com/search/label/mapgen4 | Full project series |
| Mapgen4 goals | https://simblob.blogspot.com/2018/08/mapgen4-goals.html | Scope vs mapgen2; what was kept/dropped |
| Elevation | https://simblob.blogspot.com/2018/08/mapgen4-elevation.html | Elevation for look, not look for elevation |
| Distance-field elevation painting | https://simblob.blogspot.com/2018/09/mapgen4-elevation-painting.html | |
| Multithreading | https://simblob.blogspot.com/2018/09/mapgen4-threads.html | Real-time regen while painting |
| River appearance | https://simblob.blogspot.com/2018/09/mapgen4-river-appearance.html | |
| Elevation painting revisited | https://simblob.blogspot.com/2018/09/mapgen4-elevation-painting-revisited.html | |
| Rainfall / biomes / wind | https://simblob.blogspot.com/2018/09/mapgen4-rainfall.html | Evaporation → wind → rain |
| Oblique projection | https://simblob.blogspot.com/2018/09/mapgen4-oblique-projection.html | Hand-drawn map look |
| Outlines | https://simblob.blogspot.com/2018/10/mapgen4-outlines.html | Sketch-like edges |
| Bug fixes | https://simblob.blogspot.com/2018/09/mapgen4-bug-fixes.html | |
| River representation (binary trees) | https://simblob.blogspot.com/2018/10/mapgen4-river-representation.html | |
| Boundaries rewrite (2023) pt 1 | https://simblob.blogspot.com/2023/04/improving-mapgen4s-boundaries-part-1.html | Geometry library |
| Boundaries rewrite (2023) pt 2 | https://simblob.blogspot.com/2023/04/improving-mapgen4s-boundaries-part-2.html | Edge-of-map geometry |
| Boundaries rewrite (2023) pt 3 | https://simblob.blogspot.com/2023/04/improving-mapgen4s-boundaries-part-3.html | Side-of-map rendering |
| Planet generation experiments | https://www.redblobgames.com/x/1843-planet-generation/ | Sphere / planet output ideas |
| Classic polygon map generation (mapgen2 era) | http://www-cs-students.stanford.edu/~amitp/game-programming/polygon-map-generation/ | Voronoi coasts, elevation, rivers, biomes |
| Polygon map generation part 2 | https://simblob.blogspot.com/2010/09/polygon-map-generation-part-2.html | |
| Red Blob Games home | https://www.redblobgames.com | Broader mapgen / pathfinding notes |

---

## GitHub — highly recommended (from the research chat)

| Project | URL | Why it matters for WorldForge |
| --- | --- | --- |
| `vnovak404/worldgen` (Rust) | https://github.com/vnovak404/worldgen | Closest pipeline shape: tectonics → elevation → climate → hydrology; deterministic layers |
| `fabio-t/terrain-generator` (Rust) | https://github.com/fabio-t/terrain-generator | Fractal noise, island shaping, hydraulic erosion, rivers/lakes |
| `paraglider0815/worldGen` (Go) | https://github.com/paraglider0815/worldGen | Full stack: geology, erosion, hydrology, climate, biomes, settlements |
| `redblobgames/mapgen4` | https://github.com/redblobgames/mapgen4 | Geography look + river networks + paintable elevation |
| `Azgaar/Fantasy-Map-Generator` | https://github.com/Azgaar/Fantasy-Map-Generator | Continents, biomes, mountains, rivers, forests, countries, settlements — strong feature-layer reference |
| `nvkelso/natural-earth-vector` | https://github.com/nvkelso/natural-earth-vector | Real-world coastlines / land / boundaries for a future Global Map mode |
| `bones-ai/rust-procedural-world` | https://github.com/bones-ai/rust-procedural-world | Rust + Bevy + Perlin intro; paired YouTube tutorials |
| `zacharycarter/mapgen` | https://github.com/zacharycarter/mapgen | Additional map-generator experiments |

### Other GitHub / demos mentioned in the same chat

| Project | URL |
| --- | --- |
| `alexander-miglis/procedural-map-generation-on-a-sphere` | https://github.com/alexander-miglis/procedural-map-generation-on-a-sphere |
| `manankarnik/bevy_generative` | https://github.com/manankarnik/bevy_generative |
| `TheDatawiz-dot/Worldsmith` | https://github.com/TheDatawiz-dot/Worldsmith |
| `gtkacz/worldgen` | https://github.com/gtkacz/worldgen |
| `kaylendog/kosmos` | https://github.com/kaylendog/kosmos |
| `saagar210/TerraSynth` | https://github.com/saagar210/TerraSynth |
| three.js biomes demo | https://askalice.github.io/threejs-worldgen/biomes/ |
| basabu1 Map-Generation demo | https://basabu1.github.io/Map-Generation/ |

---

## YouTube / video learning paths

| Topic | Link |
| --- | --- |
| Rust procedural world (Bones AI) | https://youtu.be/NSDdJeCmXXE |
| Search: Red Blob Games procedural map generation | https://www.youtube.com/results?search_query=Red+Blob+Games+procedural+map+generation |
| Search: hydraulic erosion tutorials | https://www.youtube.com/results?search_query=procedural+terrain+generation+hydraulic+erosion+tutorial |
| Search: climate / biomes / mountains / rivers | https://www.youtube.com/results?search_query=procedural+world+generation+climate+biomes+mountains+rivers |
| Search: Voronoi map generation | https://www.youtube.com/results?search_query=procedural+world+map+generation+Voronoi+diagrams+tutorial |

---

## Geographic data & projections (Global Map mode ideas)

| Resource | URL | Notes |
| --- | --- | --- |
| Natural Earth (data site) | https://www.naturalearthdata.com/ | Public-domain coastlines, land, admin boundaries |
| Natural Earth terms | https://www.naturalearthdata.com/about/terms-of-use/ | Public domain; attribution optional |
| Natural Earth vectors (GitHub) | https://github.com/nvkelso/natural-earth-vector | Same dataset as git |
| Cartopy projections docs | https://scitools.org.uk/cartopy/docs/v0.17/whats_new.html | Projection reference (not a WorldForge dependency) |
| Equirectangular overview | https://www.flamingpear.com/equirectangular.html | Simple game-friendly projection |
| Equal Earth vs Robinson | https://equal-earth.com/equal-earth-vs-robinson.html | Balanced global presentation |
| Winkel Tripel / projection discussion | https://kosmopolite.com/everything-you-know-about-the-world-is-a-lie/ | Why projection choice matters |

Useful projection starting set from the chat: **Equirectangular**, **Robinson**, later **Winkel Tripel** / Mercator as needed.

---

## Style & cartography inspiration (visual only)

Arcade / conquest-map *feel* without copying commercial assets. Use for palette, composition, and feature density ideas only.

| Resource | URL |
| --- | --- |
| Street Fighter II map write-up (Super Chart Island) | https://superchartisland.com/street-fighter-ii/ |
| Inkarnate (fantasy map tooling) | https://inkarnate.com/ |
| Fantasy Planet Painter (Methodox) | https://methodox.itch.io/fantasy-planet-painter/devlog/1184285/release-version-12-higher-resolution-maps |
| PlanetCreator discussion (Blender Artists) | https://blenderartists.org/t/planetcreator-procedural-planet-atmosphere-and-cloud-shader/660793 |
| Fantasy Map Simulator | (cited in chat as “Fantasy Map Simulator” — open from share Sources if needed) |
| World Anvil location example | (cited as Xentairal / Thelara — style of labeled locations) |
| Fantasy world map stock (DeviantArt example) | https://www.deviantart.com/paramenides-mapstock/art/Fantasy-World-Map-01-170505416 |
| Fantasy biome map — Imperious | https://www.deviantart.com/brennanthenerd/art/Fantasy-Biome-Map-Imperious-921255890 |
| Original fantasy map example | https://www.deviantart.com/gamera1985/art/Original-Map-143592691 |
| Post-apocalyptic map example | https://www.deviantart.com/sregan/art/Dark-Dawn-a-post-apocalyptic-map-494053877 |
| Lost continents / mythic cartography | https://ublegend.com/en/ancient_mysteries/lost_continents/ |
| World maps and perspectives essay | https://romneymanassa.wordpress.com/2013/04/08/world-maps-and-perspectives/ |
| Houdini terrain tutorial (Creative Bloq) | https://www.creativebloq.com/how-to/build-terrain-in-houdini-17 |
| Earth-like world building thread | https://www.alternatehistory.com/forum/threads/building-an-earth-like-world-mk-ii.331537/page-11 |
| r/FantasyMapGenerator | https://www.reddit.com/r/FantasyMapGenerator/ |
| Wikimedia geographic world map | https://upload.wikimedia.org/wikipedia/commons/thumb/e/eb/World_map_geographical.jpg/3840px-World_map_geographical.jpg |

---

## Host / plugin references (WorldForge adapters)

Already in `AGENTS.md`; repeated here because the source chat framed PhotoCraft as the first host.

| Resource | URL |
| --- | --- |
| PhotoCraft plugins | https://github.com/storytold/photocraft |
| VectorCraft plugins | https://github.com/storytold/vectorcraft |

---

## Suggested study matrix (from the research chat)

| WorldForge concern | Best starting references |
| --- | --- |
| Independent continents & islands | Azgaar Fantasy Map Generator |
| Mountains / tectonics pipeline | `vnovak404/worldgen` |
| Elevation & hydraulic erosion | `fabio-t/terrain-generator` |
| Rivers & lakes | `fabio-t/terrain-generator`, Mapgen4 river posts |
| Climate & temperature | `vnovak404/worldgen` |
| Forests / biomes / settlements | Azgaar, `paraglider0815/worldGen` |
| Geographic rendering / hand-drawn look | Mapgen4 interactive + blog series |
| Real-world Global Map mode | Natural Earth (+ projection notes above) |
| Rust intro / Bevy noise playground | `bones-ai/rust-procedural-world` + YouTube |

---

## Layered engine sketch (inspiration only)

The chat recommended generating **data layers** before pixels — useful north-star for `worldforge-core`, not a mandate to match any external project:

```text
Geography → Geology (elevation / mountains / erosion)
         → Climate (temperature / rainfall / wind)
         → Hydrology (rivers / lakes / drainage)
         → Biomes
         → Renderer (arcade / fantasy / atlas palettes)
```

Keep PhotoCraft ABI constraints in mind: banded fresh instances, no imports, instruction budget, determinism via `Seed::stream`.

---

## Maintenance

When adding links: keep license notes short, prefer canonical project URLs (strip `utm_source`), and never vendor third-party source trees into this repo.
