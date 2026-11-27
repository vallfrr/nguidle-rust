"""Draws the controller glyphs of the console mode (clr/assets/glyphs/*.png) in the game's crude hand-drawn style:
a white body, a thick slightly wobbly black outline, flat colours. Run: python tools/glyphs.py <title font .ttf>.
The maintainer may redraw any of them; the file names and sizes are the exchange format (docs/CONSOLE-MODE.md)."""
import math, random, sys
from PIL import Image, ImageDraw, ImageFont

FONT = sys.argv[1] if len(sys.argv) > 1 else None
OUT = "clr/assets/glyphs/"
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


def pair(name, left, right, seed):
    """Two glyphs side by side (the tutorial prompt: go back and go on)."""
    a = Image.open(OUT + left + ".png")
    b = Image.open(OUT + right + ".png")
    im = Image.new("RGBA", (a.width + b.width + 8, max(a.height, b.height)), (0, 0, 0, 0))
    im.paste(a, (0, 0), a)
    im.paste(b, (a.width + 8, 0), b)
    im.save(OUT + name + ".png")


def prompt(name, glyph, word, lit):
    """The 256 x 46 key prompt of the tutorial window: the word, then the button that does it (replaces the game's arrow sprites)."""
    im = Image.new("RGBA", (256, 46), (0, 0, 0, 0))
    g = Image.open(OUT + glyph + ("-lit" if lit else "") + ".png").resize((44, 44), Image.LANCZOS)
    im.paste(g, (256 - 46, 1), g)
    big = canvas(256, 46)
    d = ImageDraw.Draw(big)
    if FONT:
        font = ImageFont.truetype(FONT, 26 * S)
        w = d.textlength(word, font=font)
        d.text(((256 - 56) * S - w, 4 * S), word, font=font, fill=INK)
    text = big.resize((256, 46), Image.LANCZOS)
    im.alpha_composite(text)
    im.save(OUT + name + ".png")


def prompt_both(name, lit):
    im = Image.new("RGBA", (256, 46), (0, 0, 0, 0))
    big = canvas(256, 46)
    d = ImageDraw.Draw(big)
    if FONT:
        font = ImageFont.truetype(FONT, 15 * S)
        d.text((50 * S, 13 * S), "BACK", font=font, fill=INK)
        d.text((132 * S, 13 * S), "ADVANCE", font=font, fill=INK)
    text = big.resize((256, 46), Image.LANCZOS)
    for glyph, x in (("circle", 2), ("cross", 256 - 46)):
        g = Image.open(OUT + glyph + ("-lit" if lit else "") + ".png").resize((44, 44), Image.LANCZOS)
        im.paste(g, (x, 1), g)
    im.alpha_composite(text)
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
    for lit in (False, True):
        suffix = "-lit" if lit else ""
        prompt("prompt-advance" + suffix, "cross", "ADVANCE", lit)
        prompt("prompt-back" + suffix, "circle", "BACK", lit)
        prompt_both("prompt-both" + suffix, lit)
