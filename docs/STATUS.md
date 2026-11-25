# Project status

The Windows game, browser preview and Vita host build from the shared engine. Their behavior is under active verification.

- The Windows port provides the game window, save slots, controller navigation and console mode. See [console mode](CONSOLE-MODE.md) for coverage.
- The browser runs the engine in WebAssembly and stores a personal profile on the host. Its repository documents setup and save handling.
- The Vita host builds a VPK and supports controller and touch input. The physical console's frame-rate target is open. See [Vita](VITA.md).
- A platform interface (`platform-api`), a safe host (`clr::session`), a headless reference port (`ngu-headless`) and a port template (`just new-port`) are in the engine; their loop is tested in CI against a synthetic engine and, locally, against the real game. See [porting](PORTING.md).
- Engine and platform checks run on GitHub. Run [the documented checks](TESTING.md) locally.
- The interpreter passes selected scenarios against its local baseline. [Fidelity](FIDELITY.md) lists the remaining comparisons to the original.

The intended work sequence is in [roadmap](ROADMAP.md). Historical evidence remains in [the archive](archive/STATUS.md).
