# Comparison with the original game

Method (reproducible): `tools/orig.ps1` drives the original (launched through Steam, window captured with `PrintWindow` even when hidden, real mouse clicks in the foreground, cursor restored afterwards). Our side: `tools/screens.sh` / script steps. Side-by-side images and pixel diffs (`tools/imgdiff.py`) go to `out/compare/` (not versioned). The original's save folder is copied before a test and restored after.

Rule: compare **behaviour and content**, not pixel-exact output (see [fidelity](FIDELITY.md)).

## 2026-11-20: new game, 960x600 (title, intro, main screen, Boss Fight, Info 'n Stuff)
Layout, images, fonts and texts match. Differences found:

| # | difference | original | ours | cause | state |
|---|---|---|---|---|---|
| 1 | disabled buttons not greyed ("Locked", "NEED GOLD", "NOT RIPE"...) | grey | white | `Selectable` `ColorTint.disabledColor` not applied when `interactable = false` | **fixed** (A10) |
| 2 | number format follows the system language | `50 000`, `500 000 HP`, `48,9s` (Windows in French) | `50,000`, `44.2s` | the original (Mono) uses the system culture; our BCL used the invariant culture | **fixed** (A11) |
| 3 | different line breaks (Info 'n Stuff paragraphs, boss story) | shorter lines | one word more per line | glyph advances: Unity (FreeType) rounds them to whole pixels, `fontdue` keeps fractions | **fixed** (A12) |
| 4 | "4G's SELLOUT SHOP" overflows upwards | 3 lines fit in the button | first line cut at the top | Unity ignores the final empty line (`"4G's \nSellout \nShop\n"`) for vertical alignment | **fixed** (A12) |
| 5 | menu column background too light | flat | lighter | the image of a `Mask` with `showMaskGraphic = false` was drawn (Unity does not draw it); found with `inspect,x,y` | **fixed** |
| 6 | centred text off by one pixel | | | glyph x placement by `floor` (measured: half as many differing pixels as `round`), y by `round` | **fixed**, then stopped (see rule) |
| 7 | player name | Steam name | "Bob" | Steam absent (expected) | not a bug |
| 8 | tutorial bubble position | near the real cursor | near the last click | depends on mouse position | not a bug |
| 9 | HP, energy, rebirth time | different values | | different instants (logic is verified by the oracle) | not a bug |

Remaining difference on the main screen: letter anti-aliasing (`fontdue` rasterizer vs Unity's FreeType), invisible at normal size.

Measurements of the original on the maintainer's PC (2560 px monitor at **200 Hz**, Radeon RX 9070 XT):
- `vSyncCount = 1` => **200 img/s** (one per refresh); ours: capped at 60 today (A4: vsync).
- CPU: **8 % of one core**; RAM: **341 MB**.
- The game starts only through Steam (launched directly it starts Steam and relaunches from the Steam installation).

Not yet compared: unlocked menus (needs an advanced save in the original: A1b), sounds, animations, hover tooltips, drag and drop.

## 2026-11-20: unlocked menus (our "ultimate" save loaded in the original), 18 menus
Method: `tools/screens.sh ultimate do,dev/ultimate.steps`, step `s,<file>` writes our save, the original loads it as its autosave, `tools/orig.ps1` captures the 18 menus reachable from the menu column, `tools/pairs.py` builds side-by-side sheets.
| difference | cause | state |
|---|---|---|
| menus with the top input bar sat 40 px too high (titles hidden) | custom tags were exported as ids (`Tag20000`): `GetChild(0).tag == "Menu"` never matched, so every menu used the big anchor instead of the small one | **fixed** (`tags.json` exported by the preparation step, tag names resolved) |
| Adventure and Beard menus raised an exception on refresh | `Dropdown.options`, `OptionData`, `captionText` unimplemented | **fixed** |
| our save was not accepted by the original | home-made save format | **fixed**: real BinaryFormatter format |
| decimal separator `157.5` vs `157,5` | the original follows the system culture (Windows in French); WSL runs with the invariant culture | not a bug (see A11) |
| Wandoos 98 status text/box, attention colours of menu buttons, time-dependent values | game state at a different instant | not a difference of logic |
