# Project status

The Windows game, browser preview and Vita host build from the shared engine. Their behavior is under active verification.

- The Windows port provides the game window, save slots, controller navigation and console mode. See [console mode](CONSOLE-MODE.md) for coverage.
- The browser runs the engine in WebAssembly and stores a personal profile on the host. Its repository documents setup and save handling.
- The Vita host builds a VPK and supports controller and touch input. The physical console's frame-rate target is open. See [Vita](VITA.md).
- Engine and platform checks run on GitHub. Run [the documented checks](TESTING.md) locally.
- The generated Rust and interpreter pass selected differential scenarios. Both share runtime components. [Fidelity](FIDELITY.md) lists the remaining comparisons to the original.

The intended work sequence is in [roadmap](ROADMAP.md). Historical evidence remains in [the archive](archive/STATUS.md).
