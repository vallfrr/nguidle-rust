#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
. ~/.cargo/env
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$NGU_WSL_TARGET}"
cd "$NGU_RUST_REPO"
cargo check -q --message-format short -p ngu-ilgen 2>&1 | grep -E "error|warning: unused" | head -${1:-40}
