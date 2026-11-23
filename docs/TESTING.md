# Tests

## Repository checks

Run `just check` from the repository root with Rustup, Just 1.43.0 and
Python 3.11 or later. Install Ruff with
`python -m pip install ruff==0.11.13` (use `python3` where appropriate).
The pinned Rust toolchain includes Clippy and rustfmt. Linux desktop
compilation also needs libudev development headers and pkg-config.

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

## Local game verification

On WSL, prepare the game and build the generated crate with the
[contributor guide](../CONTRIBUTING.md). Then run `just verify-game`.
It calls the existing differential replay suite with isolated profiles.
Keep the prepared data, saves and logs in the local workspace.

Windows and Linux run the same `just check` recipe in GitHub Actions.
The web and Vita checkouts own their separate workflow and build tools.
Host compilation checks do not measure console performance.
