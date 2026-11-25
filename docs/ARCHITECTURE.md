# Engine architecture

The engine loads the player's `Assembly-CSharp.dll` and prepared scene.
The interpreter executes the assembly and implements the Unity and .NET calls
the game needs. The platform host supplies input, elapsed time and presentation.

| Crate | Role |
|---|---|
| `clr` | Interpreter, Unity-compatible scene, game UI, rendering and save integration |
| `cil-interp` | Instruction and evaluation primitives used by the interpreter |
| `platform-api` | Engine interface for platform hosts |
| `ngu-desktop` | PC window, audio and input |
| `ngu-headless` | Reference host without a window |
| `save-format` | Save file encoding and validation |

## Console mode (presentation layer)
See `docs/CONSOLE-MODE.md`. Engine side: `clr/src/console_mode.rs` (settings, scale, controller input, scrolling), the vertical fit in `clr/src/desktop_layout.rs` (`console_vertical`), clips and scrollbars in `clr/src/render.rs`. It only changes presented rectangles (`view_xf`), never the game's own transforms or logic.

## Platform interface
A port drives the engine through `platform-api::Engine` (Rust, implemented by `clr::session::Session`) or the C ABI (`clr::capi`). The interface, its contract and the stability policy are in [ENGINE-API.md](ENGINE-API.md); the decision is [ADR 0009](adr/0009-platform-interface.md). `crates/ngu-headless` is the reference host and `templates/port` the starting point for a new one.

## Verification

[Local game replays and .NET fixtures](TESTING.md) check the interpreter.
[Comparison notes](COMPARISON.md) explain the separate tests against the original game.

## Vita runtime

The VitaSDK host owns the vitaGL context, VBlank synchronization, touch,
controller polling and the system keyboard. It calls the shared engine
through `clr/src/capi.rs`. The engine keeps the original 960 × 600 game
coordinates and projects the right-side layout across the native 960 × 544
display. The input path maps touch and focus through that same projection.
The renderer emits clipped quad batches. The host streams vertices and
uploads textures as they are needed.

The game thread owns the VM and scene. It polls input per render iteration
and advances the game with 30 Hz fixed steps and a bounded amount of
catch-up work per iteration. The port retains accumulated simulation time
across slow rendering. The Vita worker writes completed autosave data
atomically. Preparing and encoding the save still runs on the game thread.
The benchmark associates a focus change with its scanout buffer and records
time spent in each host stage.
