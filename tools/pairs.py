"""pairs.py <orig_dir> <ours_dir> <out_prefix> : side-by-side sheets (4 menus each) of the original's captures NN.png against
our out/screens captures NN-Name.png, for the menu ids listed in IDS."""
import sys, glob, os
from PIL import Image, ImageDraw

IDS = ["00", "15", "16", "03", "04", "05", "18", "06", "14", "08", "09", "44", "39", "48", "52", "53", "55", "57"]
orig, ours_dir, prefix = sys.argv[1:4]
ours = {os.path.basename(f)[:2]: f for f in glob.glob(os.path.join(ours_dir, "[0-9][0-9]-*.png"))}
W, H = 480, 300
for s in range(0, len(IDS), 4):
    sh = Image.new("RGB", (W * 2, (H + 20) * 4), (20, 20, 20))
    d = ImageDraw.Draw(sh)
    for r, i in enumerate(IDS[s:s + 4]):
        y = r * (H + 20)
        a = Image.open(os.path.join(orig, f"{i}.png")).convert("RGB").resize((W, H))
        b = Image.open(ours[i]).convert("RGB").resize((W, H))
        sh.paste(a, (0, y + 20)); sh.paste(b, (W, y + 20))
        d.text((6, y + 4), f"ORIGINAL menu {i}", fill=(255, 255, 255)); d.text((W + 6, y + 4), "OURS " + os.path.basename(ours[i]), fill=(255, 255, 255))
    sh.save(f"{prefix}-{s // 4 + 1}.png")
