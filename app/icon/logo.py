"""Draws Juglar's icon: a lowercase "j" leaving a trail of fading copies
behind it, like the picture's own feedback, flat and monochrome on a
near-black rounded tile. A sibling of bardo's icon
(github.com/luchoov/bardo): the same tile, ink and grays.

Writes `juglar.ico` (16 to 256 pixels, for the program, its window and
its tray icon) next to this file and `docs/media/logo.png` (512) for the
README. The small sizes are drawn on their own, with a thicker letter and
one copy instead of two, so they still read at 16 pixels.

    python app/icon/logo.py            (needs Pillow)
    python app/icon/logo.py preview    (a sheet of every size, to look at)
"""

import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

HERE = Path(__file__).parent
S = 1024
INK = (236, 236, 238, 255)
TILE = (14, 14, 16, 255)
EDGE = (44, 44, 50, 255)
# The copies, farthest first: each a step darker than the one after it.
TRAIL = [(60, 60, 66, 255), (110, 110, 116, 255)]

# The letter, on the 1024 tile: the dot, the stem with its top-left corner
# rounded like a flag, and the hook bending down and left under it.
STEM = (650, 360, 640)  # centre x, top, where the hook starts
HOOK = 250  # the hook's outer radius
FLAG = 70
DOT = (232, 70)  # centre y, radius
# Where the copies lie, farthest first: up and to the left of the letter.
COPIES = [(-150, -40), (-75, -20)]
COPIES_SMALL = [(-110, -30)]
# The dark gap that keeps the letter apart from its copies.
GAP = 41


def letter(small):
    """The mask of the letter with its dot."""
    mask = Image.new('L', (S, S), 0)
    d = ImageDraw.Draw(mask)
    width = 160 if small else 140
    cx, top, bend = STEM
    left, right = cx - width / 2, cx + width / 2
    d.rectangle((left, top, right, bend), fill=255)
    # The flag: the top-left corner rounded wide, the top-right nearly square.
    d.rectangle((left - 2, top - 2, left + FLAG, top + FLAG), fill=0)
    d.pieslice((left, top, left + 2 * FLAG, top + 2 * FLAG), 180, 270, fill=255)
    d.rounded_rectangle((left + FLAG - 1, top, right, top + FLAG + 4), radius=10, fill=255)
    # The hook: a quarter ring, from the stem's foot round to under its left.
    ox, oy = right - HOOK, bend
    d.pieslice((ox - HOOK, oy - HOOK, ox + HOOK, oy + HOOK), 0, 90, fill=255)
    inner = HOOK - width
    d.pieslice((ox - inner, oy - inner, ox + inner, oy + inner), 0, 90, fill=0)
    # Its end cut square, a little past the quarter.
    d.rectangle((ox - 30, oy + inner, ox + 1, oy + HOOK), fill=255)
    y, r = DOT
    d.ellipse((cx - r, y - r, cx + r, y + r), fill=255)
    return mask


def moved(mask, dx, dy):
    return mask.transform((S, S), Image.AFFINE, (1, 0, -dx, 0, 1, -dy))


def mark(small):
    """Masks for the letter and its copies (farthest first), centred together
    on the tile and scaled about its centre for the margin bardo's mark has."""
    front = letter(small)
    copies = [moved(front, dx, dy) for dx, dy in (COPIES_SMALL if small else COPIES)]
    union = front
    for copy in copies:
        union = ImageChops.lighter(union, copy)
    x0, y0, x1, y1 = union.getbbox()
    dx, dy = S / 2 - (x0 + x1) / 2, S / 2 - (y0 + y1) / 2
    scale = 0.94 if small else 0.84
    size = round(S * scale)

    def place(mask):
        out = Image.new('L', (S, S), 0)
        out.paste(moved(mask, dx, dy).resize((size, size), Image.LANCZOS), ((S - size) // 2, (S - size) // 2))
        return out

    return place(front), [place(copy) for copy in copies]


def render(small=False):
    img = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    inset = 24 if small else 40
    d.rounded_rectangle((inset, inset, S - inset, S - inset), radius=220, fill=TILE, outline=EDGE, width=8)
    front, copies = mark(small)
    for color, copy in zip(TRAIL[-len(copies):], copies):
        img.paste(color, mask=copy)
    img.paste(TILE, mask=front.filter(ImageFilter.MaxFilter(GAP)))
    img.paste(INK, mask=front)
    return img


def preview(path):
    big, small = render(), render(small=True)
    sheet = Image.new('RGBA', (600, 300), (0, 0, 0, 0))
    for row, bg in enumerate([(32, 32, 34, 255), (243, 243, 243, 255)]):
        ImageDraw.Draw(sheet).rectangle((0, row * 150, 600, row * 150 + 150), fill=bg)
        y = row * 150 + 10
        sheet.alpha_composite(big.resize((128, 128), Image.LANCZOS), (10, y))
        sheet.alpha_composite(big.resize((64, 64), Image.LANCZOS), (150, y))
        sheet.alpha_composite(big.resize((48, 48), Image.LANCZOS), (226, y))
        sheet.alpha_composite(small.resize((32, 32), Image.LANCZOS), (286, y))
        sheet.alpha_composite(small.resize((24, 24), Image.LANCZOS), (330, y))
        sheet.alpha_composite(small.resize((16, 16), Image.LANCZOS), (366, y))
    sheet.save(path)


if __name__ == '__main__':
    if sys.argv[1:] == ['preview']:
        preview(HERE / 'preview.png')
    else:
        big, small = render(), render(small=True)
        sizes = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
        frames = [(small if s <= 32 else big).resize((s, s), Image.LANCZOS) for s in sizes]
        frames[-1].save(HERE / 'juglar.ico', sizes=[(s, s) for s in sizes], append_images=frames[:-1])
        big.resize((512, 512), Image.LANCZOS).save(HERE.parent.parent / 'docs' / 'media' / 'logo.png', optimize=True)
    print('ok')
