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
| `png,file` | 960x600 capture |
| `r,x,y[,wait]`, `mc,x,y[,wait]` | right / middle click |
| `k,<key>[,frames]`, `kd,<key>`, `ku,<key>` | tap / hold / release a key (Unity KeyCode number or name: `a`, `3`, `left shift`, `space`) |
| `wheel,x,y,notches` | mouse wheel at a point (positive = up) |
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
