#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# loaddiff.sh : 1) the oracle plays the click grid and saves; 2) oracle and native load THAT save, play 100 frames, then compare visible state + new save.
. ~/.cargo/env
export NGU_FIXED_NOW=${NGU_FIXED_NOW:-1700000000}
if [ "${NGU_GUARDS:-off}" = on ]; then unset NGU_NO_GUARD; else export NGU_NO_GUARD=1; fi
DLL="$NGU_GAME_DIR/Managed/Assembly-CSharp.dll"
cd "$NGU_RUST_REPO"
(cd clr && CARGO_TARGET_DIR="$NGU_CLR_TARGET" cargo build --release -q 2>&1 | grep -E "^error" -A6)
NGU_STATE_DIR=$(ngu_new_state_run loaddiff) || exit 1
S="$NGU_STATE_DIR/base.sav"
base_data=$(ngu_fixture_view) || exit 1
NGU_STEPS="$(bash tools/grid-steps.sh);s,$S" "$NGU_CLR_TARGET"/release/clr "$DLL" load "$base_data" 30 > "$NGU_STATE_DIR"/l0.log 2>&1
echo "starting save: $(wc -c < "$S") bytes"
for mode in oracle native; do
  replay_data=$(ngu_fixture_view) || exit 1
  export NGU_STEPS="l,$S;w,100;s,$NGU_STATE_DIR/after_$mode.sav" NGU_DUMP="$NGU_STATE_DIR"/ld_$mode.txt
  if [ $mode = oracle ]; then "$NGU_CLR_TARGET"/release/clr "$DLL" load "$replay_data" 30 > "$NGU_STATE_DIR"/l_$mode.log 2>&1; else NGU_PREPARED="$replay_data" bash tools/ngugame.sh 30 > "$NGU_STATE_DIR"/l_$mode.log 2>&1; fi
done
cmp -s "$NGU_STATE_DIR"/after_oracle.sav "$NGU_STATE_DIR"/after_native.sav && echo "SAVES AFTER LOAD IDENTICAL ($(wc -c < "$NGU_STATE_DIR"/after_oracle.sav) bytes)" || echo "SAVES AFTER LOAD DIFFERENT"
diff "$NGU_STATE_DIR"/ld_oracle.txt "$NGU_STATE_DIR"/ld_native.txt > "$NGU_STATE_DIR"/ld.diff; echo "differing state lines after load: $(grep -c '^[<>]' "$NGU_STATE_DIR"/ld.diff)"
head -${1:-20} "$NGU_STATE_DIR"/ld.diff | cut -c1-200
