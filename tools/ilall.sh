#!/bin/bash
# ilall.sh : generates the WHOLE game, cargo check, error summary in ~/build/ilall
. ~/.cargo/env
export CARGO_TARGET_DIR=/mnt/j/NGUvita/build-ngu
cd /mnt/j/NGUvita/nguvita
mkdir -p ~/build/ilall
cargo build -q --release -p ngu-ilgen 2>&1 | grep -E "^error" -A5
$CARGO_TARGET_DIR/release/ngu-ilgen "/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed/Assembly-CSharp.dll" crates/ngu-gen/src ALL NumberOutput::suffixFormat
cut -d: -f2- crates/ngu-gen/src/coverage.txt | sed "s/[0-9]\+//g" | sort | uniq -c | sort -rn | head -${1:-8}
bash tools/capbuild.sh 12 timeout 580 cargo check -p ngu-gen --message-format short 2>&1 | grep -E "error" > ~/build/ilall/errs.txt
echo "rustc errors: $(wc -l < ~/build/ilall/errs.txt)"
sed -E "s/^[^ ]+ //; s/\[E[0-9]+\]//; s/DictImpl<[^>]*>/D/g; s/C[0-9]+_[A-Za-z0-9_]+/C/g" ~/build/ilall/errs.txt | cut -c1-170 | sort | uniq -c | sort -rn | head -${2:-20}
