# Cold trial of the porting guide, engine 0.5.0 candidate (historical, does not set policy)

Procedure: [TESTING.md](../TESTING.md), "Cold trial of the porting guide". Revision tried: the `v0.5.0` candidate tag.

## Part 1 - mechanical, repeatable: `just cold-trial`
Clone of the engine into an empty temporary folder, then the guide's commands literally: `just new-port nguidle-test "Test platform" ../nguidle-rust`, then `just check` in the generated port (hygiene, format, Clippy with warnings as errors, contract test against the synthetic engine). Result: every command worked from an empty folder.

The generated port was also run against the real game on the author's machine (`just run <prepared> <throw-away profile>`): the game opened, 120 virtual frames drew 1249 quads and uploaded 13 textures. The reference host (`ngu-headless`) gave the same draw hash on two runs.

## Blockers found by this part, and their fixes
1. `just new-port nguidle-switch --platform "Nintendo Switch"`: `just` split the quoted platform name, so the guide's own example failed. The recipe now takes positional arguments (`name platform engine`) and the guide shows that form.
2. A generated port was not a Git repository, so its first `just check` (hygiene reads the Git index) could not pass. `new-port` now runs `git init` (no commit) and writes a first-run baseline.
3. The port's recipes used `--locked` before any `Cargo.lock` existed. They no longer do; the guide tells the author to commit `Cargo.lock`.
4. The generated source was formatted for the template's placeholder length, so `cargo fmt --check` failed for some port names. Imports are now one per line, the usage text no longer embeds the name, names must start with `nguidle-` (import order), and `just check` formats a short and a long generated name.
5. The template check did not run the port's hygiene step, which hid blocker 2 from CI. `just cold-trial` covers the whole sequence.

## Part 2 - a fresh agent: succeeded
A fresh AI agent received only a clone of the engine (the `v0.5.0` candidate) and the request "create a headless port `nguidle-test`". It read the README, AGENTS.md, `docs/PORTING.md`, the porting skill, `ngu-headless` and the head of `docs/ENGINE-API.md`, ran `just new-port nguidle-test "Headless test" ../nguidle-rust`, then `just check` (passed first time, about 30 s), scripted a click in the port's `main.rs`, and ran it on the prepared game data: 123 frames, 1249 quads, 13 textures, home screen drawn. No blocker, no engine change, no question to the author.

Friction it reported, and what was done:
- A path engine (third argument) leaves a local path in `Cargo.toml`, which the guide forbids in a repository: the guide now says how to switch to a tag and that the local form is for trials only.
- The guide's clone URL is not live until the engine is published: the guide's first command says to use the copy you were given meanwhile.
- The argument order of `just new-port` was only in prose: the guide now shows the full command with its three arguments.
- `just check` printed engine warnings: all removed from the engine (`clr` builds without warnings).
- Nothing explained how to script input in the port itself: the interface gained `key`, `wheel`, `press` and `reset_input` so a port can forward all input, and the template's `Event` type in `src/input.rs` covers them. Not yet re-run by a fresh agent.
- Data preparation (section 3) was not exercised by the agent, which was given a prepared folder.

Both parts passed. A port generated from the current template and the reference host print the same draw fingerprint for the same 120 frames of the real game (`31e37013f05534a5`).
