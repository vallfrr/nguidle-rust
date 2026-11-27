# Changelog

## [Unreleased]

- Fix: `GetComponentInChildren` goes on into the children of an inactive object, like the original, so that the energy purchases and beast quest menus open on an early save without an exception (ADR 0015). `OfflineProbe.cs` can open menus (`NGU_OFFLINE_PROBE_MENUS`).

- An old Vita save (the earliest internal format) put in the `import` folder is converted when it is imported from the save screen (D5); `legacy-save` is now a default feature. `clr convert-legacy-save` still works. Not tried end to end on a real old save (only the converter with a synthetic one).


- Fix: a float `+ - * /` directly followed by an integer conversion is computed in double precision, like the original runtime. `(int)((float)22560 * 0.9f)` is 20,303 in the original and was 20,304 here, which shifted the beast-quest timer by one second per threshold passed (ADR 0014). Found by comparing offline progress with the original.

- Add `tools/guard-frames.sh` (what each interpreter shortcut is worth per frame on the console scenarios) and the evidence table of `docs/GUARDS.md`.

- `NGU_GUARD_OFF=3,12` disables only those interpreter shortcuts; `NGU_GUARD_TABLE=1 tools/longrun.sh` plays once per shortcut with only it disabled and prints what each is worth (extra interpreted instructions, time) and whether the final save changes.
- `NGU_GUARD_STATS=1 clr <dll> play ...` prints how many times each interpreter shortcut answered; `tools/longrun.sh` shows it for its guarded run.
- Add the offline probe and `compare_saves.py` to compare the original and the port field by field after opening a save (see `tools/oracle/README.md`); the offline progress and the shortcut-free long run match, see `docs/FIDELITY.md`.
- Add the full-game script: `just full-game [hours]` (`clr <dll> fullgame <prepared> [report] [hours]`) plays every menu, a long progression, saves, offline progress and every console-mode element on the real game and prints OK / KO / SKIP per step. `Engine::exceptions` counts the game exceptions reported. `tools/fullgame-wasm.sh` plays it as WebAssembly (the web build), `tools/longrun.sh` runs the long game with and without the interpreter shortcuts and compares the final saves, and the developer steps `open` and `clock` open a save like the game and set its clock.

- Add an "Open saves folder" button, top right of the save and load screens of the PC app. It shows the saves folder in the system file manager. A host opts in with `vm.eng.open_folder` (the web and Vita hosts do not, so they do not show it).

- Remove the Python preparation tools (`tools/nguprep.py`, `tools/pack.py`, `tools/verify_game_data.py`, their tests and `tools/requirements-prep.txt`). `ngu-prepare` (crate `ngu-prep`) does their jobs: `verify GAME_FOLDER`, `export GAME_FOLDER OUTPUT` (prepared folder), `vita` and `web`, next to `pc`. `just prepare-data` and `just verify-data` call it; `just pack-data` becomes `just vita-data`. Migration: pass the NGU IDLE folder (with `UnityPlayer.dll`, needed to decode the sounds), not only `NGUIdle_Data`; the optional icon script now needs only `tools/requirements-icons.txt` (Pillow).

- Run the original assembly through the interpreter on all builds. Remove the translator and use local replay references to check saves, game state and number formatting.

- Add the PC window budgets (`budgets.json`) and `tools/pc_budget.py` / `just pc-perf` to check them (manual: needs a display and game data).
- Make the host-leased save mode generic: `browser_*` and `import_browser_save` become `hosted_*` and `import_hosted_save`, and `Session` now exposes them, with `text_field` and `take_resolution_request_after`, so a port no longer needs `Session::vm()`. Migration: rename the calls.

- Remove the compile-time `target_os = "vita"` code from the engine. A host chooses its tuning at run time with `host_platform::set_host_tuning` or the C function `ngu_set_host_tuning` (background autosave, single-writer storage, startup fonts worker, layout timing). `ngu_layout_us` is always available. Migration: a Vita-like host calls `ngu_set_host_tuning(15)` before `ngu_init`; the environment variables `NGU_VITA_SAVE_DIR`, `NGU_VITA_PROFILE` and `NGU_TEST_VITA_ASSETS` become `NGU_SAVE_DIR`, `NGU_PROFILE` and `NGU_TEST_PREPARED_ASSETS`.

- Add `Session::run_script` and `Session::inspect` (menus, visible texts, reachable controls) so a port's own tests no longer need `Session::vm()`.

- Rename the `legacy-vita-save` feature to `legacy-save` and the `convert-vita-save` command to `convert-legacy-save`: the engine carries no platform name. Migration: use the new names.

## [0.5.0]

- Add the documented platform interface (`platform-api::Engine`, `Action`, `Pointer`, `MouseButton`, `FixedStep`), including keys, the mouse wheel, secondary buttons and `reset_input` and `clr::session::Session`, a safe implementation that hosts the whole game.
- Document the C ABI function by function with its threading, units and memory contract; both the C ABI and the interface enforce `missing_docs`.
- Add `ngu-headless`, a reference port that runs in CI against a synthetic engine (`platform-api::testing::ScriptedEngine`) and locally against the real game.
- Add the port template (`templates/port`), `just new-port` and `just doc`; the template is built, linted and tested by `just check`.
- Replace the porting notes with a step-by-step guide and publish the public surface and migration notes in `docs/ENGINE-API.md`.
- Add a manual workflow (`pages.yml`) that publishes the reference documentation to GitHub Pages.
- The desktop host now steps time with `FixedStep`; the web port runs on `Session` (the Vita stays on the C ABI for its time slicing).
- Migration: none required; hosts built on `0.4.0` keep working.

## [0.3.0]

- Match Unity 2019's floating-point random range interpolation for the tested reference vectors.
- Match the tested seeded Mono `System.Random` sequences and bounded ranges.
- Preserve Cooking UI updates while other menus are selected.
- Add local and GitHub check recipes for the workspace and synthetic save fixtures.

## [0.2.0]

- Bound decoded save records and verify compressed null runs before expanding them.
- Keep Vita time slicing off by default and read experimental timing settings once.
- Remove the unused C exports `ngu_texture_count` and `ngu_dev_size`.

## [0.1.0]

- Establish the first versioned engine source for the desktop, web and Vita checkouts.
- Move the browser adapter and Vita tooling into their respective repositories.
- Keep the existing C host, save and prepared-data contracts for downstream builds.
