// Regenerates `<name>.tables.json` for every vendored `<name>.jison` grammar
// in this directory with the pinned jison 0.4.18 from helper/oracle. The
// Rust driver in helper/src/mermaid/jison replays these tables; it never
// runs jison itself. Run through `just jison-tables`.
//
// The JSON keeps jison's own encodings so the Rust side can be checked
// against lib/jison.js line by line: `table[state][symbol]` is `[1, state]`
// for shift, `[2, production]` for reduce, `[3]` for accept, and a bare
// state number for a goto on a nonterminal. Lexer rule patterns are the
// macro-expanded sources jison-lex compiles, without the `^(?:` `)` wrapper
// it adds at match time. Actions stay as raw JavaScript source; the Rust
// loader translates the lexer idioms and the diagram modules implement the
// grammar actions by hand.

import { createRequire } from 'node:module';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const oracle = join(here, '..', 'oracle');
const require = createRequire(join(oracle, 'package.json'));
const jison = require('jison');
const jisonVersion = require('jison/package.json').version;
// jison parses grammar text with ebnf-parser, which itself hands the `%lex`
// section to lex-parser. Resolving them relative to jison keeps the exact
// versions jison pins.
const jisonRequire = createRequire(require.resolve('jison'));
const ebnfParser = jisonRequire('ebnf-parser');

if (jisonVersion !== '0.4.18') {
  throw new Error(`expected jison 0.4.18, found ${jisonVersion}`);
}

const MERMAID_VERSION = '12.2.0';

function stripWrapper(source) {
  // jison-lex compiles each rule as `^(?:<pattern>)`.
  const open = '^(?:';
  if (!source.startsWith(open) || !source.endsWith(')')) {
    throw new Error(`unexpected lexer rule source: ${source}`);
  }
  return source.slice(open.length, -1);
}

function lexerRules(lex, generator) {
  // `generator.lexer.rules` holds the compiled RegExp per rule after macro
  // expansion, in rule order; the raw dict keeps the actions and the start
  // conditions. jison-lex mutates the dict while compiling, so the
  // generator worked on a deep copy and `lex` is still pristine.
  const compiled = generator.lexer.rules;
  if (compiled.length !== lex.rules.length) {
    throw new Error(`rule count mismatch: ${compiled.length} vs ${lex.rules.length}`);
  }
  return lex.rules.map((rule, index) => {
    const explicit = Array.isArray(rule[0]);
    const conditions = explicit ? rule[0] : null;
    const action = explicit ? rule[2] : rule[1];
    return {
      index,
      pattern: stripWrapper(compiled[index].source),
      // `null` means "every inclusive start condition" (a rule written
      // without `<...>`), `["*"]` means every start condition.
      conditions,
      action: typeof action === 'function' ? String(action) : action,
    };
  });
}

function productions(bnf, generator) {
  // Production 0 is the augmented `$accept: start $end`. The rest follow
  // jison's iteration order over the bnf object, which is the order
  // `productions_` uses.
  const out = [{ lhs: '$accept', rhs: [generator.startSymbol, '$end'], action: null }];
  for (const lhs of Object.keys(bnf)) {
    const handles = typeof bnf[lhs] === 'string' ? bnf[lhs].split(/\s*\|\s*/g) : bnf[lhs];
    for (const handle of handles) {
      let rhs;
      let action = null;
      let precedence = null;
      if (Array.isArray(handle)) {
        rhs = handle[0];
        if (typeof handle[1] === 'string') {
          action = handle[1];
          precedence = handle[2] ? handle[2].prec : null;
        } else if (handle[1] && handle[1].prec) {
          precedence = handle[1].prec;
        }
      } else {
        rhs = handle;
      }
      const symbols = typeof rhs === 'string' ? rhs.trim().split(' ').filter(Boolean) : rhs;
      out.push({ lhs, rhs: symbols, action, precedence });
    }
  }
  if (out.length !== generator.productions_.length) {
    throw new Error(`production count mismatch: ${out.length} vs ${generator.productions_.length}`);
  }
  out.forEach((p, i) => {
    const [lhsId, len] = i === 0 ? [0, 2] : generator.productions_[i];
    const expectedId = i === 0 ? 0 : generator.symbols_[p.lhs];
    if (lhsId !== expectedId || len !== p.rhs.length) {
      throw new Error(`production ${i} (${p.lhs}) does not match productions_`);
    }
  });
  return out;
}

function build(name) {
  const text = readFileSync(join(here, `${name}.jison`), 'utf8');
  const dict = ebnfParser.parse(text);
  const generator = new jison.Generator(structuredClone(dict), { type: 'lalr' });
  const conflicts = generator.conflicts || 0;
  const lex = dict.lex;
  const options = lex.options || {};
  const conditions = {};
  for (const [cond, spec] of Object.entries(generator.lexer.conditions)) {
    conditions[cond] = { rules: spec.rules, inclusive: spec.inclusive };
  }
  const tables = {
    grammar: name,
    mermaid_version: MERMAID_VERSION,
    jison_version: jisonVersion,
    conflicts,
    start: generator.startSymbol,
    symbols_: generator.symbols_,
    terminals_: generator.terminals_,
    productions_: generator.productions_.map((p, i) => (i === 0 ? [0, 2] : p)),
    productions: productions(dict.bnf, generator),
    table: generator.table,
    defaultActions: generator.defaultActions,
    lexer: {
      options: {
        flex: Boolean(options.flex),
        case_insensitive: Boolean(options['case-insensitive']),
      },
      macros: lex.macros || {},
      action_include: lex.actionInclude || null,
      conditions,
      rules: lexerRules(lex, generator),
    },
  };
  writeFileSync(join(here, `${name}.tables.json`), `${JSON.stringify(tables, null, 1)}\n`);
  return { name, states: generator.table.length, rules: lex.rules.length, conflicts };
}

const names = readdirSync(here)
  .filter((f) => f.endsWith('.jison'))
  .map((f) => f.slice(0, -'.jison'.length))
  .sort();
for (const name of names) {
  const { states, rules, conflicts } = build(name);
  console.log(`${name}: ${states} states, ${rules} lexer rules, ${conflicts} conflicts`);
}
