#!/usr/bin/env bash
set -e
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
NGU_STATE_DIR=$(ngu_new_state_run game)
scene=$(ngu_fixture_view)
exec bash "$NGU_RUST_REPO/tools/clr.sh" load "$scene" "${1:-3}"
