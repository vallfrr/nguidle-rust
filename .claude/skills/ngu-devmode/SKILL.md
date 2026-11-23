---
name: ngu-devmode
description: Drive NGU Idle by script without manual clicks: start a game, unlock everything, jump to any of the 58 menus, read the screen as text, get/set/call any game field or method, capture PNGs. Use when you need to see, reach, test or debug any screen or game state of nguidle-rust.
---

# Developer mode (scriptable, designed for the assistant)

Full reference: `docs/DEV.md`. Code: `clr/src/dev.rs`, steps in `clr/src/script.rs`. Interpreter only (same engine and rendering as native).

```
bash tools/clr.sh play /mnt/j/NGUvita/nguvita/out/ngu /tmp/final.png \
  "click,Start New Game" "click,PLAY" "do,/mnt/j/NGUvita/nguvita/dev/unlock.steps" "menu,15" "ui"
```
- `click,<text>`: clicks a visible element by its text. `ui[,file]`: screen as text (clickable elements + texts, 960x600 rectangles). `inspect,x,y`: nodes and components under a point.
- `get,Character.bossID` / `set,Character.settings.beardsOn,true` / `call,ButtonShower.updateButtons`: game state and methods by path (`Type` = game class attached to the scene; fields in `re/decomp/Assembly-CSharp/<Class>.cs`).
- `menus` / `menu,N`: list / show the 58 menus. `png,file`: capture. `do,file`: step file. `w,N`: wait N frames (1/30 s).
- `dev/unlock.steps`: everything unlocked (boss 150, features), tutorial closed.
- All menus as PNG + text: `bash tools/screens.sh <name> [steps...]`.

Rules: go through the game's own fields/methods (as a save would); never change the translated logic for a test need. File paths absolute (`clr.sh` changes into `clr/`).
- Console mode: `consolemode,on|off`, `consolescale,100|110|120`, `pad,d|u|l|r|a|s|b|prev|next|holdstart|holdend|scroll[,rows]`, `consolestate`; capture with `pngl,<png>,desktop,<w>,<h>` after `consolemode,on` (it lays the console screen out). See `docs/CONSOLE-MODE.md`.
