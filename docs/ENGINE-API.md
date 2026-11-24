# Engine interfaces

The current interfaces are in transition. Their available functions and memory ownership are described in [porting](PORTING.md). The native entry points are implemented in `clr/src/capi.rs`; the browser repository owns its WebAssembly adapter.

Initialize one engine instance on the engine thread. The C host calls input, simulation and draw-list functions on that thread. Render buffers and texture pixels returned by the engine are borrowed; copy any data needed by another thread before another engine call. A host supplies the original game data prepared with tools from the same engine revision. User saves remain in its chosen local profile directory.

Engine consumers pin a tag and an exact revision. See [the engine pin decision](adr/0001-pin-engine-consumers.md). The documented interface is an inventory of the existing integration, not yet the complete stable platform contract planned in the roadmap.
