#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# ilall.sh : generates the WHOLE game, cargo check, error summary in "$NGU_BUILD/ilall"
. ~/.cargo/env
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$NGU_WSL_TARGET}"
cd "$NGU_RUST_REPO"
mkdir -p "$NGU_BUILD/ilall"
cargo build -q --release -p ngu-ilgen 2>&1 | grep -E "^error" -A5
"$CARGO_TARGET_DIR/release/ngu-ilgen" "$NGU_GAME_DIR/Managed/Assembly-CSharp.dll" crates/ngu-gen/src ALL NumberOutput::suffixFormat
cut -d: -f2- crates/ngu-gen/src/coverage.txt | sed "s/[0-9]\+//g" | sort | uniq -c | sort -rn | head -${1:-8}
bash tools/capbuild.sh 12 timeout 580 cargo check -p ngu-gen --message-format short 2>&1 | grep -E "error" > "$NGU_BUILD/ilall"/errs.txt
echo "rustc errors: $(wc -l < "$NGU_BUILD/ilall"/errs.txt)"
sed -E "s/^[^ ]+ //; s/\[E[0-9]+\]//; s/DictImpl<[^>]*>/D/g; s/C[0-9]+_[A-Za-z0-9_]+/C/g" "$NGU_BUILD/ilall"/errs.txt | cut -c1-170 | sort | uniq -c | sort -rn | head -${2:-20}
