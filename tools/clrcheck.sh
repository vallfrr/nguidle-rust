#!/bin/bash
# cargo check of clr (WSL); prints condensed errors.
. ~/.cargo/env
export CARGO_TARGET_DIR=$HOME/build/clr
cd /mnt/j/NGUvita/nguvita/clr
cargo check --message-format short 2>&1 | grep -E "error|warning: unused" | head -${1:-40}
cargo check --message-format short 2>&1 | tail -3
