#!/bin/bash
# iter.sh [frames] : full dev loop: regenerate (ngu-ilgen) -> build ngu-gen (memory cap, target on J:) -> run the game.
. ~/.cargo/env
export CARGO_TARGET_DIR=/mnt/j/NGUvita/build-ngu
cd /mnt/j/NGUvita/nguvita
cargo build -q --release -p ngu-ilgen 2>&1 | grep -E "^error" -A6
$CARGO_TARGET_DIR/release/ngu-ilgen "/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed/Assembly-CSharp.dll" crates/ngu-gen/src ALL NumberOutput::suffixFormat || exit 1
bash tools/capbuild.sh 12 cargo build --profile game -p ngu-gen > ~/build/ngu-build.log 2>&1
if grep -qE "^error" ~/build/ngu-build.log; then grep -E "^error" -A8 ~/build/ngu-build.log | head -40; exit 1; fi
RUST_BACKTRACE=${BT:-1} bash tools/ngugame.sh ${1:-3} 2>&1 | cut -c1-300 | grep -vE "^\s+[0-9]+: (std|core|__|<std|_start|main|clone|start_thread)|^\s+at " | tail -${TAILN:-25}
