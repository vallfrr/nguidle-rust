#!/bin/bash
# vita-cycle.sh <scenario> [label] : one fully automatic measurement on the console, no human in the loop.
#   build benchmark VPK -> install (only if the package changed) -> upload scenario -> launch -> wait for the end marker
#   -> download trace/log -> analyze -> short verdict (<= 25 lines). Everything raw stays in out/vita-60/<label>/.
# Needs (WSL): VITA_AGENT = the wrapper script of the Vita agent (see nguidle-vita/deploy.sh), the Vita SDK, the data pack on the console.
# Options by environment: NO_BUILD=1 (reuse the last package), FIXTURE=<save file> (copied to bench/save/NGUSaveSteam.txt),
#   BUILD_VARS="NGU_VITA_FAST_CLOCKS=OFF ..." (extra cmake switches), BASELINE=<summary.json> (default out/vita-60/baseline.json).
# Stops with ONE line on: console unreachable, build failure, no end marker (crash or far too slow), trace missing.
set -uo pipefail
R=$(cd "$(dirname "$0")/.." && pwd); cd "$R"
SC=${1:?scenario name (nguidle-vita/scenarios/<name>.txt) or file}; LABEL=${2:-$(basename "$SC" .txt)-$(date +%m%d-%H%M)}
[ -f "$SC" ] || SC=nguidle-vita/scenarios/$SC.txt
[ -f "$SC" ] || { echo "no such scenario: $SC"; exit 2; }
SC=$(realpath "$SC")  # the agent wrapper may run from another directory: absolute paths only
: "${VITA_AGENT:?set VITA_AGENT to the Vita agent wrapper script}"
A() { bash "$VITA_AGENT" "$@"; }
OUT=$R/out/vita-60/$LABEL; mkdir -p "$OUT"
export NGU_VITA_BUILD_DIR=${NGU_VITA_BUILD_DIR:-$R/out/vita-bench-build} NGUIDLE_RUST=$R
A app running > "$OUT/agent.json" 2>&1 || { echo "console unreachable (agent: $(head -c 150 "$OUT/agent.json" | tr '\n' ' '))"; exit 3; }
if [ -z "${NO_BUILD:-}" ]; then
  env NGU_VITA_BENCHMARK=ON ${BUILD_VARS:-} bash nguidle-vita/build.sh > "$OUT/build.log" 2>&1 || { echo "build failed:"; tail -8 "$OUT/build.log"; exit 4; }
fi
PKG=$NGU_VITA_BUILD_DIR/ngu.vpk; SUM=$(sha256sum < "$PKG" | cut -c1-16)
if [ "$(cat out/vita-60/.last-deployed 2>/dev/null)" != "$SUM" ]; then
  bash nguidle-vita/deploy.sh > "$OUT/deploy.log" 2>&1 || { echo "install failed:"; tail -4 "$OUT/deploy.log"; exit 5; }
  echo "$SUM" > out/vita-60/.last-deployed
fi
B=ux0:data/ngu/bench
for f in trace.bin done.txt host.txt scenario.txt; do A fs trash $B/$f > /dev/null 2>&1; done
A fs trash ux0:data/ngu/log.txt > /dev/null 2>&1
[ -z "${KEEP_SAVES:-}" ] && A fs trash $B/save > /dev/null 2>&1  # scenarios start from a clean profile (a stale slot asks to overwrite)
[ -n "${FIXTURE:-}" ] && A fs upload "$FIXTURE" $B/save/NGUSaveSteam.txt --overwrite > /dev/null 2>&1
A fs upload "$SC" $B/scenario.txt --overwrite > "$OUT/upload.log" 2>&1 || { echo "scenario upload failed"; exit 6; }
TICKS=$(grep -E '^[0-9]+ ' "$SC" | tail -1 | cut -d' ' -f1); LIMIT=$(( TICKS / 30 * 3 / 2 + 180 ))
A app launch NGUI00001 > "$OUT/launch.log" 2>&1
T0=$(date +%s); DONE=
while [ $(( $(date +%s) - T0 )) -lt $LIMIT ]; do
  sleep 30
  rm -f "$OUT/done.txt"; A fs download $B/done.txt "$OUT/done.txt" > /dev/null 2>&1
  [ -s "$OUT/done.txt" ] && { DONE=1; break; }
done
for f in trace.bin host.txt; do A fs download $B/$f "$OUT/$f" > /dev/null 2>&1; done
A fs download ux0:data/ngu/log.txt "$OUT/log.txt" > /dev/null 2>&1
A app close NGUI00001 > /dev/null 2>&1
if [ -z "$DONE" ]; then echo "no end marker after ${LIMIT}s (crash or far too slow); last log lines:"; tail -n 5 "$OUT/log.txt" 2>/dev/null; fi
[ -s "$OUT/trace.bin" ] || { echo "no trace downloaded"; exit 7; }
BASE=${BASELINE:-out/vita-60/baseline.json}; EXTRA=(); [ -f "$BASE" ] && EXTRA=(--baseline "$BASE")
echo "== $LABEL ($(basename "$SC"), package $SUM)"
python3 tools/vita-trace.py "$OUT/trace.bin" --scenario "$SC" --log "$OUT/log.txt" --save "$OUT/summary.json" "${EXTRA[@]}" | tee "$OUT/report.txt"
grep -a "\[autoplay\] check" "$OUT/log.txt" | head -3
