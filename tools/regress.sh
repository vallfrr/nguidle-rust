#!/bin/bash
# regress.sh : non-regression battery (oracle vs native). Prerequisite: `tools/iter.sh 1` (generation + build).
cd /mnt/j/NGUvita/nguvita
ok=1
chk() { if echo "$2" | grep -q "$3"; then echo "OK    $1"; else echo "FAIL $1 : $2"; ok=0; fi; }
chk "NumberOutput (84 cases)" "$(bash tools/ildiff-number.sh 2>&1 | tail -1)" "diffs=0"
export NGU_STEPS="$(bash tools/grid-steps.sh)"
chk "state after click grid" "$(bash tools/statediff.sh 30 5 2>&1 | tail -1)" "differing lines: 0"
export NGU_STEPS="$(bash tools/input-steps.sh);w,200"
chk "state after right/middle clicks, keys and wheel" "$(bash tools/statediff.sh 40 5 2>&1 | tail -1)" "differing lines: 0"
for seed in 1 2 3 777; do
  export NGU_STEPS="$(bash tools/rand-steps.sh 300 $seed);w,300"
  chk "state after 300 random clicks (seed $seed)" "$(bash tools/statediff.sh 40 5 2>&1 | tail -1)" "differing lines: 0"
done
export NGU_STEPS="$(bash tools/rand-steps.sh 150 9);w,200"
chk "complete save identical" "$(bash tools/savediff.sh 30 2>&1 | tail -1)" "IDENTICAL"
unset NGU_STEPS
chk "load + re-save identical" "$(bash tools/loaddiff.sh 3 2>&1 | grep -E 'IDENTICAL|DIFFERENT' | head -1)" "IDENTICAL"
[ $ok = 1 ] && echo "REGRESSION: ALL OK" || echo "REGRESSION: FAILURES"
