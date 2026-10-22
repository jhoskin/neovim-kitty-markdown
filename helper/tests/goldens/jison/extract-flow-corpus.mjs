// Extracts the flowchart parser corpus from mermaid's `flow-*.spec.js` and
// `subgraph.spec.js` files by running them against a recording stub of the
// parser, so template literals, `it.each` tables and helper variables
// resolve as they do under vitest.
//
//   node extract-flow-corpus.mjs <dir with the spec files> <output dir>
//
// Writes `flow_valid.txt` (inputs the specs expect to parse, separated by a
// line of `---`) and `flow_invalid.json` (inputs the specs expect to throw,
// with the message fragment the spec checks, if any). The invalid list is
// the input to `oracle-errors.mjs`, which records the exact message the
// pinned mermaid prints.

import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import vm from 'node:vm';

const [specDir, outDir] = process.argv.slice(2);
if (!specDir || !outDir) {
  console.error('usage: extract-flow-corpus.mjs <spec dir> <out dir>');
  process.exit(2);
}

const valid = new Map();
const invalid = new Map();
let mode = { throws: false, message: null };
let currentSpec = '';

// A value that accepts any property access and call, for the FlowDB and the
// `expect` chains whose results do not matter here.
function anything() {
  const fn = function () {
    return proxy;
  };
  const proxy = new Proxy(fn, {
    get(_target, prop) {
      if (prop === Symbol.toPrimitive) {
        return () => '';
      }
      if (prop === 'then') {
        return undefined;
      }
      if (prop === 'length') {
        return 0;
      }
      return proxy;
    },
    apply() {
      return proxy;
    },
    construct() {
      return proxy;
    },
  });
  return proxy;
}

function record(input) {
  if (typeof input !== 'string') {
    return;
  }
  const target = mode.throws ? invalid : valid;
  if (!target.has(input)) {
    target.set(input, { spec: currentSpec, message: mode.message });
  }
}

const flow = {
  parser: {
    yy: anything(),
    parse(input) {
      record(input);
      return anything();
    },
  },
};

function expect(value) {
  const chain = (negated) =>
    new Proxy(
      {},
      {
        get(_target, prop) {
          if (prop === 'not') {
            return chain(!negated);
          }
          if (prop === 'toThrow' || prop === 'toThrowError') {
            return (message) => {
              if (typeof value !== 'function') {
                return chain(negated);
              }
              const saved = mode;
              mode = { throws: !negated, message: typeof message === 'string' ? message : null };
              try {
                value();
              } catch {
                // The stub never throws; a real failure here is a harness bug.
              } finally {
                mode = saved;
              }
              return chain(negated);
            };
          }
          return () => chain(negated);
        },
      }
    );
  return chain(false);
}
for (const helper of ['stringContaining', 'objectContaining', 'arrayContaining', 'any', 'anything']) {
  expect[helper] = () => anything();
}

function makeIt() {
  const it = (_name, fn) => run(fn);
  it.each = (table) => (_name, fn) => {
    for (const row of table) {
      run(() => (Array.isArray(row) ? fn(...row) : fn(row)));
    }
  };
  it.skip = () => {};
  it.only = it;
  return it;
}

function run(fn) {
  if (typeof fn !== 'function') {
    return;
  }
  try {
    fn();
  } catch (e) {
    console.error(`${currentSpec}: test body threw: ${e.message}`);
  }
}

const describe = (_name, fn) => run(fn);
describe.each = (table) => (_name, fn) => {
  for (const row of table) {
    run(() => (Array.isArray(row) ? fn(...row) : fn(row)));
  }
};
describe.skip = () => {};

function cleanupComments(text) {
  // packages/mermaid/src/diagram-api/comments.ts at mermaid 12.2.0
  return text.replace(/^\s*%%(?!{)[^\n]+\n?/gm, '').trimStart();
}

const sandbox = {
  describe,
  it: makeIt(),
  test: makeIt(),
  expect,
  beforeEach: run,
  afterEach: run,
  beforeAll: run,
  afterAll: run,
  vi: { spyOn: () => anything(), fn: () => anything() },
  FlowDB: anything(),
  setConfig: () => {},
  cleanupComments,
  log: anything(),
  flow,
  console,
};

const files = readdirSync(specDir)
  .filter((f) => (f.startsWith('flow') || f === 'subgraph.spec.js') && f.endsWith('.spec.js'))
  .sort();
for (const file of files) {
  currentSpec = file;
  const source = readFileSync(join(specDir, file), 'utf8').replace(/^import .*$/gm, '');
  // A fresh context per file: the specs share top-level names like `keywords`.
  vm.runInNewContext(source, vm.createContext({ ...sandbox }), { filename: file });
}

const SEPARATOR = '---';
const inputs = [...valid.keys()];
for (const input of inputs) {
  if (input.split('\n').includes(SEPARATOR)) {
    throw new Error(`input contains a separator line: ${JSON.stringify(input)}`);
  }
}
writeFileSync(join(outDir, 'flow_valid.txt'), `${inputs.join(`\n${SEPARATOR}\n`)}\n`);
writeFileSync(
  join(outDir, 'flow_invalid.json'),
  `${JSON.stringify(
    [...invalid.entries()].map(([input, meta]) => ({ input, spec: meta.spec, expects: meta.message })),
    null,
    2
  )}\n`
);
console.log(`${files.length} spec files, ${valid.size} valid inputs, ${invalid.size} invalid inputs`);
