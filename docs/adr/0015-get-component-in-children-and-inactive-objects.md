# ADR 0015 — GetComponentInChildren and inactive objects

## Observation

Opening the 58 menus (`MenuSwapper.swapMenu`) on the maintainer's early save raised a `NullReferenceException` in the port for two menus (energy purchases, 13, and beast quests, 48) and none in the original (checked with `tools/oracle/OfflineProbe.cs`, which opens the same save in the isolated original). The game deactivates a button (`buyAllCustom.gameObject.SetActive(false)`) and reads its caption on the next line (`buyAllCustom.GetComponentInChildren<Text>().text = ...`). The port skipped an inactive object and everything below it, so it found nothing.

## Decision

`GetComponentInChildren` (and `GetComponentsInChildren`) test the active flag of each object on its own and keep going into the children of an inactive object, as the original does: the text of a button that has just been deactivated is found. An object's own components are still ignored when it is inactive and `includeInactive` is false.

## Consequences

- All 58 menus open without an exception on the early save and on a played state, as in the original; the full-game script checks it (`menus of a new game without exception`).
- The replays of `just verify-game` give identical saves and identical interface dumps apart from the new hidden save-folder button.
- Not established: whether the original also finds a component on an inactive object itself when it is the only match; the port keeps ignoring it, as the documentation of the method says.
