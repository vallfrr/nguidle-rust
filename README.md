# nguidle-rust

Free port of **NGU Idle** (by 4G, Unity 2019.4) to Rust: the game's C# code is **converted to Rust from your own copy of the game** and runs on a Unity-compatible engine written in Rust. This repository contains the engine, the conversion tool and the PC version. Other platforms build on it: [`nguidle-web`](https://github.com/vallfrr/nguidle-web) (self-hosted web version) and [`nguidle-vita`](https://github.com/vallfrr/nguidle-vita) (PS Vita).

> **No game file is provided or published here** (no code, images or texts). You must own the game (free on Steam): the conversion reads `Assembly-CSharp.dll` and the data of your installation on your machine.

## State (in development)
- Differential testing compares the converted runtime with the original IL running in the reference interpreter. See [`docs/AUDIT-ILGEN.md`](docs/AUDIT-ILGEN.md) and [`docs/STATUS.md`](docs/STATUS.md) for the measured coverage.
- The Windows version has a resizable GPU window, F11 fullscreen, local save slots and local milestone tracking. Focus navigation uses the D-pad or left stick. The right stick scrolls and the south button confirms.
- The PS Vita preview shares the game scene at 960 × 544 and supports touch and controller navigation. See the [Vita port history and hardware audit](docs/VITA.md).
- The [release audit](docs/RELEASE-AUDIT.md) tracks platform boundaries and validation. See [vision](docs/VISION.md) and [roadmap](docs/NEXT.md) for decisions and remaining work.

## Layout
| folder | role |
|---|---|
| `crates/ngu-ilgen` | IL (.NET) -> Rust converter |
| `crates/ngu-rt`, `crates/ngu-host` | runtime of the converted code, bridge to the engine |
| `clr/` | engine (scene, UI, layout, rendering, input, developer mode) + reference interpreter for conformance tests |
| `crates/ngu-gen`, `crates/ngu-desktop` | the game: test bench and PC window |
| `tools/` | game data extraction (`nguprep.py`), build and comparison scripts |
| `docs/` | architecture, audit, status, developer mode ([`docs/DEV.md`](docs/DEV.md)) |

## Prepare the Windows build

Install the Rust MSVC toolchain, the Visual Studio C++ build tools and Python
3.11 or later. Run these commands in PowerShell from the repository root.

```powershell
python -m pip install -r tools/requirements-prep.txt
.\tools\prepare-pc.ps1 -GameData 'D:\Steam\steamapps\common\NGU IDLE\NGUIdle_Data'
```

The preparer verifies the supported Windows 1.260 build against a SHA-256
manifest before unpacking assets or building. It prints the command that
opens the game when the build succeeds. The first build runs the converter
and can require more than 10 GB of RAM. Use `-ExtractOnly` to validate and
extract data without building. The default data and build folders are
`out/pc` and `build/pc`. Local saves live under `out/pc/save`.

The Windows home screen offers **Load Game** for numbered saves,
**Load Autosave** to resume directly and **Create New Game** to pick an empty
numbered slot. The autosave is the default action when available. A new slot
can have a character name. Occupied slots can be deleted from the new-game
screen after two confirmations. **Save Game** opens with the current numbered
slot selected. Returning to the home screen uses the save already on disk.

Open **Info 'n Stuff → Achievements** to see the two original BP tabs and
the **Steam Achievements** tab. The Steam tab has two pages of milestones
with details and unlock status. The preparer caches their icons from Steam
Community in the local data directory. Refresh a prepared profile with
`python tools/fetch-achievement-icons.py out/pc`. The game reads the cache
offline and shows achievement names when an icon is unavailable.

## Web host

The browser runs the shared Rust engine and CIL interpreter in WebAssembly
and draws the game with WebGL. The host serves the build and stores a personal
profile with leased save slots. Save files can move between desktop and web.
The web repository contains the local Windows launcher and Docker Compose
configuration. Docker requires verified Steam Windows 1.260 data mounted
read-only. See the
[web host guide](https://github.com/vallfrr/nguidle-web) for setup and controls.

## License
GPL-3.0 (see `LICENSE`). NGU Idle and its contents belong to their authors; this project is not affiliated with 4G.
