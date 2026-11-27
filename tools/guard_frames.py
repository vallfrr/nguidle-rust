#!/usr/bin/env python3
"""Per-frame cost of a scenario run (CSV written by `clr <dll> autoplay`): total, 99th percentile and worst frame, in
interpreted instructions and in microseconds of game logic. Used by tools/guard-frames.sh to compare a run with one
interpreter shortcut disabled against the run with all of them.

Usage: guard_frames.py <trace.csv> -> prints "frames total_instr p99_instr max_instr p99_logic_us max_logic_us"."""
import csv
import sys


def percentile(values, share):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int(len(ordered) * share))] if ordered else 0


def summarize(path):
    with open(path, newline="") as handle:
        rows = list(csv.DictReader(handle))
    instr = [int(row["instr"]) for row in rows]
    logic = [int(row["logic_us"]) for row in rows]
    return len(rows), sum(instr), percentile(instr, 0.99), max(instr, default=0), percentile(logic, 0.99), max(logic, default=0)


if __name__ == "__main__":
    print(*summarize(sys.argv[1]))
