#!/bin/bash
# ngugame.sh [frames] : runs the generated game (ngu-gen host) on the extracted scene. Output: the host's stdout/stderr.
B=/mnt/j/NGUvita/build-ngu/game/ngu-gen
exec bash /mnt/j/NGUvita/nguvita/tools/capbuild.sh 8 timeout ${NGU_TIMEOUT:-300} $B game "/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed/Assembly-CSharp.dll" /mnt/j/NGUvita/nguvita/out/ngu ${1:-3}
