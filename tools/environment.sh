#!/usr/bin/env bash
# Source before using project paths. Existing environment overrides take precedence.
NGU_RUST_REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
if [ "$(basename -- "$(dirname -- "$NGU_RUST_REPO")")" = repos ]; then
    ngu_default_home=$(dirname -- "$(dirname -- "$NGU_RUST_REPO")")
elif [ -d "$NGU_RUST_REPO/../NGU IDLE/NGUIdle_Data" ]; then
    ngu_default_home=$(cd -- "$NGU_RUST_REPO/.." && pwd -P)
else
    ngu_default_home=$NGU_RUST_REPO
fi
: "${NGUIDLE_HOME:=$ngu_default_home}"
: "${NGU_WORK:=$NGUIDLE_HOME/work}"
: "${NGU_BUILD:=$NGUIDLE_HOME/build}"
ngu_default_game="$NGUIDLE_HOME/game/NGU IDLE/NGUIdle_Data"
if [ ! -d "$ngu_default_game" ] && [ -d "$NGUIDLE_HOME/NGU IDLE/NGUIdle_Data" ]; then
    ngu_default_game="$NGUIDLE_HOME/NGU IDLE/NGUIdle_Data"
fi
: "${NGU_GAME_DIR:=$ngu_default_game}"
if [ -d "$NGU_WORK/prepared/ngu" ]; then
    : "${NGU_PREPARED:=$NGU_WORK/prepared/ngu}"
else
    : "${NGU_PREPARED:=$NGU_RUST_REPO/out/ngu}"
fi
: "${NGU_WSL_TARGET:=$NGU_BUILD/wsl}"
: "${NGU_CLR_TARGET:=$NGU_BUILD/wsl-clr}"
: "${NGU_STATE_DIR:=$NGU_WORK/traces/statediff}"
: "${NGU_WINDOWS_PYTHON:=$NGUIDLE_HOME/venvs/prep-win/Scripts/python.exe}"
export NGUIDLE_HOME NGU_WORK NGU_BUILD NGU_GAME_DIR NGU_PREPARED NGU_RUST_REPO NGU_WSL_TARGET NGU_CLR_TARGET NGU_STATE_DIR NGU_WINDOWS_PYTHON
unset ngu_default_home ngu_default_game

ngu_new_state_run() {
    mkdir -p -- "$NGU_STATE_DIR" && mktemp -d "$NGU_STATE_DIR/$1.XXXXXX"
}

# The replay writes its own save underneath the scene directory.
ngu_fixture_view() {
    local assets view asset
    assets=$(cd -- "$NGU_PREPARED" && pwd -P) || return 1
    test -s "$assets/scene.json" || { echo "Prepare the game assets first." >&2; return 1; }
    view=$(mktemp -d "$NGU_STATE_DIR/scene.XXXXXX") || return 1
    for asset in "$assets"/*; do
        case "${asset##*/}" in save|log.txt) continue ;; esac
        ln -s -- "$asset" "$view/${asset##*/}" || return 1
    done
    printf '%s\n' "$view"
}
