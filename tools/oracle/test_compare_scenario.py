import csv
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("compare-scenario.py")
HEADER = ("scheduled_tick", "action", "menu", "energy", "idle", "cap", "gained", "bar")


class CompareScenario(unittest.TestCase):
    def compare(self, original, port):
        with tempfile.TemporaryDirectory() as directory:
            paths = [Path(directory) / "original.csv", Path(directory) / "port.csv"]
            for path, rows in zip(paths, (original, port)):
                with path.open("w", newline="", encoding="utf-8") as file:
                    writer = csv.writer(file)
                    writer.writerow(HEADER)
                    writer.writerows(rows)
            return subprocess.run([sys.executable, str(SCRIPT), *(str(path) for path in paths)], capture_output=True, text=True)

    def test_equivalent_float_spellings(self):
        row = ["0", "mark", "0", "500", "500", "500", "12934", "0.04"]
        result = self.compare([row], [row[:-1] + ["0.04000000"]])
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_changed_resource_is_reported(self):
        row = ["0", "mark", "0", "500", "500", "500", "12934", "0.04"]
        result = self.compare([row], [row[:3] + ["499"] + row[4:]])
        self.assertEqual(result.returncode, 1)
        self.assertIn("energy: original=500 port=499", result.stdout)

    def test_missing_event_fails(self):
        row = ["0", "mark", "0", "500", "500", "500", "12934", "0.04"]
        result = self.compare([row], [])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Row count differs", result.stderr)


if __name__ == "__main__":
    unittest.main()
