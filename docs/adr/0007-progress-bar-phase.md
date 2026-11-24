# ADR 0007 — Observed energy-bar phase difference

## Observation

The isolated original and the interpreter loaded the same copied save with an energy-bar progress of 0.04. After approximately 60 seconds of wall time, their reported phases were 0.9399996 and 0.8799997 respectively, while total energy gained was identical.

A follow-up compared the initial state and the state after each of the first 100 energy callbacks. Both the accumulated energy and the bar value matched bit for bit at every observation, in three runs of the original with requested frame rates of 30, 60 and 144 Hz. The original executed its first three callbacks within 10 ms of the start of observation in all three runs. The interpreter observed them at 20.83, 41.67 and 55.56 ms of simulation time. See [FIDELITY](../FIDELITY.md).

## Decision

Accept the measured startup scheduling difference for this fidelity baseline. The comparison isolates the difference to callback timing over the measured window. Changing the energy arithmetic to compensate would break the callback-aligned agreement. The exact origin of the original's startup burst is unknown.

The comparison measures one copied save and one resource. It does not establish when the two executions award each individual point in real time, how the bar looks on screen, or how other resources behave.
