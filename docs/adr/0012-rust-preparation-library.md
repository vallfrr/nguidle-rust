# ADR 0012 - Preparation as one Rust library

Status - Accepted.

## Decision

All preparation code lives in the `ngu-prep` crate, shared by the PC application and the Vita preparation application. It is built in this order, each step its own pull request:

1. Find, verify and copy the game (`locate`, `verify`, `copy_game`, per-system data folders). Done in this change.
2. Export the Unity data in Rust (scene, textures, fonts, text assets, audio), replacing `tools/nguprep.py`.
3. Write the archive the Vita reads, replacing `tools/pack.py`.
4. The two applications, which only add a window around the library.

Step 2 is checked against the Python exporter: the same game must produce identical `scene.json`, tags and texture pixels. Until step 2 matches, the Python tools stay as development tools and the applications are not released.

## Reason

Decisions D9 and D10 require preparation to need nothing installed and to behave the same on Windows, Linux and macOS. The draft that bundled a Python worker inside the executable was dropped for that reason. The draft's separate Windows and Vita executables named `nguidle-prep` also predate D9, which names the PC application `NGUIdleRustEngine`.

The library keeps no platform code except where the system decides a path (`dirs`, `locate`). Those functions take the home folder and environment as arguments so every system is tested on any machine.

## Consequences

The Windows registry lookup for Steam is added with the PC application. Until then Windows finds Steam through `ProgramFiles` and `libraryfolders.vdf`, and the player can always choose the folder.

## Sound effects

The two sound effects are FMOD Vorbis banks that refer to their Vorbis setup header by CRC-32. The headers are not downloaded or stored in this repository: they are read from the `UnityPlayer.dll` of the player's own game copy, which holds them. If a different build of the game is used, the lookup fails and preparation stops with a clear message.

## Python tools removed

The reference comparison passed on the real game, so `tools/nguprep.py`, `tools/pack.py`, `tools/verify_game_data.py`, their tests and `tools/requirements-prep.txt` are removed. `ngu-prepare` has the jobs they had: `verify`, `export` (a prepared folder for ports and developers), `vita` and `web` (the web image's build cache), next to `pc`. Python stays for development scripts and for the optional Steam achievement icons (`tools/fetch-achievement-icons.py`, needing only Pillow, `tools/requirements-icons.txt`). The sound decoding reads `UnityPlayer.dll`, so every job needs the whole game folder, not only `NGUIdle_Data`.
