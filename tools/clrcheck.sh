#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# cargo check of clr (WSL); prints condensed errors.
. ~/.cargo/env
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$NGU_CLR_TARGET}"
cd "$NGU_RUST_REPO/clr"
cargo check --message-format short 2>&1 | grep -E "error|warning: unused" | head -${1:-40}
cargo check --message-format short 2>&1 | tail -3
