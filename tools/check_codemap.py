"""Check that the code map lists each first-party Rust source file."""

import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
sources = subprocess.check_output(
    ["git", "-C", str(root), "ls-files", "--", "clr/src/*.rs", "crates/*/src/*.rs"],
    text=True,
).splitlines()
documented = (root / "docs" / "CODEMAP.md").read_text(encoding="utf-8")
missing = [name for name in sources if f"`{name}`" not in documented]
if missing:
    print("Code map is missing tracked sources:", *missing, sep="\n", file=sys.stderr)
    sys.exit(1)
print(f"Code map covers {len(sources)} tracked Rust source files.")
