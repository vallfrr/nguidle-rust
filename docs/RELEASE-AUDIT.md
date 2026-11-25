# Release audit

The release order is Windows, self-hosted web, then PS Vita. A checked item
requires evidence from the target runtime.

| Area | Current evidence | Remaining coverage |
| --- | --- | --- |
| Game rules and saves | Local regression compares the interpreter with recorded game state and save bytes. The suite covers number formatting, input, a Rebirth energy reset, deterministic random input and save round trips. | Compare progression timing against the shipped game and exercise the remaining progression-dependent states on valid saves. The interpreter shares the UI layer and cannot independently certify the presentation. |
| Desktop presentation | Captures of all 58 menu indices at 1280 × 720 and two extreme aspect ratios, plus earlier Inventory at 4K and Rebirth at 4:3. The Windows window restored its size and position after F11. Basic Training, Inventory, Cards, and Wishes were measured with VSYNC on a 200 Hz display. The milestone page follows the four standard themes. Some raw menu indices select pieces of a combined page. | Measure high-refresh frame pacing at 4K and exercise remaining late-game interactions through the public controls. |
| Hover and navigation | The maintainer confirmed smooth movement with a physical controller. Pointer hit tests batch candidate points, the focus tint sits inside controls, and tooltips use the full viewport above all desktop pages. Inventory and Cards previews use the latest pointer position. The right stick scrolls at a rate based on elapsed time. Scripted tests exercised gradual scrolling, Adventure and Beards dropdowns. | Check the revised scrolling on a physical controller. Refine focus, secondary clicks, combat shortcuts and number entry. Remapping is a separate design choice. |
| Offline milestones | The bundled game supplies the platform milestone catalog and conditions. A clean profile reported 0 of 61, the boosted fixture 44 of 61, and the all-conditions fixture 61 of 61. The 44 unlocks survived a fresh process. The Steam tab shares the original Achievements screen in Info 'n Stuff with both BP tabs. | The page displays the installed catalog text. Compare metadata differences with the store listing when reviewing the PC preview. |
| Deployment | The Windows preparer validates 115 SHA-256 hashes and extracts from the supplied data directory. The exported scene loaded in a clean profile. | Validate release packaging on a fresh Windows install. |

## Repository boundaries

- **nguidle-rust** owns extraction, IL conversion, runtime compatibility, the UI scene model, layout, draw commands, focus and scrolling. The Windows executable owns OS input and audio, high DPI, display settings, frame pacing and filesystem UI. The desktop save pages and layout are shared through the separate Vita C API.
- **nguidle-web** owns the browser input and WebGL adapter, the save API and
  slot leases, the localhost launcher, and the Compose configuration.
  The `ngu-browser` reactor runs the shared CIL engine in the browser.
  The container reads Steam Windows 1.260 data from a read-only mount and
  verifies the SHA-256 manifest before extraction or compilation. Build
  output and saves use separate volumes. The default published port is local.
- **nguidle-vita** owns the VitaSDK host, packaging, touch, buttons and console deployment. Both runtime variants booted on a console. The interpreter preview completed the 30-minute advanced-save run at 960 × 544. The 60 Hz gameplay target, two-refresh focus response, sleep/resume and remaining small-screen interactions are open. See the [Vita hardware audit](VITA.md).

See [the engine porting contract](PORTING.md) for revision pins and interface
ownership. The layout and draw list are shared. Platform entry points own display pacing,
input and save storage integration. The shared `clr` crate contains the save
interface, layout transformations and runtime hooks.

## Proposals for review

- Desktop and Vita controller profiles with remapping. Try a held bumper for a
  second set of commands, with separate shortcuts for inventory pages, secondary
  click, drag/drop and combat. Show the available choices in context.
- Vita quick menu and readable details. Use a hardware button to open the page
  selector, and show longer descriptions in a larger panel.
- Offer an offline way to earn the gold themes. The original conditions still
  use the purchase flag in the save, and the Windows port preserves that flag
  for imported saves.
