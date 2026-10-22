// Renders the oracle goldens for every `<type>/<case>.mmd` under the
// directory argument (`just goldens` passes `helper/tests/goldens`).
//
// For each case: run the helper CLI at the parity width, read the PNG size,
// then run the pinned mermaid-cli with `--size` equal to the longer side so
// both images have the same pixel size (R-181). Mindmap and cynefin layouts
// are unseeded in mermaid 12.2.0, so those types get five goldens each
// (R-182). The helper's own PNG is deleted afterwards; the tests render it
// again.

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

function fail(message) {
  console.error(`goldens: ${message}`);
  process.exit(1);
}

const dirArg = process.argv[2];
if (!dirArg) {
  fail("usage: node helper/oracle/goldens.mjs <goldens dir>");
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

function renderOracle(mmd, outPng, size) {
  return run(mmdc, [
    "-i",
    mmd,
    "-o",
    outPng,
    "--size",
    String(size),
    "-b",
    "white",
    "-c",
    mermaidConfig,
    "-p",
    puppeteerConfig,
    "-q",
  ]);
}

let rendered = 0;
let skipped = 0;

const types = readdirSync(goldensDir)
  .filter((name) => statSync(join(goldensDir, name)).isDirectory())
  .sort();

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

    let size;
    try {
      const { width, height } = pngSize(helperPng);
      size = Math.max(width, height);
    } finally {
      unlinkSync(helperPng);
    }

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
    console.log(`${label}: --size ${size} -> ${sizes.join(", ")}`);
    rendered += outputs.length;
  }
}

console.log(`goldens: ${rendered} PNG(s) written, ${skipped} case(s) skipped`);
