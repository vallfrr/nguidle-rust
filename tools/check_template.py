"""Build and test the port template the way a new port would: generate it, then format, lint and test it.

It uses this checkout as the engine, in a temporary folder, with its own target folder under `target/`.
"""

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import new_port


def run(command, cwd, env):
    print("+", " ".join(command), flush=True)
    return subprocess.run(command, cwd=cwd, env=env).returncode


def main():
    import os

    env = dict(os.environ, CARGO_TARGET_DIR=str(new_port.ENGINE / "target" / "template-check"))
    work = Path(tempfile.mkdtemp(prefix="ngu-template-"))
    try:
        # Formatting depends on the port's name (line lengths): check a short and a long one.
        for name in ("nguidle-x", "nguidle-a-rather-long-platform-name-for-formatting"):
            out = new_port.create(name, work / name, "a test platform", new_port.ENGINE.as_posix(), "v0.0.0", "0" * 40, "unused")
            (out / "Cargo.toml").write_text((out / "Cargo.toml").read_text() + "\n[workspace]\n", encoding="utf-8")
            if run(["cargo", "fmt", "--check"], out, env):
                return 1
        for command in (["cargo", "clippy", "--all-targets", "--", "-D", "warnings"], ["cargo", "test"]):
            if run(command, out, env):
                return 1
    finally:
        shutil.rmtree(work, ignore_errors=True)
    print("Port template builds, lints and passes its contract test.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
