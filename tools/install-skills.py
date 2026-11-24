"""Restore locked third-party skill files from their recorded upstream commits."""

import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def git(repo, *args):
    return subprocess.check_output(["git", "-C", str(repo), *args])


def main():
    lock = json.loads((ROOT / "skills.lock").read_text(encoding="utf-8"))
    prepared = {}
    groups = {}
    for entry in lock["files"]:
        groups.setdefault((entry["source"], entry["upstream_revision"]), []).append(entry)
    with tempfile.TemporaryDirectory(prefix="ngu-skill-source-") as temporary:
        for number, ((source, revision), entries) in enumerate(groups.items()):
            if not revision or len(revision) != 40:
                raise ValueError("Every skill source needs an exact Git commit.")
            repo = Path(temporary) / str(number)
            repo.mkdir()
            git(repo, "init", "--quiet", "--bare")
            git(repo, "fetch", "--quiet", "--depth=1", source, revision)
            resolved = git(repo, "rev-parse", "FETCH_HEAD^{commit}").decode().strip()
            if resolved != revision:
                raise ValueError(f"Fetched revision differs from lock: {source}")
            for entry in entries:
                destination = Path(entry["path"])
                skill = destination.parts[2]
                upstream = f"skills/{skill}/SKILL.md" if destination.name == "SKILL.md" else "LICENSE"
                data = git(repo, "show", f"{revision}:{upstream}").replace(b"\r\n", b"\n")
                if hashlib.sha256(data).hexdigest() != entry["sha256"]:
                    raise ValueError(f"Upstream content differs from lock: {destination}")
                prepared[ROOT / destination] = data
    for destination, data in prepared.items():
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    print(f"Restored {len(prepared)} verified third-party skill files.")


if __name__ == "__main__":
    main()
