"""Compare synthetic .NET fixture results with the CLR interpreter."""

import argparse
import subprocess
import sys
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dotnet", required=True)
    parser.add_argument("--clr", required=True)
    parser.add_argument("--dll", required=True, type=Path)
    options = parser.parse_args()
    cases = {
        "Branch": [-3, 0, 7],
        "Loop": [0, 1, 10],
        "Choice": [-1, 0, 1, 2, 3],
        "SeededRandom": [-1, 0, 1, 123456789],
    }
    failures = []
    for method, arguments in cases.items():
        for argument in arguments:
            native = subprocess.check_output(
                [options.dotnet, str(options.dll), method, str(argument)], text=True
            ).strip()
            interpreted = subprocess.check_output(
                [options.clr, str(options.dll), "run", f"Fixture::{method}", str(argument)],
                text=True,
            ).strip()
            if interpreted != f"=> {native}":
                failures.append(f"{method}({argument}): .NET={native}, interpreter={interpreted}")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print(f"{sum(len(values) for values in cases.values())} fixture cases match .NET.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
