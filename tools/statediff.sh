#!/bin/bash
# statediff.sh [frames] : same scene, same fixed clock, N frames -> visible state of the oracle (interpreter) vs generated Rust; prints the diff.
. ~/.cargo/env
N=${1:-30}
export NGU_FIXED_NOW=${NGU_FIXED_NOW:-1700000000} NGU_NO_GUARD=1
DLL="/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed/Assembly-CSharp.dll"
cd /mnt/j/NGUvita/nguvita
# the oracle writes into out/ngu/save: we work on copies so saves do not get mixed
rm -rf ~/build/st && mkdir -p ~/build/st; rm -rf out/ngu/save
NGU_DUMP=~/build/st/oracle.txt ~/build/clr/release/clr "$DLL" load out/ngu $N > ~/build/st/oracle.log 2>&1
rm -rf out/ngu/save
NGU_DUMP=~/build/st/native.txt bash tools/ngugame.sh $N > ~/build/st/native.log 2>&1
rm -rf out/ngu/save
wc -l ~/build/st/oracle.txt ~/build/st/native.txt | head -2
diff ~/build/st/oracle.txt ~/build/st/native.txt > ~/build/st/diff.txt; echo "differing lines: $(grep -c '^[<>]' ~/build/st/diff.txt)"
head -${2:-40} ~/build/st/diff.txt | cut -c1-220
