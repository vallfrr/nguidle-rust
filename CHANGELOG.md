# Changelog

## [Unreleased]

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
