# nguidle-rust

Free port of **NGU Idle** (by 4G, Unity 2019.4) to Rust: the game's C# code is **converted to Rust from your own copy of the game** and runs on a Unity-compatible engine written in Rust. This repository contains the engine, the conversion tool and the PC version. Other platforms build on it: [`nguidle-web`](https://github.com/vallfrr/nguidle-web) (self-hosted web version) and [`nguidle-vita`](https://github.com/vallfrr/nguidle-vita) (PS Vita).

> **No game file is provided or published here** (no code, images or texts). You must own the game (free on Steam): the conversion reads `Assembly-CSharp.dll` and the data of your installation on your machine.

## State (in development)
- C# -> Rust conversion: 5,327 / 5,359 methods (99.4 %); behaviour **identical** to the original, checked by automatic comparison with a reference interpreter (visible state, saves, thousands of clicks): see [`docs/AUDIT-ILGEN.md`](docs/AUDIT-ILGEN.md) and [`docs/STATUS.md`](docs/STATUS.md).
- PC version: window playable with the mouse (temporary software rendering). Coming: GPU rendering, free window / fullscreen, an interface adapting to any screen size, gamepad, sound.
- Goals and choices: [`docs/VISION.md`](docs/VISION.md); roadmap: [`docs/NEXT.md`](docs/NEXT.md).

## Layout
| folder | role |
|---|---|
| `crates/ngu-ilgen` | IL (.NET) -> Rust converter |
| `crates/ngu-rt`, `crates/ngu-host` | runtime of the converted code, bridge to the engine |
| `clr/` | engine (scene, UI, layout, rendering, input, developer mode) + reference interpreter for conformance tests |
| `crates/ngu-gen`, `crates/ngu-desktop` | the game: test bench and PC window |
| `tools/` | game data extraction (`nguprep.py`), build and comparison scripts |
| `docs/` | architecture, audit, status, developer mode ([`docs/DEV.md`](docs/DEV.md)) |

## Build (today: tooling designed for the maintainer's machine, Windows + WSL)
1. Extract your game's data: `python tools/nguprep.py` (UnityPy) -> `out/ngu/`.
2. Convert and build (WSL): `tools/iter.sh 1` (generates `crates/ngu-gen/src/gen.rs`, builds under a memory cap: the generated code is ~20 MB and rustc needs >10 GB of RAM).
3. Run the window: `ngu-gen window <path to Assembly-CSharp.dll> out/ngu 60`.

Paths are still hard-coded for the maintainer's machine; a single "point at your game and everything happens" command is planned.

## License
GPL-3.0 (see `LICENSE`). NGU Idle and its contents belong to their authors; this project is not affiliated with 4G.
