# Developer mode (tools for the assistant)

Designed to be driven by an assistant: everything is scriptable, output is text (no need to look at an image to know what is on screen), no manual clicks.
Code: `clr/src/dev.rs`, steps in `clr/src/script.rs`. **Interpreter only** for now (native state lives in the typed twins of `ngu-host`); same engine and rendering.

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

## Presets (`dev/`)
- `dev/ultimate.steps`: the "ultimate" state (everything gated by progression visible; used for `tools/screens.sh ultimate`). **Do not load saves made from it into the original game: huge `bossID` unlocks real Steam achievements.**
- `dev/unlock.steps`: closes the tutorial, boss 150, all features unlocked (conditions of `ButtonShower.updateButtons`). Still locked: Advanced Training (array `training.attackTraining[4]`, not settable with `set`).

## Known limits
- `set` does not traverse arrays or structs (values are copied).
- `swapMenu` raises an exception for menus 3 (Adventure Menu) and 39 (Beard) even when unlocked: to investigate (capture still produced).
- Not yet available in native mode or in the window (F12 console): to add if needed.

Rules: go through the game's own fields and methods (as a save would); never change the translated logic for a test need.

## Test window without the native build
`ngu-window <Assembly-CSharp.dll> <out/ngu> [fps] [max_frames]` (crate `ngu-desktop`, interpreter only): the same GPU window
and gamepad handling as `ngu-gen window`, built in seconds; `NGU_SAVE_DIR` picks the save folder, `NGU_STEPS` runs steps first.
Set `NGU_PERF=1` to log frame intervals. Slow frames over 25 ms report update, layout and quad emission, draw and submission,
and time outside those measured phases. The last value includes presentation waits and scheduling.
Build: `$env:CARGO_TARGET_DIR="J:\NGUvita\build-ngu-win"; .\tools\capbuild.ps1 10 cargo build --release -p ngu-desktop --bin ngu-window`.
