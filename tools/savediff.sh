#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# savediff.sh [frames] : output of ImportExport.getBase64Data() (complete save) of the oracle vs native, after the NGU_STEPS scenario
. ~/.cargo/env
N=${1:-30}
export NGU_FIXED_NOW=${NGU_FIXED_NOW:-1700000000} NGU_NO_GUARD=1
DLL="$NGU_GAME_DIR/Managed/Assembly-CSharp.dll"
cd "$NGU_RUST_REPO"
NGU_STATE_DIR=$(ngu_new_state_run savediff) || exit 1
oracle_data=$(ngu_fixture_view) || exit 1
native_data=$(ngu_fixture_view) || exit 1
export NGU_STEPS="${NGU_STEPS:+$NGU_STEPS;}s,$NGU_STATE_DIR/save_X.txt"
NGU_STEPS="${NGU_STEPS//save_X/save_oracle}" "$NGU_CLR_TARGET"/release/clr "$DLL" load "$oracle_data" "$N" > "$NGU_STATE_DIR"/so.log 2>&1
NGU_STEPS="${NGU_STEPS//save_X/save_native}" NGU_PREPARED="$native_data" bash tools/ngugame.sh "$N" > "$NGU_STATE_DIR"/sn.log 2>&1
ls -la "$NGU_STATE_DIR"/save_oracle.txt "$NGU_STATE_DIR"/save_native.txt
if cmp -s "$NGU_STATE_DIR"/save_oracle.txt "$NGU_STATE_DIR"/save_native.txt; then echo "SAVES IDENTICAL ($(wc -c < "$NGU_STATE_DIR"/save_oracle.txt) bytes)"; else echo "SAVES DIFFERENT"; fi
