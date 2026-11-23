# nguidle-rust: instructions for any assistant

## If the maintainer writes just "reprendre" (resume)
1. Read this file, then `docs/VISION.md` (maintainer decisions), `docs/NEXT.md` (ordered roadmap) and `docs/STATUS.md` (verification). For local work, read `work/session/SESSION.md` and `work/notes/AGENTS.local.md` in the workspace.
2. Run `git log --oneline -15` and `git status` here and in the sibling `nguidle-vita` and `nguidle-web` repositories. Finish a half-done task before starting another.
3. Take the **first unchecked task** of `docs/NEXT.md` that is not marked **[maintainer]**, and do it until its "Done when" criterion.
4. Verify yourself (tests, captures, measurements; skill `ngu-verify`). Never say "it works" without having seen or measured it; state honestly what is not verified.
5. Record the task and its verification. Commit on the topic branch and follow the maintainer's instructions for publication and scope.
6. Do not ask the maintainer questions except for a **[maintainer]** task or a decision written nowhere; write pending questions in `docs/NEXT.md` ("Questions pending") and ask when they announce their return. They only read the final report they ask for.
7. At the end, tell them plainly (in French, no jargon) what was done, what is verified, what is not, and what waits for them.

## The project in one minute
Port of NGU Idle (Unity 2019.4, Mono, Windows). The game's C# (`Assembly-CSharp.dll`) is **translated to Rust** by our generator from each player's own copy of the game; a Unity-compatible engine written in Rust (`clr/`) loads the scenes/sprites extracted from their files and draws the interface.
Three sibling repositories (GPL-3.0): **`nguidle-rust`** (this one: engine + conversion + PC version), **`nguidle-web`** (self-hosted Docker container) and **`nguidle-vita`** (PS Vita). See [CONTRIBUTING.md](CONTRIBUTING.md) for workspace setup.
- `crates/ngu-ilgen`: IL -> Rust generator (produces `crates/ngu-gen/src/gen.rs`, not versioned).
- `crates/ngu-rt`: runtime of the generated code. `crates/ngu-host`: generated code <-> engine bridge.
- `crates/ngu-gen`: the game binary (`game`, `window`, `fuzz`). `crates/ngu-desktop`: PC window (minifb), mouse/keyboard.
- `clr/`: engine (scene, UI, layout, software rendering, focus, dev mode) **and** CIL interpreter that serves as the **oracle**: we compare the generated Rust state with the interpreter's (zero difference tolerated).
- `tools/nguprep.py` extracts the game data to a local prepared directory. `tools/inventory.py` produces the screen inventory locally.
- Docs: `docs/CONSOLE-MODE.md` (console mode: status, how it works, limits), `docs/NEXT.md` (to do), `docs/STATUS.md` (proof), `docs/VISION.md`, `docs/DEV.md` (dev mode), `docs/ARCHITECTURE.md`, `docs/AUDIT-ILGEN.md` (generator conformance), `docs/COMPARISON.md` (vs the original), `docs/CONTROLLER-PROPOSAL.md`, `docs/API-SURFACE.md`.
- Skills (`.claude/skills/README.md`): `ngu-verify`, `ngu-devmode`, `rust-patterns`, `rust-testing` and the shared Ponytail guides. Each platform keeps its own platform-specific skills.

## Absolute rules
- **Language**: everything in the repositories is English (code, comments, program messages, scripts, docs, READMEs, commit messages). Only the chat with the maintainer is French.
- **Publication**: these repositories are meant to be public. Never in git: game files (code, decompilation, raw assets, extracted texts, saves), personal information (first name, e-mail, user paths, IPs, console configuration). Keep these files in the workspace outside the checkouts. Commits use the identity configured in each repository.
- **Images**: screenshots of OUR port may go into the repositories (docs, README), even if they show the original drawings, but only what is really visible in our port. Never screenshots of the original game (they stay in `out/compare/`).
- **Assets**: always use the game's **real assets** (and official assets for the LiveArea, provided by the maintainer). **Never create an asset from scratch** (icon, background, sprite, font...). The current LiveArea images are black placeholders.
  Single exception: elements of the console mode that the game does not have (controller glyphs...), drawn by us in the game's own style and validated by the maintainer (`docs/CONSOLE-MODE.md`, "Art direction"). Never take them from elsewhere.
- Treat the Steam installation and local reference sources as read-only. Follow the workstation boundaries in `work/notes/AGENTS.local.md`.
- Never load a save with an extreme state (huge `bossID`...) into the original game: its Steam integration awards real achievements on the maintainer's account.
- The original game's save folder (`%USERPROFILE%\AppData\LocalLow\NGU Industries\NGU Idle\`): the maintainer has no game to protect and allows launching/closing the original freely; still copy the folder before a test.
- **Memory**: build the generated `ngu-gen` crate under a memory cap with `tools/capbuild.sh` or `tools/capbuild.ps1`. See the local machine notes for the build location and limits.
- **Vita**: use the standalone client installed by setup and follow the local console instructions.

## Commands

From the engine checkout in WSL:
- `tools/iter.sh [frames]`: regenerate, build under the cap, run the game without a window.
- `tools/regress.sh`: oracle vs native non-regression (after `tools/iter.sh 1`). Must stay all OK.
- `tools/statediff.sh N`, `tools/savediff.sh`, `tools/divtrace.sh`, `tools/where.sh <gen.rs line>`: state, save and trace comparison, locating a panic.
- **Dev mode (for you)**: `docs/DEV.md`: `click,<text>`, `ui` (screen as text), `inspect,x,y`, `get/set/call` on game state, `dev/unlock.steps`, `tools/screens.sh` (the 58 menus as PNG + text). Prefer `ui` to screenshots to know what is displayed (cheaper).
- Console mode steps (`consolemode,on`, `consolescale,120`, `pad,...`, `consolestate`) and the test window `ngu-window`: `docs/DEV.md`.
- Input scripts `NGU_STEPS="x,y,wait;w,N;f,u;a;m,1;s,file;l,file"` (see `clr/src/script.rs`). Variables: `NGU_FIXED_NOW`, `NGU_NO_GUARD=1` (oracle), `NGU_DUMP`, `NGU_TRACE*`, `NGU_CULTURE`.

In Windows PowerShell, use `tools/prepare-pc.ps1` to generate the game, build it and print the GPU launch command. Use `window-cpu` for software window captures, `NGU_VSYNC=0` for uncapped measurements, `NGU_PERF=1` for frame timings, F1 for focus navigation and F11 for fullscreen.
- Original game for comparison: `. .\tools\orig.ps1; Start-Orig` (launches through Steam).

## What the maintainer said (respect it)
- Goal and decisions: `docs/VISION.md` (real port, "NGU Idle 2", perfect PC base, responsive, modifiable, documented; then web for oneself, then Vita).
- Compare with the original for behaviour and content, not pixel-exact output.
- The dev mode is used first by the assistant (`docs/DEV.md`).
