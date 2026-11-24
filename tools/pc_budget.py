#!/usr/bin/env python3
"""Check the PC window against the versioned budgets (budgets.json).

   python tools/pc_budget.py <ngu-window executable> <Assembly-CSharp.dll> <prepared folder> [--frames N]
   python tools/pc_budget.py --log <captured stderr>      (check an existing capture)

Runs `ngu-window` with NGU_PERF=1 on the title screen at 1280x720 and reads its "[perf]" lines. Needs a display and the player's
prepared game data. Exit code 0 when every budget holds, 1 when one fails, 2 when the capture has no measurement.
Report the machine (monitor, GPU) next to any figure you publish: the result depends on it.
"""
import argparse
import json
import os
import re
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RATE = re.compile(r"\[perf\] gpu ([0-9.]+) img/s")
INTERVAL = re.compile(r"\[perf\] frame interval \| max ([0-9.]+) ms, \d+ >7\.5 ms, (\d+) >20 ms")


def check(log, budget):
    """Return (code, lines) for one capture of the window's stderr."""
    rates = [float(m.group(1)) for m in RATE.finditer(log)]
    intervals = [(float(m.group(1)), int(m.group(2))) for m in INTERVAL.finditer(log)]
    if len(rates) < 3 or not intervals:
        return 2, ["untrusted: the capture holds fewer than 3 measurement windows"]
    rate = statistics.median(rates)
    longest = max(i[0] for i in intervals)
    slow = sum(i[1] for i in intervals)
    results = [
        (f"median {rate:.1f} img/s (at least {budget['min_img_per_s']})", rate >= budget["min_img_per_s"]),
        (f"longest frame interval {longest:.1f} ms (at most {budget['max_interval_ms']})", longest <= budget["max_interval_ms"]),
        (f"frames over 20 ms: {slow} (at most {budget['max_frames_over_20ms']})", slow <= budget["max_frames_over_20ms"]),
    ]
    lines = [f"{text}: {'ok' if ok else 'FAIL'}" for text, ok in results]
    return (0 if all(ok for _, ok in results) else 1), lines


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("window", nargs="?")
    parser.add_argument("dll", nargs="?")
    parser.add_argument("data", nargs="?")
    parser.add_argument("--frames", type=int, default=1800)
    parser.add_argument("--log", type=Path)
    args = parser.parse_args()
    budget = json.loads((ROOT / "budgets.json").read_text(encoding="utf-8"))["pc_window"]
    if args.log:
        log = args.log.read_text(encoding="utf-8", errors="replace")
    else:
        if not (args.window and args.dll and args.data):
            parser.error("give the executable, the DLL and the prepared folder, or --log")
        with tempfile.TemporaryDirectory() as saves:
            env = dict(os.environ, NGU_PERF="1", NGU_SAVE_DIR=saves)
            run = subprocess.run([args.window, args.dll, args.data, "60", str(args.frames)], env=env, capture_output=True, text=True, timeout=600)
        log = run.stderr
    code, lines = check(log, budget)
    print(f"scenario {budget['scenario']} at {budget['size']}")
    print("\n".join(lines))
    sys.exit(code)


if __name__ == "__main__":
    main()
