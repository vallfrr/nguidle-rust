# ADR 0010 — Local game baselines

Status — Accepted.

## Decision

Record game state, full saves and formatted outputs from a known good
interpreter. Keep those reference files outside the repository and replay
against them with a fixed clock and isolated profiles. Keep the synthetic
.NET fixture comparison independent of all game files.

## Consequences

Removing the translator does not remove the existing replay cases.
The local comparisons detect changes relative to the recorded engine build.
Comparison against the original Windows game remains a separate check.
