#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# divtrace.sh [frames] [window] : full oracle + native traces then first divergence (see firstdiv.py)
N=${1:-30}
export NGU_FIXED_NOW=${NGU_FIXED_NOW:-1700000000} NGU_TRACE=1 NGU_TRACE_CALLS=1 NGU_TRACE_M=1 NGU_NO_GUARD=1
DLL="$NGU_GAME_DIR/Managed/Assembly-CSharp.dll"
cd "$NGU_RUST_REPO"
(cd clr && CARGO_TARGET_DIR="$NGU_CLR_TARGET" cargo build --release -q 2>&1 | grep -E "^error" -A6)
NGU_STATE_DIR=$(ngu_new_state_run divtrace) || exit 1
oracle_data=$(ngu_fixture_view) || exit 1
native_data=$(ngu_fixture_view) || exit 1
"$NGU_CLR_TARGET"/release/clr "$DLL" load "$oracle_data" "$N" > "$NGU_STATE_DIR"/o.full 2>&1
NGU_PREPARED="$native_data" bash tools/ngugame.sh "$N" > "$NGU_STATE_DIR"/n.full 2>&1
python3 tools/firstdiv.py "$NGU_STATE_DIR"/o.full "$NGU_STATE_DIR"/n.full ${2:-14}
