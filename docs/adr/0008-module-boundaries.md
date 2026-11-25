# ADR 0008 - Boundaries and size budgets

Status - Accepted for this refactoring.

## Decision

Keep a generic frame loop, scalar operations, wire parser, and render
buffers independently testable. The compatibility layer keeps managed
object and instruction semantics, scene state and game-specific shortcuts.
The C and browser adapters retain ownership of input timing. The same
render conversion can be used by both without moving their graphics APIs.

Use physical source lines, including comments and blank lines, as a review
budget for the components touched in this change.

| File | Maximum lines |
| --- | ---: |
| `clr/src/save_slots.rs` | 1450 |
| `clr/src/unity.rs` | 1100 |
| `clr/src/vm.rs` | 1050 |
| `clr/src/vm/instructions.rs` | 800 |
| `clr/src/bcl_core.rs` and `clr/src/unity_api.rs` | 700 each |
| Vita `main.c` | 900 |
| Web `profile_store.py` | 205 |

These limits start near the measured sizes and prevent the split from
immediately eroding. Split by responsibility when a component grows.
Existing large files outside this change keep their current baseline.

The portable loop is tested without a UI dependency. Moving the complete
instruction dispatcher and managed value system out of the game host
remains a separate step and requires a larger integration comparison.
