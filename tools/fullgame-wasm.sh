#!/usr/bin/env bash
# The full-game script as WebAssembly under Node 18+ (what the web port runs): same steps, same numbers as the native run.
# Needs the wasm32-wasip1 Rust target (rustup target add wasm32-wasip1) and the prepared data.
# Usage: tools/fullgame-wasm.sh [game hours, default 0.05]
set -euo pipefail
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
hours=${1:-0.05}
target=${CARGO_TARGET_DIR:-$NGU_BUILD/wsl-wasm}
cargo build --locked --release --manifest-path "$NGU_RUST_REPO/Cargo.toml" -p clr --bin clr --target wasm32-wasip1 --target-dir "$target"
work=$(mktemp -d)
cp -r -- "$NGU_PREPARED" "$work/data"
node --no-warnings "$NGU_RUST_REPO/tools/fullgame-wasm.mjs" "$target/wasm32-wasip1/release/clr.wasm" "$work/data" "$work/out" "$hours" | grep -E "fullgame\]|FULL GAME|^exit"
