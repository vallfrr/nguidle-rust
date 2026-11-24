# ADR 0007 — Observed energy-bar phase difference

## Observation

The isolated original and the interpreter loaded the same copied save with an energy-bar progress of 0.04. After approximately 60 seconds of wall time, their reported phases were 0.9399996 and 0.8799997 respectively, while total energy gained was identical. See [FIDELITY](../FIDELITY.md) and the [replay instructions](../../tools/oracle/README.md).

## Decision

Record this difference as an unresolved visual/timing discrepancy and accept it temporarily for the documentation and fidelity baseline. Do not claim the animation or simulation is fully equivalent. The measurement does not identify whether frame scheduling, timer phase, float operations or another factor is responsible, and the reported field alone does not establish the rendered appearance.

The follow-up must record frame and tick times, compare multiple starts and frame cadences, identify the first divergence, and check whether other progress bars or actual resource totals are affected. No Vita performance or whole-game conclusion follows from this one desktop comparison.
