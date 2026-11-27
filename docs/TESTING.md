# Tests

## Repository checks

Run `just check` from the repository root with Rustup, Just 1.43.0 and
Python 3.11 or later. Install Ruff with
`python -m pip install ruff==0.11.13` (use `python3` where appropriate).
The pinned Rust toolchain includes Clippy and rustfmt. Linux desktop
compilation also needs libudev development headers and pkg-config.
On Windows, the check recipes use Windows PowerShell.

The command checks repository hygiene and the formatting of the new save
fixtures. It runs Clippy on the workspace without the generated game crate. The
existing lint findings remain visible as warnings. The new save fixtures
must pass Clippy with warnings treated as errors.

The Rust tests exercise hand-built NRBF records, a synthetic assembly
created from test source, a serial round trip and the legacy checksum
and conversion. No original game files are needed. The Python tests
exercise the file manifest verifier with temporary synthetic files.

The repository check blocks tracked build outputs and checks new lines
for workstation paths, common French phrases and broken relative Markdown
links. The recorded lot 2 revision defines the initial debt boundary.
On GitHub, the comparison starts at the pull request base or the previous
push revision. Historical findings are covered by the separate cleanup.

## Port interface checks

`just check` also tests the platform interface and its reference port with the synthetic engine
(`crates/ngu-headless/tests/synthetic.rs`), generates the port template into a temporary folder and runs its format,
lint and contract test (`tools/check_template.py`), and builds the reference documentation with warnings as errors
(`just doc`). With the player's prepared data, `NGU_TEST_DATA=<prepared> cargo test -p ngu-headless --test session -- --ignored`
runs the reference host against the real game, and `cargo run --release -p ngu-headless -- --data <prepared> --save <tmp>`
prints a reproducible draw hash.

## Cold trial of the porting guide

Done for each minor engine version. `just cold-trial` (`tools/cold_trial.py`) follows [the guide](PORTING.md) in an empty
folder from a clone of this repository and runs the generated port's `just check`: it proves the documented commands work.
The second half needs a fresh AI agent with no other context than the clone and the request "create a headless port
`nguidle-test`": it must succeed using only the documentation and the template, and every blocker it reports becomes a fix
to the guide, the template or the interface. Keep the report in the pull request or in `docs/archive/`.

## Performance budgets
The Vita budgets and reference are in nguidle-vita (`budgets.json`, `perf/`). The PC window has its own in `budgets.json` here:
on the title screen at 1280x720 the window must hold at least 60 img/s with no frame over 20 ms and no interval over 33 ms.
Check it by hand (it needs a display and the prepared data):
`just pc-perf <ngu-window executable> <Assembly-CSharp.dll> <prepared folder>`. First measurement, 24 November 2026, engine `851cb5c`,
a Windows 11 desktop (GPU window at its 200 img/s pacing): median 200.0 img/s, longest interval 13.5 ms, no frame over 20 ms. A figure
is valid only with the commit and the machine named. Measurements on the title screen say nothing about dense menus.

## Full-game script
`just full-game` plays the whole game (menus, a long progression, saves, offline progress, console mode) and reports OK / KO / SKIP per step; see [developer mode](DEV.md). It needs the player's prepared data, so it runs locally, not in `just check`; `just check` tests its report and tolerance logic. It is the engine-level script shared by every platform: the web and Vita hosts run this same engine.

## Local game verification

Prepare the game using the [contributor guide](../CONTRIBUTING.md). Run the
commands from Bash or WSL. In a separate worktree, set `NGUIDLE_HOME` to the
workspace containing the prepared data and original game.

Before changing the interpreter, run `just verify-game record` from the known
good revision to create the local reference. Then run `just verify-game` after
each change. The reference is never overwritten by either command. Set
`NGU_GAME_BASELINE` to use a different reference directory outside the
repository. The default is `$NGU_WORK/traces/game-baseline`.

The suite compares the state and full saves for deterministic click and input
scenarios, Rebirth, and a save-load-save round trip. It also checks all 84
number formatting cases. To include a local PC save at recording time, set
`NGU_REAL_SAVE` to a disposable copy. The imported backup in the shared
workspace is used if present. The runner copies it into the local reference
so later changes to the source save cannot alter the comparison.

Each run uses a fixed clock and an isolated save folder. The report prints
the path to its local logs, and verification compares saves byte for byte.
Keep references, saves and logs outside version control.

Windows and Linux run the same `just check` recipe in GitHub Actions.
The web and Vita checkouts own their separate workflow and build tools.
Host compilation checks do not measure console performance.
