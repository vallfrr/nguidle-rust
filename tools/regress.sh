#!/usr/bin/env bash
set -euo pipefail
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"

mode=${1:-verify}
case "$mode" in
    record|verify) ;;
    *) echo "Usage: $0 [record|verify]" >&2; exit 2 ;;
esac

baseline=$(realpath -m -- "${NGU_GAME_BASELINE:-$NGU_WORK/traces/game-baseline}")
prepared=$(realpath -- "$NGU_PREPARED")
game=$(realpath -- "$NGU_GAME_DIR/..")
for protected in "$NGU_RUST_REPO" "$prepared" "$game"; do
    case "$baseline/" in
        "$protected/"*) echo "Choose a reference folder outside $protected." >&2; exit 2 ;;
    esac
done
case "$mode" in
    record)
        if [[ -e "$baseline" || -L "$baseline" ]]; then
            echo "Reference already exists: $baseline" >&2
            exit 2
        fi
        ;;
    verify)
        if [[ ! -f "$baseline/inputs.sha256" ]]; then
            echo "Reference missing. Run 'just verify-game record' first: $baseline" >&2
            exit 2
        fi
        ;;
esac

NGU_STATE_DIR="$NGU_WORK/traces"
NGU_STATE_DIR=$(ngu_new_state_run game-replay)
run="$NGU_STATE_DIR"
out="$run/results"
mkdir -- "$out"
echo "Local replay logs: $run"
case "$run" in
    *','*|*';'*) echo "Replay paths cannot contain commas or semicolons." >&2; exit 2 ;;
esac

dll="$NGU_GAME_DIR/Managed/Assembly-CSharp.dll"
export NGU_FIXED_NOW=1700000000 NGU_NO_GUARD=1
{
    sha256sum -- "$dll" | cut -d' ' -f1
    sha256sum -- "$prepared/scene.json" | cut -d' ' -f1
} > "$out/inputs.sha256"
if [[ "$mode" == verify ]] && ! cmp -s "$baseline/inputs.sha256" "$out/inputs.sha256"; then
    echo "The game assembly or prepared scene differs from the recorded inputs." >&2
    exit 1
fi

target=$(realpath -m -- "${CARGO_TARGET_DIR:-$NGU_CLR_TARGET}")
cargo build --locked --release --manifest-path "$NGU_RUST_REPO/Cargo.toml" -p clr --bin clr --target-dir "$target"
clr="$target/release/clr"
if [[ -f "$clr.exe" ]]; then clr="$clr.exe"; fi

real_save=${NGU_REAL_SAVE:-$NGU_WORK/saves/imported-backup/NGUSaveSteam.txt}
if [[ "$mode" == record ]]; then
    if [[ -n "${NGU_REAL_SAVE:-}" || -f "$real_save" ]]; then
        cp -- "$real_save" "$out/real-input.sav"
    fi
elif [[ -f "$baseline/real-input.sav" ]]; then
    cp -- "$baseline/real-input.sav" "$out/real-input.sav"
fi

replay() {
    local name=$1 frames=$2 steps=$3 scene
    scene=$(ngu_fixture_view)
    if ! NGU_STEPS="$steps;s,$out/$name.sav" NGU_DUMP="$out/$name.state" \
        "$clr" "$dll" load "$scene" "$frames" > "$run/$name.log" 2>&1; then
        echo "Replay failed: $name (see $run/$name.log)" >&2
        return 1
    fi
    if [[ ! -s "$out/$name.state" || ! -s "$out/$name.sav" ]] ||
        ! base64 --decode "$out/$name.sav" >/dev/null ||
        grep -q '^\[save\] exception:' "$run/$name.log"; then
        echo "Missing state or invalid save for $name (see $run/$name.log)" >&2
        return 1
    fi
    echo "Replayed $name"
}

for value in 0.0 0.5 1.5 999.0 1000.0 12345.678 999999.0 1000000.0 1234567.0 1.5e9 7.2e12 1e18 1.234e30 9.99e100 1e200 1.7e308 -5.0 -1234567.0 123456789012.0 3.3e66 6.1e303; do
    for format in 0 1 2 3; do
        result=$("$clr" "$dll" run NumberOutput::suffixFormat "$value" "$format")
        if [[ "$result" != '=> '* ]]; then
            echo "NumberOutput failed for $value ($format): $result" >&2
            exit 1
        fi
        printf '%s\t%s\t%s\n' "$value" "$format" "${result#=> }"
    done
done > "$out/number-output.tsv"
echo "Checked 84 number formatting cases"

cd "$NGU_RUST_REPO"
replay grid 30 "$(bash tools/grid-steps.sh)"
replay input 40 "$(bash tools/input-steps.sh);w,200"
for seed in 1 2 3 777; do
    replay "random-$seed" 40 "$(bash tools/rand-steps.sh 300 "$seed");w,300"
done
replay rebirth 40 'click,Start New Game;click,PLAY;w,10;call,Rebirth.resetEnergy;call,Character.totalCapEnergy'
grep -Fxq '[call] Rebirth.resetEnergy : ok' "$run/rebirth.log"
grep -Fxq '[call] Character.totalCapEnergy -> 500' "$run/rebirth.log"
replay full-save 30 "$(bash tools/rand-steps.sh 150 9);w,200"
replay round-trip 30 "l,$out/grid.sav;w,100"
if [[ -f "$out/real-input.sav" ]]; then
    replay real-save 40 "l,$out/real-input.sav;w,100"
else
    echo "Skipped the optional local PC save (none in this reference)."
fi

if [[ "$mode" == record ]]; then
    mkdir -p -- "$(dirname -- "$baseline")"
    mv -T -n -- "$out" "$baseline"
    if [[ -d "$out" ]]; then
        echo "Reference already exists: $baseline" >&2
        exit 1
    fi
    echo "Recorded reference: $baseline"
else
    if ! diff -qr -- "$baseline" "$out"; then
        echo "Replay differs from the reference. Logs: $run" >&2
        exit 1
    fi
    echo "REGRESSION: ALL OK"
fi
