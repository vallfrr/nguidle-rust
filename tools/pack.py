"""Pack prepared game data into the engine's NGUPAK1 format."""

import argparse
import shutil
import struct
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("assets", type=Path, help="Output directory from nguprep")
    parser.add_argument("dll", type=Path, help="Assembly-CSharp.dll from your verified game installation")
    parser.add_argument("output", type=Path, help="Output file, typically ngu.pak")
    args = parser.parse_args()

    assets = args.assets
    files = {"Assembly-CSharp.dll": args.dll, "scene.json": assets / "scene.json", "tags.json": assets / "tags.json"}
    for folder in ("audio", "fonts", "tex", "text"):
        files.update((p.relative_to(assets).as_posix(), p) for p in (assets / folder).rglob("*") if p.is_file())
    index = [(name.encode("utf-8"), path, path.stat().st_size) for name, path in sorted(files.items())]
    offset = 12 + sum(18 + len(name) for name, _, _ in index)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    staging = args.output.with_suffix(".pending")
    with staging.open("wb") as output:
        output.write(struct.pack("<8sI", b"NGUPAK1\0", len(index)))
        for name, _, size in index:
            output.write(struct.pack("<H", len(name)) + name + struct.pack("<QQ", offset, size))
            offset += size
        for _, source, _ in index:
            with source.open("rb") as input_file:
                shutil.copyfileobj(input_file, output)
    staging.replace(args.output)
    print(f"Packed {len(index)} files ({offset:,} bytes) in {args.output.name}.")


if __name__ == "__main__":
    main()
