#!/bin/bash
# screens.sh <name> [steps...] : starts a game (Start New Game + PLAY), applies the steps (e.g. "do,dev/unlock.steps",
# "l,save.txt"), then captures the 58 menus (MenuSwapper.swapMenu) into out/screens/<name>/NN-Name.png + NN-Name.txt (screen as text).
# Through the interpreter (same engine/rendering as native). Contact sheets: python tools/sheet.py out/screens/<name>
cd /mnt/j/NGUvita/nguvita
R=/mnt/j/NGUvita/nguvita
OUT=$R/out/screens/$1; shift
mkdir -p "$OUT"
names=$(timeout 900 bash tools/clr.sh play $R/out/ngu /tmp/ngu-names.png menus 2>/dev/null | sed -n 's/^\[menus\] //p')
steps=("click,Start New Game" "click,PLAY")
for s in "$@"; do steps+=("${s/do,dev\//do,$R/dev/}"); done
steps+=("png,$OUT/start.png" "ui,$OUT/start.txt")
while read -r i name; do
  f=$(printf "%02d-%s" "$i" "$(echo "$name" | tr -c 'A-Za-z0-9\n' '_' | sed 's/_*$//')")
  steps+=("menu,$i" "w,5" "png,$OUT/$f.png" "ui,$OUT/$f.txt")
done <<< "$names"
timeout 1800 bash tools/clr.sh play $R/out/ngu "$OUT/last.png" "${steps[@]}" 2>&1 | grep -E "^\[(menu|click|call)\]|panicked|EXCEPTION|ERREUR"
ls "$OUT"/*.png | wc -l
