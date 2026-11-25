#!/usr/bin/env bash
set -euo pipefail
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
export RUST_BACKTRACE=${BT:-1}
bash "$NGU_RUST_REPO/tools/ngugame.sh" "${1:-3}"
