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
  In the Windows GPU window, the keyboard **L** uses the same 250 ms tap/hold gesture as the controller. While L is down,
  Up and Down open the list immediately and move the selection, without toggling F1.
- **Scale**: 100 % is the PC layout with the menu column removed and existing scroll views. 110 % and 120 % draw everything
  bigger; what no longer fits is handled in this order, and only where needed:
  1. empty bands between contents get thinner. If the regular spacing does not fit, the layout tightens internal gaps
     further while retaining space at the frame edges. The settings columns can use their own vertical spacing;
  2. the active menu can keep one scrollbar inside an existing list or a designated content block;
  3. the player column and the fight screens (Adventure, Boss) are pressed together instead of scrolling (controls get
     slightly shorter, texts keep their size); other pages are pressed together when the overflow is small (up to 12 %);
  4. any remaining overflow scrolls within the selected block. Its frame, bolts, and fixed heading stay in place.
     If no block is eligible, the layout fits the controls and adjusts text that would be cut off.
- **Scrolling**: the right stick, pointer wheel over the block, dragging the thumb, and directional focus all use the same
  viewport. The authored scrollbar sprites change with the active theme. A new menu resets the extra console offset.
- **Selection**: the port's yellow fill inside the selected control, as on PC and Vita. Square = the game's right click.
- The top and bottom sidebar junctions stay on the frame edge at all three scales. The welcome and offline-progress screens
  fit within the chosen viewport, and Save Game and Load Game occupy the right pane. When L is held, controller movement stays
  in the menu list. Turning the mode off clears the console viewport and its clipping rectangles.
- The new-game and ending screens also fit inside the console viewport. Illustrated endings use separate columns for
  the artwork and text, with Continue under the artwork. Dense ending text uses the game's text fitting within its column.
- Verified with 696 scripted captures across 58 menu indices and 12 configurations. Normal was checked at all three
  scales and resolutions (960 x 544, 1280 x 720, 1920 x 1080). Dark, Gold, and Dark Gold were checked at 120 % in
  960 x 544. Every capture had at most one content scroll region. The 58 PC captures with console mode off matched the
  previous revision pixel for pixel at 1280 x 720.
- Wheel input outside the Basic Training and Inventory blocks left their right panes pixel-identical. Dragging the
  Basic Training thumb and using the wheel both reached the same final row. The right stick was replayed in Wishes.
  An authored list at 100 % also scrolled inside its viewport and returned to the original pixels when scrolled back.
  The introduction and all seven endings were replayed at all three scales at 960 x 544.
  Live theme changes were also checked in the Windows GPU window. A physical gamepad playtest
  (stick speed, the 250 ms hold delay of L and d-pad repeat) and the Vita implementation are pending.
- **Known limits** (found while verifying, not fixed):
  - Money Pit and Fight Boss use the generic fit only: no per-menu layout yet, so e.g. the Money Pit picture
    still stretches with the page width as on PC;
  - menus 1 and 2 are empty (they are empty in the PC mode too: no button leads to them), menu 45 / 46 are the two
    "Choose your prize" screens of the game;
- **Examples at 120 %**: Basic Training scrolls its Defense skill rows with Attack and the headings fixed. Inventory
  scrolls its item grid. Settings Page 2 scrolls the Loot Filter block. Wish and card lists use their grid as the viewport.
  Steam Achievements fits its grid, details and pagination with the three tabs in the right pane.
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
   scroll views, empty bands) with `rows_of` and shrinks gaps first. The page's `fit` selects at most one scroll block.
   Prefer a named content block or an authored scroll view, with a bounded subpanel as a fallback.
   Positions come out as `rects` overrides (`view_xf` reads them), so the game scripts never see the change.
4. `clip_scroll_child` limits both rendering and hit tests to the same moving children. Frame corners stay on the
   block edges. `reserve_scrollbar_gutter` keeps content clear of the thumb, and cached quads depend on `pan_stamp()`.
5. `ScrollRegion` holds the viewport, track, console offset range, and native scroll range. Its `grip` computes the thumb
   used by rendering and pointer input. `console_mode::scroll_region` combines the native and console offsets into one
   range. `COLUMN` and `LIST` are independent sidebar regions.
6. Input: `Console::input(Pad)` maps pad events; the PC shell (`crates/ngu-desktop/src/gl.rs`) feeds it from `pad.rs`
   (gilrs) and the mouse wheel, and turns returned `Click`s into the two-step synthetic click the PC shell already used.
7. `Renderer::draw_console_bar` paints the track inside its owning block and uses that block's clip and scene order.
   The shared layout reserves a gutter within the view, clear of the outside frame. The game sprites follow
   `UIThemeController.curTheme` for Normal, Dark, Gold and Dark Gold.
8. The settings are real game objects (`prepare`): captions and buttons cloned from the Tooltips setting, kept in sync with
   the theme by `follow_theme`.

Code: `clr/src/console_mode.rs` (settings, scale, controls, scrolling), `clr/src/desktop_layout.rs` (`console_vertical`,
`fit`, `scroll_block`: the layout), `clr/src/render.rs` (`draw_console_bar`, clips), `clr/src/focus.rs` (`refresh_rect`,
`in_scroll_view`), script steps in `clr/src/script.rs`, gamepad timing in `crates/ngu-desktop/src/pad.rs`, PC window in
`crates/ngu-desktop/src/gl.rs`. Test window without the native build: `ngu-window` (see `docs/DEV.md`).

### How to change it
- Scale steps: `SCALES` in `console_mode.rs`; the zoom is capped so the page never gets narrower than `MIN_WIDTH`.
- Fit tuning: `PAD`, `GAP`, `TIGHT_GAP`, `EDGE_GAP`, `LIST_MIN` and `SQUEEZE` in `desktop_layout.rs`; screens that must never scroll:
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
