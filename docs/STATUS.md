# Status: what is verified and what is not

## Desktop verification on 2026-11-21

- Windows starts at the three-button home screen. The enlarged autosave action
  starts without a focus tint. Load Game and Create New Game open separate slot
  pickers, with zero stats and inactive gameplay controls. Creating a game in an
  empty slot writes its timestamp and character name. Occupied slots are cleared
  with two inline confirmations. Save Game selects the loaded numbered slot,
  and the autosave remembers which slot it came from. Returning home preserves
  the saved data already on disk.
  Periodic autosave preserved the manual slots byte for byte. Creation refused
  a slot that became occupied after selection. Scripted focus navigation also
  created a save from a blank profile.
- Scripted navigation in Inventory took about 380 ms per move before batching
  raycasts. After batching, 20 moves added about 0.1 seconds to the median run.
  The batch measures engine navigation from a boosted save.
- Controller-equivalent inputs selected an Adventure dropdown option and
  scrolled Augmentation and the patch notes. Focus has an interior yellow tint.
  A scripted sweep collected focus on all 58 menu indices and performed 116
  moves. The maintainer confirmed smooth navigation on a physical controller.
  Focus behaviour still needs refinement across some menus. Remapping
  remains a design proposal.
- The right-hand panels retain their Settings frame style. Inventory's tooltip
  stays above Save Game and is clamped to the visible screen. The sidebar's
  Achievements button and the page were checked in Normal, Dark, Gold and Dark
  Gold using a local fixture that had the original gold-theme entitlement.
- The tooltip pass uses the visible viewport for the tutorial, hover and
  timed messages. Standard scene elements retain their clipping rules so
  menus moved offscreen stay hidden.
- The standalone milestone grid shows 61 Steam Community icons across two
  pages. Mouse and scripted focus select both earned and locked milestones.
  The detail pane shows the original name and unlock condition. A clean
  profile had zero unlocked and the boosted test save unlocked 44. A separate
  fixture that meets all original conditions yielded all 61 IDs in the native
  runtime. The 153 BP achievements still use the original game page.
- Save, Load and Achievements draw the original sidebar bolts over their
  shared frame. The Achievements button spans the same horizontal bounds as
  the Save Game and Load Game pair. Achievement icons fill their tile frames
  and the detail pane uses the same themed frame around its larger icon.
  Selection uses the save picker color inside the icon frame.
- The runtime follows the pointer at render frequency and uses the Energy
  timer's 20 ms schedule for display interpolation. Game rules continue to
  advance in fixed steps.
- The Windows 1280 × 720 GPU test reported around 60 FPS with VSYNC on the
  session's 60 Hz display, verified from the window's active display mode.
  The Inventory run had 38–41 ms maximum intervals per
  120-frame sample. A diagnostic run with VSYNC disabled generated quads
  at over 1,100 FPS. This measures renderer capacity and frame pacing in that
  session, with no guarantee of monitor-rate delivery in every display mode.
- The native Windows settings applied 960 × 540 and 1280 × 720. F11 moved
  from 1280 × 720 to 1440 × 900 and restored the exact size and position on
  exit. Live resize kept the window responsive at 284 × 821 and 1334 × 181.
  Headless rendering confirmed the unused bands stay black in a tall window.
- The portable Windows preparer validates the 115 files in the supported
  installation with SHA-256, exports the scene, runs the converter and builds
  the Windows executable. Verified with a clean extraction and tests for
  missing or changed input files.
- See [Release audit](RELEASE-AUDIT.md) for coverage and the web/Vita boundary.

## Historical baseline on 2026-11-20

Honest table. "Verified" means seen or measured; everything else is stated as not verified.

| item | status |
|---|---|
| Generated Rust: 5,327 / 5,359 methods, compiles, runs on PC | **verified** (oracle) |
| Visible state identical to the interpreter: 30 / 300 / 600 frames, 96 clicks, 1,500 idle frames, random click batteries | **verified** (`tools/regress.sh`) |
| Save (`getBase64Data`) byte-identical oracle/native; load + 100 frames + re-save identical | **verified** |
| Save format = real .NET BinaryFormatter (same as the original): our save loads in the original game; the original's real save loads in ours (oracle == native after loading it) | **verified** |
| Unlocked menus vs the original (18 menus, our ultimate save loaded in the original) | **verified** by side-by-side sheets; remaining differences are time/state dependent |
| Dropdown (options, caption): Adventure and Beard menus no longer raise on refresh | **verified** (capture) |
| Differential fuzz (480 calls, primitive static methods) | **verified**, 0 differences |
| Number formatting by system culture (fr: `500 000`, `3,1s`), regression OK in en and fr | **verified** (dates not handled) |
| Disabled buttons greyed (ColorTint), `Mask` with `showMaskGraphic = false` no longer drawn | **verified** against the original (screenshots) |
| Text: same line breaks as the original on three screens; anti-aliasing differs (`fontdue` vs FreeType) | **verified**, remaining difference accepted |
| Window render after resize (stale cache bug: blurred text, white squares) | **fixed**, verified in capture (interpreter); not tried by hand in the window |
| First comparison with the original (new game: title, intro, main, Boss Fight, Info) | **done**, `docs/COMPARISON.md`; unlocked menus not compared |
| Dev mode (click by text, screen as text, get/set/call, unlock preset, 58-menu sheets) | **verified**, interpreter only |
| GPU window (miniquad): renders the title screen identically to the original, sharp after resizing, 60 img/s with vsync on a 60 Hz display (0.1 ms draw), 2,300 img/s without vsync | **verified** by screen capture and `NGU_PERF`; 4K, fullscreen and window input by hand not tried |
| PC window speed 1280x800 (software window) | **measured**: ~58 img/s (logic 0.7 ms, CPU draw 12.7 ms, present 2.3 ms); original: monitor rate (200 Hz), 8 % of a core, 341 MB |
| Input fidelity to the original | **implemented** (A2): left/right/middle buttons, wheel, game keys incl. Shift/Ctrl; oracle == native on a mixed scenario; the window path (minifb) is compiled but **not tried by hand**; effect on inventory items not yet observed |
| Resolution buttons of the game settings | **no effect** (`Screen.SetResolution` is a stub, task A3); free window resize works |
| Sound | **absent** (task A6) |
| Focus navigation (list, spatial move, confirm, frame) | **verified** by headless script and capture; not with a gamepad |
| Previous/next menu with L/R (`swapMenu` buttons) | **verified** by script (`m,1` / `m,-1`); Tab/E/PageUp-Down in the window not tried |
| Gamepad (`pad` feature, gilrs) | **written, never compiled** (to be replaced by SDL2) |
| Tutorial arrows, text typing, Start = settings, Select = info | **not verified / to do** |
| Vita V2 | **nothing verified**: only V1 (interpreter) ran on a console |

Note: one run of `tools/regress.sh` (right after the invocation catch-up change) reported `complete save identical` as different; the same scenario passed on two re-runs and by hand: possible rare flake, to watch.

## V1 on the Vita (interpreter), verified on the console
- VPK installs and starts; title screen shown; 960x600 scaled to 960x544 (side bars).
- ~18.3 img/s at 333/166 MHz (logic 19 ms, emit 32 ms, draw 4 ms); ~22 MB RAM free after init; start-up ~25 s (scene 20 s).
- Not verified on the console: touch precision, drag and drop, scrolling, sliders, toggles, text entry through the system keyboard, autosave/load paths, save/load buttons, long sessions, sleep/resume, layouts needing `GridLayoutGroup`/`VerticalLayoutGroup`/`ContentSizeFitter`, right click, sound, import of PC saves.
