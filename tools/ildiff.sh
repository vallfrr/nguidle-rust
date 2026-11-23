#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# ildiff.sh Type::method arg... : compares the oracle (interpreter) and the generated Rust (ngu-gen already built by ilgen.sh)
. ~/.cargo/env
M="$NGU_GAME_DIR/Managed/Assembly-CSharp.dll"
O=$("$NGU_CLR_TARGET"/release/clr "$M" run "$@" 2>&1 | tail -1)
export NGU_DLL="$M"
G=$("$NGU_WSL_TARGET/game/ngu-gen" "${@:2}" 2>&1 | tail -1)
if [ "$O" == "$G" ]; then echo "OK   $* -> $G"; else echo "DIFF $* | oracle=[$O] rust=[$G]"; exit 1; fi
