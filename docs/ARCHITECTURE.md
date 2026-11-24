# Architecture: pipeline of the generated Rust core

Actual state of the code (not the target; the target is in `docs/VISION.md`).

```
Assembly-CSharp.dll (the player's copy)
        │  clr::Program/Vm::lower   (resolved IR: ops, types, fields, external signatures `ExtMeta`)
        ▼
  ngu-ilgen  ──►  gen.rs         Rust code: one struct per class, one fn per method, registry
                  ext_stubs.rs   BCL/Unity calls not hand-written -> bridge to the engine
                  entry.rs       test entry point
                  coverage.txt   untranslated methods + reason
        ▼  (not versioned: crates/ngu-gen/src/*.rs are generated and git-ignored)
  ngu-gen (binary) = gen.rs + ngu-rt (BCL) + ngu-host (bridge) + clr (Unity-compatible engine)
```

## Crates
| crate | role |
|---|---|
| `clr` | CIL interpreter **and** Unity-compatible engine (scene, UI, layout, rendering, input, focus, dev mode). Serves as the test oracle and, for now, as the engine. Hooks: `Vm::native_inv` (redirects `invoke` to the generated Rust), `Vm::native_mode` (`construct` no longer runs the interpreted constructor), `Op::LdtokenData` (array initializer data). |
| `ngu-rt` | open runtime: `Str/Arr/List/Dict/Gc/Obj`, .NET number formats and culture, BCL (`ext.rs`), Unity value types (`unity.rs`), x86 float->int conversions (`f2i32/f2i64`), `init_array`. |
| `ngu-ilgen` | the generator. |
| `ngu-host` | typed <-> dynamic bridge: `ToV/FromV`, `Value::Obj` shells bound to generated instances, calls to engine natives by `Type::Method` key, exceptions, delegates, serialization (`deep`). |
| `ngu-gen` | binary: generated code + host (`game`, `window`, `fuzz`) + unit bench. |
| `ngu-desktop` | PC window (minifb software window, `gl.rs` miniquad GPU window): mouse, keyboard, gamepad (gilrs, `pad.rs`), fixed logic step, present; `ngu-window` binary = interpreter-only GPU window for UI work. |

## Console mode (presentation layer)
See `docs/CONSOLE-MODE.md`. Engine side: `clr/src/console_mode.rs` (settings, scale, controller input, scrolling), the vertical fit in `clr/src/desktop_layout.rs` (`console_vertical`), clips and scrollbars in `clr/src/render.rs`. It only changes presented rectangles (`view_xf`), never the game's own transforms or logic.

## Platform interface
A port drives the engine through `platform-api::Engine` (Rust, implemented by `clr::session::Session`) or the C ABI (`clr::capi`). The interface, its contract and the stability policy are in [ENGINE-API.md](ENGINE-API.md); the decision is [ADR 0009](adr/0009-platform-interface.md). `crates/ngu-headless` is the reference host and `templates/port` the starting point for a new one.

## Representation
- Game class: `Option<Gc<Cn>>`; fields `fN` in `clr` slot order; `__h` = handle to the engine shell.
- Statics: `stN()` initialised (and runs the `.cctor`) on first use.
- Enums = `i32`; `bool/char/byte/short` normalised to `i32` on the stack, converted back when stored.
- Boxing: always to an engine `Value` (`boxv`); `obj_cast` for `isinst/castclass`.
- `ref/out`: `*mut T`, function body in an `unsafe` block.
- Blocks: `loop { match pc { ... } }` state machines, carry variables `x{depth}_{type}` between blocks.
- Exceptions: panics carrying `Exc(Value)`, try/catch/finally by `catch_unwind` regions; `panic = "unwind"` is required (profile `game`).

## Engine bridge
- Engine -> game: every `Vm::invoke(mid, args)` goes through `gen::hook` -> one arm per translated method (ids = `clr` ids); `Value` arguments become typed values (`FromV`), the result returns as `Value` (`ToV`).
- Game -> engine: a Unity/BCL call not hand-written becomes `ngu_host::native("Type::Method", targs, args)` calling the same native function as the interpreter.
- Objects: the scene loader creates `Value::Obj` shells; on first encounter `FromV` builds the typed twin (generated constructor + import of serialized fields). Game state is authoritative on the Rust side.
- Known limits: arrays/lists/dictionaries are **copied** across the bridge (no aliasing with the engine); native calls with `out/ref` parameters are not bridged.

## Verification
- `tools/regress.sh`: oracle vs native battery (see `docs/AUDIT-ILGEN.md`).
- `tools/ilall.sh`: regenerates the whole game, `cargo check`, error summary.

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

The released Vita preview uses the shared CIL interpreter. The generated
engine can be linked through a separate Vita entry point. Its device results
and the reason for keeping it experimental are in the
[Vita port history](VITA.md#build-experiments).

## Build (machine constraint)
The generated crate is ~20 MB and rustc needs >10 GB. Always `tools/capbuild.sh 12 cargo ...` (cgroup memory cap, no swap) or `tools/capbuild.ps1` on Windows, with the target directory on a data drive (never the system drive). `ngu-gen` is built with `opt-level = 0` under profile `game` (see `Cargo.toml`).
