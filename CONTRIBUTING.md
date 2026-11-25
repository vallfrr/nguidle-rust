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
engine checkout. The setup can be rerun.

Run `just check` in the engine checkout. The [test guide](docs/TESTING.md)
lists the tools and explains the split between repository checks and local
game verification. Use the [web](https://github.com/vallfrr/nguidle-web)
and [Vita](https://github.com/vallfrr/nguidle-vita) repositories for their
build, setup and troubleshooting commands.

## Local changes and verification

- Use a topic branch. Keep original files, generated assets and code,
  saves, private notes and configuration outside Git.
- Read the [development guide](docs/DEV.md) and run the checks that exercise
  the changed path. Write down the command and observed result.
- Keep workstation details in `work/notes/AGENTS.local.md` and the current
  handoff in `work/session/SESSION.md`.
- Use the project skills in `.claude/skills/` as optional guidance. Verify
  against the code and tests. The third-party skill inventory is empty.
- For a legacy Windows workspace, back it up and preview the one-time
  migration with `tools/migrate-workspace.ps1 -Workspace <workspace>`.
  Pass `-Apply` after reviewing the moves and closing local consumers of
  the source folders. Existing launchers can still use historical aliases.

## Porting to a new platform
Do not start here: follow [the porting guide](docs/PORTING.md). To work on the port tooling itself, `just new-port`,
`just doc` and `tools/check_template.py` are the commands; `just check` runs all of them.
