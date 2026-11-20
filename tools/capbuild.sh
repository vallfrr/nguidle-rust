#!/bin/bash
# capbuild.sh [GB] cmd... : runs a command under a memory cap (cgroup, no swap) so the system drive is never filled through WSL's swap.
GB=${1:-12}; shift
exec systemd-run --user --scope -q -p MemoryMax=${GB}G -p MemorySwapMax=0 "$@"
