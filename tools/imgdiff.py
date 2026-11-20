"""imgdiff.py <reference.png> <ours.png> <output.png> [threshold=25]: differing pixels in red over our darkened capture; prints their count.
Example: python tools/imgdiff.py out/compare/orig/02-game.png out/compare/ours-game.png out/compare/diff-game.png"""
import sys
from PIL import Image, ImageChops

a, b, out = sys.argv[1:4]
threshold = int(sys.argv[4]) if len(sys.argv) > 4 else 25
o, n = Image.open(a).convert("RGB"), Image.open(b).convert("RGB")
m = ImageChops.difference(o, n).convert("L").point(lambda v: 255 if v > threshold else 0)
base = Image.blend(n.convert("L").convert("RGB"), Image.new("RGB", n.size), 0.6)
base.paste(Image.new("RGB", n.size, (255, 0, 0)), (0, 0), m)
base.save(out)
print(f"{sum(1 for v in m.getdata() if v)} differing pixels out of {n.size[0] * n.size[1]} -> {out}")
