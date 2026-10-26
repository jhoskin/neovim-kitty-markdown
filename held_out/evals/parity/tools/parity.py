"""Image tools for the hidden parity checks (numpy and Pillow).

Subcommands (all print one JSON object on stdout):

  crop <helper.png> <golden.png> <outdir>
      R-181: crops the golden to the helper output size from the top left
      corner. When the golden is up to 1 px smaller on an axis, the helper
      is cropped to the golden on that axis, also from the top left. No
      image is resampled (R-237). Reports "mismatch" when the helper is
      more than 1 px larger than the golden on an axis.

  ssim <a.png> <b.png>
      Independent SSIM per the SPEC.md definition: composite over white,
      BT.601 grayscale, 11x11 Gaussian window with sigma 1.5, K1 0.01,
      K2 0.03, L 255, only the window positions that fit inside the image
      (no padding), mean over those positions. When a side is shorter than
      11 px, the statistics of the whole images replace the window.

  pairs <outdir>
      Writes test image pairs for the SSIM definition checks.
"""

import json
import os
import sys

import numpy as np
from PIL import Image


def load_rgba(path):
    return Image.open(path).convert("RGBA")


def gray(path):
    a = np.asarray(load_rgba(path), dtype=np.float64)
    rgb = a[..., :3]
    alpha = a[..., 3:4] / 255.0
    rgb = rgb * alpha + 255.0 * (1.0 - alpha)
    return 0.299 * rgb[..., 0] + 0.587 * rgb[..., 1] + 0.114 * rgb[..., 2]


C1 = (0.01 * 255) ** 2
C2 = (0.03 * 255) ** 2


def gauss_1d():
    x = np.arange(11, dtype=np.float64) - 5.0
    g = np.exp(-(x * x) / (2.0 * 1.5 * 1.5))
    return g / g.sum()


def conv_valid(img, g):
    h, w = img.shape
    rows = np.zeros((h, w - 10))
    for k in range(11):
        rows += g[k] * img[:, k : k + w - 10]
    out = np.zeros((h - 10, w - 10))
    for k in range(11):
        out += g[k] * rows[k : k + h - 10, :]
    return out


def index(mx, my, sxx, syy, sxy):
    return ((2 * mx * my + C1) * (2 * sxy + C2)) / ((mx * mx + my * my + C1) * (sxx + syy + C2))


def ssim(x, y):
    if min(x.shape) < 11:
        mx, my = x.mean(), y.mean()
        sxx = ((x - mx) ** 2).mean()
        syy = ((y - my) ** 2).mean()
        sxy = ((x - mx) * (y - my)).mean()
        return float(index(mx, my, sxx, syy, sxy))
    g = gauss_1d()
    mx, my = conv_valid(x, g), conv_valid(y, g)
    sxx = conv_valid(x * x, g) - mx * mx
    syy = conv_valid(y * y, g) - my * my
    sxy = conv_valid(x * y, g) - mx * my
    return float(index(mx, my, sxx, syy, sxy).mean())


def cmd_ssim(a, b):
    x, y = gray(a), gray(b)
    if x.shape != y.shape:
        return {"error": "size differs", "a": list(x.shape), "b": list(y.shape)}
    return {"ssim": ssim(x, y)}


def cmd_crop(helper, golden, outdir):
    h = load_rgba(helper)
    g = load_rgba(golden)
    hw, hh = h.size
    gw, gh = g.size
    res = {"helper": [hw, hh], "golden": [gw, gh]}
    if hw - gw > 1 or hh - gh > 1:
        res["mismatch"] = True
        return res
    cw, ch = min(hw, gw), min(hh, gh)
    hp = os.path.join(outdir, "helper.png")
    gp = os.path.join(outdir, "golden.png")
    h.crop((0, 0, cw, ch)).save(hp)
    g.crop((0, 0, cw, ch)).save(gp)
    res.update({"size": [cw, ch], "helper_path": hp, "golden_path": gp})
    return res


def cmd_pairs(outdir):
    rng = np.random.default_rng(7)
    h, w = 120, 160
    yy, xx = np.mgrid[0:h, 0:w]
    base = np.zeros((h, w, 4), dtype=np.uint8)
    base[..., 0] = (xx * 255 // (w - 1)).astype(np.uint8)
    base[..., 1] = (yy * 255 // (h - 1)).astype(np.uint8)
    base[..., 2] = ((xx + yy) % 64 * 4).astype(np.uint8)
    base[..., 3] = 255
    base[30:70, 40:100, :3] = (20, 40, 200)
    base[80:100, 20:140, :3] = (250, 250, 250)
    shifted = np.roll(base, 3, axis=1)
    noise = base.astype(np.int16)
    noise[..., :3] += rng.integers(-40, 41, size=(h, w, 3)).astype(np.int16)
    noise = np.clip(noise, 0, 255).astype(np.uint8)
    alpha = base.copy()
    alpha[..., 3] = 128
    clear = np.zeros((h, w, 4), dtype=np.uint8)
    white = np.full((h, w, 4), 255, dtype=np.uint8)
    y = 0.299 * base[..., 0] + 0.587 * base[..., 1] + 0.114 * base[..., 2]
    grayed = base.copy()
    grayed[..., 0] = grayed[..., 1] = grayed[..., 2] = np.round(y).astype(np.uint8)
    swapped = base[..., [2, 1, 0, 3]].copy()
    # Inverted luma: the structure term is negative, so the SSIM is below 0.
    inverted = base.copy()
    inverted[..., :3] = 255 - base[..., :3]
    # Checkerboard and its inverse: equal means, opposite structure, so the
    # windowed SSIM is close to -1.
    checker = np.zeros((48, 48, 4), dtype=np.uint8)
    checker[..., 3] = 255
    yy2, xx2 = np.mgrid[0:48, 0:48]
    cells = ((yy2 // 2 + xx2 // 2) % 2).astype(bool)
    checker[cells, :3] = 255
    checker_inv = checker.copy()
    checker_inv[..., :3] = 255 - checker[..., :3]
    images = {
        "checker": checker,
        "checker_inv": checker_inv,
        "base": base,
        "shifted": shifted,
        "noise": noise,
        "alpha": alpha,
        "clear": clear,
        "white": white,
        "gray": grayed,
        "swapped": swapped,
        "inverted": inverted,
        # One side shorter than 11 px: whole-image statistics.
        "strip_a": base[50:58],
        "strip_b": noise[50:58],
        "strip_c": inverted[50:58],
        "narrow_a": base[:, 60:66],
        "narrow_b": shifted[:, 60:66],
        # Exactly 11 px high: one row of window positions.
        "row11_a": base[40:51],
        "row11_b": shifted[40:51],
    }
    names = {}
    for name, arr in images.items():
        p = os.path.join(outdir, f"{name}.png")
        Image.fromarray(np.ascontiguousarray(arr)).save(p)
        names[name] = p
    return names


def main(argv):
    cmd = argv[1]
    if cmd == "ssim":
        out = cmd_ssim(argv[2], argv[3])
    elif cmd == "crop":
        out = cmd_crop(argv[2], argv[3], argv[4])
    elif cmd == "pairs":
        out = cmd_pairs(argv[2])
    else:
        raise SystemExit(f"unknown command {cmd}")
    print(json.dumps(out))


if __name__ == "__main__":
    main(sys.argv)
