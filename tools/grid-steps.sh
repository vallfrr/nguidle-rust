#!/bin/bash
# grid-steps.sh : deterministic scenario = one click on a grid of the 960x600 screen (6 frames wait after each click)
s=""
for y in 60 130 200 270 340 410 480 550; do for x in 40 120 200 280 360 440 520 600 680 760 840 920; do s="$s;$x,$y,6"; done; done
echo "${s#;}"
