"""Check the recorded content hashes of bundled third-party skills."""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOCK = json.loads((ROOT / "skills.lock").read_text(encoding="utf-8"))
PREFIXES = (".claude/skills/ponytail", ".claude/skills/rust-patterns/", ".claude/skills/rust-testing/")


def main():
    tracked = subprocess.check_output(
        ["git", "-C", str(ROOT), "ls-files", "--", ".claude/skills"],
        text=True,
    ).splitlines()
    expected = {item["path"]: item["sha256"] for item in LOCK["files"]}
    included = {path for path in tracked if path.startswith(PREFIXES)}
    errors = []
    if included != set(expected):
        errors.append("Tracked third-party files differ from skills.lock.")
    for name, digest in expected.items():
        path = ROOT / name
        if not path.is_file():
            errors.append(f"Missing {name}.")
            continue
        normalized = path.read_bytes().replace(b"\r\n", b"\n")
        if hashlib.sha256(normalized).hexdigest() != digest:
            errors.append(f"Hash mismatch: {name}.")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Verified {len(expected)} bundled third-party skill files.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
