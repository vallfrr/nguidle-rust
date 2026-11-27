# ADR 0016 — Time the host was away

## Observation

The original game computes offline progress only when a save is opened (`OpenFileDialog.loadintoGame`: seconds since `lastTime`, at most a year, only above 10 seconds, then `Character.addOfflineProgress`). While it runs, it keeps playing in the background (Unity "Run In Background" is on). The port did not: a browser stops a hidden tab, a computer or a console can sleep, and the engine counts at most a quarter of a second of any frame, so all that time was lost. Replaying it step by step would take about a minute per game hour in a browser, which a player would not accept.

## Decision

A host calls `Vm::host_frame(counted)` once per display frame (the web, PC and console hosts do it through `Session::advance` and `ngu_frame`) with the seconds it lets the engine count. The wall clock is read; when it moved more than 10 seconds further than that, the difference is lost time. The next step applies it as offline progress, the way the game does after opening a save: the game's own `addOfflineProgress` between the two refreshes of the item stats, limited to a year. The "You were offline" screen the game opens at the end of the calculation is closed again: after opening a save it tells the player what happened, after a sleep or a hidden tab it only gets in the way (maintainer's decision after trying it on the console). Nothing is replayed and nothing is calculated by the port.

- 10 seconds is the game's own threshold, so a heavy step (a save loading in a browser takes seconds) is not mistaken for an absence.
- A clock that goes back counts as no time, like the game.
- The console host treats a frame longer than 5 seconds as a suspension and gives the game a quarter of a second of it.
- Every save the port writes is first stamped with the current time (`Character.lastTime`), as the game does before each of its own saves; without it a page reloaded after a session re-applied the time of the previous save.

## Consequences

- A hidden tab, a computer that wakes up and a console that comes out of sleep give the player what the game gives after a close and an open, in a few milliseconds (a year on a late-game state: under 40 ms natively; under 10 ms in a browser on a simple state). The data is updated and nothing is shown.
- This goes beyond the original on a running game, which loses the time of a sleep: it is a quality-of-life addition validated by the maintainer for every platform.
- `just full-game` checks that an hour away gives what the game's offline calculation gives, that ten seconds are ignored and that a year is applied in under a second.
- Not covered: a game left open for days in a browser that keeps running it in the background (the original does the same, nothing is lost there).
