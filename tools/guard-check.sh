#!/bin/bash
# guard-check.sh <scenario> : replays a scenario on PC with the native guards ON and OFF (NGU_NO_GUARD=1) and compares the save digests
# (`check` lines) and the total interpreted instructions: guards must change cost, never state. Run from WSL.
set -e
. ~/.cargo/env
R=/mnt/j/NGUvita/nguvita; cd $R
SC=$(realpath "${1:?scenario}"); export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/build/clr}
(cd clr && cargo build --release -q 2>&1 | grep -E "^error" -A8 | head -20) || true
mkdir -p out/profile
for mode in on off; do
  rm -rf out/ap-save; mkdir -p out/ap-save; rm -f out/ngu/log.txt
  [ $mode = off ] && export NGU_NO_GUARD=1
  CLR_PROF=1 NGU_VITA_SAVE_DIR=$R/out/ap-save NGU_FIXED_NOW=1700000000 $CARGO_TARGET_DIR/release/clr x autoplay out/ngu "$SC" out/profile/guard-$mode.csv > out/profile/guard-$mode.prof.txt 2>&1
  grep -a "\[autoplay\] check" out/ngu/log.txt | sed 's/ tick=.*//' > out/profile/guard-$mode.checks
  echo "$mode: $(grep -a '^total' out/profile/guard-$mode.prof.txt)  checks=$(wc -l < out/profile/guard-$mode.checks)"
done
if diff -q out/profile/guard-on.checks out/profile/guard-off.checks > /dev/null; then echo "GUARDS: save digests identical"; else echo "GUARDS: DIGESTS DIFFER"; diff out/profile/guard-on.checks out/profile/guard-off.checks | head -5; fi
