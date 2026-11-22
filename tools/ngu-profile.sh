#!/bin/bash
# ngu-profile.sh <scenario> [top] : replays a benchmark scenario headless on PC (virtual 60 Hz, fixed clock) and prints a short report:
# frame work summary (tools/vita-trace.py) and the `top` methods by exclusive interpreted instructions (hardware-independent cost).
# Full output stays in out/profile/. Run from WSL. Needs out/ngu (tools/nguprep.py) with Assembly-CSharp.dll next to scene.json.
set -e
. ~/.cargo/env
R=/mnt/j/NGUvita/nguvita; cd $R
SC=$(realpath "${1:?scenario file}"); TOP=${2:-20}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/build/clr}
(cd clr && cargo build --release -q 2>&1 | grep -E "^error" -A8 | head -20) || true
mkdir -p out/profile out/ap-save; rm -rf out/ap-save; mkdir -p out/ap-save
NAME=$(basename "$SC" .txt)
CLR_PROF=1 NGU_VITA_SAVE_DIR=$R/out/ap-save NGU_FIXED_NOW=1700000000 \
  $CARGO_TARGET_DIR/release/clr x autoplay out/ngu "$SC" out/profile/$NAME.csv > out/profile/$NAME.prof.txt 2>&1
grep -a "^\[autoplay\]" out/ngu/log.txt | tail -n +1 > out/profile/$NAME.log || true
python3 tools/vita-trace.py out/profile/$NAME.csv --scenario "$SC" --log out/profile/$NAME.log 2>/dev/null || \
  /mnt/j/NGUvita/nguvita/.venv/Scripts/python.exe tools/vita-trace.py out/profile/$NAME.csv --scenario "$SC" --log out/profile/$NAME.log
echo "== top $TOP methods (exclusive instr, inclusive, calls)"
sed -n "/^total /,\$p" out/profile/$NAME.prof.txt | head -$((TOP + 1))
