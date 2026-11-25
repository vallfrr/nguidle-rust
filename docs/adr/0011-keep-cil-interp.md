# ADR 0011 - Keep `cil-interp` as a separate crate

Status - Accepted.

## Decision

Keep `crates/cil-interp` (the generic frame loop and scalar arithmetic) as it is. Do not merge it into `clr`, and do not grow it into a separate reference interpreter.

## Reason

The translator it was meant to be compared with no longer exists, so a second reference interpreter has no use. The crate is small, has its own tests (`just check` runs them without game data) and is already required by ADR 0008. Moving 160 lines into `clr` would change nothing for the player or for a port author.

## Consequences

Revisit only with a concrete need, in a new ADR that replaces this one.
