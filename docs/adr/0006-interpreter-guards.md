# ADR 0006 — Interpreter shortcuts

## Decision

Remove the Cooking UI skip that produced a measured discrepancy. Retain the other existing shortcuts provisionally for platform performance; this is not a claim of full equivalence. Their evidence and gaps are tracked individually in [GUARDS](../GUARDS.md). Compare guarded and unguarded runs in game-data validation, and add targeted tests before treating an individual shortcut as established. If a shortcut changes game behavior, remove or correct it and measure any performance impact.

## Consequences

The four guarded/unguarded locale replays passed after removing the skip. Both paths share infrastructure, so these runs do not independently prove parity with Unity. The Vita impact of removing the Cooking skip has not been measured. Item bonus shortcuts still lack their own targeted test.
