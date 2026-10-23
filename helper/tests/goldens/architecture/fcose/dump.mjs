// Runs cytoscape-fcose 2.2.0 (cytoscape 3.34.4, cose-base 2.2.0,
// layout-base 2.0.1, the versions mermaid 12.2.0 bundles) on every
// `<name>.spec.json` in this directory and writes `<name>.expected.json`
// for `helper/tests/fcose.rs`.
//
//   npm ci --prefix helper/oracle
//   node helper/tests/goldens/architecture/fcose/dump.mjs
//
// A spec is the graph mermaid's architecture renderer hands to fcose: the
// nodes in `cy.add` order (groups, services, junctions), the edges with
// their ideal length and elasticity, the alignment and relative placement
// constraints, and the seed. The script runs the layout the way
// `layoutArchitecture` does: every node starts at the origin, the first
// run's `layoutstop` starts a second run from the first run's positions,
// and each run executes inside `withSeededRandom(seed, ...)`
// (`architectureSeed.ts`, a mulberry32 swapped in for `Math.random`).
// cytoscape runs headless here, so `eles.boundingBox()` (the centre fcose
// relocates the result to) leaves labels out; the script records that
// centre for each run and the Rust test passes it back in.

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const oracle = resolve(here, "..", "..", "..", "..", "oracle");
const require = createRequire(join(oracle, "package.json"));
const cytoscape = require("cytoscape");
const fcose = require("cytoscape-fcose");
cytoscape.use(fcose);

// `withSeededRandom` from mermaid 12.2.0 `architectureSeed.ts`.
function withSeededRandom(seed, fn) {
  if (seed === 0) {
    return fn();
  }
  const original = Math.random;
  let state = seed >>> 0;
  Math.random = function () {
    state = (state + 1831565813) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  try {
    return fn();
  } finally {
    Math.random = original;
  }
}

function runSpec(spec) {
  const cy = cytoscape({
    headless: true,
    styleEnabled: true,
    style: [
      { selector: "node", style: { "compound-sizing-wrt-labels": "include" } },
      { selector: ".node-leaf", style: { width: "data(width)", height: "data(height)" } },
      { selector: ".node-group", style: { padding: `${spec.padding}px` } },
    ],
  });
  for (const n of spec.nodes) {
    const data = { id: n.id, parent: n.parent ?? undefined, width: n.width, height: n.height };
    cy.add({ group: "nodes", data, classes: n.group ? "node-group" : "node-leaf" });
  }
  for (const e of spec.edges) {
    cy.add({ group: "edges", data: { id: e.id, source: e.source, target: e.target } });
  }
  const ideal = new Map(spec.edges.map((e) => [e.id, e.idealLength]));
  const elasticity = new Map(spec.edges.map((e) => [e.id, e.elasticity]));
  const center = () => {
    const bb = cy.elements().boundingBox();
    return [bb.x1 + bb.w / 2, bb.y1 + bb.h / 2];
  };
  const positions = () => {
    const out = {};
    cy.nodes().forEach((n) => {
      if (!n.isParent()) {
        out[n.id()] = [n.position("x"), n.position("y")];
      }
    });
    return out;
  };
  const runs = [];
  const layout = cy.layout({
    name: "fcose",
    quality: "proof",
    randomize: false,
    nodeSeparation: spec.nodeSeparation,
    numIter: spec.numIter,
    animate: false,
    nodeDimensionsIncludeLabels: false,
    idealEdgeLength: (edge) => ideal.get(edge.id()),
    edgeElasticity: (edge) => elasticity.get(edge.id()),
    alignmentConstraint: spec.alignmentConstraint,
    relativePlacementConstraint: spec.relativePlacementConstraint,
  });
  layout.one("layoutstop", () => {
    runs[0].positions = positions();
    runs.push({ center: center() });
    withSeededRandom(spec.seed, () => layout.run());
  });
  runs.push({ center: center() });
  withSeededRandom(spec.seed, () => layout.run());
  runs[runs.length - 1].positions = positions();
  return { runs };
}

for (const file of readdirSync(here).sort()) {
  if (!file.endsWith(".spec.json")) {
    continue;
  }
  const spec = JSON.parse(readFileSync(join(here, file), "utf8"));
  const result = runSpec(spec);
  const out = join(here, file.replace(".spec.json", ".expected.json"));
  writeFileSync(out, JSON.stringify(result, null, 2) + "\n");
  console.log(`fcose: ${out}`);
}
// Headless cytoscape keeps a timer alive.
process.exit(0);
