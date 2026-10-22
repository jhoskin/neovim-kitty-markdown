// Text metrics oracle for helper/tests/text_metrics.rs.
//
// Mermaid measures labels in Chrome: `getBBox()` on an SVG `<text>` for
// SVG labels and `getBoundingClientRect()` on a `display: table-cell` div
// inside a `<foreignObject>` for HTML labels. This script opens the same
// page mermaid-cli 12.0.0 opens (dist/index.html), injects the same
// @fontsource stylesheets, and records both measurements for a fixed corpus
// so the Rust measurer can be checked against the real numbers.
//
// Usage: node helper/oracle/measure.mjs [out.json]
// The default output is helper/tests/goldens/metrics.json.

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import { createRequire } from 'node:module';
import puppeteer from 'puppeteer';

const require = createRequire(import.meta.url);
const here = path.dirname(url.fileURLToPath(import.meta.url));
const out = process.argv[2] ?? path.join(here, '..', 'tests', 'goldens', 'metrics.json');

const STRINGS = {
  word: 'Hello',
  node: 'Start',
  long_word: 'flowchart',
  kerning: 'AVAWAY Type',
  sentence: 'The quick brown fox jumps over the lazy dog',
  digits: '0123456789',
  punct: '!@#$%^&*()_+-=[]{};\':",./<>?',
  cjk: '中文中 日本語',
  emoji: '🏳️‍⚧️ 🏳️‍🌈',
  empty: '',
};

const FAMILIES = {
  mermaid: '"trebuchet ms", verdana, arial, sans-serif',
  open_sans: '"Open Sans", sans-serif',
  generic: 'sans-serif',
};

const SIZES = [12, 14, 16];
const VARIANTS = [
  { weight: 400, style: 'normal' },
  { weight: 700, style: 'normal' },
  { weight: 400, style: 'italic' },
];

const WRAPPED = [
  'This is a fairly long node label that the browser has to wrap onto several lines',
  'Supercalifragilisticexpialidocious words and short ones mixed in one label',
];
const WRAP_MAX_WIDTH = 200;

// mermaid-cli injects these from its own node_modules; the fonts are loaded
// through file:// here, so Chrome needs --allow-file-access-from-files.
const CSS = [
  '@fontsource-variable/recursive/index.css',
  '@fontsource/open-sans/400.css',
  '@fontsource/open-sans/400-italic.css',
].map((p) => url.pathToFileURL(require.resolve(p)).href);

const puppeteerConfig = JSON.parse(fs.readFileSync(path.join(here, 'puppeteer.json'), 'utf8'));
// The package's exports map hides dist/, so walk from its entry point.
const pageUrl = url.pathToFileURL(
  path.join(path.dirname(require.resolve('@mermaid-js/mermaid-cli')), '..', 'dist', 'index.html')
).href;

const browser = await puppeteer.launch({
  ...puppeteerConfig,
  args: [...(puppeteerConfig.args ?? []), '--allow-file-access-from-files'],
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
        link.onerror = () => reject(new Error(`Failed to load stylesheet: ${href}`));
      });
      document.head.appendChild(link);
      return loaded;
    }, href);
  }
  // mermaid's root stylesheet sets `p { margin: 0 }`; HTML labels wrap their
  // text in a <p>, so the rule decides the measured height.
  await page.addStyleTag({ content: 'p { margin: 0 }' });
  await page.evaluate(() => Promise.all(Array.from(document.fonts, (font) => font.load())));

  const result = await page.evaluate(
    ({ STRINGS, FAMILIES, SIZES, VARIANTS, WRAPPED, WRAP_MAX_WIDTH }) => {
      const SVG_NS = 'http://www.w3.org/2000/svg';
      const ZERO_WIDTH_SPACE = '​';
      const body = document.body;

      const svg = document.createElementNS(SVG_NS, 'svg');
      svg.setAttribute('width', '2000');
      svg.setAttribute('height', '2000');
      body.appendChild(svg);

      function font(el, family, size, weight, style) {
        el.style.fontFamily = family;
        el.style.fontSize = `${size}px`;
        el.style.fontWeight = String(weight);
        el.style.fontStyle = style;
      }

      // utils.ts drawSimpleText: <text> with one <tspan x=0>, measured with getBBox.
      function measureSvg(text, family, size, weight, style) {
        const textEl = document.createElementNS(SVG_NS, 'text');
        textEl.setAttribute('x', '0');
        textEl.setAttribute('y', '0');
        textEl.style.textAnchor = 'start';
        font(textEl, family, size, weight, style);
        const span = document.createElementNS(SVG_NS, 'tspan');
        span.setAttribute('x', '0');
        span.textContent = text || ZERO_WIDTH_SPACE;
        textEl.appendChild(span);
        svg.appendChild(textEl);
        const bbox = textEl.getBBox();
        const textLength = span.getComputedTextLength();
        svg.removeChild(textEl);
        return { width: bbox.width, height: bbox.height, text_length: textLength };
      }

      // createText.ts addHtmlSpan: foreignObject > div(table-cell, nowrap,
      // line-height 1.5) > span > p (from markdownToHTML).
      function htmlLabel(html, family, size, weight, style, width) {
        const fo = document.createElementNS(SVG_NS, 'foreignObject');
        fo.setAttribute('width', `${Math.min(10 * width, 16384)}px`);
        fo.setAttribute('height', `${Math.min(10 * width, 16384)}px`);
        const div = document.createElementNS('http://www.w3.org/1999/xhtml', 'div');
        const span = document.createElement('span');
        span.innerHTML = html;
        span.className = 'nodeLabel';
        div.appendChild(span);
        div.style.display = 'table-cell';
        div.style.whiteSpace = 'nowrap';
        div.style.lineHeight = '1.5';
        if (width !== Number.POSITIVE_INFINITY) {
          div.style.maxWidth = `${width}px`;
          div.style.textAlign = 'center';
        }
        font(div, family, size, weight, style);
        fo.appendChild(div);
        svg.appendChild(fo);
        return { fo, div, span };
      }

      function measureHtml(text, family, size, weight, style) {
        const html = text ? `<p>${text}</p>` : '';
        const { fo, div } = htmlLabel(html, family, size, weight, style, Number.POSITIVE_INFINITY);
        const r = div.getBoundingClientRect();
        svg.removeChild(fo);
        return { width: r.width, height: r.height };
      }

      // The lines Chrome produced, read back from per-character client rects.
      function readLines(p) {
        const node = p.firstChild;
        const text = node.textContent;
        const lines = [];
        let lastTop = null;
        for (let i = 0; i < text.length; i++) {
          const range = document.createRange();
          range.setStart(node, i);
          range.setEnd(node, i + 1);
          const rects = range.getClientRects();
          if (rects.length === 0) {
            continue;
          }
          const top = Math.round(rects[0].top * 100) / 100;
          if (lastTop === null || Math.abs(top - lastTop) > 0.5) {
            lines.push('');
            lastTop = top;
          }
          lines[lines.length - 1] += text[i];
        }
        return lines.map((l) => l.trim());
      }

      function measureWrapped(text, family, size, weight, style, width) {
        const html = `<p>${text}</p>`;
        const { fo, div, span } = htmlLabel(html, family, size, weight, style, width);
        let r = div.getBoundingClientRect();
        const nowrap = { width: r.width, height: r.height };
        if (r.width === width) {
          div.style.display = 'table';
          div.style.whiteSpace = 'break-spaces';
          div.style.width = `${width}px`;
          r = div.getBoundingClientRect();
        }
        const lines = readLines(span.firstChild);
        svg.removeChild(fo);
        return { nowrap, width: r.width, height: r.height, lines };
      }

      // A family resolved when it measures differently from the generic it
      // would fall back to.
      function available(family) {
        const probe = 'The quick brown fox 0123456789';
        const a = measureSvg(probe, `"${family}", monospace`, 16, 400, 'normal').width;
        const b = measureSvg(probe, 'monospace', 16, 400, 'normal').width;
        return a !== b;
      }
      // Arial and Helvetica share advance widths, so compare heights too.
      function sameFamily(a, b) {
        const probe = 'The quick brown fox 0123456789 AVAWAY';
        const ma = measureSvg(probe, a, 16, 400, 'normal');
        const mb = measureSvg(probe, b, 16, 400, 'normal');
        return ma.width === mb.width && ma.height === mb.height;
      }

      const fonts = {};
      for (const f of ['Trebuchet MS', 'Verdana', 'Arial', 'Open Sans', 'Helvetica']) {
        fonts[f] = available(f);
      }
      const generics = {
        'sans-serif': ['Helvetica', 'Arial', 'DejaVu Sans', 'Liberation Sans'].find((f) =>
          sameFamily('sans-serif', `"${f}"`)
        ),
        serif: ['Times', 'Times New Roman', 'DejaVu Serif', 'Liberation Serif'].find((f) =>
          sameFamily('serif', `"${f}"`)
        ),
        monospace: ['Courier', 'Courier New', 'Menlo', 'DejaVu Sans Mono', 'Liberation Mono'].find(
          (f) => sameFamily('monospace', `"${f}"`)
        ),
      };

      const cases = [];
      for (const [familyId, family] of Object.entries(FAMILIES)) {
        for (const size of SIZES) {
          for (const { weight, style } of VARIANTS) {
            for (const [textId, text] of Object.entries(STRINGS)) {
              cases.push({
                id: `${textId}/${familyId}/${size}/${weight}/${style}`,
                text,
                family,
                size,
                weight,
                style,
                svg: measureSvg(text, family, size, weight, style),
                html: measureHtml(text, family, size, weight, style),
              });
            }
          }
        }
      }

      const wrapped = [];
      for (const [familyId, family] of Object.entries(FAMILIES)) {
        for (const size of SIZES) {
          for (const text of WRAPPED) {
            wrapped.push({
              id: `${familyId}/${size}`,
              text,
              family,
              size,
              weight: 400,
              style: 'normal',
              max_width: WRAP_MAX_WIDTH,
              ...measureWrapped(text, family, size, 400, 'normal', WRAP_MAX_WIDTH),
            });
          }
        }
      }

      // break-spaces probe: a line whose text fits but whose trailing space
      // does not. Reports where the next line starts so the port can tell
      // whether Chrome wraps the space onto the next line or hangs it.
      const probeFamily = FAMILIES.mermaid;
      const probeSize = 16;
      const w = (t) => measureSvg(t, probeFamily, probeSize, 400, 'normal').text_length;
      const longWord = 'Supercalifragilisticexpialidocious';
      const avail = w(longWord);
      let n = 1;
      while (w('i'.repeat(n + 1)) <= avail) {
        n++;
      }
      const probeText = `${longWord} ${'i'.repeat(n)} next end`;
      const { fo, div, span } = htmlLabel(
        `<p>${probeText}</p>`,
        probeFamily,
        probeSize,
        400,
        'normal',
        WRAP_MAX_WIDTH
      );
      div.style.display = 'table';
      div.style.whiteSpace = 'break-spaces';
      div.style.width = `${WRAP_MAX_WIDTH}px`;
      const divRect = div.getBoundingClientRect();
      const node = span.firstChild.firstChild;
      const lineStarts = [];
      let lastTop = null;
      for (let i = 0; i < node.textContent.length; i++) {
        const range = document.createRange();
        range.setStart(node, i);
        range.setEnd(node, i + 1);
        const rect = range.getClientRects()[0];
        if (!rect) {
          continue;
        }
        if (lastTop === null || Math.abs(rect.top - lastTop) > 0.5) {
          lineStarts.push({ char: node.textContent[i], left: rect.left - divRect.left });
          lastTop = rect.top;
        }
      }
      svg.removeChild(fo);
      const probe = {
        text: probeText,
        family: probeFamily,
        size: probeSize,
        avail,
        fits: w('i'.repeat(n)),
        fits_with_space: w(`${'i'.repeat(n)} `),
        width: divRect.width,
        height: divRect.height,
        line_starts: lineStarts,
        next_end_width: w('next end'),
        space_width: w('i i') - w('ii'),
      };

      body.removeChild(svg);
      return { fonts, generics, cases, wrapped, probe };
    },
    { STRINGS, FAMILIES, SIZES, VARIANTS, WRAPPED, WRAP_MAX_WIDTH }
  );

  // Read package.json directly: some exports maps hide it from require.
  const version = (name) =>
    JSON.parse(fs.readFileSync(path.join(here, 'node_modules', name, 'package.json'), 'utf8'))
      .version;
  const metrics = {
    generated_by: 'helper/oracle/measure.mjs',
    platform: process.platform,
    chrome: await browser.version(),
    puppeteer: version('puppeteer'),
    mermaid_cli: version('@mermaid-js/mermaid-cli'),
    mermaid: version('mermaid'),
    fontsource_open_sans: version('@fontsource/open-sans'),
    ...result,
  };
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, JSON.stringify(metrics, null, 1) + '\n');
  console.log(`wrote ${metrics.cases.length} cases and ${metrics.wrapped.length} wrapped cases to ${out}`);
} finally {
  await browser.close();
}
