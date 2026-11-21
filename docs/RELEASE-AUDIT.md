# Desktop release audit

The release order is Windows, self-hosted web, then PS Vita. A checked item
requires evidence from the target runtime.

| Area | Current evidence | Remaining coverage |
| --- | --- | --- |
| Game rules and saves | Differential regression compares generated Rust with the reference IL interpreter. The suite covers number formatting, input, a Rebirth energy reset, deterministic random input and save round trips. | Compare progression timing against the shipped game and exercise the remaining progression-dependent states on valid saves. The interpreter shares the UI layer and cannot independently certify the presentation. |
| Desktop presentation | Captures of all 58 menu indices at 1280 × 720, plus Inventory at 4K, Rebirth at 4:3 and extreme window shapes. Two resolution buttons and the F11 restoration of window size and position were verified in the Windows GPU window. The milestone page follows the four standard themes. Some raw menu indices select pieces of a combined page. | Check frame pacing in physical high-refresh and 4K modes, and remaining late-game interactions through the public controls. |
| Hover and navigation | Pointer hit tests batch all candidate points for navigation. The game renders focus tint inside its controls. It draws the hover panel above the page, including Save Game and Load Game. Right stick targets scroll viewports. D-pad and confirm work in Adventure's dropdown in scripted input. | Verify physical controller use across a long session, multiple scroll views, drag/drop, secondary clicks, combat shortcuts and number entry. |
| Offline milestones | The bundled game supplies the platform milestone catalog and conditions. A clean profile reported 0 of 61, the boosted fixture 44 of 61, and the all-conditions fixture 61 of 61. The 44 unlocks survived a fresh process. The in-game achievements that grant BP remain available in Info 'n Stuff. | The page displays the installed catalog text. Compare metadata differences with the store listing when reviewing the PC preview. |
| Deployment | The Windows preparer validates 115 SHA-256 hashes and extracts from the supplied data directory. The exported scene loaded in a clean profile. | Validate release packaging on a fresh Windows install. |

## Repository boundaries

- **nguidle-rust** owns extraction, IL conversion, runtime compatibility, the UI scene model, layout, draw commands, focus and scrolling. The Windows executable owns OS input and audio, high DPI, display settings, frame pacing and filesystem UI. The desktop save and milestone pages are enabled explicitly by the desktop entry point. The Vita C API keeps its separate entry point.
- **nguidle-web** currently has a design README. Its implementation belongs in this repository with a pinned engine dependency, a read-only bind mount for the complete game data, a supported file-hash manifest, an isolated build cache and browser persistence. The page should be served locally by default. Missing or mismatched files must prevent conversion and serving. An eventual support matrix has to track the accepted original builds.
- **nguidle-vita** owns the VitaSDK host, packaging, touch/buttons and console deployment. V1 runs the interpreter. V2 still needs a generated-Rust build, the console renderer and memory/frame pacing tests on device. The 5-inch interface needs its own layout and interaction validation.

The layout/render pipeline is shared. Windows paths, desktop slot management
and monitor refresh policy belong behind the desktop entry point. Current
layout transformations and integration hooks live in `clr`; review their API
when the web runtime has a concrete integration.

## Proposals for review

- Desktop and Vita controller profiles with remapping. Try a held bumper for a
  second set of commands, with separate shortcuts for inventory pages, secondary
  click, drag/drop and combat. Show the available choices in context.
- Vita quick menu and readable details. Use a hardware button to open the page
  selector, and show longer descriptions in a larger panel.
- Offer an offline way to earn the gold themes. The original conditions still
  use the purchase flag in the save, and the Windows port preserves that flag
  for imported saves.
- Web layout profiles for wide windows and touch. Allow local file import/export
  alongside browser autosave and document where its data is stored.
