# Responsive layout: design (tasks R2-R5)

The implementation uses plain-text [PC](../layouts/pc.txt) and [Vita](../layouts/vita.txt) profiles. Regions bind by node name beneath the root `Canvas`. See the [profile syntax](../layouts/README.md). The proposals below were written on 2026-11-20, after the UI map.

## Starting point
- The game draws into a virtual 960x600 space and believes it always does (absolute coordinates, `Screen.width/height`, menus hidden by moving them off-screen, tooltips clamped to 960x600).
- Every top-level part of the screen is its own **root canvas** in the scene: the player column (`Player Panel Canvas`), the page list (`Features Canvas`), the 58 menu canvases placed at the small/big menu anchors, the top input bar, tooltips and pop-ups. The renderer already walks root canvases one by one (`Renderer::emit`).
- 2/3 of the images are 9-sliced and only 5 menus contain a large drawn image (`docs/UI-MAP.md`): panels can be resized; menu contents are best scaled as a block.

## Mechanism: regions
The original proposal used a list of regions written in TOML. This sketch illustrates the design before the plain-text profiles were implemented.
```
[[region]]
name = "player"            # what the profile calls it
canvases = ["Player Panel Canvas"]
source = [0, 0, 170, 600]  # rectangle in the game's 960x600 space
anchor = "left"            # where it goes on the real screen: left / right / top / bottom / fill / centre
scale = "fit-height"       # fit-height, fit-width, fit, or a number
visible = "always"         # always / never / while:<action> (e.g. while:L on Vita)
```
- **Rendering**: each root canvas is drawn with the transform of its region (one uniform scale + offset). Text stays sharp (rasterised at the final size), 9-sliced images keep their borders. Canvases not listed use the default region (the whole 960x600 fitted to the window, like today).
- **Input (R2)**: a device point is mapped back by finding the region under it (top-most visible first) and applying the inverse transform, so `Engine::mouse` stays in game space and the game code is untouched. Focus navigation works on the same mapped rectangles.
- **Tooltips and pop-ups**: drawn with the transform of the region that contains the game-space mouse position (tooltips follow what they describe); pop-ups (game over, confirmations) use a centred region.
- **Scrolling a zoomed area**: a region may be larger than the screen (zoomed menu on Vita); it then gets a scroll offset that follows the focus or the touch drag, clamped to the content.
- The logic is never modified: `tools/regress.sh` must stay green with any profile.

## Profiles
- **pc (default)**: the three columns keep their proportions and are scaled to the window height; the game area is anchored to the remaining space; extra width becomes margin around the game area (decision 1 of `docs/VISION.md`: enlarge only). Scrollable lists may later show more rows.
- **vita (R5, maintainer's idea)**: page list hidden; holding L shows the page list in place of the player column, the d-pad changes the page, releasing L confirms; the game area takes the freed width, zoomed, with vertical scrolling following the focus.
- **web**: same as pc; portrait screens stack the columns (later).

## Order of work
1. Region transform in `Renderer::emit` + inverse mapping for the mouse, profile hard-coded (one region = today's behaviour) - regression unchanged.
2. Profile file reader; `pc` profile; screenshots at 16:9, 21:9, 4:3.
3. Tooltip/pop-up rules; focus rectangles through the regions.
4. `vita` profile with the L behaviour in the GPU window at 960x544 (keyboard L / gamepad L).
