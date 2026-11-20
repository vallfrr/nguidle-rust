#!/bin/bash
# input-steps.sh : deterministic scenario mixing left clicks, right clicks, middle clicks, keys (letters, digits, modifiers) and wheel
# over a grid of the 960x600 screen; used to compare oracle and native (input fidelity, task A2).
s=""
i=0
for y in 60 150 240 330 420 510; do for x in 60 180 300 420 540 660 780 900; do
  i=$((i+1))
  case $((i % 4)) in
    0) s="$s;$x,$y,5";;
    1) s="$s;r,$x,$y,5";;
    2) s="$s;wheel,$x,$y,2;k,left shift,2;$x,$y,3";;
    3) s="$s;mc,$x,$y,4;k,a,2;k,3,2";;
  esac
done; done
echo "${s#;}"
