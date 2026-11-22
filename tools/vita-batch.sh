#!/bin/bash
# vita-batch.sh label:scenario[:VAR=val,VAR=val] ... : runs several measurements one after the other (tools/vita-cycle.sh),
# prints one comparison table at the end. Example (overnight): vita-batch.sh base:session fast:session:NGU_VITA_FAST_CLOCKS=ON monkey:monkey
cd "$(dirname "$0")/.."
for spec in "$@"; do
  IFS=: read -r label sc vars <<< "$spec"
  BUILD_VARS=${vars//,/ } tools/vita-cycle.sh "$sc" "$label" > "out/vita-60-$label.txt" 2>&1 || echo "$label: stopped: $(tail -n 1 out/vita-60-$label.txt)"
done
printf '%-16s %7s %7s %7s %7s\n' label p50 p99 max over
for spec in "$@"; do
  label=${spec%%:*}; f=out/vita-60/$label/summary.json
  [ -f "$f" ] && python3 -c "import json,sys; d=json.load(open('$f')); print('%-16s %7.1f %7.1f %7.1f %7d' % ('$label', d['p50'], d['p99'], d['max'], d['over']))"
done
