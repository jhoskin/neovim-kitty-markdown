//! The d3-sankey port against the real d3-sankey 0.12.3: node boxes and
//! link breadths for four graphs, to 1e-9. The expected values come from
//! node running `sankey()` from `helper/oracle/node_modules/d3-sankey` with
//! `nodeId(d => d.id)`, `nodeWidth(10)`, and the padding, alignment, and
//! extent each graph lists, as `JSON.stringify` printed them.

use kitty_markdown_helper::mermaid::layout::d3::sankey::{Align, LinkInput, Sankey};
use serde_json::Value;

const EXPECTED: &str = r#"{
    "chain": {
      "nodes": [
        [0, 10, 7.949717273358914e-14, 400, 0],
        [295, 305, 7.700931270310023e-14, 400.00000000000006, 1],
        [590, 600, 0.0003464897306242028, 240.00034648973062, 2]
      ],
      "links": [
        [200.00000000000009, 200.00000000000009, 400],
        [120.00000000000007, 120.00034648973062, 240]
      ]
    },
    "fan": {
      "nodes": [
        [0, 10, 0, 179.8260869565219, 0],
        [0, 10, 191.8260869565219, 388.0000000000001, 0],
        [295, 305, 34.20255169093243, 361.15907343006285, 1],
        [590, 600, -2.1316282072803006e-14, 65.39130434782608, 2],
        [590, 600, 77.39130434782608, 240.8695652173913, 2],
        [590, 600, 252.8695652173913, 400, 2]
      ],
      "links": [
        [65.3913043478261, 99.59385603875853, 130.7826086956522],
        [289.91304347826105, 263.0721169083238, 196.17391304347828],
        [66.89820386484547, 32.695652173913025, 65.3913043478261],
        [181.33298647354115, 159.1304347826087, 163.47826086956525],
        [312.11559516919334, 350.95652173913044, 98.08695652173914],
        [155.30434782608697, 277.3913043478261, 49.04347826086957]
      ]
    },
    "skip": {
      "nodes": [
        [0, 10, 0, 160, 0],
        [163.33333333333334, 173.33333333333334, 49.60574024705282, 169.60574024705286, 1],
        [326.6666666666667, 336.6666666666667, 0, 119.99999999999996, 2],
        [490, 500, 10.000000000000059, 300, 3],
        [326.6666666666667, 336.6666666666667, 139.99999999999994, 300, 2]
      ],
      "links": [
        [100, 109.60574024705282, 120],
        [89.60574024705282, 80, 80],
        [20, 20, 40],
        [50, 60.00000000000006, 100],
        [219.99999999999994, 220.00000000000006, 160],
        [144.60574024705284, 125.00000000000006, 30]
      ]
    },
    "energy": {
      "nodes": [
        [0, 10, 46.67023014214739, 121.11148133953708, 0],
        [295, 305, 47.88883733461057, 280.00858191642214, 1],
        [590, 600, 0, 21.245141217146614, 2],
        [590, 600, 48.24514121714661, 64.27702545949829, 2],
        [590, 600, 91.27702545949825, 324.5713195234389, 2],
        [590, 600, 351.5713195234389, 400, 2],
        [295, 305, 0, 20.88883733461057, 1],
        [295, 305, 307.00858191642214, 327.8974192510326, 1],
        [0, 10, 327.8974192510328, 334.8241577111896, 0],
        [295, 305, 354.8974192510326, 399.9999999999999, 1],
        [0, 10, 361.8241577111896, 400, 0]
      ],
      "links": [
        [83.89085574084223, 85.10946293330541, 74.44125119738968],
        [48.066989275878605, 21.06698927587858, 0.35630388253607126],
        [56.26108333832247, 56.26108333832244, 16.03188424235167],
        [147.9284634496797, 174.92846344967964, 167.30287598036278],
        [255.79424167814165, 375.78565976171944, 48.42868047656109],
        [10.444418667305273, 10.444418667305273, 20.888837334610546],
        [317.45300058372743, 269.0243201071663, 20.888837334610546],
        [331.36078848111123, 358.36078848111106, 6.926738460156857],
        [380.9120788555948, 380.91207885559464, 38.17584228881039],
        [377.44870962551624, 302.02002914895525, 45.10258074896724]
      ]
    }
}"#;

struct Case {
    name: &'static str,
    align: Align,
    width: f64,
    height: f64,
    padding: f64,
    nodes: &'static [&'static str],
    links: &'static [(&'static str, &'static str, f64)],
}

const CASES: &[Case] = &[
    Case {
        name: "chain",
        align: Align::Justify,
        width: 600.0,
        height: 400.0,
        padding: 27.0,
        nodes: &["a", "b", "c"],
        links: &[("a", "b", 5.0), ("b", "c", 3.0)],
    },
    Case {
        name: "fan",
        align: Align::Left,
        width: 600.0,
        height: 400.0,
        padding: 12.0,
        nodes: &["s1", "s2", "hub", "t1", "t2", "t3"],
        links: &[
            ("s1", "hub", 4.0),
            ("s2", "hub", 6.0),
            ("hub", "t1", 2.0),
            ("hub", "t2", 5.0),
            ("hub", "t3", 3.0),
            ("s1", "t3", 1.5),
        ],
    },
    Case {
        name: "skip",
        align: Align::Center,
        width: 500.0,
        height: 300.0,
        padding: 20.0,
        nodes: &["a", "b", "c", "d", "e"],
        links: &[
            ("a", "b", 3.0),
            ("b", "c", 2.0),
            ("a", "c", 1.0),
            ("c", "d", 2.5),
            ("e", "d", 4.0),
            ("b", "d", 0.75),
        ],
    },
    Case {
        name: "energy",
        align: Align::Right,
        width: 600.0,
        height: 400.0,
        padding: 27.0,
        nodes: &[
            "Agricultural 'waste'",
            "Bio-conversion",
            "Liquid",
            "Losses",
            "Solid",
            "Gas",
            "Biofuel imports",
            "Biomass imports",
            "Coal imports",
            "Coal",
            "Coal reserves",
        ],
        links: &[
            ("Agricultural 'waste'", "Bio-conversion", 124.729),
            ("Bio-conversion", "Liquid", 0.597),
            ("Bio-conversion", "Losses", 26.862),
            ("Bio-conversion", "Solid", 280.322),
            ("Bio-conversion", "Gas", 81.144),
            ("Biofuel imports", "Liquid", 35.0),
            ("Biomass imports", "Solid", 35.0),
            ("Coal imports", "Coal", 11.606),
            ("Coal reserves", "Coal", 63.965),
            ("Coal", "Solid", 75.571),
        ],
    },
];

fn close(label: &str, got: f64, want: &Value) {
    let want = want.as_f64().expect("a number");
    assert!(
        (got - want).abs() <= 1e-9,
        "{label}: got {got}, d3-sankey {want}"
    );
}

#[test]
fn layout_matches_d3_sankey() {
    let expected: Value = serde_json::from_str(EXPECTED).expect("expected JSON");
    for case in CASES {
        let ids: Vec<String> = case.nodes.iter().map(|s| s.to_string()).collect();
        let links: Vec<LinkInput> = case
            .links
            .iter()
            .map(|&(source, target, value)| LinkInput {
                source: source.to_string(),
                target: target.to_string(),
                value,
            })
            .collect();
        let graph = Sankey::new()
            .node_width(10.0)
            .node_padding(case.padding)
            .node_align(case.align)
            .extent(0.0, 0.0, case.width, case.height)
            .layout(&ids, &links)
            .expect("layout");
        let want = &expected[case.name];
        for (i, node) in graph.nodes.iter().enumerate() {
            let w = &want["nodes"][i];
            let label = format!("{} node {}", case.name, node.id);
            close(&format!("{label} x0"), node.x0, &w[0]);
            close(&format!("{label} x1"), node.x1, &w[1]);
            close(&format!("{label} y0"), node.y0, &w[2]);
            close(&format!("{label} y1"), node.y1, &w[3]);
            assert_eq!(Some(node.layer as u64), w[4].as_u64(), "{label} layer");
        }
        for (i, link) in graph.links.iter().enumerate() {
            let w = &want["links"][i];
            let label = format!("{} link {i}", case.name);
            close(&format!("{label} y0"), link.y0, &w[0]);
            close(&format!("{label} y1"), link.y1, &w[1]);
            close(&format!("{label} width"), link.width, &w[2]);
        }
    }
}
