"""Build synthetic .NET fixtures and compare them with the CLR interpreter."""

import os
import subprocess
import sys
from pathlib import Path
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[2]
PROJECT = ROOT / "tests" / "il-fixtures" / "Fixtures.csproj"
DOTNET = os.environ.get("NGU_DOTNET", "dotnet")
CARGO = os.environ.get("NGU_CARGO", "cargo")


def main():
    subprocess.run([CARGO, "build", "--locked", "-p", "clr"], cwd=ROOT, check=True)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    executable = target / "debug" / ("clr.exe" if os.name == "nt" else "clr")
    with TemporaryDirectory(prefix="ngu-il-fixture-") as temporary:
        output = Path(temporary)
        subprocess.run(
            [DOTNET, "build", str(PROJECT), "--artifacts-path", str(output), "--nologo"],
            cwd=ROOT,
            check=True,
        )
        candidates = list((output / "bin").rglob("Fixtures.dll"))
        if len(candidates) != 1:
            raise RuntimeError(f"Expected one fixture assembly, found {len(candidates)}")
        subprocess.run(
            [
                sys.executable,
                str(PROJECT.parent / "compare.py"),
                "--dotnet", DOTNET,
                "--clr", str(executable),
                "--dll", str(candidates[0]),
            ],
            cwd=ROOT,
            check=True,
        )


if __name__ == "__main__":
    main()
