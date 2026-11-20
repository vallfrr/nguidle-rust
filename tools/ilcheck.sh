#!/bin/bash
. ~/.cargo/env
export CARGO_TARGET_DIR=/mnt/j/NGUvita/build-ngu
cd /mnt/j/NGUvita/nguvita
cargo check -q --message-format short -p ngu-ilgen 2>&1 | grep -E "error|warning: unused" | head -${1:-40}
