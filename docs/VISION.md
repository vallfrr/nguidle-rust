# NGU Idle Rust Engine project vision

> **Maintainer's document.** This is the authoritative document. The AI agent reads it at the start of each session and **must not modify it**.
> This is a translation of the French version in the local notes. If in doubt, the French version prevails.
> Last updated: 25/11/2026.

## 1. The objective

**An engine that runs the player's real NGU Idle on other platforms (PC, web, PS Vita), with a better interface, without ever distributing a single line of game code.**

1. The player **owns** NGU Idle on Steam (version 1.260, free).
2. An application supplied by the project finds the game, checks the version, and prepares the files **on the player's machine**.
3. The engine **reads the original game code and executes it as is**, like a player. The game's rules, calculations and progression apply.
4. On top of that, the engine displays **its own interface**: adapted to each screen, entirely playable with a controller, with save management.

## 2. What the project is not

- **Not a translation of the game into Rust.** That idea has been **abandoned permanently**. The engine is written in Rust, but the game remains the player's original code.
- **Not a remake**: the rules of the game do not change.
- **Not a distribution of the game**: without their own Steam copy, the player cannot do anything.

## 3. Fixed rules

1. **Identical for the player.** The engine can work differently from the game internally, provided the player sees no difference. A visible difference is tolerated only if **absolutely necessary**, and it must **never change the game in the long term**. Progression is as fast as in the original.
2. **A save is always a file in the original game's format**, identical on PC, web, Vita and in the original game. It is moved by copying the file.
3. **Behavior is the same on all platforms**, only the display changes.
4. **Three independent public repositories:**
   - `nguidle-rust`: engine and PC version (Windows, Linux, macOS);
   - `nguidle-web`: self-hosted web version;
   - `nguidle-vita`: PS Vita version.

   Web and Vita use only the engine, through a tagged version. **A future platform = a new repository**, without modifying `nguidle-rust`.
5. **No game code in any repository**, nor decompilation, player saves or personal information. The project's interface elements (borders, backgrounds, buttons, one image) are part of the project and serve its applications.
6. **Anyone assisted by an AI must be able to use the engine** to create their own port by reading only the `nguidle-rust` documentation.
7. **Nothing for the player to install:** ready-to-run applications at each version, with NGU Idle's visual direction.
8. **The player's original game is never modified:** we read and copy it, nothing else.

## 4. v1.0.0

v1.0.0 is released **on the same day across all three repositories**, when all of the following are true:

- [ ] The game is **100% playable**, tested end to end by script on PC, web and Vita.
- [ ] **Controller mode** works throughout **the entire game** on Vita, from beginning to end.
- [ ] Vita sustains **60 frames per second for 99% of frames**.
- [ ] **Saves transfer** between PC, web, Vita and the original game, in both directions.
- [ ] **Speed and behavior** are identical for the player across all three platforms and the original.
- [ ] PC works on **Windows, Linux and macOS**, manually tested on all three by the maintainer.
- [ ] **No known bugs.**
- [ ] The **applications** exist: the PC app (which prepares and launches the game) and the Vita preparation app (supplied alongside the VPK).
- [ ] A **README** clearly explains how to install and use everything.

**No new features in v1.** We perfect what exists.

## 5. Reserved to the maintainer

- Add or remove a v1 feature.
- Create or modify a GitHub milestone.
- Approve whether an improvement issue found by the AI enters v1.
- All public text: repository descriptions, player README, release notes, announcements.
- Every visible difference for the player, even if judged necessary.
- Controller playtests and manual tests on Windows, Linux, macOS and a real Vita.
- Any modification of this document.

**Everything else, including technical choices, is decided by the AI**, in writing (ADR), without asking.

## 6. What the AI is not allowed to do

- Modify this document.
- Push directly to `main`: everything goes through a pull request, with green CI.
- Close an issue or say “done”, “verified” or “identical” without evidence (command and result).
- Add a feature or improvement without maintainer approval.
- Add a document, process or tool that does not directly unblock a current task.
- Write into the original game folder.
- Reintroduce a translation of the game into Rust in any form.
