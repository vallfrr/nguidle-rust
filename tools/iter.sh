#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# iter.sh [frames] : full dev loop: regenerate (ngu-ilgen) -> build ngu-gen (memory cap, user build directory) -> run the game.
. ~/.cargo/env
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$NGU_WSL_TARGET}"
cd "$NGU_RUST_REPO"
cargo build -q --release -p ngu-ilgen 2>&1 | grep -E "^error" -A6
"$CARGO_TARGET_DIR/release/ngu-ilgen" "$NGU_GAME_DIR/Managed/Assembly-CSharp.dll" crates/ngu-gen/src ALL NumberOutput::suffixFormat || exit 1
mkdir -p "$NGU_WORK/traces"
if ! bash tools/capbuild.sh 12 cargo build --profile game -p ngu-gen > "$NGU_WORK/traces/ngu-build.log" 2>&1; then
    tail -40 "$NGU_WORK/traces/ngu-build.log" >&2
    exit 1
fi
RUST_BACKTRACE=${BT:-1} bash tools/ngugame.sh ${1:-3} 2>&1 | cut -c1-300 | grep -vE "^\s+[0-9]+: (std|core|__|<std|_start|main|clone|start_thread)|^\s+at " | tail -${TAILN:-25}
