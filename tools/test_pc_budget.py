"""The PC budget check reads the window's measurement lines and fails when a budget is broken."""

import json
import unittest
from pathlib import Path

import pc_budget

BUDGET = json.loads((Path(__file__).resolve().parent.parent / "budgets.json").read_text(encoding="utf-8"))["pc_window"]


def capture(rate, longest, slow, windows=4):
    window = (
        f"[perf] gpu {rate} img/s | per frame (ms): logic 0.0 emit 0.1 draw+submit 0.1 | 1280x720\n"
        f"[perf] frame interval | max {longest} ms, 0 >7.5 ms, {slow} >20 ms\n"
    )
    return window * windows


class PcBudgetTests(unittest.TestCase):
    def test_healthy_capture_passes(self):
        self.assertEqual(pc_budget.check(capture(200.0, 7.0, 0), BUDGET)[0], 0)

    def test_low_rate_fails(self):
        self.assertEqual(pc_budget.check(capture(40.0, 7.0, 0), BUDGET)[0], 1)

    def test_slow_frames_fail(self):
        self.assertEqual(pc_budget.check(capture(200.0, 7.0, 2), BUDGET)[0], 1)

    def test_long_interval_fails(self):
        self.assertEqual(pc_budget.check(capture(200.0, 80.0, 0), BUDGET)[0], 1)

    def test_empty_capture_is_untrusted(self):
        self.assertEqual(pc_budget.check("nothing", BUDGET)[0], 2)


if __name__ == "__main__":
    unittest.main()
