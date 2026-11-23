#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# Release build + run of clr (WSL). Usage: tools/clr.sh [mode] [args...]   (DLL = Assembly-CSharp)
. ~/.cargo/env
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$NGU_CLR_TARGET}"
cd "$NGU_RUST_REPO/clr"
cargo build --release -q || exit
M="$NGU_GAME_DIR/Managed/Assembly-CSharp.dll"
exec "$CARGO_TARGET_DIR/release/clr" "$M" "$@"
