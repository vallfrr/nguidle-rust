#!/bin/bash
# savediff.sh [frames] : output of ImportExport.getBase64Data() (complete save) of the oracle vs native, after the NGU_STEPS scenario
. ~/.cargo/env
N=${1:-30}
export NGU_FIXED_NOW=${NGU_FIXED_NOW:-1700000000} NGU_NO_GUARD=1
DLL="/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed/Assembly-CSharp.dll"
cd /mnt/j/NGUvita/nguvita
mkdir -p ~/build/st
export NGU_STEPS="${NGU_STEPS:+$NGU_STEPS;}s,$HOME/build/st/save_X.txt"
rm -rf out/ngu/save; NGU_STEPS="${NGU_STEPS//save_X/save_oracle}" ~/build/clr/release/clr "$DLL" load out/ngu $N > ~/build/st/so.log 2>&1
rm -rf out/ngu/save; NGU_STEPS="${NGU_STEPS//save_X/save_native}" bash tools/ngugame.sh $N > ~/build/st/sn.log 2>&1; rm -rf out/ngu/save
ls -la ~/build/st/save_oracle.txt ~/build/st/save_native.txt
if cmp -s ~/build/st/save_oracle.txt ~/build/st/save_native.txt; then echo "SAVES IDENTICAL ($(wc -c < ~/build/st/save_oracle.txt) bytes)"; else echo "SAVES DIFFERENT"; fi
