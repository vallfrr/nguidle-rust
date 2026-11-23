---
name: ngu-verify
description: Verify that a change to nguidle-rust keeps the game identical to the original NGU Idle: oracle-vs-native regression (logic, saves), screenshots, comparison with the real game. Use after ANY change to clr/, crates/ (generator, runtime, host) or tools/, before committing, and whenever claiming something "works" or "matches the original".
---

# Verify a change (conformance with the original game)

Nothing is "done" without proof. Order, cheapest first; stop at the first failure and fix it.

1. **Build under the memory cap** (rustc > 10 GB on the generated code; it has already filled the system drive): WSL `bash tools/iter.sh 1` (regenerate + build + run). Never `ulimit -v`, never `wsl --shutdown`. Do not edit `clr/` or `crates/` while a build runs.
2. **Oracle vs native non-regression**: `bash tools/regress.sh` must end with `REGRESSION: ALL OK`. If the change touches texts/numbers also run `NGU_CULTURE=fr bash tools/regress.sh`. State difference: `tools/statediff.sh`, `tools/divtrace.sh`, `tools/firstdiv.py`; panic: `tools/where.sh <gen.rs line>`.
3. **Rendering** (if `render.rs`, `ui.rs`, `layout.rs`, fonts, colours): capture with the interpreter `bash tools/clr.sh play <absolute out/ngu> <out.png> "click,Start New Game" "click,PLAY" [steps]`, or all 58 menus: `bash tools/screens.sh <name> [do,dev/unlock.steps]` + `python tools/sheet.py out/screens/<name>`. Read the screen as text (`ui`) rather than looking at the image when possible (cheaper). See `docs/DEV.md`.
4. **Against the real game** (if the change targets fidelity): PowerShell `. .\tools\orig.ps1; Start-Orig; Send-OrigClick x y; Save-OrigShot out\compare\orig\<name>.png; Stop-Orig` (launched through Steam; copy `%USERPROFILE%\AppData\LocalLow\NGU Industries\NGU Idle\` first). Compare side by side, note differences in `docs/COMPARISON.md`. Compare behaviour and content, not pixel-exact output.
5. **Console mode** (if `console_mode.rs`, `desktop_layout.rs`, `focus.rs`, `pad.rs` or the render clips change): capture the 58 menus with `consolemode,on` + `consolescale,100|110|120` at 1280x720, 1920x1080 and 960x544 and in the Dark / Gold themes (`call,UIThemeController.changeTheme,N` after `set,Character.arbitrary.boughtAscendedNewbiePack,true`), look at every sheet (borders visible, nothing cut, bars only where needed), and confirm the PC mode is pixel-identical with the mode off. See `docs/CONSOLE-MODE.md`.
6. **Windows window** (if the desktop shell changes): `.\tools\capbuild.ps1 14 cargo build --profile game -p ngu-gen` (variables in `AGENTS.md`), run with `NGU_PERF=1`.

Then: `docs/STATUS.md` (verified / not verified, honest), check off `docs/NEXT.md`, commit in English, `git push`.
Never in git: game screenshots (`out/`), decompilation (`re/`), personal information (`local/`).
