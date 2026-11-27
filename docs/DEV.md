# Developer mode (tools for the assistant)

Designed to be driven by an assistant: everything is scriptable, output is text (no need to look at an image to know what is on screen), no manual clicks.
Code: `clr/src/dev.rs`, steps in `clr/src/script.rs`.

## Run
```
# WSL: one game, some steps, a final capture
bash tools/clr.sh play /abs/path/out/ngu /tmp/final.png "click,Start New Game" "click,PLAY" "do,/abs/path/dev/unlock.steps" "menu,15" "ui"
# all menus as PNG + text (out/screens/<name>/NN-Name.png/.txt), then contact sheets
bash tools/screens.sh unlocked do,dev/unlock.steps
python tools/sheet.py out/screens/unlocked
```
File paths are absolute (`clr.sh` changes into `clr/`).

## Steps
| step | effect |
|---|---|
| `click,<text>` | clicks the visible element whose text equals (else contains) `<text>`, waits 17 frames |
| `ui[,file]` | screen as text: interactive elements (`button`, `zone`, `toggle`, `input`, `slider`...) then visible texts, with 960x600 rectangles |
| `inspect,x,y` | nodes under the point (back to front) with components: image (colour, sprite), text, Selectable, Mask, game scripts |
| `find,<name part>` | GameObjects with tag, active flag and world rectangle |
| `ultimate` | sets every unlock-like boolean reachable from the scene (see `dev/ultimate.steps`) |
| `get,Type.field.field` | reads a game value (e.g. `get,Character.bossID`) |
| `set,Type.field...,value` | writes (same type as the old value; `true/false` for booleans) |
| `call,Type[.field].method[,args]` | calls a game method (e.g. `call,ButtonShower.updateButtons`) |
| `open,<file>` | opens a save the way the game does (`loadintoGame`: offline progress and version updates apply), unlike `l,<file>` which only loads the data |
| `clock,<seconds>` | sets the time the game reads (`DateTime.UtcNow`, seconds since 1970) for the rest of the run |
| `skip,<seconds>` | the host was away for that long (a hidden tab, a sleeping computer, a suspended console): the next step applies the game's offline progress for it (ADR 0016); the scenarios of the console have the same event, `<tick> away <seconds>` |
| `do,file` | runs a step file (one per line, `#` comment) |
| `menus` / `menu,N` | lists the 58 menus / shows menu N (`MenuSwapper.swapMenu`) |
| `autoplay` mode | `clr <dll> autoplay <assets> <scenario> <trace.csv>`: replays a benchmark scenario headless (`docs/VITA-60.md`) |
| `tree,<depth>` | active hierarchy under the root objects with rectangles (960x600, top-left), canvases marked: names for screen profiles |
| `pngl,<png>,<profile>,<w>,<h>[,hold][,pointer y]` | capture through a screen profile (`layouts/README.md`) |
| `visual_motion` | enable desktop visual interpolation in a headless run |
| `pngl,<png>,desktop,<w>,<h>[,<ms>]` | capture the responsive layout, optionally sampling between fixed game ticks |
| `pointer,x,y`, `release_pointer` | move the pointer without advancing a tick, or release a held button and advance one tick |
| `uimap,<json>,<md>` | UI map of the 58 menus (see `docs/UI-MAP.md`) |
| `sounds` | prints (and clears) the sounds the game asked to play |
| `png,file` | 960x600 capture |
| `r,x,y[,wait]`, `mc,x,y[,wait]` | right / middle click |
| `k,<key>[,frames]`, `kd,<key>`, `ku,<key>` | tap / hold / release a key (Unity KeyCode number or name: `a`, `3`, `left shift`, `space`) |
| `wheel,x,y,notches` | mouse wheel at a point (positive = up); in console mode the parts it squeezed scroll first |
| `consolemode,on\|off`, `consolescale,100\|110\|120` | console mode switch and scale (`docs/CONSOLE-MODE.md`) |
| `pad,u\|d\|l\|r\|a\|s\|b\|prev\|next\|holdstart\|holdend\|scroll[,rows]` | console controller input (d-pad, cross, square, circle, L / R tap, L held / released, right stick) |
| `consolestate` | console mode: rows shown and the parts that scroll after the last capture (`pngl,...,desktop,w,h` lays the screen out) |
| `x,y[,wait]`, `w,N`, `h,x,y`, `d,x1,y1,x2,y2`, `f,u/d/l/r/list`, `a`, `m,+-1`, `s,f`, `l,f` | click, wait, hold, drag, focus, activate, next menu, save probe/load |

`Type` = game class attached to the scene (first component of that type): `Character`, `ButtonShower`, `HoverTooltip`, `MenuSwapper`...
To find a field, read the decompiled class (`re/decomp/Assembly-CSharp/<Class>.cs`, local, not versioned; produced by `tools/decompile.sh`).

## Full-game script
`just full-game [hours]` (or `bash tools/fullgame.sh [hours]`, or `clr <dll> fullgame <prepared> [report] [hours]`) plays the whole game on the real data and prints one line per step: title screen, new game and 60 s of progress (the original gains 58 energy), every menu with the unlock flags set, a long progression (default 1 h of game time, about 35 s), a save that loads back to the same bytes, offline progress (one hour computed by the game equals one hour played, and a save loaded one hour later or earlier applies it) and every console-mode element of every menu. The exit code is 1 when a step is KO. A step that needs the game's own classes is SKIP on a synthetic assembly. It writes only to a temporary folder. `tools/fullgame-wasm.sh` runs the same script as WebAssembly (what the web port runs) under Node 18 or later: the numbers are identical to the native run, about 1.6 times slower. The same run is `NGU_TEST_PREPARED=<prepared> cargo test --release -p clr --test fullgame -- --ignored`.

## Long-run test (interpreter shortcuts)
`bash tools/longrun.sh [rounds] [seed]` plays the same long game twice, with and without the interpreter shortcuts (`NGU_NO_GUARD=1`), from the imported PC save or from an unlocked new game: rounds of 150 pseudo-random clicks and 20 minutes of game time, with two hours of offline progress in the middle, on a fixed clock. The final saves must be byte-identical; otherwise it prints the differing fields and exits with 1. The run without shortcuts is slower (13 minutes per game hour on a late-game save). `play` accepts `NGU_DESKTOP=1` to use the desktop screens and save picker of the applications.

`bash tools/guard-frames.sh [scenario...]` replays the console scenarios (nguidle-vita `session` and `monkey` by default) through the C interface once with every interpreter shortcut, once with none and once per shortcut disabled (`NGU_GUARD_OFF=<id>`), and prints the change in total instructions, in the 99th percentile frame and in the worst frame (see [GUARDS.md](GUARDS.md)).

## Presets (`dev/`)
- `dev/ultimate.steps`: the "ultimate" state (everything gated by progression visible; used for `tools/screens.sh ultimate`). **Do not load saves made from it into the original game: huge `bossID` unlocks real Steam achievements.**
- `dev/unlock.steps`: closes the tutorial, boss 150, all features unlocked (conditions of `ButtonShower.updateButtons`). Still locked: Advanced Training (array `training.attackTraining[4]`, not settable with `set`).

## Known limits
- `set` does not traverse arrays or structs (values are copied).
- `swapMenu` raises an exception for menus 3 (Adventure Menu) and 39 (Beard) even when unlocked: to investigate (capture still produced).
- Developer commands can run before the desktop window opens using `NGU_STEPS`.

Rules: go through the game's own fields and methods (as a save would); never change the game rules for a test need.

## Test the PC window

Build with `cargo build --locked --release -p ngu-desktop --bin ngu-window`.
Run `ngu-window <Assembly-CSharp.dll> <prepared folder> [fps] [max_frames]`.
Set `NGU_SAVE_DIR` to an isolated profile, `NGU_STEPS` to run scripted inputs
before opening the window, and `NGU_PERF=1` to log frame intervals.
