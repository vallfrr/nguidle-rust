#!/bin/bash
# loaddiff.sh : 1) the oracle plays the click grid and saves; 2) oracle and native load THAT save, play 100 frames, then compare visible state + new save.
. ~/.cargo/env
export NGU_FIXED_NOW=${NGU_FIXED_NOW:-1700000000} NGU_NO_GUARD=1
DLL="/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed/Assembly-CSharp.dll"
cd /mnt/j/NGUvita/nguvita
(cd clr && CARGO_TARGET_DIR=$HOME/build/clr cargo build --release -q 2>&1 | grep -E "^error" -A6)
mkdir -p ~/build/st; S=~/build/st/base.sav
rm -rf out/ngu/save; NGU_STEPS="$(bash tools/grid-steps.sh);s,$S" ~/build/clr/release/clr "$DLL" load out/ngu 30 > ~/build/st/l0.log 2>&1
echo "starting save: $(wc -c < $S) bytes"
for mode in oracle native; do
  rm -rf out/ngu/save
  export NGU_STEPS="l,$S;w,100;s,$HOME/build/st/after_$mode.sav" NGU_DUMP=~/build/st/ld_$mode.txt
  if [ $mode = oracle ]; then ~/build/clr/release/clr "$DLL" load out/ngu 30 > ~/build/st/l_$mode.log 2>&1; else bash tools/ngugame.sh 30 > ~/build/st/l_$mode.log 2>&1; fi
done
rm -rf out/ngu/save
cmp -s ~/build/st/after_oracle.sav ~/build/st/after_native.sav && echo "SAVES AFTER LOAD IDENTICAL ($(wc -c < ~/build/st/after_oracle.sav) bytes)" || echo "SAVES AFTER LOAD DIFFERENT"
diff ~/build/st/ld_oracle.txt ~/build/st/ld_native.txt > ~/build/st/ld.diff; echo "differing state lines after load: $(grep -c '^[<>]' ~/build/st/ld.diff)"
head -${1:-20} ~/build/st/ld.diff | cut -c1-200
