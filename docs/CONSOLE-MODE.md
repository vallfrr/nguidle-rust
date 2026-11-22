# Console mode: target design

A gamepad- and touch-first presentation of the same game, shared by every console port (Vita first, later PlayStation and
Switch). It is built and tested on PC first, where it is **off by default** and enabled in the settings; PC players keep the
original layout. The game logic, scenes and saves are unchanged: the console mode is a presentation and input layer of the
engine, and `tools/regress.sh` must stay all OK.

Status: first PC version (2026-11-22), behind Settings > Some Settings > Console Mode (off by default). It reuses the PC
responsive layout (`clr/src/desktop_layout.rs`): the menu column is hidden and each page is laid out again in its width, so
every menu keeps its native arrangement and one scale; holding L lays the menu list over the player column. Code:
`clr/src/console_mode.rs`; gamepad in `crates/ngu-desktop/src/pad.rs`; interpreter-only test window `ngu-window`.
Not done yet: amount pad, in-game keyboard, Adventure face-button skills, inventory pick-up, touch, Vita port.

## Art direction (mandatory)
Everything added must look as if it had shipped with NGU Idle.
1. **Reuse the game first**: its panels (`Border`, `BorderRed`, the corner screws), buttons (`ThinBorderWhite`, `ThinBorderBlue`,
   tinted like the menu buttons), its scrollbar (`UISprite`), its fonts (title font for headings, Liberation Sans for text),
   its colours, and the port's focus mark (the yellow fill inside the selected control, as on PC and Vita).
2. **When the game has no such element** (controller glyphs, a touch hint), we draw it ourselves in the game's crude,
   hand-drawn style: white body, thick slightly wobbly black outline, flat colours. Never reuse glyphs or art from elsewhere.
   The maintainer may redraw any of them; the sizes below are the exchange format.
3. **No modern UI styling**: no rounded "pills", drop shadows, blur, gradients, dimmed overlays or toasts.
4. **Text tone**: headings may carry a short joke subtitle in the game's manner ("HOW MUCH? (MATH IS HARD)"). One line, only
   where a precise label is not needed. Prompts, buttons and categories stay plain and exact.

### Glyph sheet (transparent PNG, drawn at 2x the on-screen size)
| glyph | size (px) |
|---|---|
| cross, circle, square, triangle | 48 x 48 |
| d-pad (neutral, left/right, up/down, all), left stick, right stick | 48 x 48 |
| L, R | 64 x 40 |
| START | 92 x 40 |
| SELECT | 96 x 40 |
| front touch, rear touch | 48 x 48 |

PlayStation layout only for now; other consoles get their own sheet later.

## Screen layout (960 x 544 reference)
- **Left column**: the game's own player column (stats, rebirth, spend EXP, save/load, settings) stays on screen and can be
  selected like the rest. The menu list is no longer shown permanently.
- **Content**: the current menu, scaled into all the remaining width (about 1.2x the PC size). Taller menus scroll and follow the
  selection.
- **Prompt bar** (game panel, bottom): the actions available for the current selection, glyph + plain label, changing with the
  context; on its right, the current menu name between the L and R glyphs.

## Controls (PlayStation layout)
| input | action |
|---|---|
| d-pad / left stick | move the selection (left from the page reaches the player column) |
| cross | press the selected control |
| circle | back / close |
| square | secondary action: the game's right click (equip, +10 zones...), "Cap" on allocation rows |
| triangle | show / hide the selected control's tooltip, docked in free space opposite the selection; on a number, open the amount pad |
| L / R (tap) | previous / next menu |
| L (hold) | the menu list replaces the player column; move the cursor with the d-pad or stick, release L to open the menu under it |
| START | apply in the amount pad / keyboard |
| SELECT | next skill page in Adventure |
| right stick | scroll the current page |

Context rules:
- **Allocation rows** (+ / - / Cap): left/right = - / +, holding repeats with acceleration, square = Cap, triangle = amount pad.
- **Adventure**: the four face buttons fire the four skills of the current page (Attacks, Defense, Special); SELECT switches
  page; left/right change zone.
- **Inventory and loadouts**: cross picks up and drops (merge as in the game), square equips, triangle shows info, circle puts
  the item back; the rear touch pad changes pages on the Vita.
- **Instant feedback**: the menu name in the prompt bar changes on the very next frame after L/R, before the game has finished
  building the menu.
- Selection is remembered per menu; entering a menu for the first time selects its main control.

## Touch (Vita, later Switch)
- Tap = click. Touch targets grow to finger size (nearest control within a few millimetres).
- Hold = the game's right click, shown by a yellow ring in the game's colours; hold on a name = its tooltip.
- Swipe scrolls; touching the menu name in the prompt bar opens the menu list; the rear pad scrolls lists and changes pages.

## In-game text and number entry
No system keyboard: the game opens its own **amount pad** (digits, K / M / B, x2, /2, Max, Half, Idle, Clear) in a game panel,
usable with buttons and touch. Free text (save names) uses an in-game keyboard in the same style.

## Order of work
1. Final look: mockups built from real renders of our port + our glyphs, validated by the maintainer.
2. PC implementation behind a settings switch (off by default), playable with a gamepad; tested with scripted scenarios,
   captures and `tools/regress.sh`.
3. Maintainer playtest and validation.
4. Vita port of the console mode (touch, rear pad), then frame-time checks on hardware.
