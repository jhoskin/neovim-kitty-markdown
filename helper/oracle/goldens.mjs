// Renders the oracle goldens for every `<type>/<case>.mmd` under the
// directory argument (`just goldens` passes `helper/tests/goldens`).
//
// For each case: run the helper CLI at the parity width, read the PNG size,
// render the oracle SVG once to learn the viewBox aspect ratio and whether
// the SVG is `width="100%"` (`useMaxWidth`), then run the pinned
// mermaid-cli with the `--size` that puts the diagram at the helper's
// scale (R-181). Mindmap and cynefin layouts are unseeded in mermaid
// 12.2.0, so those types get five goldens each (R-182). The helper's own PNG
// and the SVG are deleted afterwards; the tests render the helper side
// again.
//
// Why `--size` is not simply the helper's longer side: mermaid-cli 12.0.0
// (`src/index.js`) draws a `useMaxWidth` diagram 8 px narrower than the PNG
// it writes. It opens a `size + 16` viewport for the 8 px body margin
// (lines 503-508), sets the SVG's `max-width` and `max-height` to `size`
// (747-752), reads the SVG's client rect and rounds it into the screenshot
// clip (827-846), then calls `page.setViewport` with `clip.x + clip.width`
// (848-852). That viewport is 8 px too narrow for margin plus SVG, so a
// `width="100%"` SVG reflows to `clip.width - 8` before the screenshot
// (855-858) while the clip keeps its first size. A tall diagram (viewBox
// aspect below 1) first gets `max-width` reduced to `round(size * aspect)`
// (834-836) and then reflows the same way. A fixed-size SVG
// (`useMaxWidth: false`) keeps its `width` attribute and does not reflow.
//
// So the oracle's drawn width is `clip.width - 8` for a `useMaxWidth`
// diagram and the helper's width otherwise. This script picks `size` so
// that `clip.width` is the helper's width plus 8: `size = width + 8` for a
// landscape diagram, and for a tall one the smallest integer `n` with
// `round(n * aspect) == width + 8`. The golden is then 8 px wider and
// taller than the helper output, with page background on the right and
// bottom, and the diagram at the helper's scale in the top-left corner.
// `tests/common/mod.rs` crops a larger golden to the helper's size there
// before comparing. The script checks the oracle PNG's size after every run.

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync, unlinkSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");
const helperBin = join(repoRoot, "helper", "target", "release", "kitty-markdown-helper");
const mmdc = join(here, "node_modules", ".bin", "mmdc");
const mermaidConfig = join(here, "mermaid-config.json");
const puppeteerConfig = join(here, "puppeteer.json");

/** The width the parity tests render at, in pixels. */
const PARITY_WIDTH = 960;
/** Diagram types whose layout is unseeded; they get five goldens (R-182). */
const UNSEEDED = new Set(["mindmap", "cynefin"]);
const UNSEEDED_RUNS = 5;
/**
 * Diagram types whose layout reads the available width (gantt lays its time
 * scale over the body width). They must be laid out at the helper's width,
 * so the oracle runs with `--size` equal to it and the screenshot then
 * reflows the drawing to `width - 8`; the parity test scales its render the
 * same way and crops the golden.
 */
const WIDTH_DEPENDENT = new Set(["gantt"]);
/**
 * Folders whose `.mmd` cases are checked against oracle SVG geometry, not
 * against PNG goldens. Rendering PNGs there would only leave stray files.
 */
const SVG_ORACLE_DIRS = new Set(["unified"]);

function fail(message) {
  console.error(`goldens: ${message}`);
  process.exit(1);
}

const dirArg = process.argv[2];
/** Optional: regenerate one diagram type only. */
const onlyType = process.argv[3] ?? null;
if (!dirArg) {
  fail("usage: node helper/oracle/goldens.mjs <goldens dir> [type]");
}
const goldensDir = resolve(dirArg);
if (!existsSync(goldensDir)) {
  fail(`${goldensDir} does not exist`);
}
if (!existsSync(helperBin)) {
  fail(`${helperBin} is missing; run \`just build\` first`);
}
if (!existsSync(mmdc)) {
  fail(`${mmdc} is missing; run \`npm ci --prefix helper/oracle\``);
}

/** Reads width and height from the PNG IHDR chunk. */
function pngSize(path) {
  const buf = readFileSync(path);
  const signature = "\x89PNG\r\n\x1a\n";
  if (buf.length < 24 || buf.toString("latin1", 0, 8) !== signature) {
    throw new Error(`${path} is not a PNG`);
  }
  return { width: buf.readUInt32BE(16), height: buf.readUInt32BE(20) };
}

function run(cmd, args) {
  const result = spawnSync(cmd, args, { encoding: "utf8" });
  if (result.error) {
    return { ok: false, detail: result.error.message };
  }
  if (result.status !== 0) {
    const detail = (result.stderr || result.stdout || "").trim().split("\n").at(-1) ?? "";
    return { ok: false, detail: `exit ${result.status}: ${detail}` };
  }
  return { ok: true, detail: "" };
}

function renderOracle(mmd, out, size) {
  const sizeArgs = size === null ? [] : ["--size", String(size)];
  return run(mmdc, [
    "-i",
    mmd,
    "-o",
    out,
    ...sizeArgs,
    "-b",
    "white",
    "-c",
    mermaidConfig,
    "-p",
    puppeteerConfig,
    "-q",
  ]);
}

/** The viewBox aspect ratio and the `useMaxWidth` flag of the oracle SVG. */
function svgShape(path) {
  const svg = readFileSync(path, "utf8");
  const root = svg.match(/<svg\b[^>]*>/)?.[0] ?? "";
  const viewBox = root.match(/\bviewBox="([^"]*)"/)?.[1]?.trim().split(/[\s,]+/).map(Number);
  if (!viewBox || viewBox.length !== 4 || !(viewBox[2] > 0) || !(viewBox[3] > 0)) {
    throw new Error(`${path}: no usable viewBox`);
  }
  const width = root.match(/\swidth="([^"]*)"/)?.[1] ?? "";
  return { aspect: viewBox[2] / viewBox[3], useMaxWidth: width.trim() === "100%" };
}

/**
 * The `--size` that makes mermaid-cli's screenshot clip `width + 8` wide for
 * a `useMaxWidth` diagram (see the top), or the longer side for a fixed-size
 * one. Returns the size and the width the oracle PNG must have.
 */
function oracleSize(helperSize, shape, type) {
  const { width, height } = helperSize;
  if (WIDTH_DEPENDENT.has(type)) {
    return { size: width, expectWidth: width };
  }
  if (!shape.useMaxWidth) {
    // mermaid-cli still sets max-width: round(size * aspect), which can be
    // 1 px under the natural width; R-181 allows 1 px.
    return { size: Math.max(width, height), expectWidth: width, widthSlack: 1 };
  }
  const target = width + 8;
  if (shape.aspect >= 1) {
    return { size: target, expectWidth: target };
  }
  // `round(size * aspect)` steps by less than 1 per unit of `size`, so an
  // integer that rounds to `target` exists; start at the first candidate.
  let size = Math.ceil((target - 0.5) / shape.aspect);
  while (Math.round(size * shape.aspect) < target) {
    size += 1;
  }
  if (Math.round(size * shape.aspect) !== target) {
    throw new Error(`no --size rounds to a ${target} px wide SVG at aspect ${shape.aspect}`);
  }
  return { size, expectWidth: target };
}

let rendered = 0;
let skipped = 0;

const types = readdirSync(goldensDir)
  .filter((name) => statSync(join(goldensDir, name)).isDirectory())
  .filter((name) => !SVG_ORACLE_DIRS.has(name))
  .sort()
  .filter((type) => onlyType === null || type === onlyType);

for (const type of types) {
  const typeDir = join(goldensDir, type);
  const cases = readdirSync(typeDir)
    .filter((name) => name.endsWith(".mmd"))
    .sort();
  for (const file of cases) {
    const caseName = file.slice(0, -".mmd".length);
    const mmd = join(typeDir, file);
    const helperPng = join(typeDir, `${caseName}.helper.png`);
    const label = `${type}/${caseName}`;

    const helper = run(helperBin, [
      "render",
      "--mermaid",
      mmd,
      "--width",
      String(PARITY_WIDTH),
      "-o",
      helperPng,
    ]);
    if (!helper.ok) {
      console.log(`${label}: skipped, helper failed (${helper.detail})`);
      skipped += 1;
      continue;
    }

    let helperSize;
    try {
      helperSize = pngSize(helperPng);
    } finally {
      unlinkSync(helperPng);
    }

    const svgPath = join(typeDir, `${caseName}.oracle.svg`);
    let shape;
    try {
      const probe = renderOracle(mmd, svgPath, null);
      if (!probe.ok) {
        console.log(`${label}: oracle SVG failed (${probe.detail})`);
        skipped += 1;
        continue;
      }
      shape = svgShape(svgPath);
    } finally {
      if (existsSync(svgPath)) {
        unlinkSync(svgPath);
      }
    }
    const { size, expectWidth, widthSlack = 0 } = oracleSize(helperSize, shape, type);

    const runs = UNSEEDED.has(type) ? UNSEEDED_RUNS : 1;
    const outputs = [];
    let failed = null;
    for (let i = 1; i <= runs; i += 1) {
      const outPng = join(typeDir, runs === 1 ? `${caseName}.png` : `${caseName}.${i}.png`);
      const oracle = renderOracle(mmd, outPng, size);
      if (!oracle.ok) {
        failed = oracle.detail;
        break;
      }
      const got = pngSize(outPng);
      if (Math.abs(got.width - expectWidth) > widthSlack || got.height < helperSize.height) {
        failed = `oracle PNG is ${got.width}x${got.height}, expected ${expectWidth} wide and at least ${helperSize.height} tall for a ${helperSize.width}x${helperSize.height} helper output`;
        unlinkSync(outPng);
        break;
      }
      outputs.push(outPng);
    }
    if (failed) {
      console.log(`${label}: oracle failed at --size ${size} (${failed})`);
      skipped += 1;
      continue;
    }
    const sizes = outputs.map((p) => {
      const { width, height } = pngSize(p);
      return `${width}x${height}`;
    });
    console.log(
      `${label}: helper ${helperSize.width}x${helperSize.height}, --size ${size} -> ${sizes.join(", ")}`,
    );
    rendered += outputs.length;
  }
}

console.log(`goldens: ${rendered} PNG(s) written, ${skipped} case(s) skipped`);
