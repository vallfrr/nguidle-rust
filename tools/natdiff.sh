#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# natdiff.sh [frames] : compares the sequence of engine->game calls and game->engine UnityEngine calls (oracle vs native). First divergence = root cause.
. ~/.cargo/env
N=${1:-30}
export NGU_FIXED_NOW=${NGU_FIXED_NOW:-1700000000} NGU_TRACE=1 NGU_TRACE_CALLS=1 NGU_TRACE_M=1 NGU_NO_GUARD=1
DLL="$NGU_GAME_DIR/Managed/Assembly-CSharp.dll"
cd "$NGU_RUST_REPO"
(cd clr && CARGO_TARGET_DIR="$NGU_CLR_TARGET" cargo build --release -q 2>&1 | grep -E "^error" -A6)
NGU_STATE_DIR=$(ngu_new_state_run natdiff) || exit 1
oracle_data=$(ngu_fixture_view) || exit 1
native_data=$(ngu_fixture_view) || exit 1
f() { grep -E "^\[m\]|^\[call\]|^\[native\] UnityEngine[A-Za-z.]*::(set_|Set|Translate|Destroy|Instantiate|AddComponent|Invoke|InvokeRepeating|CancelInvoke|StartCoroutine|AddListener|RemoveListener|Play|Stop|Apply|Find|GetComponent)" | grep -v "::\.ctor\|::\.cctor" | sed "s/ ptr/ struct/g"; }
"$NGU_CLR_TARGET"/release/clr "$DLL" load "$oracle_data" "$N" 2>&1 | f > "$NGU_STATE_DIR"/o.seq
NGU_PREPARED="$native_data" bash tools/ngugame.sh "$N" 2>&1 | f > "$NGU_STATE_DIR"/n.seq
wc -l "$NGU_STATE_DIR"/o.seq "$NGU_STATE_DIR"/n.seq | head -2
diff "$NGU_STATE_DIR"/o.seq "$NGU_STATE_DIR"/n.seq > "$NGU_STATE_DIR"/seq.diff
echo "divergences: $(grep -c '^[<>]' "$NGU_STATE_DIR"/seq.diff)"
head -${2:-30} "$NGU_STATE_DIR"/seq.diff | cut -c1-200
