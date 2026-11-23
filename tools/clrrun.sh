#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# Runs a game method on PC: tools/clrrun.sh Type::Method arg...
exec bash "$NGU_RUST_REPO/tools/clr.sh" run "$@"
