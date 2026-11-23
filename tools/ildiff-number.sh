#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# NumberOutput::suffixFormat bench: values x formats, oracle vs generated Rust
fail=0; n=0
for v in 0.0 0.5 1.5 999.0 1000.0 12345.678 999999.0 1000000.0 1234567.0 1.5e9 7.2e12 1e18 1.234e30 9.99e100 1e200 1.7e308 -5.0 -1234567.0 123456789012.0 3.3e66 6.1e303; do
  for t in 0 1 2 3; do
    n=$((n+1)); bash "$NGU_RUST_REPO/tools/ildiff.sh" NumberOutput::suffixFormat $v $t || fail=$((fail+1))
  done
done
echo "total=$n diffs=$fail"
