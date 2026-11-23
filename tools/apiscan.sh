#!/bin/bash
. "$(dirname -- "${BASH_SOURCE[0]}")/environment.sh"
set -e
export DOTNET_ROOT=~/.dotnet PATH=~/.dotnet:$PATH DOTNET_CLI_TELEMETRY_OPTOUT=1
cd "$NGU_RUST_REPO/tools/apiscan"
dotnet build -c Release -o "$NGU_BUILD/apiscan" -v q 2>&1 | tail -3
M="$NGU_GAME_DIR/Managed"
O="$NGU_WORK/decomp/api"
mkdir -p "$O"
for a in Assembly-CSharp Assembly-CSharp-firstpass; do
  dotnet "$NGU_BUILD/apiscan"/apiscan.dll "$M/$a.dll" > "$O/$a.members.txt"
  echo "$a: $(wc -l < "$O/$a.members.txt") external members"
done
echo "--- by namespace/type (Assembly-CSharp), top 60"
sed 's/::.*//' "$O/Assembly-CSharp.members.txt" | sort | uniq -c | sort -rn | head -60
