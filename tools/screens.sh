#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# screens.sh <name> [steps...] : starts a game (Start New Game + PLAY), applies the steps (e.g. "do,dev/unlock.steps",
# "l,save.txt"), then captures the 58 menus (MenuSwapper.swapMenu) into work/captures/<name>/NN-Name.png + NN-Name.txt (screen as text).
# Through the interpreter (same engine/rendering as native). Contact sheets: python tools/sheet.py work/captures/<name>
cd "$NGU_RUST_REPO"
R=$NGU_RUST_REPO
OUT=$NGU_WORK/captures/$1; shift
mkdir -p "$OUT"
names=$(timeout 900 bash tools/clr.sh play "$NGU_PREPARED" "$OUT/names.png" menus 2>/dev/null | sed -n 's/^\[menus\] //p')
steps=("click,Start New Game" "click,PLAY")
for s in "$@"; do steps+=("${s/do,dev\//do,$R/dev/}"); done
steps+=("png,$OUT/start.png" "ui,$OUT/start.txt")
while read -r i name; do
  f=$(printf "%02d-%s" "$i" "$(echo "$name" | tr -c 'A-Za-z0-9\n' '_' | sed 's/_*$//')")
  steps+=("menu,$i" "w,5" "png,$OUT/$f.png" "ui,$OUT/$f.txt")
done <<< "$names"
timeout 1800 bash tools/clr.sh play "$NGU_PREPARED" "$OUT/last.png" "${steps[@]}" 2>&1 | grep -E "^\[(menu|click|call)\]|panicked|EXCEPTION|ERREUR"
ls "$OUT"/*.png | wc -l
