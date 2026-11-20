"""Verify a local game data directory against a supported build's SHA-256 manifest."""

import argparse
import hashlib
import json
from pathlib import Path
import sys


def verify(data_dir: Path, files: dict[str, str]) -> list[str]:
    errors = []
    root = data_dir.resolve()
    for name, expected in sorted(files.items()):
        path = (root / name).resolve()
        if not path.is_relative_to(root):
            errors.append(f"Path outside the game data: {name}")
        elif not path.is_file():
            errors.append(f"Missing: {name}")
        else:
            with path.open("rb") as stream:
                actual = hashlib.file_digest(stream, "sha256").hexdigest()
            if actual != expected:
                errors.append(f"Wrong SHA-256: {name}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("data_dir", type=Path, help="Directory containing Managed/Assembly-CSharp.dll")
    parser.add_argument("--manifest", type=Path, default=Path(__file__).with_name("game-data.sha256.json"))
    args = parser.parse_args()
    try:
        manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
        if not isinstance(manifest["files"], dict) or not manifest["files"]:
            raise ValueError("the manifest must list at least one required file")
        errors = verify(args.data_dir, manifest["files"])
    except (ValueError, OSError, KeyError) as error:
        print(f"Cannot verify game data: {error}", file=sys.stderr)
        return 1
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        print(f"{len(errors)} required files failed verification for {manifest['build']}.", file=sys.stderr)
        return 1
    print(f"Verified {len(manifest['files'])} files for {manifest['build']}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
