# Vision (maintainer, 2026-11-20): read before any interface decision

## What the maintainer wants
- **A real port, not a recompilation** ("almost an NGU Idle 2, the ultimate version").
- **First a PERFECT PC base**: usable, portable everywhere, **fully modifiable and documented**. Readability on Vita is the final goal and becomes easy if the base is good.
- **Responsive**, in the PC-game sense (adapts to any window size and shape) **and** the web sense. The original's fixed 16:10 resolutions are **the biggest challenge**.
- On PC: resize the window freely; the original's fixed sizes are removed from the settings; the game stays **sharp at any size** at the **monitor's refresh rate**.
- Anything the original "draws by hand" (fine for a fixed size, painful for an adaptive UI) may be **changed** to become adaptive.
- **Example (one idea among others, not a spec)** on Vita, where space is short: the page list is hidden by default; holding **L** replaces the energy/magic/save column with the page list, the d-pad changes page, releasing L confirms. The space gained goes to the right-hand game area (zoomed page, scrolling).
- Every trade-off must be **known and documented**.

## Principles
1. **Logic stays the original's, verified identical** (generated Rust vs reference interpreter). **Presentation becomes ours**: the game's controllers keep writing to their texts/images; our layout layer decides where, how big, and whether it is visible. Everything can be rearranged without touching game rules, and the conformance checks stay valid.
2. **Layers**: logic (generated) -> scene model (UI tree extracted from the game) -> **layout** (ours: per-target profiles in readable files) -> **rendering** (quad list -> GPU) -> **platform** (window, input, gamepad, sound, on-screen keyboard).
3. **Layout profiles**: "PC free", "Vita", "web / portrait"... Each defines regions (player column, page list, game area, tooltips), their visibility, shortcuts (e.g. Vita's L) and zoom/scroll of the game area.
4. **Override a game method cleanly** when presentation requires it (e.g. `NGUResolutions`): a documented mechanism of hand-written Rust in one file, each override justified.
5. **Documentation generated from data** (it cannot lie): UI map, elements, controlling script, baked-drawing images; plus hand-written guides ("how to move / hide / redraw an element").

## Known trade-offs
| topic | reality | consequence / option |
|---|---|---|
| Central menus (~55, 668x600) | laid out pixel by pixel, no automatic layout (74 % of elements have fixed anchors) | cannot "reflow" automatically: scale as a block (sharp, redrawn on GPU), then redo the most used by hand |
| Images (sprites) | fixed resolution, much of it pixel art | text and flat fills stay sharp at any size; icons are simply enlarged (filtering by default), no more detail than the source |
| Baked backgrounds | some menu backgrounds contain boxes/lines at button positions | moving buttons breaks the drawing: inventory of those images needed, redraw with 9-slice or simple shapes (using the game's real assets only) |
| Absolute coordinates in game code | menus hidden by moving off-screen, tooltips placed at the mouse and clamped to 960x600, drag and drop, `Screen.width/height` (45 uses) | the game always believes it lives in 960x600: our layer converts to the real screen |
| Speed | CPU drawing ~13 ms at 1280x800, ~100 ms estimated at 4K | GPU rendering also on PC |
| Graphics API | the original uses Direct3D 11 | ours: OpenGL ES 2 / WebGL / vitaGL (same role, portable) |
| Small buttons (+ / Cap / -) | designed for a mouse | on touch/gamepad: larger hit areas, focus, zoom |

## Decisions (2026-11-20)
1. **Large screen**: the game area **only scales up**. Exception, page by page: if the enlarged render is bad, or the page already scrolled (list), try to **show more content**.
2. **Enlarged images**: simply enlarged (default filtering); revisit later.
3. **Style**: keep **the original drawings as much as possible**. A new interface is allowed if it **copies the original exactly**.
4. **Display**: offer **common resolutions** (+ fullscreen) in addition to free resizing. **No frame-rate setting for the player**: runs like the original. Measured: the original forces `QualitySettings.vSyncCount = 1` (`Character.cs`), i.e. synchronised to the monitor (60, 144, 200 Hz...) through Direct3D 11. We do the same: vsync, one frame per refresh.
5. **Rendering stack** (2026-11-20): `miniquad` (OpenGL / WebGL, pure Rust) for PC and web; a vitaGL host in C for the Vita; all fed by the engine's quad list.
5b. **Platform order**: PC first, then web, then Vita (the Vita is not available now; web is testable without hardware and keeps the platform separation clean).
6. **GPU**: of course, like the original.
7. **Dev mode** in the game: see everything, unlock everything, test without depending on a save or clicks (`docs/DEV.md`); it is used first by the assistant.
8. **Free publication, three repositories (GPL-3.0)**: `nguidle-rust` (engine + conversion of the original game to Rust + PC version), `nguidle-web` and `nguidle-vita` (they use only the engine: a Cargo dependency on a tagged version of `nguidle-rust`; their build calls the conversion on the player's own files). Nothing from the game is published (no code, assets, extracted texts, saves) and no personal information.
9. **Web**: self-hosting only. A documented Docker container: the user mounts their own game files, it converts, builds and serves. **Vita**: same principle with a small "preparer" tool on PC that produces the SD-card data folder (+ the VPK).
10. **Images and assets**: screenshots of OUR port may go into the repositories (only what is really visible in our port); always the game's real assets and official assets (LiveArea provided by the maintainer); **never create an asset from scratch**.
11. **Language**: everything in the repositories is English (code, comments, messages, docs); only the chat with the maintainer is French.

## Open point: logic cadence
In the original, `Update()` (86 classes) runs once per displayed frame with real `Time.deltaTime` (125 uses) and most computation goes through `InvokeRepeating` (108 calls, e.g. `updateNGU` every 0.02 s). Ours: fixed 1/30 s step. So the original's result depends slightly on the monitor rate; ours does not. To settle by measurement (task A9).
