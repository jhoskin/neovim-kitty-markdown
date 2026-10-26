"""Writes the image fixtures for the hidden images checks.

Run once with Pillow installed: python3 make_fixtures.py
The output files are committed, so the checks do not need Pillow.
"""
import os
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))


def out(name):
    return os.path.join(HERE, name)


def solid(w, h, rgba):
    return Image.new("RGBA", (w, h), rgba)


# JPEG: no alpha. 400x200, mid grey-green.
solid(400, 200, (40, 160, 90, 255)).convert("RGB").save(out("photo.jpg"), quality=95)
# WebP 60x30, lossless so the pixel color is exact.
solid(60, 30, (20, 40, 220, 255)).save(out("pic.webp"), lossless=True)
# BMP 50x25.
solid(50, 25, (230, 200, 10, 255)).convert("RGB").save(out("pic.bmp"))
# Static GIF 30x15.
solid(30, 15, (255, 0, 0, 255)).convert("RGB").save(out("still.gif"))
# Animated GIF 40x20: frame 1 red, frame 2 blue, frame 3 green.
frames = [
    solid(40, 20, c).convert("RGB")
    for c in [(255, 0, 0, 255), (0, 0, 255, 255), (0, 255, 0, 255)]
]
frames[0].save(
    out("anim.gif"), save_all=True, append_images=frames[1:], duration=100, loop=0
)
# TIFF: a raster format that the spec does not list.
solid(20, 10, (10, 10, 10, 255)).save(out("pic.tiff"))
