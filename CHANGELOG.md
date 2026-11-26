# Changelog

## [Unreleased]

- Run the original assembly through the interpreter on all builds. Remove the translator and use local replay references to check saves, game state and number formatting.

- Add the PC window budgets (`budgets.json`) and `tools/pc_budget.py` / `just pc-perf` to check them (manual: needs a display and game data).
- Make the host-leased save mode generic: `browser_*` and `import_browser_save` become `hosted_*` and `import_hosted_save`, and `Session` now exposes them, with `text_field` and `take_resolution_request_after`, so a port no longer needs `Session::vm()`. Migration: rename the calls.

- Remove the compile-time `target_os = "vita"` code from the engine. A host chooses its tuning at run time with `host_platform::set_host_tuning` or the C function `ngu_set_host_tuning` (background autosave, single-writer storage, startup fonts worker, layout timing). `ngu_layout_us` is always available. Migration: a Vita-like host calls `ngu_set_host_tuning(15)` before `ngu_init`; the environment variables `NGU_VITA_SAVE_DIR`, `NGU_VITA_PROFILE` and `NGU_TEST_VITA_ASSETS` become `NGU_SAVE_DIR`, `NGU_PROFILE` and `NGU_TEST_PREPARED_ASSETS`.

- Add `Session::run_script` and `Session::inspect` (menus, visible texts, reachable controls) so a port's own tests no longer need `Session::vm()`.

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
