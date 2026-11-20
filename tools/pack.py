"""Packs out/ngu (+ Assembly-CSharp.dll) into a single ngu.pak file (NGUPAK1 format).
Usage: python pack.py [output_folder]   (default: out/ngu_pak)
Format: "NGUPAK1\\0" u32 n; n x (u16 len, name, u64 offset, u64 size); data.
"""
import os, struct, sys

SRC = r"J:\NGUvita\nguvita\out\ngu"
DLL = r"J:\NGUvita\NGU IDLE\NGUIdle_Data\Managed\Assembly-CSharp.dll"
OUT = sys.argv[1] if len(sys.argv) > 1 else r"J:\NGUvita\nguvita\out\ngu_pak"
os.makedirs(OUT, exist_ok=True)

files = []
for root, _, names in os.walk(SRC):
    for n in names:
        p = os.path.join(root, n)
        rel = os.path.relpath(p, SRC).replace("\\", "/")
        if rel.startswith("save/") or rel.endswith(".pak"):
            continue
        files.append((rel, p))
files.append(("Assembly-CSharp.dll", DLL))
files.sort()

hdr = 12 + sum(2 + len(r.encode()) + 16 for r, _ in files)
off = hdr
index = []
for rel, p in files:
    size = os.path.getsize(p)
    index.append((rel, off, size))
    off += size
with open(os.path.join(OUT, "ngu.pak"), "wb") as f:
    f.write(b"NGUPAK1\0" + struct.pack("<I", len(files)))
    for rel, o, s in index:
        b = rel.encode()
        f.write(struct.pack("<H", len(b)) + b + struct.pack("<QQ", o, s))
    assert f.tell() == hdr
    for rel, p in files:
        with open(p, "rb") as g:
            while True:
                chunk = g.read(1 << 20)
                if not chunk:
                    break
                f.write(chunk)
print(f"{len(files)} files, {off/1e6:.1f} Mo -> {OUT}\\ngu.pak")
