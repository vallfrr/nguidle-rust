# Fidelity

The engine's automated comparison covers selected input sequences, saves and game state. Both compared implementations share the Unity compatibility layer and parts of the runtime, so the comparisons cannot establish equivalence to the original game by themselves. See [IL generator audit](AUDIT-ILGEN.md) and [comparison notes](COMPARISON.md).

Open work includes broader measurements of progression against the original, additional random-number cases, and comparison of reproducible original-game traces to the port. The interpreter's game-specific shortcuts also require individual evidence.

The first original-game scenario recorder supports menu changes and state snapshots, but not pointer/controller inputs, fixed clock, or automatic cross-runtime trace comparison. See its [scope and instructions](../tools/oracle/README.md).

## Recorded comparisons

- The isolated Windows 1.260 copy reported Unity 2019.4.22f1. Its measured random vectors cover six seeds and 16 draws per seed for Unity values, integer and floating ranges, saved and restored states, and seeded Mono random methods. Run `cargo test -p clr --lib unity_random_tests` and `cargo test -p clr --lib dotnet_random::tests`. See [the probe](../tools/oracle/README.md). Further bounds, default seeding and gameplay uses remain open.
- In three isolated fresh-game runs at approximately 60 seconds of wall time, the original gained 58 energy, from 250 to 308, with a 500 cap. Steam initialization and autosave updates were disabled in the copied executable for the probe. The port interpreter's 1,800 fixed steps likewise gained 58 energy. The initial bar phases differed, so these runs do not prove identical bar animation or full progression. The [probe](../tools/oracle/README.md) describes the setup.
- The first full guarded replay produced differing Cooking text while the menu was parked off-screen. That skip was removed and a targeted game-data test added. Performance impact on the Vita has not been measured.
- With the same copied save loaded into the original and interpreter, both started at 500/500 energy and 12,934 total gained, and reached 12,992 gained after respectively 60.00114 and 60.000221 seconds of real time. The interpreter was paced at 30 ticks per second. The energy bar began at 0.04 and ended at 0.9399996 in the original versus 0.8799997 in the interpreter. The initial state and the first 100 updates matched when aligned by callback count. The original ran its first three callbacks within 10 ms of observation, before the interpreter's first simulated callback. [ADR 0007](adr/0007-progress-bar-phase.md) accepts this observed startup timing difference for the measured baseline.
- `just verify-fixtures` compares 60 synthetic cases between .NET 8 and the interpreter. The suite covers control flow, seeded random draws, exception handling, captured lambdas, generic lists, LINQ summation, iterator state machines, and structures with properties, copies, nested fields, arrays, boxing and by-reference and output parameters. Each case covers a specific construction and does not establish general support for it.

## Unresolved intermittent save difference

The historical report in [archive/STATUS](archive/STATUS.md) records one differing complete save after an invocation change, followed by passing runs, without retaining the differing files. The four current guard and locale combinations passed, but that cannot establish the cause of the historical result. The comparison remains enabled; a future failure must retain both saves, inputs, seeds, logs and environment for investigation. This issue remains open rather than being declared fixed by retries.
