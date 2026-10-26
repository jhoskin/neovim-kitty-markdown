//! Ordered diagram type detection, in mermaid 12.2.0 detector order.
//!
//! Port of `detectType` (`diagram-api/detectType.ts`) with the detector
//! table that `addDiagrams` (`diagram-api/diagram-orchestration.ts`)
//! registers in the full `mermaid.esm.mjs` bundle mermaid-cli loads: the
//! `error` and `---` pseudo diagrams, then the three large features
//! (`flowchart-elk`, `mindmap`, `architecture`), then the main list. The
//! first detector that matches wins, so `graph` text with a `gitGraph`
//! keyword later on is still a flowchart, and `stateDiagram-v2` is
//! `stateDiagram` because that detector's regex is a prefix match.
//! mermaid-cli then calls `registerExternalDiagrams([zenuml])`
//! (`src/index.js`), which adds the zenuml detector of
//! `@mermaid-js/mermaid-zenuml` after all of these.

use std::fmt;
use std::sync::LazyLock;

use regex::Regex;

use super::directive::{js_trim, js_trim_start, remove_directives};
use super::preprocess::{match_front_matter, strip_any_comments};
use super::theme::khroma::JS_WHITESPACE;

/// The ids mermaid's detectors answer with, in detection order, minus the
/// `error` and `---` pseudo diagrams.
pub fn all_types() -> &'static [&'static str] {
    &[
        "flowchart-elk",
        "mindmap",
        "architecture",
        "agentflow",
        "c4",
        "kanban",
        "classDiagram",
        "er",
        "gantt",
        "info",
        "pie",
        "requirement",
        "sequence",
        "swimlane",
        "flowchart-v2",
        "timeline",
        "gitGraph",
        "stateDiagram",
        "journey",
        "quadrantChart",
        "sankey",
        "packet",
        "xychart",
        "block",
        "eventmodeling",
        "treeView",
        "radar",
        "ishikawa",
        "treemap",
        "railroad",
        "railroadEbnf",
        "railroadAbnf",
        "railroadPeg",
        "venn",
        "wardley",
        "cynefin",
        "usecase",
        "zenuml",
    ]
}

/// The diagram ids whose renderers the helper does not ship (R-176).
pub const UNSUPPORTED_TYPES: &[&str] = &[
    "zenuml",
    "info",
    "agentflow",
    "railroad",
    "railroadEbnf",
    "railroadAbnf",
    "railroadPeg",
];

/// `error` is the pseudo diagram for the text `error`; `---` stands for a
/// front matter block that never closed.
pub const ERROR_TYPE: &str = "error";
pub const FRONT_MATTER_TYPE: &str = "---";

/// The message the `---` pseudo diagram's parser throws.
pub const FRONT_MATTER_ERROR: &str = "Diagrams beginning with --- are not valid. If you were trying to use a YAML front-matter, please ensure that you've correctly opened and closed the YAML front-matter with un-indented `---` blocks";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectError {
    /// `UnknownDiagramError`; the message carries the cleaned text as
    /// mermaid prints it.
    Unknown(String),
}

impl fmt::Display for DetectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DetectError::Unknown(text) => write!(
                f,
                "No diagram type detected matching given configuration for text: {text}"
            ),
        }
    }
}

impl std::error::Error for DetectError {}

/// `^\s*` with JS whitespace, the prefix of nearly every detector.
fn re(pattern: &str) -> Regex {
    Regex::new(&pattern.replace(r"\s", JS_WHITESPACE)).expect("valid detector regex")
}

static DETECTORS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    vec![
        ("flowchart-elk", re(r"^\s*flowchart-elk")),
        ("mindmap", re(r"^\s*mindmap")),
        ("architecture", re(r"^\s*architecture")),
        ("agentflow", re(r"^\s*agentflow-beta(?-u:\b)")),
        // Alternation without grouping, as written in `c4Detector.ts`: only
        // `C4Context` is anchored.
        (
            "c4",
            re(r"^\s*C4Context|C4Container|C4Component|C4Dynamic|C4Deployment"),
        ),
        ("kanban", re(r"^\s*kanban")),
        ("classDiagram", re(r"^\s*classDiagram")),
        ("er", re(r"^\s*erDiagram")),
        ("gantt", re(r"^\s*gantt")),
        ("info", re(r"^\s*info")),
        ("pie", re(r"^\s*pie")),
        ("requirement", re(r"^\s*requirement(Diagram)?")),
        ("sequence", re(r"^\s*sequenceDiagram")),
        ("swimlane", re(r"^\s*swimlane-beta(?-u:\b)")),
        ("flowchart-v2", re(r"^\s*(graph|flowchart)")),
        ("timeline", re(r"^\s*timeline")),
        ("gitGraph", re(r"^\s*gitGraph")),
        ("stateDiagram", re(r"^\s*stateDiagram")),
        ("journey", re(r"^\s*journey")),
        ("quadrantChart", re(r"^\s*quadrantChart")),
        ("sankey", re(r"^\s*sankey(-beta)?")),
        ("packet", re(r"^\s*packet(-beta)?")),
        ("xychart", re(r"^\s*xychart(-beta)?")),
        ("block", re(r"^\s*block(-beta)?")),
        ("eventmodeling", re(r"^\s*eventmodeling")),
        ("treeView", re(r"^\s*treeView-beta")),
        ("radar", re(r"^\s*radar-beta")),
        ("ishikawa", re(r"(?i)^\s*ishikawa(-beta)?(?-u:\b)")),
        ("treemap", re(r"^\s*treemap")),
        ("railroad", re(r"(?i)^\s*railroad-beta")),
        ("railroadEbnf", re(r"(?i)^\s*railroad-ebnf-beta")),
        ("railroadAbnf", re(r"(?i)^\s*railroad-abnf-beta")),
        ("railroadPeg", re(r"(?i)^\s*railroad-peg-beta")),
        ("venn", re(r"^\s*venn-beta")),
        ("wardley", re(r"(?i)^\s*wardley-beta")),
        ("cynefin", re(r"^\s*cynefin-beta(?:[\s:]|$)")),
        ("usecase", re(r"^\s*usecase-beta(?:\s|$)")),
        // `@mermaid-js/mermaid-zenuml` 1.0.1 `detector.ts`: a prefix match,
        // so `zenumlfoo` is zenuml too.
        ("zenuml", re(r"^\s*zenuml")),
    ]
});

/// The text `detectType` runs the detectors on: front matter sliced off,
/// directives removed, `%%` comments stripped.
pub fn detection_text(text: &str) -> String {
    let after_front_matter = match match_front_matter(text) {
        Some(fm) => &text[fm.length..],
        None => text,
    };
    strip_any_comments(&remove_directives(after_front_matter))
}

/// `detectType(text)`: the id of the first detector that matches, or the
/// `UnknownDiagramError` message.
pub fn detect_type(text: &str) -> Result<&'static str, DetectError> {
    let text = detection_text(text);
    if js_trim(&text).to_lowercase() == "error" {
        return Ok(ERROR_TYPE);
    }
    if js_trim_start(&text.to_lowercase()).starts_with("---") {
        return Ok(FRONT_MATTER_TYPE);
    }
    for (id, regex) in DETECTORS.iter() {
        if regex.is_match(&text) {
            return Ok(id);
        }
    }
    Err(DetectError::Unknown(text))
}

/// `getDiagramConfigKey` (`diagram-api/diagramConfigKeys.ts`): the
/// `MermaidConfig` section a diagram type reads. `info` and `error` have
/// none, and the key is returned as is for them.
pub fn config_key(diagram_type: &str) -> &str {
    match diagram_type {
        "flowchart-v2" | "flowchart-elk" => "flowchart",
        "classDiagram" => "class",
        "stateDiagram" => "state",
        "xychart" => "xyChart",
        "railroadAbnf" | "railroadEbnf" | "railroadPeg" => "railroad",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_common_types() {
        let cases = [
            ("graph TD\nA-->B", "flowchart-v2"),
            ("flowchart LR\nA-->B", "flowchart-v2"),
            ("  flowchart-elk LR\nA-->B", "flowchart-elk"),
            ("sequenceDiagram\nA->>B: x", "sequence"),
            ("classDiagram\nA <|-- B", "classDiagram"),
            ("classDiagram-v2\nA <|-- B", "classDiagram"),
            ("stateDiagram\n[*] --> A", "stateDiagram"),
            ("stateDiagram-v2\n[*] --> A", "stateDiagram"),
            ("erDiagram\nA ||--o{ B : has", "er"),
            ("pie\n\"a\": 1", "pie"),
            ("gantt\ntitle x", "gantt"),
            ("xychart-beta\nx-axis [a]", "xychart"),
            ("xychart\nx-axis [a]", "xychart"),
            ("quadrantChart\nx-axis a --> b", "quadrantChart"),
            ("gitGraph\ncommit", "gitGraph"),
            ("mindmap\n  root", "mindmap"),
            ("timeline\n  2020 : x", "timeline"),
            ("journey\n  title x", "journey"),
            ("sankey-beta\na,b,1", "sankey"),
            ("block-beta\na", "block"),
            ("packet-beta\n0-1: x", "packet"),
            ("kanban\n  a", "kanban"),
            ("architecture-beta\n", "architecture"),
            ("C4Context\n  title x", "c4"),
            ("  C4Container\n", "c4"),
            ("requirementDiagram\n", "requirement"),
            ("radar-beta\n", "radar"),
            ("treemap-beta\n", "treemap"),
            ("venn-beta\n", "venn"),
            ("Ishikawa\n", "ishikawa"),
            ("WARDLEY-BETA\n", "wardley"),
            ("cynefin-beta\n", "cynefin"),
            ("treeView-beta\n", "treeView"),
            ("eventmodeling\n", "eventmodeling"),
            ("usecase-beta\n", "usecase"),
            ("swimlane-beta\n", "swimlane"),
            ("info", "info"),
            ("agentflow-beta\n", "agentflow"),
            ("railroad-beta\n", "railroad"),
            ("railroad-ebnf-beta\n", "railroadEbnf"),
            ("zenuml\n  A.method()", "zenuml"),
            ("  zenuml\n", "zenuml"),
            ("zenumlfoo\n", "zenuml"),
            ("error", "error"),
            ("  Error  ", "error"),
        ];
        for (text, want) in cases {
            assert_eq!(detect_type(text), Ok(want), "{text:?}");
        }
    }

    #[test]
    fn graph_wins_over_gitgraph_only_by_prefix() {
        assert_eq!(detect_type("gitGraph\n  commit"), Ok("gitGraph"));
        assert_eq!(detect_type("graph TD\n  gitGraph"), Ok("flowchart-v2"));
    }

    #[test]
    fn directives_front_matter_and_comments_are_ignored() {
        let text = "---\ntitle: t\n---\n%%{init: {\"theme\": \"dark\"}}%%\n%% a comment\nsequenceDiagram\nA->>B: hi";
        assert_eq!(detect_type(text), Ok("sequence"));
        assert_eq!(
            detect_type("%%{init: {'theme':'dark'}}%%\ngraph TD"),
            Ok("flowchart-v2")
        );
    }

    #[test]
    fn unclosed_front_matter_is_the_pseudo_type() {
        assert_eq!(
            detect_type("---\ntitle: x\n\ngraph TD"),
            Ok(FRONT_MATTER_TYPE)
        );
    }

    #[test]
    fn unknown_text_carries_the_cleaned_text() {
        let err = detect_type("%% c\nfoo bar").unwrap_err();
        assert_eq!(
            err.to_string(),
            "No diagram type detected matching given configuration for text: \nfoo bar"
        );
    }

    #[test]
    fn cynefin_and_usecase_need_a_separator() {
        assert_eq!(detect_type("cynefin-beta\n"), Ok("cynefin"));
        assert!(detect_type("cynefin-betax").is_err());
        assert_eq!(detect_type("usecase-beta"), Ok("usecase"));
    }

    #[test]
    fn config_keys_alias_the_renderer_ids() {
        assert_eq!(config_key("flowchart-v2"), "flowchart");
        assert_eq!(config_key("flowchart-elk"), "flowchart");
        assert_eq!(config_key("classDiagram"), "class");
        assert_eq!(config_key("stateDiagram"), "state");
        assert_eq!(config_key("xychart"), "xyChart");
        assert_eq!(config_key("railroadPeg"), "railroad");
        assert_eq!(config_key("pie"), "pie");
    }

    #[test]
    fn all_types_matches_the_detector_table() {
        let ids: Vec<&str> = DETECTORS.iter().map(|(id, _)| *id).collect();
        assert_eq!(all_types(), ids.as_slice());
    }
}
