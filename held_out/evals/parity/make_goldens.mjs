// Renders the oracle goldens for the hidden parity corpus.
//
// Usage: node held_out/evals/parity/make_goldens.mjs [type]
//
// For every held_out/evals/parity/<type>/<case>.mmd:
//
// 1. Render the case with the helper at the parity target width of 960 px
//    (R-236) and read the PNG size W x H.
// 2. Render the oracle SVG once with `--size 960` and read its viewBox: the
//    natural size (SPEC.md definitions). The expected helper size is the
//    natural size when it is narrower than 960 (R-180), else the natural
//    size scaled to 960 wide.
// 3. Run the oracle PNG with `--size` equal to the longer side of the
//    helper output (R-181). Mindmap and cynefin get five goldens (R-182).
// 4. For the seeded types, render <case>.ideal.png: mermaid's own drawing at
//    the helper's scale (mermaid-cli draws a `width="100%"` SVG 8 px
//    narrower than its clip, so the --size is chosen to make the drawn width
//    equal the helper width, as helper/oracle/goldens.mjs does). The checks
//    frame it like a helper output to learn the best SSIM that the R-237
//    framing allows for that case (resampling text costs SSIM). Gantt is
//    skipped: its layout follows the --size.
//
// The script writes only under held_out/evals/parity/: the PNGs next to the
// cases and manifest.json with the sizes. The checks render the helper side
// again; when the helper's longer side no longer matches the manifest, the
// checks render a fresh golden at the new size.

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';

const here = path.dirname(url.fileURLToPath(import.meta.url));
const root = path.resolve(here, '..', '..', '..');
const helperBin =
  process.env.QA_HELPER_BIN ?? path.join(root, 'helper', 'target', 'release', 'kitty-markdown-helper');
const oracle = path.join(root, 'helper', 'oracle');
const mmdc = path.join(oracle, 'node_modules', '.bin', 'mmdc');
const config = path.join(oracle, 'mermaid-config.json');
const pconfig = path.join(oracle, 'puppeteer.json');

const WIDTH = 960;
const UNSEEDED = new Set(['mindmap', 'cynefin']);
const RUNS = 5;
// Gantt lays its time axis over the available width, so a render at another
// --size is another layout and cannot serve as the ideal.
const WIDTH_DEPENDENT = new Set(['gantt']);

const only = process.argv[2] ?? null;
const tmp = path.join(here, '.tmp');
fs.rmSync(tmp, { recursive: true, force: true });
fs.mkdirSync(tmp, { recursive: true });

function run(cmd, args) {
  const r = spawnSync(cmd, args, { encoding: 'utf8' });
  if (r.error) return r.error.message;
  if (r.status !== 0) return `exit ${r.status}: ${(r.stderr || r.stdout || '').trim().split('\n').slice(-3).join(' | ')}`;
  return null;
}

function pngSize(p) {
  const b = fs.readFileSync(p);
  return { width: b.readUInt32BE(16), height: b.readUInt32BE(20) };
}

function oracleRun(mmd, out, size) {
  return run(mmdc, ['-i', mmd, '-o', out, '--size', String(size), '-b', 'white', '-c', config, '-p', pconfig, '-q']);
}

function viewBox(svgPath) {
  const svg = fs.readFileSync(svgPath, 'utf8');
  const tag = svg.match(/<svg\b[^>]*>/)?.[0] ?? '';
  const vb = tag.match(/\bviewBox="([^"]*)"/)?.[1]?.trim().split(/[\s,]+/).map(Number);
  if (!vb || vb.length !== 4) return null;
  return { width: vb[2], height: vb[3], useMaxWidth: /\swidth="100%"/.test(tag) };
}

// The --size whose screenshot clip is width + 8 wide, so the drawing is
// `width` wide (see helper/oracle/goldens.mjs for the mermaid-cli lines).
function idealSize(width, aspect) {
  const target = width + 8;
  if (aspect >= 1) return target;
  let n = Math.ceil((target - 0.5) / aspect);
  while (Math.round(n * aspect) < target) n += 1;
  return n;
}

const manifest = fs.existsSync(path.join(here, 'manifest.json'))
  ? JSON.parse(fs.readFileSync(path.join(here, 'manifest.json'), 'utf8'))
  : {};
const t0 = Date.now();
let pngs = 0;
const problems = [];

const types = fs
  .readdirSync(here)
  .filter((d) => !d.startsWith('.') && fs.statSync(path.join(here, d)).isDirectory())
  .filter((d) => only === null || d === only)
  .sort();

for (const type of types) {
  const dir = path.join(here, type);
  for (const f of fs.readdirSync(dir).filter((n) => n.endsWith('.mmd')).sort()) {
    const name = f.slice(0, -4);
    const key = `${type}/${name}`;
    const mmd = path.join(dir, f);
    for (const old of fs.readdirSync(dir)) {
      if (old.startsWith(`${name}.`) && old.endsWith('.png')) fs.rmSync(path.join(dir, old));
    }
    const entry = { type, case: name };

    const hpng = path.join(tmp, `${type}-${name}.helper.png`);
    const herr = run(helperBin, ['render', '--mermaid', mmd, '--width', String(WIDTH), '--theme', 'default', '-o', hpng]);
    if (herr) {
      entry.helper_error = herr;
    } else {
      entry.helper = pngSize(hpng);
    }

    const runs = UNSEEDED.has(type) ? RUNS : 1;
    entry.natural = [];
    entry.goldens = [];
    for (let i = 1; i <= runs; i++) {
      const svg = path.join(tmp, `${type}-${name}.${i}.svg`);
      const serr = oracleRun(mmd, svg, WIDTH);
      if (serr) {
        entry.oracle_error = serr;
        break;
      }
      entry.natural.push(viewBox(svg));
    }
    if (entry.oracle_error) {
      problems.push(`${key}: oracle failed (${entry.oracle_error})`);
      manifest[key] = entry;
      continue;
    }
    const nat = entry.natural[0];
    const ew = Math.min(nat.width, WIDTH);
    entry.expected = { width: ew, height: (nat.height * ew) / nat.width };

    // R-181: --size is the longer side of the helper output. Without a
    // helper output, fall back to the expected size from the oracle.
    const base = entry.helper ?? { width: Math.ceil(entry.expected.width), height: Math.ceil(entry.expected.height) };
    entry.size = Math.max(base.width, base.height);
    entry.size_from = entry.helper ? 'helper' : 'oracle';
    for (let i = 1; i <= runs; i++) {
      const out = path.join(dir, runs === 1 ? `${name}.png` : `${name}.${i}.png`);
      const err = oracleRun(mmd, out, entry.size);
      if (err) {
        problems.push(`${key}: oracle PNG failed (${err})`);
        break;
      }
      entry.goldens.push({ file: path.basename(out), ...pngSize(out) });
      pngs += 1;
    }
    if (!UNSEEDED.has(type) && !WIDTH_DEPENDENT.has(type) && entry.helper && nat.useMaxWidth) {
      const isize = idealSize(entry.helper.width, nat.width / nat.height);
      const out = path.join(dir, `${name}.ideal.png`);
      const err = oracleRun(mmd, out, isize);
      if (err) {
        problems.push(`${key}: ideal PNG failed (${err})`);
      } else {
        entry.ideal = { file: path.basename(out), size: isize, ...pngSize(out) };
      }
    }
    manifest[key] = entry;
    const h = entry.helper ? `${entry.helper.width}x${entry.helper.height}` : 'helper failed';
    console.log(
      `${key}: helper ${h}, natural ${nat.width.toFixed(1)}x${nat.height.toFixed(1)}, --size ${entry.size} -> ${entry.goldens
        .map((g) => `${g.width}x${g.height}`)
        .join(', ')}`
    );
  }
}

fs.writeFileSync(path.join(here, 'manifest.json'), JSON.stringify(manifest, null, 1) + '\n');
fs.rmSync(tmp, { recursive: true, force: true });
for (const p of problems) console.log(`problem: ${p}`);
console.log(`goldens: ${pngs} PNG(s) in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
