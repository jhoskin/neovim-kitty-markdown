// Records the exact error text the pinned mermaid (helper/oracle) prints for
// invalid diagrams, so `helper/tests/jison_flow.rs` can compare the Rust
// `ParseError` display character for character.
//
//   node oracle-errors.mjs <goldens/jison dir> [--only N]
//
// Reads `flow_invalid.json` (from extract-flow-corpus.mjs) and
// `extra_invalid.json` (hand-written cases, with a `grammar` field) and
// writes `errors.json` with, per case, the text mermaid's jison parser saw
// (`parsed`) and the message mmdc printed. mermaid hands the parser the
// comment-stripped text plus a trailing newline; the cases here avoid
// comments, front matter, entities and HTML tags so that `parsed` is the
// input with leading whitespace trimmed and `\n` appended.

import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const oracle = join(here, '..', '..', '..', 'oracle');
const mmdc = join(oracle, 'node_modules', '.bin', 'mmdc');

const [dir, ...rest] = process.argv.slice(2);
if (!dir) {
  console.error('usage: oracle-errors.mjs <goldens/jison dir> [--only N]');
  process.exit(2);
}
const only = rest[0] === '--only' ? Number(rest[1]) : null;

function parsedText(grammar, input) {
  for (const [what, re] of [
    ['comment', /%%/],
    ['front matter', /^---/m],
    ['entity', /#\w+;/],
    ['html tag', /<\w+[^>]*>/],
    ['carriage return', /\r/],
    ['style line with a hash', /style.*:\S*#.*;/],
  ]) {
    if (re.test(input)) {
      throw new Error(`case needs mermaid preprocessing (${what}): ${JSON.stringify(input)}`);
    }
  }
  const text = `${input.trimStart()}\n`;
  if (grammar === 'sankey') {
    // sankeyDiagram.ts wraps parse with sankeyUtils.prepareTextForParsing.
    return text
      .replaceAll(/^[^\S\n\r]+|[^\S\n\r]+$/g, '')
      .replaceAll(/([\n\r])+/g, '\n')
      .trim();
  }
  return text;
}

const cases = [];
for (const c of JSON.parse(readFileSync(join(dir, 'flow_invalid.json'), 'utf8'))) {
  cases.push({ grammar: 'flow', input: c.input, spec: c.spec, expects: c.expects });
}
for (const c of JSON.parse(readFileSync(join(dir, 'extra_invalid.json'), 'utf8'))) {
  cases.push({ grammar: c.grammar, input: c.input, note: c.note });
}

const work = mkdtempSync(join(tmpdir(), 'kitty-markdown-oracle-'));
const out = [];
cases.forEach((c, i) => {
  if (only !== null && i !== only) {
    return;
  }
  const file = join(work, `case-${i}.mmd`);
  writeFileSync(file, c.input);
  const result = spawnSync(
    mmdc,
    [
      '-p',
      join(oracle, 'puppeteer.json'),
      '-c',
      join(oracle, 'mermaid-config.json'),
      '-i',
      file,
      '-o',
      join(work, `case-${i}.svg`),
    ],
    { encoding: 'utf8' }
  );
  const stderr = result.stderr || '';
  // mmdc prints `Error: <message>` followed by the stack, whose frames are
  // `Name (https://...` or `    at ...` lines; keep the message only.
  const start = stderr.indexOf('Error: ');
  let message = null;
  if (start >= 0) {
    const body = stderr.slice(start + 'Error: '.length);
    const stack = body.search(/\n(?:[\w.$#<>]+ \((?:https?|file):\/\/|\s+at )/);
    message = (stack >= 0 ? body.slice(0, stack) : body).replace(/\n$/, '');
  }
  const entry = {
    grammar: c.grammar,
    input: c.input,
    parsed: parsedText(c.grammar, c.input),
    status: result.status,
    message,
  };
  if (c.spec) {
    entry.spec = c.spec;
  }
  if (c.expects) {
    entry.expects = c.expects;
  }
  if (c.note) {
    entry.note = c.note;
  }
  if (only !== null) {
    console.log(JSON.stringify({ stdout: result.stdout, stderr }, null, 2));
  }
  out.push(entry);
  console.error(`case ${i}: exit ${result.status}: ${message === null ? '(no error)' : message.split('\n')[0]}`);
});
if (only === null) {
  writeFileSync(join(dir, 'errors.json'), `${JSON.stringify(out, null, 2)}\n`);
}
