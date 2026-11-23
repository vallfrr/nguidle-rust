# Fidelity

The engine's automated comparison covers selected input sequences, saves and game state. Both compared implementations share the Unity compatibility layer and parts of the runtime, so the comparisons cannot establish equivalence to the original game by themselves. See [IL generator audit](AUDIT-ILGEN.md) and [comparison notes](COMPARISON.md).

Open work includes measuring the original game's progression rate, generating random-number reference vectors from the original Unity runtime, and comparing reproducible original-game traces to the port. The interpreter's game-specific shortcuts also require individual evidence. Record measured differences and their conditions here as they become available.
