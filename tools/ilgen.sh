#!/bin/bash
# ilgen.sh <Type::method> [classes] : generates the Rust from the player's DLL, builds ngu-gen. Then use: ngu-gen args...
. ~/.cargo/env
export CARGO_TARGET_DIR=/mnt/j/NGUvita/build-ngu
cd /mnt/j/NGUvita/nguvita
DLL="/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed/Assembly-CSharp.dll"
cargo build -q --release -p ngu-ilgen 2>&1 | grep -E "^error" -A5
$CARGO_TARGET_DIR/release/ngu-ilgen "$DLL" crates/ngu-gen/src "${2:-ALL}" "$1" || exit 1
bash tools/capbuild.sh 12 cargo build -q --profile game -p ngu-gen --message-format short 2>&1 | grep -E "error" | head -${ILGEN_ERRS:-30}
