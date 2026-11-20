#!/bin/bash
# natdiff.sh [frames] : compares the sequence of engine->game calls and game->engine UnityEngine calls (oracle vs native). First divergence = root cause.
. ~/.cargo/env
N=${1:-30}
export NGU_FIXED_NOW=${NGU_FIXED_NOW:-1700000000} NGU_TRACE=1 NGU_TRACE_CALLS=1 NGU_TRACE_M=1 NGU_NO_GUARD=1
DLL="/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed/Assembly-CSharp.dll"
cd /mnt/j/NGUvita/nguvita
(cd clr && CARGO_TARGET_DIR=$HOME/build/clr cargo build --release -q 2>&1 | grep -E "^error" -A6)
mkdir -p ~/build/st
f() { grep -E "^\[m\]|^\[call\]|^\[native\] UnityEngine[A-Za-z.]*::(set_|Set|Translate|Destroy|Instantiate|AddComponent|Invoke|InvokeRepeating|CancelInvoke|StartCoroutine|AddListener|RemoveListener|Play|Stop|Apply|Find|GetComponent)" | grep -v "::\.ctor\|::\.cctor" | sed "s/ ptr/ struct/g"; }
rm -rf out/ngu/save; ~/build/clr/release/clr "$DLL" load out/ngu $N 2>&1 | f > ~/build/st/o.seq; rm -rf out/ngu/save
rm -rf out/ngu/save; bash tools/ngugame.sh $N 2>&1 | f > ~/build/st/n.seq; rm -rf out/ngu/save
wc -l ~/build/st/o.seq ~/build/st/n.seq | head -2
diff ~/build/st/o.seq ~/build/st/n.seq > ~/build/st/seq.diff
echo "divergences: $(grep -c '^[<>]' ~/build/st/seq.diff)"
head -${2:-30} ~/build/st/seq.diff | cut -c1-200
