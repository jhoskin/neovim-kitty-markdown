// Runs the measurement oracle method (helper/oracle/measure.mjs: Chrome
// through puppeteer, getBBox for an SVG label and getBoundingClientRect for
// an HTML label, on the page mermaid-cli 12.0.0 opens) on the hidden label
// corpus, and writes held_out/evals/parity/measure_fixture.json.
//
// Usage: node held_out/evals/parity/measure_corpus.mjs
//
// measure.mjs has a fixed corpus and writes into helper/, so this script
// repeats its two measurements with the same page, stylesheets, and DOM
// shape for the strings below.

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import { createRequire } from 'node:module';

const here = path.dirname(url.fileURLToPath(import.meta.url));
const oracleDir = path.resolve(here, '..', '..', '..', 'helper', 'oracle');
const require = createRequire(path.join(oracleDir, 'package.json'));
const { default: puppeteer } = await import(url.pathToFileURL(require.resolve('puppeteer')).href);

// The class field is informational: the checks treat a label with any
// character outside ASCII as fallback text (3 px), others as Latin (0.5 px).
const STRINGS = [
  ['latin', 'Order received'],
  ['latin', 'Is the cart valid?'],
  ['latin', 'Charge card & reserve stock'],
  ['latin', 'POST /login (HTTP 401)'],
  ['latin', 'WAVE Tympany, fjord; quiz'],
  ['latin', '3,141.59 % ~ [x] {y} (z)'],
  ['latin', 'Résumé généré à Zürich'],
  ['latin', 'Straße, Ærø, naïve façade'],
  ['latin', 'll iii mmm WWW'],
  ['fallback', '学习计划'],
  ['fallback', '撰写用户手册的第一章'],
  ['fallback', '開始する データ'],
  ['fallback', '완료 상태'],
  ['fallback', 'Done ✅ 🚀'],
  ['fallback', 'Build 🔧 and ship 📦 today'],
  ['fallback', 'Mixed 中文 and Latin'],
];

const FAMILIES = {
  mermaid: '"trebuchet ms", verdana, arial, sans-serif',
  open_sans: '"Open Sans", sans-serif',
};
const SIZES = [14, 16, 20];

const CSS = [
  '@fontsource-variable/recursive/index.css',
  '@fontsource/open-sans/400.css',
  '@fontsource/open-sans/400-italic.css',
].map((p) => url.pathToFileURL(require.resolve(p)).href);

const pconf = JSON.parse(fs.readFileSync(path.join(oracleDir, 'puppeteer.json'), 'utf8'));
const pageUrl = url.pathToFileURL(
  path.join(path.dirname(require.resolve('@mermaid-js/mermaid-cli')), '..', 'dist', 'index.html')
).href;

const browser = await puppeteer.launch({
  ...pconf,
  args: [...(pconf.args ?? []), '--allow-file-access-from-files'],
});
try {
  const page = await browser.newPage();
  await page.goto(pageUrl);
  for (const href of CSS) {
    await page.evaluate((href) => {
      const link = document.createElement('link');
      link.setAttribute('crossorigin', 'anonymous');
      link.setAttribute('rel', 'stylesheet');
      link.setAttribute('href', href);
      const loaded = new Promise((resolve, reject) => {
        link.onload = () => resolve();
        link.onerror = () => reject(new Error(`stylesheet ${href}`));
      });
      document.head.appendChild(link);
      return loaded;
    }, href);
  }
  await page.addStyleTag({ content: 'p { margin: 0 }' });
  await page.evaluate(() => Promise.all(Array.from(document.fonts, (f) => f.load())));

  const cases = await page.evaluate(
    ({ STRINGS, FAMILIES, SIZES }) => {
      const NS = 'http://www.w3.org/2000/svg';
      const svg = document.createElementNS(NS, 'svg');
      svg.setAttribute('width', '2000');
      svg.setAttribute('height', '2000');
      document.body.appendChild(svg);
      const font = (el, family, size) => {
        el.style.fontFamily = family;
        el.style.fontSize = `${size}px`;
        el.style.fontWeight = '400';
        el.style.fontStyle = 'normal';
      };
      const measureSvg = (text, family, size) => {
        const t = document.createElementNS(NS, 'text');
        t.setAttribute('x', '0');
        t.setAttribute('y', '0');
        t.style.textAnchor = 'start';
        font(t, family, size);
        const span = document.createElementNS(NS, 'tspan');
        span.setAttribute('x', '0');
        span.textContent = text;
        t.appendChild(span);
        svg.appendChild(t);
        const b = t.getBBox();
        svg.removeChild(t);
        return { width: b.width, height: b.height };
      };
      const measureHtml = (text, family, size) => {
        const fo = document.createElementNS(NS, 'foreignObject');
        fo.setAttribute('width', '10000px');
        fo.setAttribute('height', '10000px');
        const div = document.createElementNS('http://www.w3.org/1999/xhtml', 'div');
        const span = document.createElement('span');
        span.innerHTML = `<p>${text}</p>`;
        span.className = 'nodeLabel';
        div.appendChild(span);
        div.style.display = 'table-cell';
        div.style.whiteSpace = 'nowrap';
        div.style.lineHeight = '1.5';
        font(div, family, size);
        fo.appendChild(div);
        svg.appendChild(fo);
        const r = div.getBoundingClientRect();
        svg.removeChild(fo);
        return { width: r.width, height: r.height };
      };
      const out = [];
      for (const [familyId, family] of Object.entries(FAMILIES)) {
        for (const size of SIZES) {
          for (const [cls, text] of STRINGS) {
            out.push({
              class: cls,
              text,
              family_id: familyId,
              family,
              size,
              svg: measureSvg(text, family, size),
              html: measureHtml(text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'), family, size),
            });
          }
        }
      }
      return out;
    },
    { STRINGS, FAMILIES, SIZES }
  );
  const outPath = path.join(here, 'measure_fixture.json');
  fs.writeFileSync(outPath, JSON.stringify({ generated_by: 'measure_corpus.mjs', cases }, null, 1) + '\n');
  console.log(`measure: ${cases.length} cases -> ${outPath}`);
} finally {
  await browser.close();
}
