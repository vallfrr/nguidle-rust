# nguidle-rust: instructions for any assistant

## If the maintainer writes just "reprendre" (resume)
1. Read this file, then `docs/VISION.md` (what the maintainer really wants, trade-offs), `docs/NEXT.md` (ordered roadmap), `docs/STATUS.md` (verified / not verified). Local unpublished notes, if present: `local/docs/HANDOFF.md` (history, console connection).
2. `git log --oneline -15` and `git status` here and in `nguidle-vita/`, `nguidle-web/`: see where work stopped (finish a half-done task before starting another).
3. Take the **first unchecked task** of `docs/NEXT.md` that is not marked **[maintainer]**, and do it until its "Done when" criterion.
4. Verify yourself (tests, captures, measurements; skill `ngu-verify`). Never say "it works" without having seen or measured it; state honestly what is not verified.
5. Check the task off in `docs/NEXT.md`, update `docs/STATUS.md`, commit, push, go on to the next one.
6. Do not ask the maintainer questions except for a **[maintainer]** task or a decision written nowhere; write pending questions in `docs/NEXT.md` ("Questions pending") and ask when they announce their return. They only read the final report they ask for.
7. At the end, tell them plainly (in French, no jargon) what was done, what is verified, what is not, and what waits for them.

## The project in one minute
Port of NGU Idle (Unity 2019.4, Mono, Windows). The game's C# (`Assembly-CSharp.dll`) is **translated to Rust** by our generator from each player's own copy of the game; a Unity-compatible engine written in Rust (`clr/`) loads the scenes/sprites extracted from their files and draws the interface.
Three repositories (GPL-3.0): **`nguidle-rust`** (this one: engine + conversion + PC version), **`nguidle-web`** (self-hosted Docker container) and **`nguidle-vita`** (PS Vita). The latter two use only the engine; they are cloned into `nguidle-web/` and `nguidle-vita/` (git-ignored here).
- `crates/ngu-ilgen`: IL -> Rust generator (produces `crates/ngu-gen/src/gen.rs`, not versioned).
- `crates/ngu-rt`: runtime of the generated code. `crates/ngu-host`: generated code <-> engine bridge.
- `crates/ngu-gen`: the game binary (`game`, `window`, `fuzz`). `crates/ngu-desktop`: PC window (minifb), mouse/keyboard.
- `clr/`: engine (scene, UI, layout, software rendering, focus, dev mode) **and** CIL interpreter that serves as the **oracle**: we compare the generated Rust state with the interpreter's (zero difference tolerated).
- `tools/nguprep.py` extracts the game data to `out/ngu/` (not versioned). `tools/inventory.py`: screen inventory (local output, not published).
- Docs: `docs/NEXT.md` (to do), `docs/STATUS.md` (proof), `docs/VISION.md`, `docs/DEV.md` (dev mode), `docs/ARCHITECTURE.md`, `docs/AUDIT-ILGEN.md` (generator conformance), `docs/COMPARISON.md` (vs the original), `docs/CONTROLLER-PROPOSAL.md`, `docs/API-SURFACE.md`.
- Skills (`.claude/skills/README.md`): `ngu-verify` (follow after every change), `ngu-devmode`, `rust-patterns`, `rust-testing`, ponytail (minimal code); Vita skills in `nguidle-vita/.claude/skills/`; web testing in `nguidle-web/.claude/skills/`.

## Absolute rules
- **Language**: everything in the repositories is English (code, comments, program messages, scripts, docs, READMEs, commit messages). Only the chat with the maintainer is French.
- **Publication**: these repositories are meant to be public. Never in git: game files (code, decompilation, raw assets, extracted texts, saves), personal information (first name, e-mail, user paths, IPs, console configuration). Local material goes to `local/` (ignored). Commits use the identity configured in each repository.
- **Images**: screenshots of OUR port may go into the repositories (docs, README), even if they show the original drawings, but only what is really visible in our port. Never screenshots of the original game (they stay in `out/compare/`).
- **Assets**: always use the game's **real assets** (and official assets for the LiveArea, provided by the maintainer). **Never create an asset from scratch** (icon, background, sprite, font...). The current LiveArea images are black placeholders.
- On the maintainer's machine, write **only** in `J:\NGUvita\nguvita` (and its sub-repositories). Read-only: `J:\NGUvita\NGU IDLE` (the game) and everything else on `J:\`.
- Never load a save with an extreme state (huge `bossID`...) into the original game: its Steam integration awards real achievements on the maintainer's account.
- The original game's save folder (`%USERPROFILE%\AppData\LocalLow\NGU Industries\NGU Idle\`): the maintainer has no game to protect and allows launching/closing the original freely; still copy the folder before a test.
- **Memory / system drive**: rustc exceeds 10 GB on the generated crate (20 MB) and has already filled the system drive. Always build `ngu-gen` under a memory cap: `tools/capbuild.sh` (WSL) or `tools/capbuild.ps1` (Windows), build folders on `J:` (`/mnt/j/NGUvita/build-ngu`, `J:\NGUvita\build-ngu-win`). Never `ulimit -v`. Never `wsl --shutdown`.
- Vita: procedure and rules in `local/docs/HANDOFF.md` ("Vita connection"); one game at a time; never re-enable VitaCompanion.

## Commands (maintainer's machine; adapt paths elsewhere)
WSL (`/mnt/j/NGUvita/nguvita`):
- `tools/iter.sh [frames]`: regenerate, build under the cap, run the game without a window.
- `tools/regress.sh`: oracle vs native non-regression (after `tools/iter.sh 1`). Must stay all OK.
- `tools/statediff.sh N`, `tools/savediff.sh`, `tools/divtrace.sh`, `tools/where.sh <gen.rs line>`: state, save and trace comparison, locating a panic.
- **Dev mode (for you)**: `docs/DEV.md`: `click,<text>`, `ui` (screen as text), `inspect,x,y`, `get/set/call` on game state, `dev/unlock.steps`, `tools/screens.sh` (the 58 menus as PNG + text). Prefer `ui` to screenshots to know what is displayed (cheaper).
- Input scripts `NGU_STEPS="x,y,wait;w,N;f,u;a;m,1;s,file;l,file"` (see `clr/src/script.rs`). Variables: `NGU_FIXED_NOW`, `NGU_NO_GUARD=1` (oracle), `NGU_DUMP`, `NGU_TRACE*`, `NGU_CULTURE`.

Windows (PowerShell, in `J:\NGUvita\nguvita`): the window does not display through WSLg, use Windows.
- Generate `gen.rs` first on the WSL side (`tools/iter.sh 1`), then build:
  `$env:RUST_MIN_STACK="536870912"; $env:CARGO_TARGET_DIR="J:\NGUvita\build-ngu-win"; .\tools\capbuild.ps1 14 cargo build --profile game -p ngu-gen`
- Run: `J:\NGUvita\build-ngu-win\game\ngu-gen.exe window "J:\NGUvita\NGU IDLE\NGUIdle_Data\Managed\Assembly-CSharp.dll" out/ngu 60` (+ `NGU_PERF=1` for per-frame timings on stderr).
- Original game for comparison: `. .\tools\orig.ps1; Start-Orig` (launches through Steam).

## What the maintainer said (respect it)
- Goal and decisions: `docs/VISION.md` (real port, "NGU Idle 2", perfect PC base, responsive, modifiable, documented; then web for oneself, then Vita).
- Compare with the original for behaviour and content, not pixel-exact output.
- The dev mode is used first by the assistant (`docs/DEV.md`).
