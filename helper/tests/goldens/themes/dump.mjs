// Dumps the oracle's theme variables and default config for the Rust parity
// tests in helper/tests/theme_parity.rs and helper/tests/mermaid_config_*.rs.
//
// Run from the repo root after `npm ci --prefix helper/oracle`:
//
//   node helper/tests/goldens/themes/dump.mjs
//
// It imports the pinned mermaid (12.2.0) straight from helper/oracle's
// node_modules. The ESM core build initializes without a DOM, so no jsdom or
// puppeteer is needed: `initialize`, `getConfig`, and `parse` run in plain
// node. The files land next to this script.
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const mermaidPath = join(here, '../../../oracle/node_modules/mermaid/dist/mermaid.core.mjs');
const { default: mermaid } = await import(mermaidPath);

// The base config from SPEC.md: what helper/oracle/mermaid-config.json holds,
// plus mermaid-cli's own `startOnLoad: false`.
const BASE = { startOnLoad: false, layout: 'dagre', look: 'classic', theme: 'default', handDrawnSeed: 1 };
const THEMES = ['default', 'dark', 'neutral', 'forest', 'base'];
const OVERRIDE = { primaryColor: '#ff0000', fontFamily: 'Arial' };

const write = (name, value) =>
  writeFileSync(join(here, name), JSON.stringify(value, null, 2) + '\n');

const withoutThemeVariables = (config) => {
  const copy = { ...config };
  delete copy.themeVariables;
  return copy;
};

// Same walk as `keyify` in defaultConfig.ts: every key at every depth, arrays
// excluded, function- and undefined-valued keys included. `sanitizeDirective`
// drops any directive key outside this set.
const keyify = (obj, prefix = '') =>
  Object.keys(obj).reduce((res, el) => {
    if (Array.isArray(obj[el])) {
      return res;
    } else if (typeof obj[el] === 'object' && obj[el] !== null) {
      return [...res, prefix + el, ...keyify(obj[el], '')];
    }
    return [...res, prefix + el];
  }, []);

write('default-config.json', withoutThemeVariables(mermaid.mermaidAPI.defaultConfig));
write('config-keys.json', [...new Set(keyify(mermaid.mermaidAPI.defaultConfig))].sort());

mermaid.initialize(BASE);
write('site-config.json', withoutThemeVariables(mermaid.mermaidAPI.getConfig()));

for (const theme of THEMES) {
  mermaid.initialize({ ...BASE, theme });
  write(`${theme}.json`, mermaid.mermaidAPI.getConfig().themeVariables);
  mermaid.initialize({ ...BASE, theme, themeVariables: OVERRIDE });
  write(`${theme}-override.json`, mermaid.mermaidAPI.getConfig().themeVariables);
}

// End-to-end cases through `parse`: the config mermaid holds after the
// diagram's directives and front matter are applied. `parse` leaves the
// diagram config scope in its `finally`, so the dump reflects the merged
// config without the per-diagram appearance resolution; the Rust test skips
// the section-level `theme`, `look`, and `layout` keys for that reason.
const CASES = {
  'directive-theme-forest': '%%{init: {"theme": "forest"}}%%\ngraph TD\n  A-->B\n',
  'directive-theme-vars': '%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#00ff00", "fontFamily": "Courier"}}}%%\ngraph TD\n  A-->B\n',
  'directive-vars-no-theme': '%%{init: {"themeVariables": {"primaryColor": "#00ff00"}}}%%\ngraph TD\n  A-->B\n',
  'directive-font-family': "%%{init: {'fontFamily': 'Courier New'}}%%\nsequenceDiagram\n  A->>B: hi\n",
  'directive-section': "%%{init: {'flowchart': {'nodeSpacing': 99, 'curve': 'linear'}, 'sequence': {'mirrorActors': false}}}%%\ngraph LR\n  A-->B\n",
  'directive-config-key': "%%{init: {'config': {'nodeSpacing': 77}}}%%\nflowchart LR\n  A-->B\n",
  'frontmatter-config': '---\ntitle: Hello\nconfig:\n  theme: dark\n  flowchart:\n    rankSpacing: 123\n---\ngraph TD\n  A-->B\n',
  'frontmatter-and-directive': '---\nconfig:\n  theme: dark\n  flowchart:\n    rankSpacing: 123\n---\n%%{init: {"theme": "neutral", "flowchart": {"nodeSpacing": 45}}}%%\ngraph TD\n  A-->B\n',
  'directive-dark-vars': '%%{init: {"theme": "dark", "themeVariables": {"primaryColor": "#336699", "lineColor": "red", "pie1": "#ABCDEF"}}}%%\npie\n  "a": 1\n',
  'directive-neutral-vars': '%%{init: {"theme": "neutral", "themeVariables": {"primaryColor": "hsl(200, 50%, 50%)", "background": "#000"}}}%%\nerDiagram\n  A ||--o{ B : has\n',
  'directive-secure-keys': '%%{init: {"maxTextSize": 1, "securityLevel": "loose", "theme": "forest", "unknownKey": 1, "gantt": {"barHeight": 33, "bogus": 2}}}%%\ngantt\n  title x\n',
  'directive-wrap': '%%{wrap}%%\nsequenceDiagram\n  A->>B: hi\n',
  'directive-multiple': '%%{init: {"theme": "dark"}}%%\n%%{init: {"flowchart": {"padding": 3}}}%%\ngraph TD\n  A-->B\n',
  'displaymode': '---\ndisplayMode: compact\n---\ngantt\n  title x\n',
  'themevars-sanitized': '%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#ff0000", "fontFamily": "sans-serif", "lineColor": "<b>", "darkMode": true}}}%%\ngraph TD\n  A-->B\n',
};

// The config is set before the diagram parser runs, so a parser that needs a
// DOM (gantt's title goes through DOMPurify) still leaves the config to read.
const cases = {};
for (const [name, text] of Object.entries(CASES)) {
  mermaid.initialize(BASE);
  let diagramType;
  let parseError = null;
  try {
    ({ diagramType } = await mermaid.parse(text));
  } catch (error) {
    diagramType = mermaid.detectType(text);
    parseError = error.message;
  }
  const current = mermaid.mermaidAPI.getConfig();
  const themeVariables = current.themeVariables;
  cases[name] = {
    text,
    diagramType,
    parseError,
    config: withoutThemeVariables(current),
    themeVariables,
  };
}
write('parse-cases.json', cases);
