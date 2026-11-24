# Fidelity

The engine's automated comparison covers selected input sequences, saves and game state. Both compared implementations share the Unity compatibility layer and parts of the runtime, so the comparisons cannot establish equivalence to the original game by themselves. See [IL generator audit](AUDIT-ILGEN.md) and [comparison notes](COMPARISON.md).

Open work includes broader measurements of progression against the original, additional random-number cases, and comparison of reproducible original-game traces to the port. The interpreter's game-specific shortcuts also require individual evidence.

## Recorded comparisons

- The isolated Windows 1.260 copy reported Unity 2019.4.22f1. Its measured random vectors cover four seeds and 16 draws per seed for Unity values, integer and floating ranges, and seeded Mono random methods. Run `cargo test -p clr --lib unity_random_tests` and `cargo test -p clr --lib dotnet_random::tests`. See [the probe](../tools/oracle/README.md). Further bounds, default seeding and Unity state restoration remain open.
- In three isolated fresh-game runs at approximately 60 seconds of wall time, the original gained 58 energy, from 250 to 308, with a 500 cap. Steam initialization and autosave updates were disabled in the copied executable for the probe. The port interpreter's 1,800 fixed steps likewise gained 58 energy. The initial bar phases differed, so these runs do not prove identical bar animation or full progression. The [probe](../tools/oracle/README.md) describes the setup.
- The first full guarded replay produced differing Cooking text while the menu was parked off-screen. That skip was removed and a targeted game-data test added. Performance impact on the Vita has not been measured.
- `just verify-fixtures` compares synthetic control-flow and seeded random cases with .NET 8 and the interpreter. Generated Rust is not covered by this fixture suite yet.
