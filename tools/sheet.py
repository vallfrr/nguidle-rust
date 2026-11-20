"""sheet.py <folder>: assembles the NN-*.png captures of a folder into sheets of 12 (sheet-1.png, ...), with each menu's name."""
import sys, glob, os
from PIL import Image, ImageDraw

d = sys.argv[1]
files = sorted(f for f in glob.glob(os.path.join(d, "[0-9][0-9]-*.png")))
W, H, BAND, COLS, ROWS = 640, 400, 26, 3, 4
for s in range(0, len(files), COLS * ROWS):
    sheet = Image.new("RGB", (COLS * W, ROWS * (H + BAND)), (20, 20, 20))
    dr = ImageDraw.Draw(sheet)
    for k, f in enumerate(files[s:s + COLS * ROWS]):
        x, y = (k % COLS) * W, (k // COLS) * (H + BAND)
        sheet.paste(Image.open(f).convert("RGB").resize((W, H), Image.LANCZOS), (x, y + BAND))
        dr.text((x + 8, y + 6), os.path.basename(f)[:-4].replace("_", " "), fill=(255, 255, 255))
    sheet.save(os.path.join(d, f"sheet-{s // (COLS * ROWS) + 1}.png"))
    print(os.path.join(d, f"sheet-{s // (COLS * ROWS) + 1}.png"))
