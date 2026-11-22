# Console mode

A gamepad-first presentation of the same game, shared by every console port (Vita first, later PlayStation and Switch). It
is built and tested on PC first, where it is **off by default**; PC players keep the original layout. The game logic, scenes
and saves are unchanged: the console mode is a presentation and input layer of the engine, and `tools/regress.sh` must stay
all OK.

## Where it stands (2026-11-23)

Implemented on PC (Windows window and the scripted captures), not yet on the Vita:
- **Settings > Page 2 > Bloopydoops** (the game's empty column): *Console Mode* On / Off and *Console Scale* 100 % / 110 % /
  120 %. Both are built from the game's own setting buttons, follow the UI themes and are kept in PlayerPrefs
  (`nguConsoleMode`, `nguConsoleScale`).
- **Menus without the menu column**: tap L / R for the previous / next menu; hold L and the menu list replaces the player
  column (bigger buttons, it scrolls with the cursor); release L to open the menu under the cursor.
- **Scale**: 100 % is the PC layout with the menu column removed (no zoom, no added scrollbar). 110 % and 120 % draw everything
  bigger; what no longer fits is handled in this order, and only where needed:
  1. empty bands between contents get thinner;
  2. the game's own scroll views (a skill list, the augment list, a log...) get shorter and keep scrolling;
  3. the player column and the fight screens (Adventure, Boss) are pressed together instead of scrolling (controls get
     slightly shorter, texts keep their size); other pages are pressed together when the overflow is small (up to 12 %);
  4. what still does not fit scrolls, only inside the page frame: the bordered frame (and its bolts) stays fixed and the
     content slides under it, clipped to the inside of the border (`freeze_frame` in `desktop_layout.rs`), with a scrollbar drawn from the game's own scrollbar images (the Augments menu's).
- **Scrolling**: right stick (the part under the selection, else the page's list), mouse wheel (the part under the pointer),
  and the selection itself: moving past the last visible row scrolls the part and reaches the next row. A new menu starts at
  its top.
- **Selection**: the port's yellow fill inside the selected control, as on PC and Vita. Square = the game's right click.
- Verified (2026-11-23), scripted captures of the 58 menus at 100 / 110 / 120 % in 1280 x 720, 120 % in 960 x 544 (Vita size)
  and 1920 x 1080 (Dark theme), 100 % in 960 x 544 (Gold theme); stick, wheel and d-pad scrolling, hold-L list, L / R; PC mode
  pixel-identical to before when the console mode is off; `tools/regress.sh` all OK.
- Not verified: a physical gamepad on this version (the maintainer's playtest), mouse wheel in the real window.
- Not done yet: per-menu reorganisations beyond the generic fit (the maintainer's notes: Fight Boss bottom smaller and
  portraits bigger, Money Pit picture proportions, Inventory bigger), amount pad, in-game keyboard, Adventure face-button
  skills, inventory pick-up with buttons, the glyph prompts, touch, the Vita port.

Code: `clr/src/console_mode.rs` (settings, scale, controls, scrolling), `clr/src/desktop_layout.rs` (`console_vertical`: the
fit), `clr/src/render.rs` (`draw_console_bars`), gamepad timing in `crates/ngu-desktop/src/pad.rs`, PC window in
`crates/ngu-desktop/src/gl.rs`. Test window without the native build: `ngu-window` (see `docs/DEV.md`).

### How to change it
- Scale steps: `SCALES` in `console_mode.rs`; the zoom is capped so the page never gets narrower than `MIN_WIDTH`.
- Fit tuning: `PAD`, `GAP`, `EDGE_GAP`, `LIST_MIN` and `SQUEEZE` in `desktop_layout.rs`; screens that must never scroll:
  the names in `has_named(..., &["Adventure Menu", "Boss Menu"])`.
- Scroll speed: `STEP` (d-pad at an edge, one wheel notch) in `console_mode.rs`, stick speed in `pad.rs`.
- Debug: `NGU_CONSOLE_DEBUG=1` prints each fit (rows, runs, what scrolls); the script step `consolestate` lists the parts that
  scroll.

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

## Screen layout
- **Left column**: the game's own player column (stats, rebirth, spend EXP, save/load, settings); it can be selected like the
  rest. Its build number is hidden when zoomed in.
- **Content**: the current menu, laid out by the PC layout in all the remaining width, at the chosen scale.
- **Prompt bar**: dropped (maintainer's decision, 2026-11-22).

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
| right stick | scroll the part under the selection (else the page's list) |

Context rules (planned; only L / R, hold L, cross, circle, square and the scrolling are implemented):
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
