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

## Part 2 - not done: a fresh agent
The trial's real criterion is a fresh AI agent that receives only a clone of the engine and the request "create a headless port `nguidle-test`", and succeeds without help or engine changes. This was **not run**: the person who wrote the guide cannot play the part of a reader who has never seen it. Run it as follows and record the blockers here or in the pull request:
1. Create an empty folder; clone the engine into it; give the agent the folder and the request, and nothing else (no chat history, no workspace notes, no game files path except the prepared data folder if it is to test against the real game).
2. The agent may read only the clone. Note every question it would have had to ask and every command that failed.
3. Fix the guide, the template or the interface for each; repeat until it needs no help.
