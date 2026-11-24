# Engine API: what a port may rely on

Only what is listed here is public. Everything else in the repository, including all other `pub` items of the `clr`
crate, is an implementation detail that may change in any release. To build a port, follow [PORTING.md](PORTING.md).

## Versions and stability

The engine is versioned with immutable Git tags `v0.MINOR.PATCH`. In the `0.x` series a **breaking change to anything
listed below requires a new minor version**, a [CHANGELOG](../CHANGELOG.md) entry and a "Migration" note for ports.
Patch versions never break this surface. A port pins a tag **and** its exact revision (the tag in `engine.tag` and `Cargo.toml`, the revision in `Cargo.lock` or the submodule pointer).
There is no promise beyond `0.x`; `1.0` will come with a public stability commitment.

Generated reference documentation: `just doc` (`cargo doc --no-deps` for the crates below, warnings are errors; the CI
job uploads it as the `engine-docs` artifact; the manual `pages.yml` workflow publishes it to GitHub Pages once Pages is enabled
for the repository).

## Rust interface (documented in the code, `deny(missing_docs)`)

| item | crate | use |
|---|---|---|
| `Engine`, `DrawTarget`, `Action`, `Pointer`, `TextRequest`, `Texture` | `platform-api` (also `clr::platform_api`) | the contract between a port and the engine |
| `Quad`, `Vertex`, `Batch`, `TriangleList`, `tessellate` | `platform-api` | the frame as quads, and as vertex and batch arrays |
| `FixedStep`, `STEP_SECONDS`, `GAME_WIDTH`, `GAME_HEIGHT`, `TEXTURE_ATLAS` | `platform-api` | logic time and fixed numbers |
| `testing::ScriptedEngine` | `platform-api` | a synthetic engine for tests without game files |
| `Session`, `SessionConfig`, `SessionError` | `clr::session` | the real engine behind `Engine`; one per process |
| `Host`, `Script`, `Step`, `Report` | `ngu-headless` | the reference port and its script format |

## C ABI (`clr::capi`, declared by the port's own header, for example `nguidle-vita/engine_api.h`)

All functions are called from the one thread that called `ngu_init`; the rules for units and memory are in the module
documentation of `capi.rs`.

| function | purpose |
|---|---|
| `ngu_init(data_dir, w, h)` | load the game; `0` ok, `1` DLL missing, `2` scene, `3` save menu |
| `ngu_frame(dt, x, y, down)` | advance time and set the pointer, once per display frame |
| `ngu_pad(action)` | navigation, one ASCII code `u d l r a b p n` (`platform_api::Action::code`); returns `1` if the focus moved |
| `ngu_scroll(dx, dy)` | scroll the focused list |
| `ngu_emit()` / `ngu_emit_seq()` | build the frame, return its batch count / counter of built lists |
| `ngu_batches()` / `ngu_vertices()` | the frame as `NguBatch` (32 bytes) and `NguVertex` (20 bytes), borrowed |
| `ngu_tex(id, w, h)` / `ngu_tex_release(id)` | pixels of a texture / free the engine's copy after upload |
| `ngu_atlas(dirty, size)` | the glyph atlas (texture id 1) and whether it changed |
| `ngu_text_request(buf, cap, max, numeric)` / `ngu_text_submit(text, ok)` | text fields |
| `ngu_clock_us`, `ngu_frame_info`, `ngu_layout_us`, `ngu_autoplay_load` | host diagnostics and scripted measurements (not needed by a port) |

## Data and tools

* **Prepared data**: a folder produced by `tools/nguprep.py` from the player's installation, with `Assembly-CSharp.dll`
  copied in, or the single `NGUPAK1` file produced by `tools/pack.py` (`ngu.pak`). It is only valid for the engine
  revision whose tools produced it. Commands: [PORTING.md](PORTING.md), step 3.
* **Tools** a port may call: `tools/verify_game_data.py`, `tools/nguprep.py`, `tools/pack.py`, `tools/new_port.py`
  (`just new-port`), `tools/fetch-achievement-icons.py` (optional). Their command lines are stable within `0.x`.
* **Saves** are the game's own files in the save folder; the format is the game's, readable by `save-format`.

## Migration notes
- `0.5.0`: new `Engine` / `Session` interface, documented C ABI, port template. Nothing removed: ports built on `0.4.0`
  keep working and may move to `Session` when convenient.
