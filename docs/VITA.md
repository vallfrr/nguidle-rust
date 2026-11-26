# PS Vita port

## Current preview

[Performance Preview 2](https://github.com/vallfrr/nguidle-vita/releases/tag/vita-performance-preview-2)
is the standard VPK installed after the endurance test. It uses engine revision
[`cf675cd`](https://github.com/vallfrr/nguidle-rust/commit/cf675cd) and Vita host
revision [`2a80edb`](https://github.com/vallfrr/nguidle-vita/commit/2a80edb).
Install it with a locally prepared `ngu.pak` from the verified Steam Windows
1.260 data. The engine and LiveArea placeholders are in the VPK. The game data
and saves are separate.

The Vita uses the shared desktop scene at **960 × 544 (30:17)**. Drawing, touch
and controller focus use the same responsive geometry. The standard VPK runs
the original game assembly in the shared Rust CIL interpreter. A separate
translated-Rust build also boots on hardware. See [Build experiments](#build-experiments)
for the performance and memory results of that alternative.

The standard preview returned to the title after verification. Three idle-title
submission counts taken after a fresh launch were 59.8–60.0 FPS (historical; see the reproducible measurements below). The persistent
D-pad freeze is fixed. Late-game menus still miss refreshes, and the focus
feedback test exceeded the two-refresh target.

## Development history

| Stage | Implemented and observed |
| --- | --- |
| November 19, 2026 · toolchain | A minimal Rust `std` VPK cross-compiled with nightly and VitaSDK for `armv7-sony-vita-newlibeabihf`. It installed and launched on hardware, and the console log confirmed that Rust `std` ran. The colored framebuffer in this probe had no visual capture. The first Unity extraction established the scene and asset pack, with 6,730 game objects in the prepared scene. |
| November 20 · first playable VPK | V1 added a C host, vitaGL rendering, a Rust static library and the external scene and texture pack. It installed, started and produced a TGA capture. The original 960 × 600 logical canvas was fitted into the Vita display with side bars. Startup took around 25 s and the render loop ran around 18–19 FPS at 333/166 MHz. The console checks at this stage covered boot and rendering. |
| November 21 · shared UI and saves | The Vita switched to the desktop layout, save picker, game tooltips and shared focus navigation. D-pad and Cross opened Load Game, touch loaded an autosave and entered Basic Training, and the console subsequently wrote an autosave. An earlier Vita save was converted after checksum validation and successfully loaded on hardware. |
| November 21 · memory and late-game work | The host added benchmark and callback-profiling builds, a bounded texture cache with reload support, and a single streamed vertex buffer per frame. It moved from 333/166 MHz to 444/222 MHz for the default CPU/GPU clocks. The engine optimized idle Wishes, Titan eligibility checks and repeated Cooking recipe evaluation. Both runtime variants were built and tried on hardware. |
| November 21 · stall reproduction | The older build stayed near 8.5–9 FPS after Load Autosave → dismiss offline progress → Fight Boss → Right twice. While focus remained in place, pointer hit testing repeatedly walked the scene and cost about 31–35 ms per game tick. An interim build still hit the slow path because the dismissed offline window remained active in the scene and had been moved offscreen. |
| November 21 · stall fix and diagnosis | Stable controller hover reuses its hit target. Directional focus reuses the candidate list while its geometry is valid. Real clicks still perform a fresh hit test. The renderer and input walks skip disabled menus parked outside the viewport. The offline window is tested against the actual viewport. More work went into resource totals for achievements, interpreter field loads, immutable text caching, rotation and timer dispatch. Autosave file writes moved to a bounded worker. |
| November 21–22 · corrected archive and endurance | The oldest pack omitted Unity tags. Small menus were centered about 40 logical pixels too high, so their titles fell underneath the resource strip and their panels left a black band at the bottom. The tag-complete archive was checked and installed after retaining the old one. Seven PC/Vita pairs verified the corrected panels. The follow-up isolated run covered 30 min 28 s with the full save. The standard VPK was then installed for play. |
| November 22 · latest code benchmark | A later build reduced repeated boss-threshold and loadout polling work. A scanout probe timed the frame with the changed focus. It observed 12 focus changes beyond two refreshes and a 141.2 ms maximum on the advanced save. The full endurance run used the earlier engine revision published in Preview 2. |

The matching engine commits are [shared layouts and legacy saves](https://github.com/vallfrr/nguidle-rust/commit/62d56b3),
[late-game simulation](https://github.com/vallfrr/nguidle-rust/commit/bf9acc3),
[hover and autosave changes](https://github.com/vallfrr/nguidle-rust/commit/33a873a),
and [boss and loadout calls](https://github.com/vallfrr/nguidle-rust/commit/71622e2).
The host milestones are [V1](https://github.com/vallfrr/nguidle-vita/commit/126a3cb),
[shared UI and controls](https://github.com/vallfrr/nguidle-vita/commit/9a3928b),
[deployment fixes](https://github.com/vallfrr/nguidle-vita/commit/73373d4),
[rendering and profiling](https://github.com/vallfrr/nguidle-vita/commit/15ebed4),
[display sampling](https://github.com/vallfrr/nguidle-vita/commit/2a80edb),
and [focus latency measurement](https://github.com/vallfrr/nguidle-vita/commit/7a97153).

## What was tested

| Area | Hardware result | Coverage still needed |
| --- | --- | --- |
| Startup and presentation | VPK validation, installation and boot, the title, Load Autosave, offline progress and the save picker. Seven full-height panels confirmed at 960 × 544 against PC captures. | Readability across the remaining dense states on the physical screen. |
| Layout and navigation | The seven PC/Vita comparison pairs cover Basic Training, Augmentation, Advanced Training, Time Machine, Blood Magic, Wandoos and NGU. An earlier wide sweep captured 25 screen visits. The advanced-save endurance loop cycled Basic Training, Fight Boss, Adventure, Inventory, Wishes, Cards and Cooking. | Full hardware coverage of every subpage, modal, scroll region and control style. A separate host test covers focus targets across 56 enabled menu indices. |
| Buttons and touch | Three early-save reproductions of the two Right presses. A fresh game created a test slot, passed the opening screen and reached Fight Boss after two Right presses. Touch moved an inventory boost across slots, opened the Adventure dropdown and worked with D-pad/Cross selection. Tutorial Zone combat and drops appeared in the log. The right stick scrolled the offline patch notes. An earlier Cooking test changed an ingredient. | Complete button-only playthrough, prolonged scrolling and drag use, system keyboard naming and other IME inputs. |
| Saves | A converted V1 save loaded and autosaved. The full PC fixture reached the expected late-game menus on the isolated Vita profile. A test slot saved on Vita reloaded there and on PC, which confirmed the boss and Wishes, Cards and Cooking unlock flags. | Wider coverage of save formats, interrupted writes and recovery during suspend. |
| Personal profile | The full PC save was copied to slot 5 and verified byte for byte. Slots 1–4 were empty before and after. The profile's autosave matched its backup immediately after provision. A later check found an autosave with about 55 s of live progress plus offline progress. Both copies were kept and the newer autosave was left in place. The game was left at the title. | Keep using a separate profile for further timing and destructive save tests. |
| Timing and memory | Completed 30 min 28 s of wall time and 30 min 28 s of simulation with the advanced save. Heap use stabilized after menus and textures loaded. | Sustain 60 Hz in gameplay and during transitions, verify a first visual response within two refreshes, and test actual sleep/resume. |

The corresponding host and shared-engine tests passed. They cover the 960 × 544
menu projection and touch targets, 56 enabled focus menus, idle and allocated
Wishes, Titan auto-kill on and off, Cooking recipe changes, achievement and BP
thresholds, boss difficulty thresholds and loadout shortcuts. The interpreter
and generated-Rust regression suite compared state and save bytes. It passed
also under the French formatting locale. This locale test exercises formatting
and serialization. The game interface remains English. With the same save,
900 ticks advanced the rebirth clock by 30.000 s on PC and in the browser's
WASM runtime.

## Measurements and remaining stalls

### Reproducible measurements (lot 8)

Two runs of the scripted `tour` scenario (the whole menu tour on a new game), standard interpreter VPK, 444/222 MHz clocks,
Vita host `c65a5ab`, engine `d92a347`, package `669eae535dd415e6`. Command: `bash tools/vita-cycle.sh tour <label>` in
[nguidle-vita](https://github.com/vallfrr/nguidle-vita), then `python3 tools/vita-budget.py <folder>`.

| Measure | Run 1 | Run 2 |
| --- | --- | --- |
| Render loop | 59.9 FPS, 1 interval over 25 ms | 59.9 FPS, 3 intervals over 25 ms |
| Work per frame, p50 / p95 / p99 | 8.1 / 24.0 / 50.9 ms | 8.3 / 22.4 / 48.6 ms |
| Frames over 16.7 ms | 26.0 % | 25.9 % |
| Largest spike | 2905 ms at boot, then 427 ms in a menu | 2881 ms at boot |

The refresh rate holds, but the p99 of the work per frame is three times the 16.7 ms goal: the dense menus (a p99 of 140 to 170 ms in
the worst ones) cause the missed refreshes. The goal and the regression limit are in `budgets.json`; the reference is
`perf/tour-baseline.json`. The counters behind these figures were repaired in lot 4. Every figure below this table was
taken earlier, with earlier counters and revisions, and is **historical**: it is kept for the record and is not a current claim.

The early readings were application render-loop rates. Later readings count
framebuffer changes at hardware VBlank. A submission counter records another
part of the pipeline. One old standalone title sample reported 27.4
submissions per second. Later tests observed 300 framebuffer changes and zero
repeats per five seconds at an instrumented idle title, alongside about 60 FPS
from the external counter. After a fresh start of the standard VPK, the
external counter reported 59.8–60.0 FPS in three windows.

| Scenario | Device result |
| --- | --- |
| V1 render loop | About 18–19 FPS at 333/166 MHz, about 25 s startup and about 21–22 MiB free user RAM after initialization. |
| Initial late-game offline progress | 0.27–0.28 render-loop FPS at 333/166 MHz. Later interpreter runs reached 13.86–14.61 at 444/222 MHz. |
| Early save, Fight Boss after Right twice | About 8.5–9 FPS persisted after both releases on the old build. Later steady windows displayed roughly 53–54 FPS. The worst measured early-save D-pad dispatch fell from about 141.5 ms to 7.5 ms. Dispatch timing ends before simulation and display. |
| Advanced save, dense menus | Individual display samples were about 29 FPS in Inventory, 31 in Adventure, 23 in Beards and 26 in Wishes on the earlier build. Cards reached about 28 FPS in steady windows on the later benchmark. A Cards window on the earlier build contained a frame around 590 ms. |
| Advanced Cooking, historical renderer measurements | About 9–10 render-loop FPS before recipe caching, with repeated intervals around 850 ms. Later samples at the same 444/222 MHz clocks reached 35–39 render-loop FPS. Changing an ingredient recomputes the optimum. |
| Endurance, advanced save | 1,828.3 s of wall time and 1,828.2 s of game time. Screenshot-free windows counted 53,160 changed framebuffers and 54,919 repeats with zero sampler errors or gaps. Heap use settled around 94 MiB, with 20 MiB of kernel user memory free at the end. |
| Endurance CPU, per render-loop iteration | Median 20.8 ms simulation, 2.7 ms layout, 5.4 ms draw-list generation and 2.7 ms draw submission. The display budget is about 16.7 ms per refresh. Draw submission is measured on the CPU. |
| Advanced-save focus feedback | The frame with the new focus reached scanout in all 12 tracked samples. Every sample exceeded two refreshes and the maximum was 141.2 ms. Timing starts when the host handles the press. |
| Startup after tuning | Roughly 15.5 s from initialization entry to a successful return at 444/222 MHz. First usable title and first texture and glyph uploads are separate measurements. |

Three costs remain.

1. **Periodic stalls.** During the follow-up soak, after the initial menu
   sweep, 66 five-second windows contained a frame of at least 450 ms.
   Sixty-one of those windows recorded no texture upload. Many spikes repeat
   at roughly 30 s, matching the autosave interval. The autosave worker
   handles the atomic file write and slot bookkeeping. The main thread still
   prepares and serializes the save. Typical background writes for the
   244,556-byte fixture took roughly 106–140 ms in the earlier logs.
   Per-stage timings are still needed to isolate the cost of snapshot
   generation and serialization and explain every spike.
2. **Sustained low cadence.** The game thread owns the simulation and the UI
   scene. It spends longer than one display period on simulation alone in
   many advanced-save windows. The layout and draw list add further CPU work.
   The 30 Hz game steps still catch up over time, as the endurance clock
   comparison shows. A delayed frame can make the game feel slow while time
   and progression advance at the correct average speed.
3. **Transitions.** A first visit can load textures and change layout. A
   cached visit still builds the active screen and waits for the presentation
   queue. Touch and controller dispatch times only cover the front of that
   pipeline. The focus scanout probe measures the delay the player sees
   after the host receives the press.

The initial report described single-digit FPS after a few controller actions
and CPU and RAM meters reaching 100%. The console later reported free kernel
user and internal vitaGL memory during the measured workloads. Pools already
reserved by the application can appear full
to an external meter while the allocator can still serve requests inside the
reservation. Track both sets of readings. The 30-minute data shows no sustained
memory growth after loading the sampled menus. Direct GPU timing, CPU time
across all threads and memory behavior in other workloads remain open.

## Build experiments

- The CIL build is the default. It packages the shared engine and the Vita
  host, with thin LTO under the memory cap. The benchmark build leaves the
  original game data pack external and uses a separate profile.
- The generated-Rust VPK cross-compiled and passed package checks. It booted
  and loaded a save on Vita. An early translated run showed about 2 FPS at
  offline progress and 3 MiB free user RAM after initialization. The interpreter
  reported about 21 MiB free in that comparison.
- The translated build with the idle-Wishes guard reached roughly 6.1
  render-loop FPS in advanced Basic Training with about 2 MiB user RAM free.
  A full translated build at optimization level 1 hit the **12 GiB compiler
  memory cap**. The preview kept the interpreter, with targeted optimization
  and parity tests for individual hot methods.
- At the same clocks, replacing client vertex arrays with one streamed buffer
  per frame reduced CPU draw time in Basic Training from roughly 4.9 to 3.3 ms.
  The screenshot was unchanged in that comparison. The selected host still
  uses vitaGL. A direct GXM renderer remains an alternative if later device
  profiling identifies a GPU bottleneck.
- Package uploads were keyed by SHA-256 after fixed-name deployment could
  reuse a previously verified transfer. The host now selects the standard
  VitaSDK vitaGL archive when there is no local stock backup. Old data packs
  require repacking to include `tags.json`.
- During the first long run, the network command connection became
  unavailable. The recovered console log showed the app continued to run,
  reaching 42 min 37 s of simulated time. The display counter had wrapped
  twice. The sampler now computes the elapsed VBlanks modulo 65,536.
- TGA capture switched to a contiguous write and records the bottom origin
  used by the GPU readback. The readback itself still interrupts rendering
  and is accounted for separately during benchmarks.

## Reproduce and inspect

Follow the [Vita build and install guide](https://github.com/vallfrr/nguidle-vita)
for prerequisites, controls, data preparation and legacy conversion. From the
engine root, build the isolated benchmark in a separate output directory.

```sh
NGU_VITA_BENCHMARK=ON NGU_VITA_BUILD_DIR="$PWD/out/vita-bench-build" \
  bash nguidle-vita/build.sh
```

For testing, copy a known fixture to
`ux0:data/ngu/bench/save/NGUSaveSteam.txt`. Test a new game, an early autosave
and a copy of a full save. Capture the title and then reproduce Load Autosave →
dismiss offline progress → Fight Boss → Right twice, including time after
release. Repeat the same path three times. Compare first visits and warm
transitions at the same clock settings and with the same data pack.

Read `ux0:data/ngu/bench/host.txt` for five-second windows and
`ux0:data/ngu/log.txt` for the game log. With callback profiling enabled,
capture a separate diagnostic pass. The sampling overhead makes that pass
unsuitable for build-to-build timing comparisons. Benchmark Select captures
`ux0:data/ngu/bench/shot.tga`. Give capture readback and catch-up their own
windows, since reading and writing a TGA can stall the app. For final timing,
prefer the framebuffer change and repeat counts, then record the error and
sample-gap counters to check that the measurement stayed valid.

Set `NGU_TEST_DLL`, `NGU_TEST_ASSETS` and `NGU_TEST_SAVE` for the
fixture-based tests. The layout regression also uses
`NGU_TEST_PREPARED_ASSETS`, a prepared directory containing `ngu.pak` or the
equivalent loose data and assembly, and `NGU_SAVE_DIR`, a disposable
test directory.

```sh
cargo test --release -p clr --test wish_tick -- --ignored
cargo test --release -p clr --lib parked_canvas_pruning_preserves_focus_and_hit_targets -- --ignored
cargo test --release -p clr --lib vita_autosave_projects_small_menus_to_the_display -- --ignored
bash tools/regress.sh
```

Local audit evidence lives under `out/vitalogs/` (V1), `out/vita-bench/`
(early device experiments) and `out/vita-60/` (input, scanout and endurance).
Useful logs from the latest investigation are `baseline-run.log`,
`tagged-early-run.log`, `tagged-soak/soak-complete.log` and
`latency-hardware.log`. `tagged-soak/` also contains the seven PC/Vita
comparison pairs and the separate interaction captures. Build artifacts,
captures, personal saves and local device configuration stay outside Git.

## Remaining acceptance work

1. Time autosave snapshot creation, encoding and disk work independently.
   Move work that can operate on an immutable snapshot to the save worker.
   Verify the saved result and all slot transitions on the same fixtures.
2. Profile the main-thread simulation at the late-game bottlenecks. Optimize
   hot methods with parity tests. Reuse unchanged display geometry and draw
   work, and distribute first-visit texture loading.
3. Verify one changed framebuffer per refresh during representative play and
   transitions, and a visible focus response within two refreshes. Run three
   passes per critical scenario and a 30-minute advanced-save run on the
   release candidate. Keep screenshots, frame-time samples, memory data and
   clock comparisons.
4. Verify suspend/resume, the Vita system keyboard, remaining menus and
   controller paths, and the Vita audio output. Keep the official LiveArea
   art and any small-screen redesign as separate work.

Open Vita tasks are tracked in the GitHub milestones of `nguidle-vita`.
