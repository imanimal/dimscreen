"""Draws dimscreen's icons into icons/: the app icon (icon.ico, icon.png) and the tray icons
(tray-on.png, tray-off.png). A graphite tile with a half-lit disc; the off icon is an empty ring.

    python tools/make_icons.py
"""
from pathlib import Path

from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parent.parent / "icons"
TILE = (42, 39, 34, 255)        # Pocket chrome graphite
LIT = (246, 243, 236, 255)      # Cream surface
DARK = (110, 104, 94, 255)
S = 1024                        # draw big, scale down


def tile(draw):
    draw.rounded_rectangle((40, 40, S - 40, S - 40), radius=210, fill=TILE)


def icon_on():
    im = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    tile(d)
    box = (232, 232, S - 232, S - 232)
    d.ellipse(box, fill=DARK)
    d.pieslice(box, 90, 270, fill=LIT)          # left half lit: the screen, half dimmed
    d.ellipse(box, outline=LIT, width=56)
    return im


def icon_off():
    im = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    tile(d)
    d.ellipse((232, 232, S - 232, S - 232), outline=DARK, width=72)
    return im


def main():
    OUT.mkdir(exist_ok=True)
    on = icon_on()
    on.resize((512, 512), Image.LANCZOS).save(OUT / "icon.png")
    on.save(OUT / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    on.resize((32, 32), Image.LANCZOS).save(OUT / "tray-on.png")
    icon_off().resize((32, 32), Image.LANCZOS).save(OUT / "tray-off.png")


if __name__ == "__main__":
    main()
