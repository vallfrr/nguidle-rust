# Historical record

This document records earlier work. Consult the current status and roadmap for present priorities.

# Proposal: making NGU Idle playable with buttons / on a small screen

Windows and the current Vita preview support directional focus, confirmation,
dropdown navigation, menu shortcuts and right-stick scrolling. The Vita also
supports touch and wires text entry to the system keyboard. See the
[Vita hardware audit](../VITA.md#what-was-tested) for the device checks.

## 1. What the game really asks for as input (measured in the code)

| need | measure (Assembly-CSharp) | note |
|---|---|---|
| Buttons to click | 1,151 `Button`, 516 `EventTrigger` | 100 % of the game is played with the mouse |
| Hover tooltips | 97 `PointerEnter` / 96 `PointerExit` scripts | no hover on touch: V1 shows a tooltip on press and hides it on release |
| Right click | `ItemController`, `LoadoutController`, `DaycareItemController`, `DeckPodUI`, `ZoneForward/BackwardsClick`... (~25 uses) | equips / moves / "+10 zones" depending on the menu |
| Drag and drop | 10 `IDrag/IDrop` scripts (inventory, loadouts...) | works with touch drag |
| Scrolling lists | 26 `ScrollRect`, 25 `Scrollbar` | touch drag |
| Sliders | 121 `Slider` | touch drag |
| Text / number entry | 73 `InputField` (121 `Input.text` reads) | system keyboard on Vita |
| Keyboard shortcuts | 54 `GetKeyDown`, 48 `GetKey` (+ Shift/Ctrl by string name) | combat (Q W E R T Y / A S D F G H / Z X C V B + arrows), inventory pages (1-9), tutorial (arrows) |
| Mouse position | 29 `Input.mousePosition` | tooltip placement |

The original game uses a 960 × 600 canvas. The current Vita preview reflows
the shared layout at 960 × 544 (30:17). Small resource controls such as
“+”, “−”, “Cap” and “1/2” remain an area for touch and readability testing.

## 2. Principles
1. **Default = strict PC behaviour**; every adaptation is an option.
2. Do not modify scenes, assets or logic: add an **input layer** on top of the existing interface.
3. Everything testable on PC (headless harness) before the Vita.

## 3. Proposals
- **A. Gamepad cursor** (priority 1): left stick = virtual cursor, cross = left click, circle = right click, square = latch a drag, triangle = tooltip; right stick scrolls; L/R = previous/next menu.
- **B. D-pad focus navigation** (priority 1, **implemented** in `clr/src/focus.rs`): list the visible interactive elements, jump to the best neighbour in a direction, tint the control interior, use confirm to activate, and trigger `PointerEnter` for help at the focused control.
- **C. Keyboard shortcuts -> buttons** (priority 1): combat menu 18 keys -> 3 pages of 6 actions; inventory pages = L/R; tutorial/zone arrows = d-pad left/right; inject `Input.GetKey/GetKeyDown` on the VM side.
- **D. Number entry** (priority 2): on-screen numeric pad (big keys, K/M/B, x2, /2, Max) besides the system keyboard.
- **E. Readability at 5 inches** (priority 2): magnifier on long press, UI zoom by pinch, larger hit areas (+-6-8 px, nearest target wins).
- **F. Better touch** (priority 2): long press = right click, inertial scrolling, palm rejection.
- **G. Idle / battery** (priority 3): adaptive frame rate (30 while interacting, ~10 when nothing moves), resume after sleep from the clock (`Epoch.Current`).
- **H. Save compatibility** (priority 2): import/export of PC saves (.NET `BinaryFormatter` / base64 string).

## 4. Validation criteria
Verify the remaining button-only and touch-only actions and preserve the
shared game rules. The Vita target is one presented image per refresh
during play and transitions, with first visual feedback within two refreshes.
Game logic advances at 30 Hz. See the [acceptance checklist](../VITA.md#remaining-acceptance-work).
