#!/bin/bash
# where.sh <gen.rs line> : enclosing generated function (to locate a panic)
awk -v n="$1" 'NR<=n && /^pub fn/ {l=$0} NR==n {print substr(l,1,200)}' /mnt/j/NGUvita/nguvita/crates/ngu-gen/src/gen.rs
