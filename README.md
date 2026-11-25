# nguidle-rust

A Rust engine that runs your own copy of **NGU Idle** by 4G. It runs the game's
original assembly and provides the PC window, save slots and controller
controls. The [web](https://github.com/vallfrr/nguidle-web) and
[PS Vita](https://github.com/vallfrr/nguidle-vita) ports also use this engine.

You need to own NGU Idle on Steam (version 1.260).

## State (in development)
- The [test guide](docs/TESTING.md) explains the interpreter checks and local replay coverage.
- The Windows version has a resizable GPU window, F11 fullscreen, local save slots and local milestone tracking. Focus navigation uses the D-pad or left stick. The right stick scrolls and the south button confirms.
- The PS Vita preview shares the game scene at 960 × 544 and supports touch and controller navigation. See the [Vita port history and hardware audit](docs/VITA.md).
- The [release audit](docs/RELEASE-AUDIT.md) tracks platform boundaries and validation. See [vision](docs/VISION.md), [roadmap](docs/ROADMAP.md) and [testing](docs/TESTING.md) for decisions and remaining work.

## Layout
| folder | role |
|---|---|
| `clr/` | engine (scene, UI, layout, rendering, input, developer mode) and interpreter |
| `crates/ngu-desktop` | PC window |
| `tools/` | game data extraction (`nguprep.py`), build and comparison scripts |
| `docs/` | architecture, audit, status, developer mode ([`docs/DEV.md`](docs/DEV.md)) |

## Prepare the Windows build

Install the Rust MSVC toolchain, the Visual Studio C++ build tools and Python
3.11 or later. Run these commands in PowerShell from the repository root.

```powershell
python -B tools/setup
$gameData = Read-Host 'Path to NGUIdle_Data'
python -B tools/doctor --game-dir $gameData
.\tools\prepare-pc.ps1 -GameData $gameData
```

The preparer verifies the supported Windows 1.260 build against a SHA-256
manifest before unpacking assets or building. It prints the command that
opens the game when the build succeeds. Use `-ExtractOnly` to validate and
extract data without building. The default data and build folders are
`work/prepared/pc` and `build/win` in the workspace. Local saves live under
`work/prepared/pc/save`. See [CONTRIBUTING.md](CONTRIBUTING.md) for the
workspace layout and alternate locations.

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
`python tools/fetch-achievement-icons.py <prepared-data-directory>`. The game reads the cache
offline and shows achievement names when an icon is unavailable.

## Web host

The browser runs the shared Rust engine and CIL interpreter in WebAssembly
and draws the game with WebGL. The host serves the build and stores a personal
profile with leased save slots. Save files can move between desktop and web.
The web repository contains the local Windows launcher and Docker Compose
configuration. Docker requires verified Steam Windows 1.260 data mounted
read-only. See the
[web host guide](https://github.com/vallfrr/nguidle-web) for setup and controls.

## Porting to another platform
Anyone can build a port on the engine without asking: see [the porting guide](docs/PORTING.md) (`just new-port` creates a starting repository) and [the engine API](docs/ENGINE-API.md).

## License
GPL-3.0 (see `LICENSE`). NGU Idle and its contents belong to their authors; this project is not affiliated with 4G.
