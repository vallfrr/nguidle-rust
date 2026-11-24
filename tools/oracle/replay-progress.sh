#!/usr/bin/env bash
# Replay the fresh-state progression probe for 60 seconds of fixed engine time.
set -euo pipefail
. "$(dirname -- "${BASH_SOURCE[0]}")/../environment.sh"
run=$(ngu_new_state_run progress) || exit 1
export NGU_STATE_DIR="$run"
prepared=$(ngu_fixture_view) || exit 1
export NGU_FIXED_NOW=${NGU_FIXED_NOW:-1700000000}
load=''
if [[ -n ${NGU_PROGRESS_PROBE_SAVE:-} ]]; then
    [[ -f $NGU_PROGRESS_PROBE_SAVE && $NGU_PROGRESS_PROBE_SAVE != *';'* ]] || { echo 'Invalid save path' >&2; exit 1; }
    load="l,$NGU_PROGRESS_PROBE_SAVE;"
fi
export NGU_STEPS="call,MainMenuController.finishMainMenu;${load}get,Character.curEnergy;get,Character.idleEnergy;get,Character.capEnergy;get,Character.energyGained;get,Character.energyBarProgress;w,1800;get,Character.curEnergy;get,Character.idleEnergy;get,Character.capEnergy;get,Character.energyGained;get,Character.energyBarProgress"
output="$run/progress.log"
"$NGU_CLR_TARGET/release/clr" "$NGU_GAME_DIR/Managed/Assembly-CSharp.dll" load "$prepared" 90 >"$output" 2>&1
grep -E '^\[(get|call)\]' "$output"
printf 'Full output: %s\n' "$output"
