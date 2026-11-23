# Engine interface for ports

Engine revisions are selected with immutable `v0.x.y` Git tags. A consuming
repository updates its tag and its exact revision together. In the `0.x`
series, breaking changes require a new minor version and a changelog entry.

## Build and prepare data

- Run `tools/verify_game_data.py` before extracting the player's Steam files.
  Run `tools/nguprep.py` from the same engine revision to prepare sprites,
  scenes, layouts and other local data.
- For a single file, `tools/pack.py` writes the `NGUPAK1` format. The port
  supplies `Assembly-CSharp.dll` from the verified installation. Generated
  sources, prepared data and user profiles stay out of the repository.
- The desktop checkout owns the translator, the shared runtime and the
  desktop executable. The web checkout owns the Wasm adapter and save
  service. The Vita checkout owns the VitaSDK host and platform tools.

## Host interfaces

- The web adapter uses the Rust `clr` library and exposes its own functions
  for input, rendering and saves to the browser. The web repository owns
  that interface and the session protocol.
- A native C host links the engine's static library and uses
  [the engine exports](../clr/src/capi.rs). Initialize with `ngu_init`,
  deliver time and input with `ngu_frame`, `ngu_pad` and `ngu_scroll`,
  and read the draw list via `ngu_emit`, `ngu_batches` and `ngu_vertices`.
  Upload textures using `ngu_tex` or `ngu_atlas`. Read and consume
  borrowed render buffers on the engine thread before calling the engine
  again. The exported structures in `capi.rs` describe their layout.
- Some current host and target glue remains in the engine pending the
  generic platform API. The existing ports are examples for the current
  interfaces. Coordinate contract changes in an engine release first.

Use the consuming repository's `just check` and build from a standalone
checkout before updating the pin.
