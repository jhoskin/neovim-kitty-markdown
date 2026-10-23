//! The marker table matches `markers.js` of mermaid 12.2.0: every key of its
//! `markers` object is either ported or listed as neo-only, and every ported
//! function creates the same marker ids.

use std::collections::BTreeSet;

use kitty_markdown_helper::mermaid::scene::markers::{
    MarkerType, NEO_ONLY, insert_markers, marker_id, markers_for, stylesheet_rule,
};
use kitty_markdown_helper::mermaid::scene::{Color, Element, MarkerUnits, Paint};

/// The keys of `const markers = { ... }` at the bottom of `markers.js`, in
/// source order.
const MARKERS_JS_KEYS: [&str; 23] = [
    "extension",
    "composition",
    "aggregation",
    "dependency",
    "lollipop",
    "point",
    "circle",
    "cross",
    "barb",
    "barbNeo",
    "hierarchy",
    "only_one",
    "zero_or_one",
    "one_or_more",
    "zero_or_more",
    "only_one_neo",
    "zero_or_one_neo",
    "one_or_more_neo",
    "zero_or_more_neo",
    "requirement_arrow",
    "requirement_contains",
    "requirement_arrow_neo",
    "requirement_contains_neo",
];

#[test]
fn every_markers_js_key_is_ported_or_neo_only() {
    let ported: BTreeSet<&str> = MarkerType::ALL.iter().map(|m| m.name()).collect();
    let neo: BTreeSet<&str> = NEO_ONLY.iter().copied().collect();
    for key in MARKERS_JS_KEYS {
        let is_neo = key.ends_with("_neo") || key == "barbNeo";
        if is_neo {
            assert!(
                neo.contains(key) || key.starts_with("requirement_"),
                "{key} is a neo marker that NEO_ONLY does not list"
            );
            assert!(!ported.contains(key), "{key} is neo-only but ported");
        } else {
            assert!(ported.contains(key), "{key} from markers.js is not ported");
            assert_eq!(MarkerType::from_name(key).map(|m| m.name()), Some(key));
        }
    }
    let all: BTreeSet<&str> = MARKERS_JS_KEYS.iter().copied().collect();
    for name in ported.iter().chain(neo.iter()) {
        assert!(all.contains(name), "{name} is not a key of markers.js");
    }
}

/// The marker ids each function appends, in `markers.js` order.
fn expected_ids(name: &str) -> Vec<&'static str> {
    match name {
        "extension" => vec![
            "extensionStart",
            "extensionEnd",
            "extensionStart-margin",
            "extensionEnd-margin",
        ],
        "composition" => vec![
            "compositionStart",
            "compositionEnd",
            "compositionStart-margin",
            "compositionEnd-margin",
        ],
        "aggregation" => vec![
            "aggregationStart",
            "aggregationEnd",
            "aggregationStart-margin",
            "aggregationEnd-margin",
        ],
        "dependency" => vec![
            "dependencyStart",
            "dependencyEnd",
            "dependencyStart-margin",
            "dependencyEnd-margin",
        ],
        "lollipop" => vec![
            "lollipopStart",
            "lollipopEnd",
            "lollipopStart-margin",
            "lollipopEnd-margin",
        ],
        "point" => vec![
            "pointEnd",
            "pointStart",
            "pointEnd-margin",
            "pointStart-margin",
        ],
        "circle" => vec![
            "circleEnd",
            "circleStart",
            "circleEnd-margin",
            "circleStart-margin",
        ],
        "cross" => vec![
            "crossEnd",
            "crossStart",
            "crossEnd-margin",
            "crossStart-margin",
        ],
        "barb" => vec!["barbEnd"],
        "hierarchy" => vec!["hierarchyEnd", "hierarchyStart"],
        "only_one" => vec!["onlyOneStart", "onlyOneEnd"],
        "zero_or_one" => vec!["zeroOrOneStart", "zeroOrOneEnd"],
        "one_or_more" => vec!["oneOrMoreStart", "oneOrMoreEnd"],
        "zero_or_more" => vec!["zeroOrMoreStart", "zeroOrMoreEnd"],
        "requirement_arrow" => vec!["requirement_arrowEnd"],
        "requirement_contains" => vec!["requirement_containsStart"],
        other => panic!("no expectation for {other}"),
    }
}

#[test]
fn marker_ids_match_markers_js() {
    for ty in MarkerType::ALL {
        let markers = markers_for(ty, "classDiagram", "mermaid-1");
        let ids: Vec<String> = markers.iter().map(|m| m.id.clone()).collect();
        let expected: Vec<String> = expected_ids(ty.name())
            .into_iter()
            .map(|n| marker_id("mermaid-1", "classDiagram", n))
            .collect();
        assert_eq!(ids, expected, "{}", ty.name());
        for m in &markers {
            assert!(m.id.starts_with("mermaid-1_classDiagram-"));
            assert!(!m.children.is_empty(), "{} has no shape", m.id);
        }
    }
}

#[test]
fn geometry_spot_checks() {
    let point = markers_for(MarkerType::Point, "flowchart-v2", "s");
    let end = &point[0];
    assert_eq!((end.ref_x, end.ref_y), (5.0, 5.0));
    assert_eq!((end.marker_width, end.marker_height), (8.0, 8.0));
    assert_eq!(end.marker_units, MarkerUnits::UserSpaceOnUse);
    let vb = end.view_box.expect("pointEnd has a viewBox");
    assert_eq!((vb.width, vb.height), (10.0, 10.0));
    let Element::Path(p) = &end.children[0] else {
        panic!("pointEnd is a path");
    };
    assert_eq!(p.d.to_svg(), "M0,0L10,5L0,10Z");
    assert_eq!(p.common.style.stroke_dasharray, Some(vec![1.0, 0.0]));

    let start_margin = &point[3];
    assert_eq!(start_margin.ref_x, 1.0);
    assert_eq!(start_margin.marker_width, 11.5);
    assert!(matches!(start_margin.children[0], Element::Polygon(_)));

    let cross = markers_for(MarkerType::Cross, "flowchart-v2", "s");
    assert_eq!((cross[0].ref_x, cross[0].ref_y), (12.0, 5.2));
    assert_eq!(cross[2].ref_x, 17.7);

    let er = markers_for(MarkerType::ZeroOrMore, "er", "s");
    assert_eq!(er[0].marker_units, MarkerUnits::StrokeWidth);
    assert_eq!((er[0].marker_width, er[0].marker_height), (57.0, 36.0));
    assert_eq!(er[1].ref_x, 39.0);
    let Element::Path(crow) = &er[1].children[1] else {
        panic!("zeroOrMoreEnd ends with the crow path");
    };
    assert_eq!(crow.d.to_svg(), "M21,18Q39,0,57,18Q39,36,21,18");

    let composition = markers_for(MarkerType::Composition, "classDiagram", "s");
    assert_eq!(
        (composition[0].marker_width, composition[0].marker_height),
        (190.0, 240.0)
    );
    assert_eq!(composition[3].ref_x, 3.5);
    let aggregation = markers_for(MarkerType::Aggregation, "classDiagram", "s");
    assert_eq!(aggregation[3].ref_x, 1.0);
}

#[test]
fn insert_markers_concatenates_in_order() {
    let all = insert_markers(
        &[MarkerType::Point, MarkerType::Circle, MarkerType::Cross],
        "flowchart-v2",
        "g",
    );
    assert_eq!(all.len(), 12);
    assert_eq!(all[0].id, "g_flowchart-v2-pointEnd");
    assert_eq!(all[4].id, "g_flowchart-v2-circleEnd");
    assert_eq!(all[11].id, "g_flowchart-v2-crossStart-margin");
}

#[test]
fn class_stylesheet_rules_fill_the_solid_markers_only() {
    let line = Color::rgb(0x33, 0x33, 0x33);
    let bkg = Color::rgb(0xEC, 0xEC, 0xFF);
    let fill = |ty| stylesheet_rule("classDiagram", ty, line, bkg).and_then(|s| s.fill);
    assert_eq!(fill(MarkerType::Composition), Some(Paint::Color(line)));
    assert_eq!(fill(MarkerType::Dependency), Some(Paint::Color(line)));
    assert_eq!(fill(MarkerType::Extension), Some(Paint::None));
    assert_eq!(fill(MarkerType::Aggregation), Some(Paint::None));
    assert_eq!(fill(MarkerType::Lollipop), Some(Paint::Color(bkg)));
    assert!(stylesheet_rule("flowchart-v2", MarkerType::Point, line, bkg).is_none());

    // The rule paints the hollow arrowhead's path; the lollipop circle keeps
    // its own `fill="transparent"`.
    let rule = stylesheet_rule("classDiagram", MarkerType::Extension, line, bkg).expect("rule");
    let mut ext = markers_for(MarkerType::Extension, "classDiagram", "s").remove(1);
    ext.apply_rule(&rule);
    assert_eq!(ext.style.stroke_width, Some(1.0));
    let Element::Path(head) = &ext.children[0] else {
        panic!("extensionEnd is a path");
    };
    assert_eq!(head.common.style.fill, Some(Paint::None));
    assert_eq!(head.common.style.stroke, Some(Paint::Color(line)));
    let rule = stylesheet_rule("classDiagram", MarkerType::Lollipop, line, bkg).expect("rule");
    let mut lollipop = markers_for(MarkerType::Lollipop, "classDiagram", "s").remove(1);
    lollipop.apply_rule(&rule);
    let Element::Circle(circle) = &lollipop.children[0] else {
        panic!("lollipopEnd is a circle");
    };
    assert_eq!(circle.common.style.fill, Some(Paint::None));
}
