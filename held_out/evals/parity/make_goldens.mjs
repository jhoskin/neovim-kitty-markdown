// Renders the oracle goldens for the hidden parity corpus.
//
// Usage: node held_out/evals/parity/make_goldens.mjs [type]
//
// For every held_out/evals/parity/<type>/<case>.mmd:
//
// 1. Render the case with the helper at the parity target width of 960 px
//    (R-236) and read the PNG size W x H.
// 2. Render the oracle SVG once with `--size 960` and read its viewBox (the
//    natural size, for the record and the aspect) and whether the SVG has a
//    fixed size (no `width="100%"`).
// 3. Run the oracle PNG with the `--size` of R-237:
//    - gantt: W
//    - a fixed SVG size: the longer side of W x H
//    - W >= H: W + 8
//    - taller: the smallest n with round(n * aspect) == W + 8, where aspect
//      is the viewBox width divided by its height (the ratio mermaid-cli
//      uses, so that it draws the diagram W wide).
//    Mindmap and cynefin get five goldens (R-182).
//
// The script writes only under held_out/evals/parity/: the PNGs next to the
// cases and manifest.json. The checks crop each golden to the helper size
// from the top left (R-181). When the helper size no longer matches the
// manifest, the checks render fresh goldens with the same rule.

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

/** R-237: the oracle --size for a helper output of w x h. */
export function oracleSize(type, w, h, fixed, aspect) {
  if (type === 'gantt') return w;
  if (fixed) return Math.max(w, h);
  const target = w + 8;
  if (w >= h) return target;
  let n = Math.max(1, Math.ceil((target - 0.5) / aspect));
  while (Math.round(n * aspect) < target) n += 1;
  return n;
}

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

function svgShape(svgPath) {
  const svg = fs.readFileSync(svgPath, 'utf8');
  const tag = svg.match(/<svg\b[^>]*>/)?.[0] ?? '';
  const vb = tag.match(/\bviewBox="([^"]*)"/)?.[1]?.trim().split(/[\s,]+/).map(Number);
  if (!vb || vb.length !== 4) return null;
  return { width: vb[2], height: vb[3], fixed: !/\swidth="100%"/.test(tag) };
}

const manifestPath = path.join(here, 'manifest.json');
const manifest = fs.existsSync(manifestPath) ? JSON.parse(fs.readFileSync(manifestPath, 'utf8')) : {};
const t0 = Date.now();
let pngs = 0;
const problems = [];

const types = fs
  .readdirSync(here)
  .filter((d) => !d.startsWith('.') && d !== 'tools' && fs.statSync(path.join(here, d)).isDirectory())
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
    if (herr) entry.helper_error = herr;
    else entry.helper = pngSize(hpng);

    const svg = path.join(tmp, `${type}-${name}.svg`);
    const serr = oracleRun(mmd, svg, WIDTH);
    if (serr) {
      entry.oracle_error = serr;
      problems.push(`${key}: oracle failed (${serr})`);
      manifest[key] = entry;
      continue;
    }
    entry.natural = svgShape(svg);
    const nat = entry.natural;
    const aspect = nat.width / nat.height;
    // Without a helper output, use the size R-180 asks for.
    const ew = Math.min(nat.width, WIDTH);
    const base = entry.helper ?? { width: Math.ceil(ew), height: Math.ceil(ew / aspect) };
    entry.size_from = entry.helper ? 'helper' : 'natural';
    entry.size = oracleSize(type, base.width, base.height, nat.fixed, aspect);

    const runs = UNSEEDED.has(type) ? RUNS : 1;
    entry.goldens = [];
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
    manifest[key] = entry;
    const h = entry.helper ? `${entry.helper.width}x${entry.helper.height}` : 'helper failed';
    console.log(
      `${key}: helper ${h}, natural ${nat.width.toFixed(1)}x${nat.height.toFixed(1)}${nat.fixed ? ' fixed' : ''}, --size ${entry.size} -> ${entry.goldens
        .map((g) => `${g.width}x${g.height}`)
        .join(', ')}`
    );
  }
}

fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 1) + '\n');
fs.rmSync(tmp, { recursive: true, force: true });
for (const p of problems) console.log(`problem: ${p}`);
console.log(`goldens: ${pngs} PNG(s) in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
