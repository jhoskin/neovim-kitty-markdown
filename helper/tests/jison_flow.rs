//! Parity of the jison driver with mermaid's own parser tests.
//!
//! `goldens/jison/flow_valid.txt` holds every input mermaid's flowchart
//! specs parse (extracted by `extract-flow-corpus.mjs`); each must parse.
//! `goldens/jison/errors.json` holds the exact message the pinned mermaid
//! printed for invalid inputs across the grammars (`oracle-errors.mjs`);
//! the Rust `ParseError` must print the same text.

use std::fs;
use std::path::PathBuf;

use kitty_markdown_helper::mermaid::jison::{
    Actions, ErrorKind, Lit, Loc, ParseError, Yy, grammar, parse,
};

/// No-op semantic actions that record the production sequence and answer
/// flow's `yy.lex.firstGraph()` the way FlowDB does: true once.
struct Recorder {
    productions: Vec<usize>,
    first_graph: bool,
}

impl Recorder {
    fn new() -> Recorder {
        Recorder {
            productions: Vec::new(),
            first_graph: true,
        }
    }
}

impl Yy for Recorder {
    fn yy_call(&mut self, name: &str, _args: &[Lit]) -> Lit {
        if name == "lex.firstGraph" {
            let first = self.first_graph;
            self.first_graph = false;
            return Lit::Bool(first);
        }
        Lit::Undefined
    }
}

impl Actions for Recorder {
    type Value = ();

    fn terminal(&mut self, _yytext: &str) {}

    fn reduce(
        &mut self,
        production: usize,
        _rhs: &[()],
        _locs: &[Loc],
        _yytext: &str,
    ) -> Result<(), ParseError> {
        self.productions.push(production);
        Ok(())
    }
}

fn goldens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("goldens")
        .join("jison")
}

fn valid_corpus() -> Vec<String> {
    let text = fs::read_to_string(goldens().join("flow_valid.txt")).expect("flow_valid.txt");
    let text = text.strip_suffix('\n').unwrap_or(&text);
    text.split("\n---\n").map(str::to_string).collect()
}

fn oracle_errors() -> Vec<serde_json::Value> {
    let text = fs::read_to_string(goldens().join("errors.json")).expect("errors.json");
    serde_json::from_str(&text).expect("errors.json is a JSON array")
}

fn is_jison_message(message: &str) -> bool {
    message.starts_with("Parse error on line ") || message.starts_with("Lexical error on line ")
}

#[test]
fn every_valid_flow_input_parses() {
    let flow = grammar("flow").expect("flow grammar loads");
    let corpus = valid_corpus();
    assert!(corpus.len() > 800, "corpus has {} inputs", corpus.len());
    let mut failures = Vec::new();
    for input in &corpus {
        let mut actions = Recorder::new();
        if let Err(e) = parse(flow, input, &mut actions) {
            failures.push(format!("{input:?}\n{e}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} valid inputs failed:\n\n{}",
        failures.len(),
        corpus.len(),
        failures.join("\n\n")
    );
}

#[test]
fn every_invalid_flow_input_is_rejected() {
    let flow = grammar("flow").expect("flow grammar loads");
    let mut checked = 0;
    let mut failures = Vec::new();
    for case in oracle_errors() {
        if case["grammar"] != "flow" {
            continue;
        }
        let Some(message) = case["message"].as_str() else {
            continue;
        };
        if !is_jison_message(message) {
            // The oracle failed in FlowDB (for example an unknown shape),
            // so the grammar itself accepts the input.
            continue;
        }
        let input = case["input"].as_str().expect("input");
        checked += 1;
        let mut actions = Recorder::new();
        match parse(flow, input, &mut actions) {
            Ok(()) => failures.push(format!("accepted: {input:?}")),
            Err(e) if e.kind == ErrorKind::Parse || e.kind == ErrorKind::Lexical => {}
            Err(e) => failures.push(format!("{input:?}: wrong error kind {:?}: {e}", e.kind)),
        }
    }
    assert!(checked >= 20, "only {checked} invalid inputs checked");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn error_text_matches_the_oracle() {
    let mut compared = 0;
    let mut failures = Vec::new();
    for case in oracle_errors() {
        let name = case["grammar"].as_str().expect("grammar");
        let parsed = case["parsed"].as_str().expect("parsed");
        let g = grammar(name).expect("grammar loads");
        let mut actions = Recorder::new();
        let result = parse(g, parsed, &mut actions);
        match case["message"].as_str() {
            Some(message) if is_jison_message(message) => {
                compared += 1;
                match result {
                    Err(e) if e.to_string() == message => {}
                    Err(e) => failures.push(format!(
                        "{name} {parsed:?}\n--- oracle\n{message}\n--- helper\n{e}"
                    )),
                    Ok(()) => failures.push(format!(
                        "{name} {parsed:?}: accepted, oracle said\n{message}"
                    )),
                }
            }
            // A db error or no error at all: the grammar accepts the input.
            _ => {
                if let Err(e) = result {
                    failures.push(format!(
                        "{name} {parsed:?}: rejected, oracle accepted:\n{e}"
                    ));
                }
            }
        }
    }
    assert!(compared >= 12, "only {compared} oracle messages compared");
    assert!(
        failures.is_empty(),
        "{} of {} messages differ:\n\n{}",
        failures.len(),
        compared,
        failures.join("\n\n")
    );
}

#[test]
fn error_fields_carry_line_and_tokens() {
    let flow = grammar("flow").expect("flow grammar loads");
    let mut actions = Recorder::new();
    let e = parse(flow, "graph TD\nA-->\n", &mut actions).expect_err("rejects");
    assert_eq!(e.kind, ErrorKind::Parse);
    assert_eq!(e.line, 3);
    assert_eq!(e.got, "EOF");
    assert!(e.expected.iter().any(|t| t == "NODE_STRING"));
    assert!(e.expected.iter().all(|t| !t.starts_with('\'')));
}

#[test]
fn production_sequence_is_recorded() {
    let flow = grammar("flow").expect("flow grammar loads");
    let mut actions = Recorder::new();
    parse(flow, "graph TD;\nA-->B;", &mut actions).expect("parses");
    let last = *actions.productions.last().expect("reduced something");
    assert_eq!(flow.tables.productions[last].lhs, "start");
    assert!(
        actions
            .productions
            .iter()
            .any(|p| flow.tables.productions[*p].lhs == "link")
    );
}
