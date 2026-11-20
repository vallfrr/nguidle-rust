#!/bin/bash
# rand-steps.sh [N] [seed] : N clicks at pseudo-random positions (960x600) with varying waits; deterministic
N=${1:-300}; s=${2:-12345}; out=""
for i in $(seq 1 $N); do
  s=$(( (s * 1103515245 + 12345) & 0x7fffffff )); x=$(( 10 + s % 940 ))
  s=$(( (s * 1103515245 + 12345) & 0x7fffffff )); y=$(( 10 + s % 580 ))
  s=$(( (s * 1103515245 + 12345) & 0x7fffffff )); w=$(( 2 + s % 25 ))
  out="$out;$x,$y,$w"
done
echo "${out#;}"
