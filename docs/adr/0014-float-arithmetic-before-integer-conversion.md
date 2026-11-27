# ADR 0014 — Float arithmetic before an integer conversion

## Observation

The original and the port opened the same save after an absence of 100,000 seconds and of more than a year, on a state of eight game hours (7,815 fields). Every field matched except the beast-quest daily timer, which differed by 5 seconds and by 1,554 seconds. The game computes the timer's threshold as `(int)((float)num * 0.9f)` with `num` 22,560: the product of 22,560 and the single-precision 0.9 is 20,303.99946. The original truncates it to 20,303 (the removal of 5 and 1,553 thresholds explains both differences exactly). The port rounded the product to single precision first, 20,304.0, and truncated to 20,304.

## Decision

The original runtime (Mono as shipped with Unity 2019.4) keeps float arithmetic wide until a value is stored. The port now does the same where it can be decided from the code alone: an `+ - * /` of two floats directly followed by a conversion to an integer type is computed in double precision (`Op::WideArith`, set in `lower.rs`). Every other float operation is unchanged.

## Consequences

- The same two saves now match the original on all 7,815 fields; the earlier comparisons (7,316 and 7,146 fields) still match. The full-game script checks the threshold (`float arithmetic like the original`: 20,303) and the guard tests still pass.
- Not covered: wide intermediate values that are not converted to an integer right away (`a * b * c`, `a * b > c`, a product stored in a local). Mono's exact behaviour there depends on its register allocation. A difference found later by comparing saves with the original is handled the same way, case by case.
