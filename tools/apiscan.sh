#!/bin/bash
set -e
export DOTNET_ROOT=~/.dotnet PATH=~/.dotnet:$PATH DOTNET_CLI_TELEMETRY_OPTOUT=1
cd /mnt/j/NGUvita/nguvita/tools/apiscan
dotnet build -c Release -o ~/build/apiscan -v q 2>&1 | tail -3
M="/mnt/j/NGUvita/NGU IDLE/NGUIdle_Data/Managed"
O=/mnt/j/NGUvita/nguvita/re/api
mkdir -p $O
for a in Assembly-CSharp Assembly-CSharp-firstpass; do
  dotnet ~/build/apiscan/apiscan.dll "$M/$a.dll" > $O/$a.members.txt
  echo "$a: $(wc -l < $O/$a.members.txt) external members"
done
echo "--- by namespace/type (Assembly-CSharp), top 60"
sed 's/::.*//' $O/Assembly-CSharp.members.txt | sort | uniq -c | sort -rn | head -60
