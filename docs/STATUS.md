# Status: what is verified and what is not

## Console mode on PC, 2026-11-23 (details and limits: `docs/CONSOLE-MODE.md`)
- Scrollbars render with their owning content block, with an inset from the outside frame. The 58-menu, 12-configuration
  capture sweep and the wheel, thumb, and stick replays were repeated after the draw-order change. The 58 classic PC
  captures at 1280 x 720 remained pixel-identical.
- Both Battle Points tabs and both Steam Achievements pages were captured at 100 / 110 / 120 % at 960 x 544,
  1280 x 720, and 1920 x 1080. Selecting an achievement and paging both ways worked at all three scales. Steam
  Achievements' details and footer stay within the right pane, which also follows the active theme.
- Keyboard L in the GPU window uses the controller's shared menu gesture. A Rust test covers taps, the 250 ms hold
  threshold, key repeats and early opening with a direction. The engine's hold, select and release sequence was replayed
  from Basic Training to Fight Boss and back. The same menu change closes the Steam Achievements panel before moving on.
  Windows input automation did not deliver the keys in the live test, so a
  physical keyboard and controller check remains open.
- Console menus fit their contents with tighter spacing and at most one scrollable block in the right pane. The frame
  and its corners stay fixed. The scrollbar uses the active game theme. The new-game introduction and all seven ending
  screens were replayed at all three scales with the fitted viewport at 960 x 544.
- The console layout keeps the sidebar junctions aligned with the outer frame at every scale. Home, offline progress,
  and the save pickers fit at 960 x 544 and 1280 x 720. Scripted verification captured the 58 menus at all three
  scales in 960 x 544, 1280 x 720 and 1920 x 1080, plus the Dark, Gold, and Dark Gold themes at 120 % in 960 x 544.
  All 696 captures registered zero or one scrollable content block.
- Switching the console layout off restores Settings Page 2 pixel for pixel with the same preferences. In the
  Windows harness, a save and reload in Slot 2 at 120 % left the existing Slot 5 byte-for-byte identical.
- With a late-game save at 1280 x 720 and a 60 Hz Windows output, two short Inventory runs at 110 % averaged
  around 59.9 frames/s after startup. Their longest frame intervals were 23.4 ms and 22.8 ms. The earlier
  150 ms stalls did not recur in these runs.
- The wheel and thumb reached the same final Basic Training row. The wheel over a fixed area left the right pane
  pixel-identical in Basic Training and Inventory. The right stick scrolled Wishes. Nine Rust tests and the native
  comparison suite (`tools/regress.sh`) passed.
- The mouse wheel reached the final Basic Training row in the Windows GPU window, and the in-game Off button restored
  Settings Page 2. A physical gamepad on this version and the new console mode on the Vita remain to be verified.
- The maintainer's first playtest of the earlier builds found: stray bolts, wrong selection mark, useless scrollbars, no
  scale setting; all fixed in this version, to be re-checked by him.

## Desktop verification on 2026-11-21

- Windows starts at the three-button home screen. The enlarged autosave action
  starts without a focus tint. The home screen credits vallfrr for the port
  alongside the original game and art credits. Load Game and Create New Game
  open separate slot pickers, with zero stats and inactive gameplay controls.
  Creating a game in an empty slot writes its timestamp and character name.
  Occupied slots are cleared with two inline confirmations. Save Game selects
  the loaded numbered slot, and the autosave remembers which slot it came
  from. Returning home preserves the saved data already on disk.
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
  stays above Save Game and is clamped to the visible screen. The milestone
  grid was previously checked in Normal, Dark, Gold and Dark Gold using
  a local fixture with the original gold-theme entitlement.
- The tooltip pass uses the visible viewport for the tutorial, hover and
  timed messages. Standard scene elements retain their clipping rules so
  menus moved offscreen stay hidden.
- **Info 'n Stuff → Achievements** has the original **1–108** and
  **109–216** tabs, followed by **Steam Achievements**. The Steam tab
  opens the 61-milestone grid with two pages. The Windows flow checked both
  BP tabs, milestone details, pagination, Back, and leaving for Inventory
  or Save Game. The former player-sidebar shortcut was removed.
  Mouse and scripted focus select both earned and locked milestones.
  The detail pane shows the original name and unlock condition. A clean
  profile had zero unlocked and the boosted test save unlocked 44. A separate
  fixture that meets all original conditions yielded all 61 IDs in the native
  runtime. The 153 BP achievements still use the original game page.
- Save and Load draw the original sidebar bolts over their shared frame.
  Achievement icons fill their tile frames and the detail pane uses the same
  themed frame around its larger icon. Selection uses the save picker color
  inside the icon frame.
- All 121 noninteractive scene sliders use one renderer-side interpolation
  path. It draws between the original timer samples, preserves forward
  progress through a cycle, and lets damage reduce a health bar smoothly.
  The Inventory and Cards drag previews use the latest mapped pointer
  position and stay beneath tooltips. Held scrolling and the color picker
  accept mouse movement between game ticks. The controller's right stick
  scrolls by elapsed time, with discrete navigation in dropdowns.
  The fixed-step simulation still owns combat, resource gain, and saving.
- Native screenshots covered all 58 menu indices at 1280 × 720, 400 × 900,
  and 1800 × 270. The unused portrait bands stayed black. Pixel checks
  sampled the Basic Training bar every 5 ms and moved Inventory and Cards
  drag previews without advancing the simulation.
- On the 200 Hz Windows display, GPU runs at 1280 × 720 with VSYNC enabled
  had median frame rates of 197.7–199.8 FPS across Basic Training, Inventory,
  Cards, and Wishes. Fixed game updates still produced longer intervals in
  these runs. The slowest measured interval was 31.8 ms.
- An earlier Windows 1280 × 720 GPU test reported around 60 FPS with VSYNC
  on a 60 Hz display, verified from the window's active display mode.
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

## Browser port on 2026-11-21

- `ngu-browser` builds the shared CIL interpreter, UI and layout as a WASI
  WebAssembly reactor. The browser draws textured quads with WebGL and hosts
  the original game canvas, including the save controls and tooltips.
- Chromium captured all 55 enabled menu indices with a boosted fixture at
  1280 × 720, 250 × 900 and 1500 × 200. The wide views were compared with
  Windows captures from the same fixture. All tall captures stayed clear
  outside the game area. The two BP pages and the Steam tab use the game UI.
- Browser acceptance covered new game naming, byte-exact import from the
  desktop fixture, manual saving, 30-second autosave, second-device locks,
  offline export, and two-hour offline progression on reopening. Firefox
  also started the engine and opened the save picker. In Chromium a
  touchscreen drag moved inventory equipment between slots, and an emulated
  gamepad shoulder press changed menus.
- Web hover uses the engine's pointer enter and exit callbacks at display
  cadence. Pointer coordinates are remapped when the viewport changes.
  Holding a touch exposes the existing tooltip without activating a button.
  A Chromium test verified the original Rebirth tooltip with mouse movement,
  viewport resizing, keyboard focus, emulated controller focus and touch.
- The verified Windows Steam 1.260 manifest accepted all 115 files. The
  Compose file, startup script, asset packing and local server were checked.
  Seven save-service tests and three Rust library tests passed. The
  interpreter/native comparison passed including the Rebirth energy reset,
  input, 84 number cases, and save round trips.
  A full Docker run, a physical phone and gamepad, and a live reverse proxy
  still require verification.

## PS Vita port on 2026-11-21

See the [Vita port history and hardware audit](VITA.md) for the chronological
record, the released build's provenance, the test coverage and the acceptance
work still open. The early measurements below describe specific test builds.

- The interpreter VPK installed on a Vita and displayed the shared 960 × 544
  layout through vitaGL. Directional input and Cross opened Load Game. Touch
  loaded an autosave, displayed offline progress and entered Basic Training.
- A real autosave from the earlier Vita interpreter passed its legacy checksum,
  converted to the shared save format and loaded on the console. The original
  copy was retained. The console also wrote a subsequent autosave.
- The device log measured about 60 FPS on the idle home page and roughly 29–31
  FPS in the basic training session outside the screenshot capture. The latter
  result covers one character and one screen.
- Both engine builds passed Vita cross-compilation and structural VPK checks.
  The packer produced a separate player-supplied data archive. The console
  smoke test used the data archive already on the device.
- The translated-Rust VPK also installed, booted and opened the same autosave.
  On the offline-progress screen its logs reported about 2 FPS and about
  3 MiB free user RAM after initialization, compared with about 21 MiB free
  for the interpreter. The interpreter is the default and was reinstalled
  for testing and play.
- Naming with the Vita keyboard and suspend/resume still need hardware
  verification. See the later endurance measurements for sustained memory use.

### Vita performance benchmarks

Benchmarks used copies of an early-game save and an advanced save on the
console. Each log window covers about five seconds and reports elapsed time
between returns from `vglSwapBuffers`. These rates describe the instrumented
build's render loop. An external framebuffer-submission counter reported
27.4 FPS during one five-second sample at the standard build's title screen,
which differed from the in-process title screen rate. The newer
framebuffer measurements are reported below.
Screen captures were taken after the timing samples. The benchmark build
isolates save writes under `ux0:data/ngu/bench/save/`.

| Scenario | Initial measurement | After optimization |
| --- | --- | --- |
| Title screen | 59.94 FPS at 333/166 MHz | 59.94 FPS at 444/222 MHz, 17 ms p95 |
| Early save, Basic Training | — | 46.13–51.93 FPS at 444/222 MHz, 35–61 ms p95 |
| Advanced save, offline-progress screen | 0.27–0.28 FPS at 333/166 MHz | 13.86–14.61 FPS at 444/222 MHz, 85–86 ms p95 |
| Advanced save, Basic Training | 8.9–9.1 FPS after skipping unallocated wish updates, at 333/166 MHz | 28.84–29.06 FPS at 444/222 MHz, 50–51 ms p95 |
| Advanced save, Cooking at 444/222 MHz | 8.79–10.18 FPS, with repeated frame intervals around 850 ms | 34.98–38.78 FPS in that tuning pass, 47–51 ms p95 |
| Advanced save, Cards at 444/222 MHz | 18.19–19.58 FPS with client vertex arrays | 22.97–23.36 FPS in that tuning pass, 58–60 ms p95 |
| Initialization before first scene emission | 26.7 s at 333/166 MHz | 19.6–21.9 s at 333/166 MHz, about 15.5 s at 444/222 MHz |

The slowest advanced-save path iterated over every wish at 50 Hz. With no
working allocations, each pass spent about 200 ms on the Vita computing
bonuses for wishes that could not progress. The interpreter now bypasses
those calculations and still calls the original code for allocated wishes.
Titan checks reuse attack and defense only while evaluating the eligibility
conditions for the same character. Cooking caches the original optimal
score against the full set of recipe inputs. Changing a recipe invalidates
it. Tests compared the guarded and original paths with active and idle
wishes, both Titan auto-kill settings, and a recipe change. The existing
desktop state and save regression suite also passed.

To rerun the late-game regression tests, set `NGU_TEST_DLL` to the verified
`Assembly-CSharp.dll`, `NGU_TEST_ASSETS` to the prepared asset directory and
`NGU_TEST_SAVE` to a save with Wishes, Titans and Cooking unlocked. Run
`bash tools/capbuild.sh 12 cargo test --release -p clr --test wish_tick -- --ignored`.

The Vita host advances simulation in 30 Hz steps with bounded catch-up and
interpolates progress bars for presentation. A budget overrun can still
reduce the visible frame rate and delay input. The controller and touch
smoke tests included Adventure's dropdown, a manual save to an empty test
slot, loading a numbered slot, scrolling patch notes with the right stick,
moving an inventory item between slots with touch, and changing an ingredient
in Cooking. Inventory, Wishes, Cards and Cooking were captured from the
advanced profile.

Streaming the vertices in one buffer per frame preserved the rendered Basic
Training panel pixel for pixel. On that screen the CPU-side draw stage fell
from about 4.9 ms to 3.3 ms at the same clocks.

The low-memory translated-Rust build with the generated wish guard reached
Basic Training at 6.12–6.13 FPS on the same advanced save, with a median frame
interval of 102 ms. It reported 2 MiB free user RAM after initialization,
compared with 20 MiB for the interpreter. The full generated-Rust build at
optimization level 1 exceeded the 12 GiB compiler memory limit. The standard
VPK uses the interpreter.

### Gamepad stalls and display cadence

The early-save reproduction loads Autosave, dismisses the offline screen,
opens Fight Boss and presses right twice. The previous build stayed at
8.5–9 FPS after releasing the controller. Measurement attributed about
31–35 ms per simulation tick to pointer hit testing while a control remained
focused. The offline splash remains an active object after it is parked
outside the screen. Treating it as visible after dismissal made the same
stall recur in an intermediate build.

The Vita uses a stable focused hover until its target, position or save
screen changes. Clicks still perform live hit testing. Navigation reuses
its candidate list while input geometry is valid. The input and renderer
walks omit disabled menu canvases that the original scene has parked well
outside the viewport. The title, tooltips and Basic Training reuse the
desktop layout at 960 × 544, with the full-height training panel pinned to
its viewport when there is no scroll range.

The oldest Vita data archive omitted the Unity tag table. Without the
`Menu` tag, the original scripts place the short menus on the large-menu
anchor, 40 logical pixels too high. This hides their title behind the
resource bar and leaves a band at the bottom. Rebuild `ngu.pak` with the
current packer. The 960 × 544 host regression loads the advanced autosave,
selects each of the seven short menus with touch, and checks both the
selected menu and the visible bottom of its panel.

Achievement scans reuse the exact six resource totals during each original
scan. On the early profile the measured scan fell from about 19.8 ms to
5.2 ms. The comparison test checks the same completion flags and BP totals.
The host keeps 30 Hz simulation time across slow frames and records pending
simulation time separately from rendering and scanout.
On PC and in the browser WASM runtime, 900 steps advanced the same late-game
save's rebirth clock by 30.000 seconds. The browser test opened the save
through the title and acknowledged the lease before measuring.

The hardware benchmark samples the currently scanned framebuffer at each
vertical blank and records the vitaGL submission callback independently.
A repeated address counts as a missed refresh. Sampling errors and missed
samples are logged separately. The in-process render loop rate is another
metric. Use the scanout counts to compare performance. Screenshot windows
and the time needed for their synchronous readback are unsuitable for
frame-rate comparisons.

| Vita at 444/222 MHz | Observed scanout in a five-second window |
| --- | --- |
| Home, no save loaded | 300 changed, 0 repeated in three windows |
| Early save, Fight Boss, baseline after the two D-pad presses | About 43 changed, 260 repeated |
| Early save, Fight Boss, after the input fixes | 247–259 changed, 41–53 repeated in steady windows |
| Advanced save, Inventory | 143 changed, 158 repeated, about 29 FPS |
| Advanced save, Adventure | About 31 FPS during a sampled window |
| Advanced save, Beards | About 23 FPS during a sampled window |
| Advanced save, Wishes | About 26 FPS during a sampled window |
| Advanced save, Cards | About 15 FPS in a window with a 590 ms frame |

The longest measured D-pad dispatch in the early-save reproduction fell
from about 141.5 ms to 7.4 ms. Those timings measure the host input section,
before simulation, drawing and the display queue. They cannot establish
two-refresh input-to-display latency. The advanced character still misses
the 60 Hz target. Dense menus need both simulation and draw-list work.

The Vita writes autosaves from a worker using a bounded queue, then records
which slot the autosave belongs to. Explicit file reads and save-menu
operations wait for the write to finish. For a 244,556-byte save, a typical
worker write took roughly 106–140 ms. The simulation still prepares and
serializes the save on the game thread.

The first advanced-save hardware run captured 25 screen visits, then cycled
Basic Training, Fight Boss, Adventure, Inventory, Wishes, Cards and Cooking.
The journal recovered after reconnecting reached 42 minutes 37 seconds of
simulation. A stretch starting after its first menu sweep recorded 1,803.2
seconds of wall time and 1,803.23 seconds of game time. Heap use stayed
between 97 and 98 MiB over that stretch, with 20 MiB of free kernel user
memory and about 71 MiB of vitaGL video memory free. The console continued
to run after losing the network connection. Its display counter wrapped
twice. The updated sampler subtracts the counter modulo 65,536.

After reconnecting, the verified archive was installed and the old archive
retained on the device. The early-save input sequence was repeated three
times. In steady Fight Boss windows about 53–54 frames reached the display
per second, with a maximum of about 7.5 ms spent dispatching the two D-pad
presses. The new-save test created a numbered save, passed the opening
screen and reached Fight Boss after two right presses, with about 53–54
displayed FPS in the final steady windows.

The short menus were then captured on the console with the updated archive.
The seven 960 × 544 PC/Vita pairs all show the right pane reaching the
screen bottom. The follow-up soak cycled seven gameplay screens on the same
hardware and fully unlocked save. It recorded 1,828.3 seconds of wall time
and 1,828.2 seconds of game time. In windows without a screenshot, 53,160
refreshes had a changed framebuffer and 54,919 repeated the previous one.
There were no display sampler errors or gaps. After initial page and
texture loads, heap use was about 94 MiB, with 20 MiB of free kernel user
memory at the end. In the gameplay windows, median CPU time per
render-loop iteration was 20.8 ms for simulation, 2.7 ms for layout,
5.4 ms for draw-list generation, and 2.7 ms for draw submission.

A later benchmark also measured latency from the host's controller
dispatch to the scanout of the buffer containing the changed focus.
All 12 sampled changes on the advanced save exceeded two display periods.
The worst measured change took 141.2 ms. This metric excludes the time
between physical contact and the next host poll. On the same build the
external submission counter reported 60.0 FPS at the idle title, where
the scanout sampler observed 300 changed frames per five seconds.

The later build compiled repeated boss-threshold checks to direct Rust
field reads and skipped idle loadout keyboard polling. A focused
interpreter comparison covered the difficulty thresholds, wrapping
integer inputs and both shortcut keys. Four PC trials with 3,000 game
ticks measured 2.46–2.55 s before those changes and 2.41–2.47 s after.
On Vita, the later build displayed about 28 FPS in steady Cards windows.
The 30-minute run used the preceding build. Neither meets the required
gameplay cadence.

The full PC save was copied into the Vita's physical slot 5. Its downloaded
copy matched the PC original byte for byte. The Vita's personal autosave also
matched its backup immediately after installing and checking slot 5. The
numbered slot directory was empty before the copy. All timed gameplay used
a separate benchmark profile.
The standard preview VPK from the 30-minute run was installed afterward.
Its Load Game screen showed the full save in slot 5 and slots 1–4 empty.
After a later restart, three external submission counts at the idle title
were 59.8–60.0 FPS. A second check found a more recent personal autosave
containing roughly 55 seconds of live progress plus offline progress.
Both copies were kept, with the newer one left in place. The console was
left at the title.

On that separate profile, touch moved a Power Boost between two inventory
cells. A numbered save was written on the Vita and reloaded there. The PC
also read it and confirmed the same boss and unlock flags for Wishes,
Cards and Cooking. The right stick scrolled the offline patch notes. The
Adventure dropdown opened with touch and selected Tutorial Zone with the
D-pad and Cross. The resulting fight and drops appeared in the log.
Suspend/resume on hardware remains unverified.

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
