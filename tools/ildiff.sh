#!/bin/bash
# ildiff.sh Type::method arg... : compares the oracle (interpreter) and the generated Rust (ngu-gen already built by ilgen.sh)
. ~/.cargo/env
M="/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed/Assembly-CSharp.dll"
O=$(HOME=$HOME $HOME/build/clr/release/clr "$M" run "$@" 2>&1 | tail -1)
export NGU_DLL="$M"
G=$(/mnt/j/NGUvita/build-ngu/game/ngu-gen "${@:2}" 2>&1 | tail -1)
if [ "$O" == "$G" ]; then echo "OK   $* -> $G"; else echo "DIFF $* | oracle=[$O] rust=[$G]"; exit 1; fi
