# ADR 0013 - Interface art and the setup window

Status - Accepted.

## Decision

The only game-derived files kept in Git are the interface elements listed in `assets/ui/MANIFEST.txt`, one line per file (`sha256  name`): a window frame, a button frame, a backdrop, a hand-drawn cat picture, a title font and a body font. `.github/repo-check.py` refuses any other picture, font or sound outside `docs/img/`, and checks that each listed file exists and matches its hash. Nothing else from the game, such as code, text, sounds, saves or prepared data, is ever stored.

The setup window of the PC application is drawn by the same graphics layer as the game window (miniquad, quads, fontdue text) from these embedded files, so it can run before any game data exists. It has its own small set of widgets (frame, button, label, progress bar, folder browser) and does not use the game's scene, assembly or prepared data.

## Reason

The window must look like part of the game while the player has not yet given us the game. A separate toolkit would not look like it. Embedding a handful of small files keeps the executable self-contained, and one manifest keeps the repository check strict about everything else.

## Consequences

Changing an interface file means changing the manifest in the same commit. The window has no native file dialog: a folder browser is drawn with the same widgets, so Windows, Linux and macOS behave identically.
