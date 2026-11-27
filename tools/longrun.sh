#!/usr/bin/env bash
# D4 long-run test (local, real game): the same long game with and without the interpreter shortcuts must end in
# byte-identical saves. The game starts from a save (NGU_REAL_SAVE, default the imported PC backup) or from a new game
# with every feature unlocked, then plays ROUNDS rounds of 150 pseudo-random clicks and 20 minutes of game time, with
# two hours of offline progress applied in the middle. Both runs use a fixed clock. The unguarded run is about 20 times
# slower: expect roughly 13 minutes per game hour.
# Usage: tools/longrun.sh [rounds, default 9 = 3 game hours] [seed, default 1]
set -euo pipefail
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
rounds=${1:-9}
seed=${2:-1}
target=${CARGO_TARGET_DIR:-$NGU_CLR_TARGET}
cargo build --locked --release --manifest-path "$NGU_RUST_REPO/Cargo.toml" -p clr --bin clr --target-dir "$target"
clr="$target/release/clr"
if [[ -f "$clr.exe" ]]; then clr="$clr.exe"; fi
dll="$NGU_PREPARED/Assembly-CSharp.dll"
run=$(mktemp -d)
real=${NGU_REAL_SAVE:-$NGU_WORK/saves/imported-backup/NGUSaveSteam.txt}

steps=()
if [[ -f "$real" ]]; then
    cp -- "$real" "$run/start.sav"
    steps+=("click,Create New Game" "click,Slot 1" "click,Create New Game" "w,30" "click,PLAY" "w,10" "open,$run/start.sav" "w,100")
else
    steps+=("click,Create New Game" "click,Slot 1" "click,Create New Game" "w,30" "click,PLAY" "w,10" "do,$NGU_RUST_REPO/dev/unlock.steps" "ultimate" "w,100")
fi
for round in $(seq 1 "$rounds"); do
    IFS=';' read -ra clicks <<< "$(bash "$NGU_RUST_REPO/tools/rand-steps.sh" 150 $((seed * 1000 + round)))"
    steps+=("${clicks[@]}" "w,36000")
    if [[ $round -eq $(( (rounds + 1) / 2 )) ]]; then steps+=("call,Character.addOfflineProgress,7200" "w,300"); fi
done
steps+=("s,SAVE")

play() { # name, extra environment
    local name=$1; shift
    mkdir -p "$run/$name"
    cp -r -- "$NGU_PREPARED" "$run/$name/data"
    local list=("${steps[@]/SAVE/$run/$name/final.sav}")
    local start=$(date +%s)
    env NGU_FIXED_NOW=1700000000 NGU_DESKTOP=1 NGU_SAVE_DIR="$run/$name/save" "$@" \
        "$clr" "$dll" play "$run/$name/data" "$run/$name/end.png" "${list[@]}" > "$run/$name/log.txt" 2>&1 || true
    echo "$name: $(( $(date +%s) - start )) s, $(grep -c 'exception' "$run/$name/log.txt" || true) exception lines"
}
play guarded NGU_GUARD_STATS=1 &
play unguarded NGU_NO_GUARD=1 &
wait
echo "Shortcut answers in the guarded run (guard id: count, see docs/GUARDS.md):"
grep -h '^\[guard\]' "$run/guarded/log.txt" | sed 's/^\[guard\] \([0-9]*\) \([0-9]*\)$/  \1: \2/' | paste -sd' ' || true
if cmp -s "$run/guarded/final.sav" "$run/unguarded/final.sav"; then
    echo "LONG RUN: IDENTICAL ($(wc -c < "$run/guarded/final.sav") bytes, $rounds rounds). Logs: $run"
else
    echo "LONG RUN: DIFFERENT. Logs: $run"
    for name in guarded unguarded; do NRBF_FLAT=1 "$clr" "$dll" nrbf "$run/$name/final.sav" > "$run/$name/flat.txt" 2>&1 || true; done
    diff "$run/guarded/flat.txt" "$run/unguarded/flat.txt" | head -40 || true
    exit 1
fi
