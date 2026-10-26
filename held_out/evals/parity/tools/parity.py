"""Image tools for the hidden parity checks (numpy and Pillow).

Subcommands (all print one JSON object on stdout):

  frame <helper.png> <oracle.png> <outdir>
      Frames the helper output as mermaid-cli 12.0.0 frames its screenshot
      (R-237): composite over white, scale, and pad with white to the
      helper's own size (its longer side is the oracle --size). Writes one
      framed helper PNG per framing reading and the oracle composited over
      white, both cropped to the common size when the sizes differ by at
      most 1 px (R-181).

      Readings: "spec" scales the longer side to size - 8. "cli" scales the
      width to width - 8, which is what mermaid-cli's viewport reflow does
      for a tall diagram. For a landscape diagram the two are the same.

  ssim <a.png> <b.png>
      Independent SSIM per the SPEC.md definition: composite over white,
      BT.601 grayscale (0..255), 11x11 Gaussian window with sigma 1.5,
      K1 0.01, K2 0.03, L 255, mean over the image. Reports two border
      readings: "valid" (mean over the windows that fit in the image) and
      "same" (window centered on every pixel, zero weight outside, weights
      renormalised).

  fit <in.png> <w> <h> <out.png>
      Composites over white and crops or pads with white to w x h,
      anchored top left.

  pairs <outdir>
      Writes test image pairs for the SSIM definition check.
"""

import json
import os
import sys

import numpy as np
from PIL import Image


def load_rgba(path):
    return Image.open(path).convert("RGBA")


def over_white(img):
    bg = Image.new("RGBA", img.size, (255, 255, 255, 255))
    return Image.alpha_composite(bg, img).convert("RGB")


def gray(path):
    a = np.asarray(load_rgba(path), dtype=np.float64)
    rgb = a[..., :3]
    alpha = a[..., 3:4] / 255.0
    rgb = rgb * alpha + 255.0 * (1.0 - alpha)
    return 0.299 * rgb[..., 0] + 0.587 * rgb[..., 1] + 0.114 * rgb[..., 2]


def gauss_1d():
    x = np.arange(11, dtype=np.float64) - 5.0
    g = np.exp(-(x * x) / (2.0 * 1.5 * 1.5))
    return g / g.sum()


def conv_valid(img, g):
    # Separable 2-D correlation, valid region only.
    h, w = img.shape
    rows = np.zeros((h, w - 10))
    for k in range(11):
        rows += g[k] * img[:, k : k + w - 10]
    out = np.zeros((h - 10, w - 10))
    for k in range(11):
        out += g[k] * rows[k : k + h - 10, :]
    return out


def conv_same(img, g):
    # Zero padding, then renormalise by the in-image window weight.
    p = np.pad(img, 5)
    ones = np.pad(np.ones_like(img), 5)
    return conv_valid(p, g) / conv_valid(ones, g)


def ssim(x, y, conv):
    g = gauss_1d()
    c1 = (0.01 * 255) ** 2
    c2 = (0.03 * 255) ** 2
    mx, my = conv(x, g), conv(y, g)
    sxx = conv(x * x, g) - mx * mx
    syy = conv(y * y, g) - my * my
    sxy = conv(x * y, g) - mx * my
    m = ((2 * mx * my + c1) * (2 * sxy + c2)) / ((mx * mx + my * my + c1) * (sxx + syy + c2))
    return float(m.mean())


def cmd_ssim(a, b):
    x, y = gray(a), gray(b)
    if x.shape != y.shape:
        return {"error": "size differs", "a": list(x.shape), "b": list(y.shape)}
    out = {"same": ssim(x, y, conv_same)}
    if min(x.shape) >= 11:
        out["valid"] = ssim(x, y, conv_valid)
    return out


def cmd_frame(helper, oracle, outdir):
    h = over_white(load_rgba(helper))
    o = over_white(load_rgba(oracle))
    w, hh = h.size
    size = max(w, hh)
    ow, oh = o.size
    res = {"helper": [w, hh], "oracle": [ow, oh], "size": size, "framed": {}}
    if abs(ow - w) > 1 or abs(oh - hh) > 1:
        res["size_mismatch"] = True
        return res
    cw, ch = min(w, ow), min(hh, oh)
    oc = o.crop((0, 0, cw, ch))
    opath = os.path.join(outdir, "oracle.png")
    oc.save(opath)
    res["oracle_path"] = opath
    scales = {"spec": (size - 8) / size, "cli": (w - 8) / w}
    done = {}
    for name, s in scales.items():
        nw, nh = max(1, round(w * s)), max(1, round(hh * s))
        if (nw, nh) in done:
            res["framed"][name] = done[(nw, nh)]
            continue
        scaled = h.resize((nw, nh), Image.LANCZOS)
        canvas = Image.new("RGB", (w, hh), (255, 255, 255))
        canvas.paste(scaled, (0, 0))
        canvas = canvas.crop((0, 0, cw, ch))
        p = os.path.join(outdir, f"helper-{name}.png")
        canvas.save(p)
        entry = {"path": p, "scaled": [nw, nh]}
        done[(nw, nh)] = entry
        res["framed"][name] = entry
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
    # Half transparent copy: composited over white it differs from base.
    alpha = base.copy()
    alpha[..., 3] = 128
    # Fully transparent black: over white it is pure white.
    clear = np.zeros((h, w, 4), dtype=np.uint8)
    white = np.full((h, w, 4), 255, dtype=np.uint8)
    # Same picture in BT.601 gray: SSIM depends on the luma formula.
    y = 0.299 * base[..., 0] + 0.587 * base[..., 1] + 0.114 * base[..., 2]
    grayed = base.copy()
    grayed[..., 0] = grayed[..., 1] = grayed[..., 2] = np.round(y).astype(np.uint8)
    # Channels swapped: equal under a channel mean, not under BT.601.
    swapped = base[..., [2, 1, 0, 3]].copy()
    names = {}
    for name, arr in {
        "base": base,
        "shifted": shifted,
        "noise": noise,
        "alpha": alpha,
        "clear": clear,
        "white": white,
        "gray": grayed,
        "swapped": swapped,
    }.items():
        p = os.path.join(outdir, f"{name}.png")
        Image.fromarray(arr).save(p)
        names[name] = p
    return names


def cmd_fit(src, w, h, out):
    img = over_white(load_rgba(src))
    canvas = Image.new("RGB", (w, h), (255, 255, 255))
    canvas.paste(img.crop((0, 0, min(w, img.size[0]), min(h, img.size[1]))), (0, 0))
    canvas.save(out)
    return {"path": out, "from": list(img.size)}


def main(argv):
    cmd = argv[1]
    if cmd == "ssim":
        out = cmd_ssim(argv[2], argv[3])
    elif cmd == "frame":
        out = cmd_frame(argv[2], argv[3], argv[4])
    elif cmd == "fit":
        out = cmd_fit(argv[2], int(argv[3]), int(argv[4]), argv[5])
    elif cmd == "pairs":
        out = cmd_pairs(argv[2])
    else:
        raise SystemExit(f"unknown command {cmd}")
    print(json.dumps(out))


if __name__ == "__main__":
    main(sys.argv)
