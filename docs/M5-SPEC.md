# M5 — Console mode for the Vita: specification

This is the maintainer's brief for milestone M5 (Vita: gamepad from start to end, 60 FPS), written down on
27 November 2026 so that nothing is lost between sessions. `CONSOLE-MODE.md` describes what exists; this file says what
must exist when M5 is done. Requirement numbers (R01...) are used in issues, commits and tests.

**Done when** (D12, phase 5): the gamepad-only script plays the whole game; the maintainer has played a full game with the
controller and approved it; 99 % of the frames of the reference scenario take at most 16.7 ms on the console (menu changes
may dip, they must stay above 40 FPS, and nobody investigates a single 40 ms menu for long).

## Scope rules
- No new feature outside this brief (D12). Ideas found on the way become issues labelled `proposal`. Bugs are fixed at once.
- Everything added looks as if it shipped with NGU Idle (game panels, buttons, fonts, hand-drawn glyphs in the game's
  style; never emoji, never borrowed art). The maintainer may redraw any glyph.
- The console mode is a presentation and input layer; game logic, scenes and saves do not change (`tools/regress.sh` and
  `just full-game` stay green).
- The maintainer is away while the first version is built: decide alone, record the decision in this file.

## Requirements

### Platforms and settings
- **R01** The console mode is the same on every platform (Vita, PC, web, later consoles). It stays a setting in the engine
  (Settings > Page 2). **On the Vita it is always on and cannot be switched off**: the Vita host removes the setting.
- **R02** Scale buttons 100 %, 110 %, 120 % and a new **130 %** (not more). The Vita default is **120 %**.
- **R03** The Vita touch screen stays usable together with the controller. The rear touch pad does nothing.
- **R04** A new setting, in English, switches the **cursor memory** (R15) on or off; on by default
  (proposed name: "Remember Cursor").

### Buttons (PlayStation layout, same on every console page)
- **R05** Cross validates (not circle, whatever the console region). Circle goes back / closes. Square is the game's right
  click ("Cap" on allocation rows). Triangle shows or hides the selected control's information.
- **R06** The **triangle information is a toggle**: pressed once it stays on while the selection moves to other items;
  pressed again it goes off and stays off. (Applied to the other toggle-like buttons only where it feels natural.)
- **R07** L and R tap: previous / next menu. L held: the menu list replaces the player column (existing).
  **R held** (same 250 ms gesture as L): the game's keyboard shortcuts become buttons (recover all energy, all magic,
  and the other shortcuts of the original). R pressed together with another button does **not** change menu; the other
  button gives the shortcut. A plain R tap changes menu. No accidental shortcut is possible.
- **R08** Start opens **Settings**. Select opens **Info 'n Stuff**.
- **R09** Left stick and d-pad move the selection; right stick: see R16 (free cursor) and scrolling.

### Screens
- **R10** The big green **4G's Sellout Shop** button at the bottom of the menu list moves to the middle of
  **Info 'n Stuff** (same function). Holding L shows **all** menu entries at once, at the chosen scale, **without a
  scrollbar**; the buttons may be as large as the screen allows.
- **R11** **Number entry**: pressing cross on a number field (for instance the input at the top of Basic Training)
  opens an in-game **number pad** drawn in the game's style (digits, K / M / B, x2, /2, Max, Half, Idle, Clear, apply with
  Start or its own button). It replaces the system keyboard for every field that expects only digits. Other text fields use
  an in-game keyboard in the same style, **except the player's name**, which keeps the console's own keyboard.
- **R12** **Allocation rows** (+ / - / Cap): left / right give - / +, holding repeats with acceleration, square is Cap.
- **R13** **Adventure**: the face buttons fire the attack and defence skills of the current page; the page changes with a
  button that is not Select (to be chosen from the code and the wiki; Select is Info 'n Stuff); left / right change zone.
  The 18 keyboard actions of the original (Q W E R T Y / A S D F G H / Z X C V B and arrows) must all be reachable.
- **R14** **Inventory**: cross picks up and puts down (merging when it is put on an item of the same kind on the selected
  slot), square equips at the right place, triangle shows information (toggle, R06), circle puts the item back.
- **R15** **Cursor memory**: each menu remembers its last selection; the first time, the main control is selected.
  Can be switched off (R04).
- **R16** **Free cursor**: a small cross moved with the right stick, with the same effect as the mouse on PC (hover,
  highlight in the console selection colour). Any other input, anywhere, hides it and goes back to control-by-control
  movement. Low priority: little time on it.

### Help and texts
- **R17** **No help pictograms spread over the game.** Only replace what the game already shows as a key prompt:
  the blinking arrow at the top right of the tutorial windows, and the texts of tutorials and "How to play" that name a
  key or the mouse: in console mode they use controller glyphs drawn in the game's style (`CONSOLE-MODE.md`, glyph sheet).
  Never an emoji.
- **R18** **Tutorials work with the controller**: wherever a cursor was needed to advance (the ADVANCE window with its
  arrow, welcome screens, endings), cross advances.

### Layout and performance
- **R19** **Layout by importance**: the parts of a page used all the time get the room; rarely used parts (for example the
  upper block of Basic Training) may shrink when zoomed. Dose so that the useful areas stand out. Per-menu
  reorganisations noted before (Fight Boss, Money Pit, Inventory) are in scope when needed for comfort at 5 inches.
- **R20** **Performance**: 60 FPS on 99 % of the frames in normal play; menu changes may dip but stay above 40 FPS; quick
  fixes only for single slow menus (`budgets.json`, `tools/vita-cycle.sh`).

### Work method
- **R21** Study the whole game first (wiki, code, counting what is used how often), keep the study in the local work
  folder (not in the repository), derive tests from it, then build.
- **R22** Tests: any number of saves, including end-game saves made with the developer mode / cheats; PC, web and Vita; the
  gamepad-only script (`NGU_GAMEPAD_ONLY`) over every menu; captures at 960 x 544 for review.
- **R23** The Vita is plugged in, charging, game closed: it may be used freely (install, launch, capture, measure).
  One data folder, `ux0:data/nguidle`, with `ngu.pak` only (plus saves and log); the package is installed, not kept there.

## Order of work
1. Study of the game and of the controller code (local notes). 2. Gamepad-only script over every menu; fix the traps found
(unreachable controls, stuck dropdowns, text cut off). 3. Global buttons (R05-R09), Vita defaults (R01-R04), shop button
(R10). 4. Special screens: number pad (R11), allocation (R12), Adventure (R13), inventory (R14), tutorials (R17, R18), cursor
memory (R15). 5. Layout by importance (R19), free cursor (R16). 6. Measure and fix performance on the console (R20). 7. Full
test run, captures, maintainer playtest and adjustments.

## Decisions taken while building (28 November 2026)
The maintainer was away; each of these is open to correction after his playtest.
- **R05 / R06** triangle is a toggle kept in the console state (`Console::info`); when it is off the selection does not raise the
  game's hover tooltips (`tooltips_on`).
- **R07** the R layer is decoded by `clr/src/gamepad.rs` (hold of 250 ms, or any other button pressed with R); it presses the
  game's own keys (the key edge of the engine), so every original shortcut works the way the game wrote it. Adventure: three pages
  of six skills (attacks W E R T Y Q, defence A S D F G H, special Z X C V B), page changed with R + L, zone with left / right.
  Fight Boss: the combination lock keys. Inventory: pages, ctrl + click, A, D. Elsewhere: shift + click, and the game's R, T, F.
  The original's other held-key clicks (Q / W / E + click to transform a boost) are not mapped.
- **R10** the shop button is a copy built from the game's own button (same look and click) placed in Info 'n Stuff; the original is
  hidden in console mode. The menu list shows every slot at the game's size in one column, scrolling inside its frame when the rows are too few (two columns were tried and rejected on 28 November).
- **R11** a field takes the pad when its content type is a number, or when it holds only digits (the Input box of Basic Training
  is typed as text in the game, "100 %" in the cap modifier too). The pad is modal: nothing behind it can be selected.
  Other text fields use the host's keyboard: the maintainer chose it on 28 November 2026 (no in-game keyboard). Max, Half
  and Idle are not on the pad: the same screens show the game's own Cap, 1/2, 1/4 and Idle buttons next to the field.
- **R12** an allocation row is one selection stop (its "+" button); the "-" and "Cap" buttons are not stops. Left / right and
  square act on them. Hold repeats at the decoder's rate (every 0.12 s, every 0.05 s after 1.5 s held).
- **R14** an item is picked up when the control has a drag handler: cross presses and holds, cross again releases on the
  selection (merges and swaps are the game's), circle releases where it was picked up. The PC window clicks whole.
- **R15** the memory is applied two frames after a menu opens, when its page has been laid out; the first time the selection
  goes to the first control of the page content (reading order). Setting "Remember Cursor" in Settings, page 2.
- **R16** dropped after the maintainer's playtest (28 November): the free cursor was unpleasant. The right stick scrolls the
  part under the selection, else the page's biggest scrolling part. The 130 % scale is dropped too.
- **R17 / R18** only the tutorial prompts and the sentences naming the arrow keys are replaced; the glyphs are drawn by
  `tools/glyphs.py` in the game's style. In a tutorial with no selection, cross goes on and circle goes back.
- **R19** only the generic fit and the squeezed menu list so far; per-menu work waits for the playtest.
- **Uniform scale** (maintainer, 28 November): the scale must keep every proportion (button / text, title / text), like the
  game's own resolutions. A first try that enlarged only the texts was dropped. The view still fills the whole screen at
  any aspect ratio (the project's first rule): an attempt that centred a fixed-width page with empty sides was reverted.
- **Selection and information** (checked on the console with a save holding items, 28 November): a control whose graphic is
  a child (item and equipment slots, toggles) shows the selection as a frame; a control that draws nothing (a locked accessory
  slot) is not a stop; with the information off (triangle) the selection hovers nothing, so no item tooltip covers the bag.
  Picking up, carrying, merging (two Sticks -> level 22), equipping (square) and protecting (R + square) work on the device.
- **Shortcuts from the wiki study:** inventory protect (R + square), boost transforms (R + up / down / L), A and D as key + click
  (the game ignores the bare key unless "Simple Inventory Shortcuts" is on); cards protect and discard.
- **Not started:** glyphs inside long texts, the browser's gamepad (issue 82), the performance pass.
