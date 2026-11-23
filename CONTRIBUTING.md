# Contributing

## Workspace

Keep the three checkouts side by side. The Windows game supplies the original
Steam 1.260 data. Mount or use that installation read-only.

```text
<workspace>/
  repos/nguidle-rust/
  repos/nguidle-web/
  repos/nguidle-vita/
  game/NGU IDLE/NGUIdle_Data/
  work/       local data, saves, tests and notes
  build/      compiler output
  venvs/      Python environments
  toolchains/ Vita build dependencies
  vita-agent/ local Vita tooling
```

`NGUIDLE_HOME` can point to another workspace, and `NGU_GAME_DIR` to an
existing installation's `NGUIdle_Data`. `NGU_WORK` and `NGU_BUILD` override
the work and build locations. Keep them on a drive with sufficient space.

## Windows

Install Python 3.11 or later, Rustup for the MSVC host, and the Visual Studio
C++ build tools with a Windows SDK. In PowerShell, from `repos/nguidle-rust`:

```powershell
python -B tools/setup
python -B tools/doctor
.\tools\prepare-pc.ps1
```

The last command validates the game data, prepares local assets, generates
Rust and builds the executable. Run the PowerShell command it prints to open
the GPU window. `-GameData` overrides the input location. CI or offline
preparation can pass `-SkipMilestoneIcons` to omit the optional download.
Build the generated game under the existing memory cap.

## WSL or Linux

Use Rustup, Python 3.11 or later with venv support (or `uv`), and a native
compiler. Run `python3 -B tools/setup` and `python3 -B tools/doctor` from the
engine checkout. For Vita, install VitaSDK, CMake and Make, set `VITASDK` if
needed, then run:

```sh
python3 -B tools/setup --vita
python3 -B tools/doctor --vita
cd ../nguidle-vita
bash build.sh
```

`setup` can be rerun. The Vita setup prepares the pinned Rust nightly,
builds the pinned and patched vitaGL library, and installs a separate Vita
agent. A paired, awake console can be checked with
`python3 -B tools/doctor --vita --probe-console` in the engine checkout.
See the web checkout's README for the browser build.

## Local changes and verification

- Use a topic branch. Keep original files, generated assets and code,
  saves, private notes and configuration outside Git.
- Read the [development guide](docs/DEV.md) and run the checks that exercise
  the changed path. Write down the command and observed result.
- Keep workstation details in `work/notes/AGENTS.local.md` and the current
  handoff in `work/session/SESSION.md`.
- For a legacy Windows workspace, back it up and preview the one-time
  migration with `tools/migrate-workspace.ps1 -Workspace <workspace>`.
  Pass `-Apply` after reviewing the moves and closing local consumers of
  the source folders. Existing launchers can still use historical aliases.
