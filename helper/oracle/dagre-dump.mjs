// Dumps dagre-d3-es 7.0.14 layouts as parity goldens for the Rust port.
//
//   npm ci --prefix helper/oracle
//   node helper/oracle/dagre-dump.mjs helper/tests/goldens/dagre
//
// Every graph runs in a fresh node process, because lodash's `_.uniqueId`
// counter is module-global: dummy node ids such as `_bt12` take part in
// string comparisons inside dagre, so the counter must start at 0 for each
// graph exactly as the Rust port resets it before a layout.

import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { Graph } from 'dagre-d3-es/src/graphlib/index.js';
import { layout } from 'dagre-d3-es/src/dagre/index.js';

// Deterministic PRNG so the random graph is the same on every run.
function mulberry32(seed) {
  let a = seed >>> 0;
  return function () {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// Each case is { options, nodes: [{id, width, height, parent}], edges: [{v, w,
// name, minlen, weight, width, height, labelpos, labeloffset}] }.
function node(id, width, height, parent) {
  const n = { id, width, height };
  if (parent !== undefined) n.parent = parent;
  return n;
}

function edge(v, w, extra = {}) {
  return { v, w, ...extra };
}

function chain(n, width = 40, height = 20) {
  const nodes = [];
  const edges = [];
  for (let i = 0; i < n; i++) {
    nodes.push(node(`n${i}`, width, height));
    if (i > 0) edges.push(edge(`n${i - 1}`, `n${i}`));
  }
  return { nodes, edges };
}

function randomGraph(seed, count, edgeCount) {
  const rnd = mulberry32(seed);
  const nodes = [];
  for (let i = 0; i < count; i++) {
    nodes.push(node(`r${i}`, 20 + Math.floor(rnd() * 80), 15 + Math.floor(rnd() * 40)));
  }
  const edges = [];
  const seen = new Set();
  while (edges.length < edgeCount) {
    const v = Math.floor(rnd() * count);
    const w = Math.floor(rnd() * count);
    if (v === w) continue;
    const key = `${v}->${w}`;
    if (seen.has(key)) continue;
    seen.add(key);
    const e = edge(`r${v}`, `r${w}`);
    if (rnd() < 0.25) {
      e.width = 20 + Math.floor(rnd() * 40);
      e.height = 10 + Math.floor(rnd() * 10);
    }
    edges.push(e);
  }
  return { nodes, edges };
}

function cases() {
  const out = {};

  out.chain = { options: {}, ...chain(4) };

  out.diamond = {
    options: { rankdir: 'TB' },
    nodes: [node('a', 50, 30), node('b', 40, 20), node('c', 60, 40), node('d', 50, 30)],
    edges: [edge('a', 'b'), edge('a', 'c'), edge('b', 'd'), edge('c', 'd')],
  };

  out.fan_out = {
    options: { rankdir: 'TB' },
    nodes: [node('root', 60, 30), ...Array.from({ length: 8 }, (_, i) => node(`leaf${i}`, 30 + 5 * i, 20))],
    edges: Array.from({ length: 8 }, (_, i) => edge('root', `leaf${i}`)),
  };

  out.multi_edges = {
    options: { rankdir: 'TB' },
    nodes: [node('a', 50, 30), node('b', 50, 30), node('c', 50, 30)],
    edges: [
      edge('a', 'b', { name: 'e1' }),
      edge('a', 'b', { name: 'e2', weight: 2 }),
      edge('a', 'b', { name: 'e3', width: 30, height: 12 }),
      edge('b', 'c', { name: 'e4' }),
      edge('b', 'c', { name: 'e5', minlen: 2 }),
    ],
  };

  out.cycles = {
    options: { rankdir: 'TB' },
    nodes: ['a', 'b', 'c', 'd', 'e'].map((id) => node(id, 40, 20)),
    edges: [edge('a', 'b'), edge('b', 'c'), edge('c', 'a'), edge('c', 'd'), edge('d', 'e'), edge('e', 'b'), edge('e', 'e')],
  };

  out.cycles_greedy = { ...out.cycles, options: { rankdir: 'TB', acyclicer: 'greedy' } };

  out.disconnected = {
    options: { rankdir: 'TB' },
    nodes: [...chain(3).nodes, node('x', 70, 50), node('y', 20, 20), node('lonely', 30, 30)],
    edges: [...chain(3).edges, edge('x', 'y')],
  };

  out.varied_sizes = {
    options: { rankdir: 'TB' },
    nodes: [node('a', 120, 20), node('b', 10, 90), node('c', 55, 55), node('d', 200, 10), node('e', 5, 5)],
    edges: [edge('a', 'b'), edge('a', 'c'), edge('b', 'd'), edge('c', 'd'), edge('c', 'e')],
  };

  out.edge_labels = {
    options: { rankdir: 'TB' },
    nodes: [node('a', 50, 30), node('b', 50, 30), node('c', 50, 30), node('d', 50, 30)],
    edges: [
      edge('a', 'b', { width: 40, height: 16 }),
      edge('b', 'c', { width: 60, height: 20, labelpos: 'l' }),
      edge('c', 'd', { width: 30, height: 10, labelpos: 'c' }),
      edge('a', 'd', { width: 50, height: 14, labelpos: 'r', labeloffset: 4 }),
    ],
  };

  out.edge_labels_lowercase_tb = { ...out.edge_labels, options: { rankdir: 'tb' } };
  out.edge_labels_default_rankdir = { ...out.edge_labels, options: {} };

  const base = out.varied_sizes;
  out.rankdir_lr = { ...base, options: { rankdir: 'LR' } };
  out.rankdir_bt = { ...base, options: { rankdir: 'BT' } };
  out.rankdir_rl = { ...base, options: { rankdir: 'RL' } };
  out.rankdir_lr_labels = { ...out.edge_labels, options: { rankdir: 'LR' } };

  const alignBase = {
    nodes: [node('a', 30, 20), node('b', 80, 20), node('c', 30, 20), node('d', 30, 20), node('e', 30, 20), node('f', 30, 20)],
    edges: [edge('a', 'b'), edge('a', 'c'), edge('b', 'd'), edge('c', 'd'), edge('c', 'e'), edge('e', 'f'), edge('b', 'f')],
  };
  for (const align of ['UL', 'UR', 'DL', 'DR']) {
    out[`align_${align.toLowerCase()}`] = { ...alignBase, options: { rankdir: 'TB', align } };
  }

  out.compound = {
    options: { rankdir: 'TB' },
    nodes: [
      node('outer', 0, 0),
      node('inner', 0, 0),
      node('a', 40, 20),
      node('b', 40, 20, 'outer'),
      node('c', 40, 20, 'inner'),
      node('d', 40, 20, 'inner'),
      node('e', 40, 20, 'outer'),
      node('f', 40, 20),
    ],
    edges: [edge('a', 'b'), edge('b', 'c'), edge('c', 'd'), edge('d', 'e'), edge('e', 'f'), edge('a', 'f'), edge('b', 'e')],
  };
  out.compound.nodes[1].parent = 'outer';

  out.compound_lr = { ...out.compound, options: { rankdir: 'LR' } };

  out.compound_two_clusters = {
    options: { rankdir: 'TB' },
    nodes: [
      node('c1', 0, 0),
      node('c2', 0, 0),
      node('a', 40, 20, 'c1'),
      node('b', 40, 20, 'c1'),
      node('c', 40, 20, 'c2'),
      node('d', 40, 20, 'c2'),
      node('x', 40, 20),
    ],
    edges: [edge('x', 'a'), edge('x', 'c'), edge('a', 'b'), edge('c', 'd'), edge('b', 'd', { width: 30, height: 12 })],
  };

  out.ranker_tight_tree = { ...out.varied_sizes, options: { rankdir: 'TB', ranker: 'tight-tree' } };
  out.ranker_longest_path = { ...out.varied_sizes, options: { rankdir: 'TB', ranker: 'longest-path' } };

  out.large_seps = {
    ...out.diamond,
    options: { rankdir: 'TB', nodesep: 120, ranksep: 200, edgesep: 40, marginx: 15, marginy: 25 },
  };

  out.numeric_ids = {
    options: { rankdir: 'TB' },
    nodes: [node('b', 40, 20), node('10', 40, 20), node('a', 40, 20), node('2', 40, 20), node('1', 40, 20)],
    edges: [edge('b', '10'), edge('b', 'a'), edge('10', '2'), edge('a', '2'), edge('a', '1'), edge('2', '1')],
  };

  out.random60 = { options: { rankdir: 'TB' }, ...randomGraph(42, 60, 90) };

  return out;
}

function buildGraph(spec) {
  const g = new Graph({ multigraph: true, compound: true });
  g.setGraph({ ...spec.options });
  g.setDefaultEdgeLabel(() => ({}));
  for (const n of spec.nodes) {
    g.setNode(n.id, { width: n.width, height: n.height });
  }
  for (const n of spec.nodes) {
    if (n.parent !== undefined) g.setParent(n.id, n.parent);
  }
  for (const e of spec.edges) {
    const { v, w, name, ...label } = e;
    g.setEdge(v, w, label, name);
  }
  return g;
}

function dump(spec) {
  const g = buildGraph(spec);
  layout(g);
  const nodes = {};
  for (const v of g.nodes()) {
    const n = g.node(v);
    nodes[v] = { x: n.x, y: n.y };
    if (g.children(v).length) {
      nodes[v].width = n.width;
      nodes[v].height = n.height;
    }
  }
  const edges = {};
  for (const e of g.edges()) {
    const label = g.edge(e);
    const key = `${e.v}\u0001${e.w}\u0001${e.name === undefined ? '' : e.name}`;
    const out = { points: label.points };
    if (Object.prototype.hasOwnProperty.call(label, 'x')) {
      out.x = label.x;
      out.y = label.y;
    }
    edges[key] = out;
  }
  const { width, height } = g.graph();
  return { ...spec, out: { nodes, edges, graph: { width, height } } };
}

const self = fileURLToPath(import.meta.url);
const args = process.argv.slice(2);

if (args[0] === '--one') {
  const [, name, outDir] = args;
  const spec = cases()[name];
  writeFileSync(join(outDir, `${name}.json`), JSON.stringify(dump(spec), null, 2) + '\n');
} else {
  const outDir = args[0];
  if (!outDir) {
    console.error('usage: node dagre-dump.mjs <out-dir>');
    process.exit(2);
  }
  mkdirSync(outDir, { recursive: true });
  const names = Object.keys(cases());
  for (const name of names) {
    execFileSync(process.execPath, [self, '--one', name, outDir], { stdio: 'inherit' });
  }
  console.log(`wrote ${names.length} goldens to ${outDir}`);
}
