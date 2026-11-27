#!/usr/bin/env bash
# What each interpreter shortcut is worth per frame, on the console scenarios (nguidle-vita/scenarios) replayed through the
# console's C interface on PC: one replay with every shortcut, one with none, one per shortcut disabled. The instruction
# counts do not depend on the machine; the microseconds do. Prints, per scenario and shortcut, the change in total
# instructions, in the 99th percentile frame and in the worst frame when the shortcut is disabled.
# Usage: tools/guard-frames.sh <scenario file>...   (default: the nguidle-vita session and monkey scenarios)
set -euo pipefail
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
target=${CARGO_TARGET_DIR:-$NGU_CLR_TARGET}
cargo build --locked --release --manifest-path "$NGU_RUST_REPO/Cargo.toml" -p clr --bin clr --target-dir "$target"
clr="$target/release/clr"; if [[ -f "$clr.exe" ]]; then clr="$clr.exe"; fi
dll="$NGU_PREPARED/Assembly-CSharp.dll"
scenarios=("$@")
if [[ ${#scenarios[@]} -eq 0 ]]; then scenarios=("$NGU_RUST_REPO/../nguidle-vita/scenarios/session.txt" "$NGU_RUST_REPO/../nguidle-vita/scenarios/monkey.txt"); fi
run=$(mktemp -d)
replay() { # name scenario env...
    local name=$1 scenario=$2; shift 2
    mkdir -p "$run/$name"; cp -r -- "$NGU_PREPARED" "$run/$name/data"
    env NGU_FIXED_NOW=1700000000 NGU_SAVE_DIR="$run/$name/save" "$@" "$clr" "$dll" autoplay "$run/$name/data" "$scenario" "$run/$name/trace.csv" > "$run/$name/log.txt" 2>&1 || true
}
for scenario in "${scenarios[@]}"; do
    label=$(basename "$scenario" .txt)
    replay "$label-on" "$scenario" &
    replay "$label-none" "$scenario" NGU_NO_GUARD=1 &
    wait
    for id in $(seq 2 21); do
        replay "$label-off$id" "$scenario" NGU_GUARD_OFF=$id &
        if (( id % 4 == 1 )); then wait; fi
    done
    wait
    read -r _ total p99 worst _ _ < <(python3 -B "$NGU_RUST_REPO/tools/guard_frames.py" "$run/$label-on/trace.csv")
    echo "== $label: all shortcuts on: $total instructions, 99th percentile frame $p99, worst frame $worst"
    echo "guard  total  p99 frame  worst frame   (change when only this shortcut is disabled)"
    for id in none $(seq 2 21); do
        name="$label-off$id"; [[ $id == none ]] && name="$label-none"
        read -r _ t p w _ _ < <(python3 -B "$NGU_RUST_REPO/tools/guard_frames.py" "$run/$name/trace.csv")
        printf '%5s  %+6.2f%%  %+8.2f%%  %+9.2f%%\n' "$id" "$(echo "($t - $total) * 100 / $total" | bc -l)" "$(echo "($p - $p99) * 100 / $p99" | bc -l)" "$(echo "($w - $worst) * 100 / $worst" | bc -l)"
    done
done
echo "Logs and traces: $run"
