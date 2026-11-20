#!/bin/bash
# Release build + run of clr (WSL). Usage: tools/clr.sh [mode] [args...]   (DLL = Assembly-CSharp)
. ~/.cargo/env
export CARGO_TARGET_DIR=$HOME/build/clr
cd /mnt/j/NGUvita/nguvita/clr
cargo build --release -q 2>&1 | grep -E "^error|^warning: unused" | head -20
M="/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed/Assembly-CSharp.dll"
exec $CARGO_TARGET_DIR/release/clr "$M" "$@"
