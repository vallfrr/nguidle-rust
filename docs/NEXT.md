# Roadmap: what is left to do, in order

Read after `AGENTS.md`. Take the **first unchecked task** (`[ ]`), do it until its "Done when" criterion holds, check it off
(`[x]` + date), update `docs/STATUS.md`, commit, push, move on. A task marked **[maintainer]** waits for the maintainer: skip it,
do the next one, and ask at the end.

Goal and decisions: `docs/VISION.md`. Proof of conformance to the original game: `docs/AUDIT-ILGEN.md`, `docs/STATUS.md`.

## Current state (2026-11-20)
- The game's C# is translated to Rust (99.4 % of methods) and produces exactly the same state as the reference interpreter (same visible state, saves, thousands of clicks).
- The PC window runs at ~58 img/s at 1280x800 with a CPU rasterizer (13 ms/frame). The original runs at the monitor rate (200 Hz here), 8 % of one core, 341 MB.
- Done today: dev mode (`docs/DEV.md`), first comparison with the real game (`docs/COMPARISON.md`), disabled-button tint, system-culture number formatting, Unity-like text metrics.
- Nothing of V2 has run on the Vita (only V1, the interpreter at ~19 img/s, was installed on a console).

## Rule for comparisons with the original
Compare **behaviour and content** (what is shown, states, text that fits its box), not pixel-exact output. Do not spend time on
sub-pixel anti-aliasing or rounding unless a defect is visible to the naked eye: once rendering is on the GPU at any size, pixel parity is meaningless.

## Questions pending for the maintainer (ask when they announce their return)
1. Official LiveArea assets for `nguidle-vita` (icon, background, startup image). Current images are **black placeholders**. Never create assets from scratch.

---

## Phase 0: clean publication
- [x] **P0.1** Everything in English (code comments, program messages, scripts, docs, READMEs, skills); update the scripts that grep program output.
- [x] **P0.2** Audit: no French left, no personal information, no game content; regression suite all OK.
- [x] **P0.3** Fresh history pushed to the three repositories (one initial commit each on top of the remote `Initial commit`).
- [x] **P0.4** `nguidle-web`: README documenting the self-hosting Docker container (documentation only for now).
- [x] **P0.5** Black LiveArea placeholders in `nguidle-vita`.

## Phase 1: faithful PC base (behaviour)
- [x] **A2. Faithful input** (2026-11-20: mouse button numbers, `PointerEventData.button`, left-only Selectable clicks/slider/scroll drag, wheel with per-list sensitivity, `GetKey`/`GetKeyDown` by code or name incl. Shift/Ctrl, window forwards keys to the game, F1 toggles focus navigation; script steps `r`, `mc`, `k`, `kd`, `ku`, `wheel`; regress covers them; right-click effect on inventory items not yet observed on screen because the inventory is empty: check in DEV4/A1b). (a) `Input.GetMouseButton/Down/Up` ignore the button number (`clr/src/unity_api.rs`): a left click is seen as a right click by `ItemController`, `DaycareItemController`, `LoadoutController`. Track buttons 0/1/2 in `Engine`, set `PointerEventData.button` (29 uses, 24 of them `Right`). (b) The window never fills `keys_down`/`keys_edge`: no game shortcut works (A D Q W E R T S F Z, digits, arrows; `GetKey("left shift"/"right shift"/"left ctrl"/"right ctrl")` as strings is unhandled). (c) Mouse wheel (ScrollRect sensitivity differs per list: 20/25/30/50). (d) Default: keyboard/mouse = original behaviour, every key goes to the game; focus navigation is for the gamepad and, on keyboard, only after toggling with **F1**. **Done when:** scripted steps (`r,x,y` right click, `k,<code>` key) give identical oracle/native state (`tools/regress.sh` OK) and left vs right click differ on inventory items.
- [~] **DEV4 (first version done 2026-11-20: `ultimate` step + `dev/ultimate.steps` set 165 unlock flags and the sheets show many more elements; sub-pages, array-based unlocks such as Advanced Training and `docs/UI-MAP.md` remain)**. "Ultimate" state: everything shown at once.** Many elements appear only with progression (buttons, tabs, checkboxes, list rows, late-game difficulties, titans, perks, quirks, hacks, wishes, cards, cooking...). List **all** display conditions from the code (`SetActive` 451, `interactable` 332, `enabled` 67 sites; boolean unlock flags), build `dev/ultimate.steps` that makes them all visible, then capture the 58 menus and their sub-pages. **Done when:** `docs/UI-MAP.md` lists each conditional element with its condition and menu, and the "ultimate" contact sheet shows no element hidden by progression (remaining ones listed and justified).
- [ ] **A1b. Compare unlocked menus with the original.** Build an advanced save with the dev mode (`do,dev/unlock.steps`, then `s,<file>`), load it as the original's autosave (copy the save folder first), capture each menu on both sides. Also proves our saves load in the original.
- [ ] **A5. Saves both ways.** Our saves load in the original and vice versa; the file dialogs of `OpenFileDialog` (Windows-only) are replaced by a fixed `save/export/` folder.
- [ ] **A7. The 6 untranslated `order*Cost` sort methods.** Refused by the generator ("diverging stacks": at a block join the evaluation stack has different types per path). Fix in `crates/ngu-ilgen/src/main.rs` by unifying the carry variable types at joins. **Done when:** `coverage.txt` no longer lists them and a test clicks the sort buttons with identical state.
- [ ] **A9. Logic cadence like Unity.** Original: `Update()` once per displayed frame with real `Time.deltaTime`, `InvokeRepeating` on its own timers. Ours: fixed 1/30 s step. Measure the original (progress of a bar over 60 s; how Unity catches up a 0.02 s `InvokeRepeating` on a long frame), then match it; tests keep a fixed `deltaTime`. **Done when:** same progression speed as the original over 60 s (difference measured and noted), regression OK in fixed step.
- [ ] **A6. Sound.** `AudioSource.Play/set_clip/set_mute` are stubs (`clr/src/unity_api.rs`); the game has only 2 `AudioClip`s. Extract them (`tools/nguprep.py`), play them through SDL2 (same code on PC, web, Vita), honour the game's mute setting.
- [x] A10 Disabled buttons greyed (ColorTint). [x] A11 Number formatting by system culture. [x] A12 Text metrics like Unity.

## Phase 2: platform and rendering
- [ ] **A4. SDL2 + GPU rendering** (sharp at any size, monitor refresh rate even at 4K). The engine already produces a quad list (texture, uv, colour, clip); it becomes the GPU input. SDL2 (window, input, gamepad, audio, on-screen keyboard) exists on PC, web (emscripten) and Vita (VitaSDK); OpenGL ES 2 / WebGL / vitaGL. Keep the CPU rasterizer for test captures. **Start with the risk**: a "hello quad" on the Vita as soon as the console is reachable; fallback = separate Vita backend (sceGxm) behind the same quad list. Presentation like the original: vsync, one frame per monitor refresh (`vSyncCount = 1`). **Done when:** monitor rate held at 1280x800 and 4K (`NGU_PERF=1`), text sharp at any size.
- [ ] **A3. Free window.** The original offers fixed sizes 640x400..2880x1800 (`NGUResolutions.cs`); `Screen.SetResolution` is a stub here. Override `NGUResolutions` (documented): its buttons offer common windowed resolutions (<= the screen: 1280x720, 1366x768, 1600x900, 1920x1080, 2560x1440, 3840x2160, 1280x800, 1440x900, 1680x1050, 1920x1200, 2560x1600, 3440x1440) + fullscreen; free resize with the mouse; size remembered. No frame-rate setting for the player. **Done when:** every button applies its size, F11/fullscreen works, free resize has no distortion.
- [ ] **B1. Gamepad on Windows** through SDL2 (remove the never-compiled `gilrs` `pad` feature). Mapping: d-pad/stick = focus, cross = confirm, circle = back, L/R = previous/next menu, Start = settings, Select = info, square = right click, triangle = tooltip, hold +/- = repeat. **[maintainer]** for the physical test.

## Phase 3: responsive, modifiable, documented base (the heart of the project)
Principles and trade-offs in `docs/VISION.md`.
- [ ] **R1. Generated UI map** (`docs/UI-MAP.md` + JSON): per screen its elements, the game controller that writes them, anchors, display conditions, and the list of **baked-drawing images** (menu backgrounds with boxes/lines at button positions).
- [ ] **R2. Game space vs real screen.** The game believes it lives in 960x600 (`Screen.width/height` 45 uses, menus hidden by `Translate` off-screen, tooltips clamped to 960x600, drag and drop). Convert between that space and the real screen so tooltips, drags and clicks stay correct with a moved/enlarged game area.
- [ ] **R3. Layout profiles** in readable files (`pc`, `vita`, `web`): regions (player column, page list, game area, tooltips), placement, visibility, zoom/scroll of the game area, region shortcuts. Applied by the engine without touching the logic. **Done when:** the `pc` profile fills any window shape without useless bars and `tools/regress.sh` stays OK.
- [ ] **R4. Documented overrides**: replace a game method by hand-written Rust through one justified table (first use: `NGUResolutions`) + guide `docs/MODIFY.md` ("how to move / hide / redraw / override").
- [ ] **R5. Vita profile prototype on PC at 960x544**: page list hidden by default; holding L shows the page list in place of the player column, d-pad changes page, releasing L confirms; enlarged game area with scrolling that follows the focus. Screenshots for the maintainer's opinion **[maintainer]**.

## Phase 4: playing with buttons
- [ ] **B2** Tooltips at focus. **B3** Tutorial arrows and adventure zone arrows = d-pad left/right (depends on A2). **B4** Number entry with a gamepad (73 input fields; on Vita the system keyboard). **B5** Filter non-actionable elements from the focus list and check every menu is reachable. **Done when:** every action in `docs/CONTROLLER-PROPOSAL.md` section 1 is possible with buttons only, tested by script on each menu.

## Phase 5: web (self-hosted Docker container)
- [ ] **W1** Container: the user mounts their own game files, the container runs the conversion, builds the web target and serves it. Documented in `nguidle-web` first (P0.4), implemented here.
- [ ] **W2** `web` profile (any browser window, mobile touch), save in browser storage + file export/import. **Done when:** runs in Chrome/Firefox on PC and phone, same logic conformance checks, same screen sheet as PC. Hosting for oneself only: a public site serving the converted game would distribute the game's code.

## Phase 6: Vita V2 (when the console is available)
- [ ] **C1** Cross-compile the generated Rust (`armv7-sony-vita-newlibeabihf`, nightly + build-std), always under the memory cap. **C2** vitaGL GPU rendering from the quad list, texture atlas within the 112 MB VRAM. **C3** Touch = mouse, buttons = actions of phase 4, system keyboard for text. **C4** Fast start: binary scene instead of `scene.json` (V1: 20 s load, target < 5 s). **C5** Measurements: 30 img/s in every menu at 333/166 MHz, RAM, autosave, sleep/resume, 1 h session. **C6** "Preparer" tool producing the SD-card data folder (+ VPK). **C7** Official LiveArea assets **[maintainer]**.

## Phase 7: polish
- [ ] README for installation, readability on 5 inches (R3/R5 refinement), release process.

---

## Log of finished tasks
- 2026-11-20 A10, A11, A12, DEV1-3, A1 first pass.
