#!/bin/bash
# divtrace.sh [frames] [window] : full oracle + native traces then first divergence (see firstdiv.py)
N=${1:-30}
export NGU_FIXED_NOW=${NGU_FIXED_NOW:-1700000000} NGU_TRACE=1 NGU_TRACE_CALLS=1 NGU_TRACE_M=1 NGU_NO_GUARD=1
DLL="/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed/Assembly-CSharp.dll"
cd /mnt/j/NGUvita/nguvita
(cd clr && CARGO_TARGET_DIR=$HOME/build/clr cargo build --release -q 2>&1 | grep -E "^error" -A6)
mkdir -p ~/build/st
rm -rf out/ngu/save; ~/build/clr/release/clr "$DLL" load out/ngu $N > ~/build/st/o.full 2>&1
rm -rf out/ngu/save; bash tools/ngugame.sh $N > ~/build/st/n.full 2>&1; rm -rf out/ngu/save
python3 tools/firstdiv.py ~/build/st/o.full ~/build/st/n.full ${2:-14}
