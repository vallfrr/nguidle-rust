#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
# Decompiles the game's assemblies to re/decomp/<name> (C# project). Read-only on the game.
set -e
export DOTNET_ROOT=~/.dotnet PATH=~/.dotnet:~/.dotnet/tools:$PATH DOTNET_CLI_TELEMETRY_OPTOUT=1
M="$NGU_GAME_DIR/Managed"
O="$NGU_WORK/decomp/decomp"
for a in Assembly-CSharp Assembly-CSharp-firstpass; do
  mkdir -p "$O/$a"
  ilspycmd -p -o "$O/$a" -r "$M" "$M/$a.dll" > "$O/$a.log" 2>&1 || echo "FAILED $a (see $O/$a.log)"
done
for a in Assembly-CSharp Assembly-CSharp-firstpass; do
  echo "$a: $(find "$O/$a" -name '*.cs' | wc -l) files, $(cat $(find "$O/$a" -name '*.cs') | wc -l) lines"
done
