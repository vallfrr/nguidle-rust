# ADR 0013 - Interface art and the setup window

Status - Accepted.

## Decision

The only game-derived files kept in Git are the interface elements listed in `assets/ui/MANIFEST.txt`, one line per file (`sha256  name`). The sprites are the game's own, exported unchanged: the bolted frame (`BorderFrame`), the white button and field (`ThinBorderWhite`), the blue panel (`ThinBorderBlue`), the green and red labels (`ThinBorderLightGreen`, `ThinBorderRed`) and the bar fill (`ThinBarFill`, tinted with the energy bar's green). Besides them: a backdrop, a hand-drawn cat picture, a title font and a body font. No picture is drawn by us. `.github/repo-check.py` refuses any other picture, font or sound outside `docs/img/`, and checks that each listed file exists and matches its hash. Nothing else from the game, such as code, text, sounds, saves or prepared data, is ever stored.

The setup window of the PC application is drawn by the same graphics layer as the game window (miniquad, quads, fontdue text) from these embedded files, so it can run before any game data exists. It has its own small set of widgets (frame, button, label, progress bar) and does not use the game's scene, assembly or prepared data.

## Reason

The window must look like part of the game while the player has not yet given us the game. A separate toolkit would not look like it. Embedding a handful of small files keeps the executable self-contained, and one manifest keeps the repository check strict about everything else.

## Consequences

Changing an interface file means changing the manifest in the same commit. Folders are chosen with the system's own folder dialog, never with a drawn browser.

## Installer and shortcut

The downloaded file is only the installer. On the first launch it sets the game up, then copies itself to the application folder (`NGUIdleRustEngine.exe`, or the system's equivalent) and, on Windows and Linux, adds a desktop shortcut named "NGU Idle Rust Engine" that uses the game's own icon. The game always starts from the installed copy. The Vita preparation program does the same in its own folder, with the name "NGU Idle Vita Prep". A folder chosen with `NGU_APP_DIR` is a developer's test folder and is never installed into. The window takes only fixed sizes (`sizes.rs`, all in the 960 x 600 proportions): on Windows the drag itself snaps to them, elsewhere the size is corrected when the drag ends. The game folder is chosen with the system's own folder dialog (`rfd`), and the window says whether the chosen place is valid before anything starts; the Vita program uses the same dialog for the output folder.
