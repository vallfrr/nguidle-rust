# Console mode: target design

A gamepad- and touch-first presentation of the same game, shared by every console port (Vita first, later PlayStation and
Switch). It is built and tested on PC first, where it is **off by default** and enabled in the settings; PC players keep the
original layout. The game logic, scenes and saves are unchanged: the console mode is a presentation and input layer of the
engine, and `tools/regress.sh` must stay all OK.

Status: design stage. The mockups were reviewed by the maintainer on 2026-11-22; the final look is being settled before any code.

## Art direction (mandatory)
Everything added must look as if it had shipped with NGU Idle.
1. **Reuse the game first**: its panels (`Border`, `BorderRed`, the corner screws), buttons (`ThinBorderWhite`, `ThinBorderBlue`,
   tinted like the menu buttons), its scrollbar (`UISprite`), its fonts (title font for headings, Liberation Sans for text),
   its colours, and its own "selected" mark (yellow frame with a dark edge, as on the Adventure skill bar).
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
- **Top bar** (game panel): energy, magic, HP bars, gold, and the current menu name between the L and R glyphs.
- **Content**: the current menu only, scaled to the full width (about 1.4x the PC size). Taller menus scroll and follow the
  selection; the game's scrollbar shows the position.
- **Prompt bar** (game panel): the actions available for the current selection, glyph + plain label. It changes with the context.
- The player column and the menu list are not shown permanently (see controls).

## Controls (PlayStation layout)
| input | action |
|---|---|
| d-pad / left stick | move the selection |
| cross | press the selected control |
| circle | back / close |
| square | secondary action: the game's right click (equip, +10 zones...), "Cap" on allocation rows |
| triangle | show / hide the selected control's tooltip, docked opposite the selection; on a number, open the amount pad |
| L / R | previous / next menu |
| START (hold) | menu grid: every menu as its real button; release to open the chosen one |
| SELECT | player column drawer (stats, rebirth, save) |
| right stick | scroll the current panel |

Context rules:
- **Allocation rows** (+ / - / Cap): left/right = - / +, holding repeats with acceleration, square = Cap, triangle = amount pad.
- **Adventure**: the four face buttons fire the four skills of the current page; holding R switches page (Attacks, Defense,
  Special); left/right change zone.
- **Inventory and loadouts**: cross picks up and drops (merge as in the game), square equips, triangle shows info, circle puts
  the item back; the rear touch pad changes pages on the Vita.
- **Instant feedback**: the top bar shows the next menu name on the very next frame after L/R, before the game has finished
  building the menu.
- Selection is remembered per menu; entering a menu for the first time selects its main control.

## Touch (Vita, later Switch)
- Tap = click. Touch targets grow to finger size (nearest control within a few millimetres).
- Hold = the game's right click, shown by a yellow ring in the game's colours; hold on a name = its tooltip.
- Swipe scrolls; a swipe from the left edge opens the player column; the rear pad scrolls lists and changes pages.

## In-game text and number entry
No system keyboard: the game opens its own **amount pad** (digits, K / M / B, x2, /2, Max, Half, Idle, Clear) in a game panel,
usable with buttons and touch. Free text (save names) uses an in-game keyboard in the same style.

## Order of work
1. Final look: mockups built from real renders of our port + our glyphs, validated by the maintainer.
2. PC implementation behind a settings switch (off by default), playable with a gamepad; tested with scripted scenarios,
   captures and `tools/regress.sh`.
3. Maintainer playtest and validation.
4. Vita port of the console mode (touch, rear pad), then frame-time checks on hardware.
