#!/usr/bin/env bash
# The full-game script on the real game (Linux, macOS, WSL, Git Bash): every menu, a long progression, saves, offline
# progress and every console-mode element, one OK / KO / SKIP line each. Exit code 1 if a step fails.
# Usage: tools/fullgame.sh [game hours, default 1] [report file]
set -euo pipefail
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
hours=${1:-1}
report=${2:-$NGU_WORK/traces/fullgame-report.txt}
mkdir -p -- "$(dirname -- "$report")"
dll="$NGU_PREPARED/Assembly-CSharp.dll"
target=${CARGO_TARGET_DIR:-$NGU_CLR_TARGET}
cargo build --locked --release --manifest-path "$NGU_RUST_REPO/Cargo.toml" -p clr --bin clr --target-dir "$target"
clr="$target/release/clr"
if [[ -f "$clr.exe" ]]; then clr="$clr.exe"; fi
exec "$clr" "$dll" fullgame "$NGU_PREPARED" "$report" "$hours"
