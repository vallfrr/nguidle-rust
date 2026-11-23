#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# statediff.sh [frames] : same scene, same fixed clock, N frames -> visible state of the oracle (interpreter) vs generated Rust; prints the diff.
. ~/.cargo/env
N=${1:-30}
export NGU_FIXED_NOW=${NGU_FIXED_NOW:-1700000000} NGU_NO_GUARD=1
DLL="$NGU_GAME_DIR/Managed/Assembly-CSharp.dll"
cd "$NGU_RUST_REPO"
NGU_STATE_DIR=$(ngu_new_state_run statediff) || exit 1
oracle_data=$(ngu_fixture_view) || exit 1
native_data=$(ngu_fixture_view) || exit 1
NGU_DUMP="$NGU_STATE_DIR"/oracle.txt "$NGU_CLR_TARGET"/release/clr "$DLL" load "$oracle_data" "$N" > "$NGU_STATE_DIR"/oracle.log 2>&1
NGU_DUMP="$NGU_STATE_DIR"/native.txt NGU_PREPARED="$native_data" bash tools/ngugame.sh "$N" > "$NGU_STATE_DIR"/native.log 2>&1
wc -l "$NGU_STATE_DIR"/oracle.txt "$NGU_STATE_DIR"/native.txt | head -2
diff "$NGU_STATE_DIR"/oracle.txt "$NGU_STATE_DIR"/native.txt > "$NGU_STATE_DIR"/diff.txt; echo "differing lines: $(grep -c '^[<>]' "$NGU_STATE_DIR"/diff.txt)"
head -${2:-40} "$NGU_STATE_DIR"/diff.txt | cut -c1-220
