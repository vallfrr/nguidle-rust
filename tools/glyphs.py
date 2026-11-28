"""Draws the controller glyphs of the console mode (assets/ui/glyphs/*.png) in the game's crude hand-drawn style:
a white body, a thick slightly wobbly black outline, flat colours. Run: python tools/glyphs.py <title font .ttf>.
The maintainer may redraw any of them; the file names and sizes are the exchange format (docs/CONSOLE-MODE.md)."""
import math, random, sys
from PIL import Image, ImageDraw, ImageFont

FONT = sys.argv[1] if len(sys.argv) > 1 else None
OUT = "assets/ui/glyphs/"
S = 4  # supersampling
INK = (0, 0, 0, 255)
BODY = (255, 255, 255, 255)
LIT = (255, 233, 138, 255)  # the pale yellow the game uses to mark a selection (blinking variant)


def wobble(rng, n, amount):
    return [1 + rng.uniform(-amount, amount) for _ in range(n)]


def blob(draw, cx, cy, rx, ry, body, ink_w, rng):
    pts_out, pts_in = [], []
    n = 72
    jitter = wobble(rng, n, 0.018)
    for i in range(n):
        a = 2 * math.pi * i / n
        pts_out.append((cx + math.cos(a) * rx * jitter[i], cy + math.sin(a) * ry * jitter[i]))
        pts_in.append((cx + math.cos(a) * (rx - ink_w) * jitter[i], cy + math.sin(a) * (ry - ink_w) * jitter[i]))
    draw.polygon(pts_out, fill=INK)
    draw.polygon(pts_in, fill=body)


def rounded(draw, box, radius, body, ink_w, rng):
    x0, y0, x1, y1 = box
    draw.rounded_rectangle(box, radius=radius, fill=INK)
    j = rng.uniform(-1.2, 1.2) * S
    draw.rounded_rectangle((x0 + ink_w, y0 + ink_w + j, x1 - ink_w, y1 - ink_w - j), radius=max(radius - ink_w, 2), fill=body)


def stroke(draw, pts, w):
    draw.line(pts, fill=INK, width=int(w), joint="curve")
    for x, y in (pts[0], pts[-1]):
        draw.ellipse((x - w / 2, y - w / 2, x + w / 2, y + w / 2), fill=INK)


def canvas(w, h):
    return Image.new("RGBA", (w * S, h * S), (0, 0, 0, 0))


def save(img, name, w, h):
    img.resize((w, h), Image.LANCZOS).save(OUT + name + ".png")


def face(name, body, symbol, seed):
    rng = random.Random(seed)
    w = h = 96
    im = canvas(w, h)
    d = ImageDraw.Draw(im)
    blob(d, w * S / 2, h * S / 2, 44 * S, 44 * S, body, 7 * S, rng)
    c = w * S / 2
    r = 19 * S
    t = 7 * S
    if symbol == "cross":
        stroke(d, [(c - r, c - r), (c + r, c + r)], t)
        stroke(d, [(c + r, c - r), (c - r, c + r)], t)
    elif symbol == "circle":
        d.ellipse((c - r, c - r, c + r, c + r), outline=INK, width=int(t))
    elif symbol == "square":
        stroke(d, [(c - r, c - r), (c + r, c - r), (c + r, c + r), (c - r, c + r), (c - r, c - r)], t)
    elif symbol == "triangle":
        stroke(d, [(c, c - r - 3 * S), (c + r + 2 * S, c + r - 2 * S), (c - r - 2 * S, c + r - 2 * S), (c, c - r - 3 * S)], t)
    save(im, name, w, h)


def label(name, text, w, h, seed, body=BODY):
    rng = random.Random(seed)
    im = canvas(w, h)
    d = ImageDraw.Draw(im)
    rounded(d, (2 * S, 2 * S, (w - 2) * S, (h - 2) * S), 14 * S, body, 5 * S, rng)
    if FONT:
        font = ImageFont.truetype(FONT, int(h * 0.55) * S)
        bbox = d.textbbox((0, 0), text, font=font)
        d.text(((w * S - (bbox[2] - bbox[0])) / 2 - bbox[0], (h * S - (bbox[3] - bbox[1])) / 2 - bbox[1]), text, font=font, fill=INK)
    save(im, name, w, h)


def dpad(name, seed):
    rng = random.Random(seed)
    w = h = 96
    im = canvas(w, h)
    d = ImageDraw.Draw(im)
    arm, half = 30 * S, 15 * S
    c = w * S / 2
    for box in ((c - arm - half + half, c - arm - half + half, c + arm, c + arm),):
        pass
    d.rounded_rectangle((c - half - 5 * S, 6 * S, c + half + 5 * S, (h - 6) * S), radius=7 * S, fill=INK)
    d.rounded_rectangle((6 * S, c - half - 5 * S, (w - 6) * S, c + half + 5 * S), radius=7 * S, fill=INK)
    d.rounded_rectangle((c - half, 11 * S, c + half, (h - 11) * S), radius=4 * S, fill=BODY)
    d.rounded_rectangle((11 * S, c - half, (w - 11) * S, c + half), radius=4 * S, fill=BODY)
    save(im, name, w, h)


def stick(name, seed, side):
    rng = random.Random(seed)
    w = h = 96
    im = canvas(w, h)
    d = ImageDraw.Draw(im)
    blob(d, w * S / 2, h * S / 2, 44 * S, 44 * S, BODY, 7 * S, rng)
    blob(d, w * S / 2, h * S / 2, 22 * S, 22 * S, (200, 200, 200, 255), 5 * S, rng)
    save(im, name, w, h)


def cursor(name):
    """The free cursor: a small black cross with a white edge."""
    w = h = 28
    im = canvas(w, h)
    d = ImageDraw.Draw(im)
    c, r = w * S / 2, 10 * S
    for color, width in (((255, 255, 255, 255), 7 * S), (INK, 3 * S)):
        d.line([(c - r, c), (c + r, c)], fill=color, width=int(width))
        d.line([(c, c - r), (c, c + r)], fill=color, width=int(width))
    save(im, name, w, h)


def pair(name, left, right, seed):
    """Two glyphs side by side (the tutorial prompt: go back and go on)."""
    a = Image.open(OUT + left + ".png")
    b = Image.open(OUT + right + ".png")
    im = Image.new("RGBA", (a.width + b.width + 8, max(a.height, b.height)), (0, 0, 0, 0))
    im.paste(a, (0, 0), a)
    im.paste(b, (a.width + 8, 0), b)
    im.save(OUT + name + ".png")


def prompt(name, lit, back, advance):
    """The 256 x 46 key prompt of the tutorial window, laid out like the game's own (replaces its arrow-key sprites): the
    button in the key's place (2..46 for back, 209..253 for advance) and the word beside it, caps 14 px high (rows 16..29)."""
    im = Image.new("RGBA", (256, 46), (0, 0, 0, 0))
    big = canvas(256, 46)
    d = ImageDraw.Draw(big)
    font = ImageFont.truetype(FONT, 18 * S)
    for on, glyph, word, key_x, word_at in ((back, "circle", "BACK", 5, ("left", 51)), (advance, "cross", "ADVANCE", 212, ("right", 205))):
        if not on:
            continue
        g = Image.open(OUT + glyph + ("-lit" if lit else "") + ".png").resize((38, 38), Image.LANCZOS)
        im.paste(g, (key_x, 4), g)
        bbox = d.textbbox((0, 0), word, font=font)
        x = word_at[1] * S - bbox[0] if word_at[0] == "left" else (word_at[1] + 1) * S - bbox[2]
        d.text((x, 16 * S - bbox[1]), word, font=font, fill=INK)
    im.alpha_composite(big.resize((256, 46), Image.LANCZOS))
    im.save(OUT + name + ".png")


if __name__ == "__main__":
    for i, (n, sym) in enumerate([("cross", "cross"), ("circle", "circle"), ("square", "square"), ("triangle", "triangle")]):
        face(n, BODY, sym, 10 + i)
        face(n + "-lit", LIT, sym, 10 + i)
    label("l", "L", 128, 64, 21)
    label("r", "R", 128, 64, 22)
    label("start", "START", 184, 64, 23)
    label("select", "SELECT", 192, 64, 24)
    dpad("dpad", 31)
    stick("stick", 32, "left")
    cursor("cursor")
    for lit in (False, True):
        suffix = "-lit" if lit else ""
        prompt("prompt-advance" + suffix, lit, False, True)
        prompt("prompt-back" + suffix, lit, True, False)
        prompt("prompt-both" + suffix, lit, True, True)
