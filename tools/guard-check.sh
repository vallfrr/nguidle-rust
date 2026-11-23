#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# guard-check.sh <scenario> : replays a scenario on PC with the native guards ON and OFF (NGU_NO_GUARD=1) and compares the save digests
# (`check` lines) and the total interpreted instructions: guards must change cost, never state. Run from WSL.
set -e
. ~/.cargo/env
R=$NGU_RUST_REPO; cd "$R"
SC=$(realpath "${1:?scenario}"); export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$NGU_CLR_TARGET}"
(cd clr && cargo build --release -q)
NGU_STATE_DIR=$(ngu_new_state_run guard)
for mode in on off; do
  assets=$(ngu_fixture_view)
  prefix=$NGU_STATE_DIR/guard-$mode
  [ "$mode" = off ] && export NGU_NO_GUARD=1 || unset NGU_NO_GUARD
  mkdir -p "$prefix-save"
  CLR_PROF=1 NGU_VITA_SAVE_DIR="$prefix-save" NGU_FIXED_NOW=1700000000 \
    "$CARGO_TARGET_DIR/release/clr" x autoplay "$assets" "$SC" "$prefix.csv" > "$prefix.prof.txt" 2>&1
  grep -a "\[autoplay\] check" "$assets/log.txt" | sed 's/ tick=.*//' > "$prefix.checks"
  echo "$mode: $(grep -a '^total' "$prefix.prof.txt")  checks=$(wc -l < "$prefix.checks")"
done
if diff -q "$NGU_STATE_DIR/guard-on.checks" "$NGU_STATE_DIR/guard-off.checks"; then
  echo "GUARDS: save digests identical"
else
  echo "GUARDS: DIGESTS DIFFER"
  exit 1
fi
