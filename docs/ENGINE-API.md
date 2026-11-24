# Engine interfaces

The current interfaces are in transition. Their available functions and memory ownership are described in [porting](PORTING.md). The native entry points are implemented in `clr/src/capi.rs`; the browser repository owns its WebAssembly adapter.

Initialize one engine instance on the engine thread. The C host calls input, simulation and draw-list functions on that thread. Render buffers and texture pixels returned by the engine are borrowed; copy any data needed by another thread before another engine call. A host supplies the original game data prepared with tools from the same engine revision. User saves remain in its chosen local profile directory.

## Native thread and pointer contract

Call `ngu_init` once on the thread that owns the engine. Calls to all of the
`ngu_*` entry points, including the getters, must run on that same thread.
Calls must be sequential and must not re-enter the engine. Hosts that present
on a separate thread publish copies with their own synchronization.

`ngu_emit` returns a batch count. Read the vertex and batch arrays it produced
before the next engine call that can rebuild the draw list. Copy them before
publishing them to another thread. Copy texture pixels before releasing the
texture or giving the work to another thread. Atlas pixels belong to the
renderer and may change after drawing the next frame.

Pass valid writable output pointers of the sizes documented in [porting](PORTING.md).
Text input buffers and C strings belong to the caller and must be valid until
the corresponding call returns. The public draw structures use `repr(C)`.
The Vita host owns the renderer and system dialogs separately from the Rust
engine thread.

Engine consumers pin a tag and an exact revision. See [the engine pin decision](adr/0001-pin-engine-consumers.md). The documented interface is an inventory of the existing integration, not yet the complete stable platform contract planned in the roadmap.
