# Ideas and known original-game bugs (from community mods)

Source studied 2026-11-20: [jshepler/jshepler.ngu.mods](https://github.com/jshepler/jshepler.ngu.mods) (BepInEx plugins for the original game, ~320 files, ~70 features). It has **no license file**: nothing is copied, it is only a map of what players miss and of what is broken in the original. Not a dependency.

## Why it is useful
- **Same code base**: the mods patch the very methods we translate, so each class name is a pointer to a hook point for our override mechanism (task R4) and a check that our behaviour matches.
- **Backlog of player wishes** for the "NGU Idle 2" direction (nothing to do now; candidates for optional profiles once the base is done).
- **List of bugs of the original game** that we reproduce faithfully today (we translate the original code): an optional "fixes" layer could correct them. Files named `...Fix`/`...Bugfix` there: basic-training auto-advance toggle not saved, daily spin, daycare shockwave, 24h challenge reward, cap-button calculations, target boxes on advanced training, Wandoos item bug, Ygg seeds display format, adventure zone dropdown, tooltip clipped at the top of the screen, offline magic gain, perk 34, Walderp double explosion and hide-and-seek.

## Candidate quality-of-life features (counts from the mod list)
- Input and navigation: tab navigation between fields, number keys change pages (ygg, diggers, beards, hacks, wishes, item list), scientific notation in number fields (`1e9`), fullscreen toggle (we have F11), copy/paste of allocations.
- Automation: auto-allocator, auto harvest of fruits, automated major quests, auto merge/transform of pendants, auto filter of maxed items, wish list/queue.
- Information: effective gold per second, damage percentages to know if a boss can be fought, richer tooltips (bars, boosts, perks, hacks, wishes, fruits, money pit), total time to max level, titan auto-kill requirements.
- Saves: auto-save on rebirth/challenge/pit, quick save/load keys, extra data persisted inside a save that vanilla still loads (we now write the real format, so the same trick is possible).
- Interface: the mods' popups use a DPI-based scaler; our responsive layout (R3) replaces that need for the core screens.
