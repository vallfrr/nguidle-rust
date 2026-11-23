#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# ngugame.sh [frames] : runs the generated game (ngu-gen host) on the extracted scene. Output: the host's stdout/stderr.
B="$NGU_WSL_TARGET/game/ngu-gen"
exec bash "$NGU_RUST_REPO/tools/capbuild.sh" 8 timeout "${NGU_TIMEOUT:-300}" "$B" game "$NGU_GAME_DIR/Managed/Assembly-CSharp.dll" "$NGU_PREPARED" "${1:-3}"
