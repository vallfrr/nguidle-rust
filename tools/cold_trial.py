"""Mechanical part of the porting-guide cold trial: follow docs/PORTING.md literally in an empty folder.

It clones this engine into a temporary folder, generates a port with `just new-port` exactly as the guide shows, runs the
port's `just check`, and prints a report that can be pasted into a pull request or `docs/archive/`. It proves the commands
of the guide work from nothing. The other half of the trial (a fresh AI agent that is given only the clone and the request
"create a headless port nguidle-test" and reports every blocker) cannot be automated: see docs/TESTING.md.

    python tools/cold_trial.py [--report file.md]
"""

import argparse
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ENGINE = Path(__file__).resolve().parent.parent


def run(command, cwd, log):
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    log.append((" ".join(command), result.returncode, (result.stdout + result.stderr)[-1500:]))
    return result.returncode == 0


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--report", type=Path)
    options = parser.parse_args()
    if not shutil.which("just"):
        raise SystemExit("`just` is required (docs/PORTING.md, step 1).")
    work = Path(tempfile.mkdtemp(prefix="ngu-cold-trial-"))
    log = []
    try:
        head = subprocess.run(["git", "-C", str(ENGINE), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
        clone = work / "nguidle-rust"
        steps = [
            (["git", "clone", "-q", str(ENGINE), str(clone)], work),
            (["git", "checkout", "-q", head], clone),
            (["just", "new-port", "nguidle-test", "Test platform", "../nguidle-rust"], clone),
            (["just", "check"], work / "nguidle-test"),
        ]
        ok = all(run(command, cwd, log) for command, cwd in steps)
        lines = [f"# Cold trial, mechanical part (engine {head[:12]})", ""]
        for command, code, tail in log:
            lines += [f"- `{command}` -> {'ok' if code == 0 else f'FAILED ({code})'}"]
            if code:
                lines += ["", "```text", tail.strip(), "```", ""]
        lines += ["", "Result: " + ("every documented command worked from an empty folder." if ok else "a documented command failed: fix the guide, template or tool."), ""]
        text = "\n".join(lines)
        print(text)
        if options.report:
            options.report.write_text(text, encoding="utf-8")
        return 0 if ok else 1
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
