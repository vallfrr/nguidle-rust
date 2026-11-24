# ADR 0009 - One documented platform interface and a port template

Status - Accepted for this refactoring.

## Context
Three ports (desktop, web, Vita) each re-implemented the same host loop: fixed-step time, input translation, press and
release timing, texture upload, text requests, sounds. The contract existed only as code and as an inventory in prose, and
no one outside the project could build a fourth port without reading the engine's internals.

## Decision
* `platform-api` holds the contract: the `Engine` trait, `DrawTarget`, `Action`, `Pointer`, `FixedStep`, and a synthetic
  `ScriptedEngine`. It depends on nothing, so a port can be tested against it alone. `#![deny(missing_docs)]`.
* `clr::session::Session` is the real implementation of `Engine`, with no process-global state of its own and a safe,
  documented API. The C ABI (`clr::capi`) stays as it is for C hosts and the Vita scheduling, and is documented
  function by function (`deny(missing_docs)`). The two are not merged: the Vita host depends on its timing behaviour, and
  changing it needs hardware measurements (lot 8).
* `ngu-headless` is the reference port: it runs in CI with the synthetic engine and locally against the real game.
* `templates/port/` and `just new-port` generate a compiling port with CI and instructions. The template is built,
  linted and tested in `just check`, so it cannot rot.
* The public surface and the version policy are listed in `docs/ENGINE-API.md`; anything else is internal.

## Consequences
* Existing ports keep working unchanged. Moving the desktop, web and Vita hosts onto `Session` is optional follow-up work,
  to do when each is next changed and its behaviour can be re-verified.
* `Session` handles pointer and navigation input like the browser adapter; console-mode input and the Vita's time slicing
  remain host features until they are moved behind the interface.
* A change to the listed surface needs a new minor version.
