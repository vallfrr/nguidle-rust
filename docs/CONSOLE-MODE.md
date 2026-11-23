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
- **Not verified**: a physical gamepad with this version (the maintainer's playtest is pending; stick speed, the 250 ms hold
  delay of L and the d-pad repeat are the first things to tune), the mouse wheel in the real window (verified with the
  `wheel` script step only), the Vita itself (nothing of the console mode runs there yet).
- **Known limits** (found while verifying, not fixed):
  - a page that scrolls to its end leaves some empty frame colour under its last content (the scroll range includes the
    bottom margin that the squeeze removed);
  - a control that sits for a moment under the bottom border can still be selected with the pad (the clip only hides it);
  - the focus rectangle of a control inside a clipped region is not clipped (it is drawn as the game draws it);
  - Money Pit, Fight Boss and Inventory use the generic fit only: no per-menu layout yet, so e.g. the Money Pit picture
    still stretches with the page width as on PC;
  - menus 1 and 2 are empty (they are empty in the PC mode too: no button leads to them), menu 45 / 46 are the two
    "Choose your prize" screens of the game;
  - 110 % never needs a scrollbar on the 58 menus; 120 % scrolls 24 of them (see the table below).
- **What scrolls at 120 % (1280 x 720, measured with `consolestate` over the 58 menus; menu index = `MenuSwapper` id, `menu,N`)**:
  - the page itself, inside its fixed frame: menus 4, 11, 12, 14, 18, 19, 20, 24, 25, 28, 30, 31, 39, 40, 41, 42, 44, 48,
    49, 50, 52, 53, 54, 55;
  - only a game list (the page fits, its list scrolls like in the game): menus 3, 5, 10, 13, 15, 21, 22, 32, 33, 34, 35, 38,
    43, 51 (Basic Training's skill lists are menu 0 and scroll with the page header fixed);
  - nothing scrolls: the others, and none at 100 % and 110 %.
- **Not done yet**: per-menu reorganisations beyond the generic fit (the maintainer's notes: Fight Boss bottom part smaller
  and portraits bigger, Money Pit picture proportions, Inventory bigger), amount pad, in-game keyboard, Adventure
  face-button skills, inventory pick-up with buttons, the glyph prompts (the glyph sheet below is design only), touch, the
  Vita port (the layout code lives in the shared engine, so the Vita host only has to provide L / R / hold L, the stick,
  the scale preference and the same `console_mode::sync` / `fit_screen` calls each frame).

### How it works (for whoever continues)
1. `console_mode::sync(vm)` (every frame, shell side) turns the mode on or off from the preference, hides the menu column
   (`Features Canvas`), the edge bolts and, when zoomed, the build number, and creates / drops `Engine::console`.
2. `console_mode::fit_screen(vm, w, h)` picks the scale: `view()` returns (pixels per game pixel, game rows shown,
   logical width); at 100 % it is the PC fit, above it the zoom is capped so the logical width stays >= 834 rows.
   `DesktopLayout::console_view_h` receives the rows shown.
3. `DesktopLayout::rebuild` runs the PC layout (horizontal pass, the page starts where the menu column was) and then
   `console_vertical`: for the player column, the menu list (L held) and the page it computes `runs` (solid content, game
   scroll views, empty bands) with `rows_of`, shrinks empty bands to `GAP` / `EDGE_GAP`, then scroll views to `LIST_MIN`,
   then (columns, fights, small overflows) presses everything proportionally, and only then leaves `rem` rows to scroll.
   Positions come out as `rects` overrides (`view_xf` reads them), so the game scripts never see the change.
4. A scrolling page keeps its frame: `freeze_frame` pins the bordered panel and registers `clips` (the inside of the
   border) which `render.rs` applies to the children; the cached quads of a subtree depend on `pan_stamp()`.
5. `ScrollRegion`s (`DesktopLayout::regions`, rebuilt every layout) describe what scrolls: key (`PAGE`, `COLUMN`, `LIST` or
   a game ScrollRect's viewport), the view in game rows, how many rows are hidden (`extra`), and for a game scroll view
   its own range. `console_mode::scroll_region` moves a region (the game's own scroll first when going down, the squeeze
   offset first when going up) and relayouts.
6. Input: `Console::input(Pad)` maps pad events; the PC shell (`crates/ngu-desktop/src/gl.rs`) feeds it from `pad.rs`
   (gilrs) and the mouse wheel, and turns returned `Click`s into the two-step synthetic click the PC shell already used.
7. Scrollbars are drawn last by `Renderer::draw_console_bars` with the Augments menu's own scrollbar images; the bars are
   `DesktopLayout::bars` (track rectangle, handle start and length).
8. The settings are real game objects (`prepare`): captions and buttons cloned from the Tooltips setting, kept in sync with
   the theme by `follow_theme`.

Code: `clr/src/console_mode.rs` (settings, scale, controls, scrolling), `clr/src/desktop_layout.rs` (`console_vertical`,
`fit`, `freeze_frame`: the layout), `clr/src/render.rs` (`draw_console_bars`, clips), `clr/src/focus.rs` (`refresh_rect`,
`in_scroll_view`), script steps in `clr/src/script.rs`, gamepad timing in `crates/ngu-desktop/src/pad.rs`, PC window in
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
1. [x] Final look: mockups (2026-11-22, rejected once for ignoring the PC layout, then redone on it).
2. [x] PC implementation behind a settings switch (off by default); scripted captures, `tools/regress.sh` (2026-11-23).
3. [ ] Maintainer gamepad playtest and validation: **next**.
4. [ ] Per-menu reorganisations, amount pad, in-game keyboard, glyph prompts, face-button skills.
5. [ ] Vita port of the console mode (touch, rear pad), then frame-time checks on hardware.

## Journey, step by step (2026-11-22 to 2026-11-23)
Everything that led here, in order, with the commit (nguidle-rust unless noted) and what was decided or learned.

### A. Vita smoothness (docs/VITA-60.md has the measurements)
1. Scripted play sessions, per-frame trace and analyzer on the console (earlier commits `6b04260`, `8fde8d3`).
2. Optimisations measured on hardware: base64 without cloning arrays (`9506ccf`), hoisted flags in native `specBonus`
   (`8eb7ce7`); first 2 h `monkey` soak (`7d99453`): no crash, 54 fps presented.
3. Two-core Vita host (nguidle-vita `8b37a5c`, `2cb8fe1`): logic on one core, rendering / presentation on the other, triple
   buffer, texture and atlas hand-over, asynchronous log, in-game screenshot scenario verb (`ef72b6c`); the `late` scenario
   (advanced save). Result: 59.9 fps presented with 1 to 3 late frames per scenario.
4. Second 2 h soak on the two-thread build (`15260dc`, `bc1f8c4`): 431,842 frames, no crash, 59.9 fps, 4 late frames, longest gap
   40 ms. Lower GPU / CPU clocks were also tried and hold 60 Hz, but game logic gets slower, so fast clocks stay.
5. What is not solved: simulation latency. A menu switch still costs ~65 ms of interpreted work (`updateBonuses` x 11,
   `updateEquipmentBonus`; ~95 k instructions), first visits more (Cooking `getOptimalScore`, up to ~600 ms in the soak).
   The picture stays at 60 Hz, a button can answer 4 to 20 frames late. Only a change-tracking cache in the engine could
   cut it, at the risk of diverging from the original; not done.
6. vitaGL's boot logo removed (nguidle-vita `48ba596`: `--wrap=invoke_splashscreen`); the standard VPK was reinstalled on the console.

### B. Console mode design (docs/CONSOLE-MODE.md)
7. Ten mockups of a console UI (overlay look): approved in principle; the maintainer then made the art direction mandatory:
   everything must look as if it shipped in NGU Idle, glyphs drawn by us in the game's style, never borrowed
   (`9f6d689`, rule in AGENTS.md / VISION.md; private memory notes).
8. Mockups v2 (game panels, buttons, fonts + hand-drawn PlayStation glyphs, `local/poc/glyphs.py`, not versioned) and v3
   (idea of the maintainer: keep the player column, menu list on hold of L, page zoomed in the freed width).
   Decisions: no prompt bar, in-game amount pad instead of the system keyboard, PlayStation layout first, console mode
   switchable in the settings and off by default on PC, jokes only in headings and sparingly.

### C. Console mode on PC
9. `f4ecfd0` first version (Settings switch, hold-L list, L / R): a plain zoom of the 960 x 600 page. Rejected by the maintainer
   ("horrible"): two scales side by side, pages cut, extra bolts, wrong selection frame. Lesson: judge renders at the real
   window sizes and compare with the PC mode, not only at 960 x 544.
10. Rebuilt on the PC responsive layout in the freed width (same commit series): every menu keeps its native arrangement.
11. `c02f573` the selection mark goes back to the port's yellow fill inside the control (an outer frame had been invented).
12. `34a7296` zoomed view with the first scrolling parts (Basic Training body), game scrollbar images, stray bolts hidden,
    menu list scaled to the column and following the cursor. Rejected: scrollbars everywhere, no scale control.
13. `e9d4870` scale setting 100 / 110 / 120 % (Settings page 2), fit only what overflows (thin gaps, shorter game scroll
    views, pressed column and fights, scroll the rest), stick / wheel / selection scrolling, theme sync, test window
    `ngu-window`, script steps. Verified on 58 menus x 6 configurations.
14. `0317a89` a scrolling page keeps its frame and bolts fixed, only the inside slides (clipped); found by the maintainer
    after a review of the captures.
15. Docs brought up to date (this commit): CONSOLE-MODE (status, how it works, limits, decisions), DEV, ARCHITECTURE, STATUS,
    NEXT, AGENTS, the ngu-devmode and ngu-verify skills.

### D. Next
Maintainer gamepad playtest -> per-menu reorganisations -> amount pad / keyboard -> face-button skills -> Vita port (see NEXT.md B-console).

## History of decisions
- 2026-11-22 mockups v1-v3 (own overlay look) rejected: not the game's look and a plain zoom cut the pages. Rule kept: only
  the game's panels, buttons, fonts and our hand-drawn glyphs.
- 2026-11-22 first PC version: a profile-region zoom of the 960 x 600 page. Rejected ("horrible"): two scales side by side,
  pages cut, extra bolts. Replaced by the PC layout in the freed width.
- 2026-11-22 the selection mark stays the yellow fill inside the control (an outer frame was tried and removed).
- 2026-11-23 scrollbars only where needed, scale setting (100 / 110 / 120 %), frame fixed while its inside scrolls.
- No prompt bar at the bottom (maintainer's decision).
