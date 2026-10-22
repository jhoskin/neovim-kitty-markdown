//! Every vendored grammar must load: valid JSON, every lexer regex
//! compiles, every lexer action translates. A grammar bump that brings a
//! new action idiom fails here, not at first use in a preview.

use kitty_markdown_helper::mermaid::jison::{grammar, grammar_names, tables};

#[test]
fn all_vendored_grammars_load() {
    let mut failures = Vec::new();
    for name in grammar_names() {
        if let Err(e) = grammar(name) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(
        failures.is_empty(),
        "grammars failed to load:\n{}",
        failures.join("\n")
    );
    assert_eq!(grammar_names().count(), 18);
}

#[test]
fn tables_are_from_the_pinned_tools() {
    for name in grammar_names() {
        let g = grammar(name).expect("loads");
        assert_eq!(g.tables.grammar, name);
        assert_eq!(g.tables.mermaid_version, "12.2.0", "{name}");
        assert_eq!(g.tables.jison_version, "0.4.18", "{name}");
        assert_eq!(g.tables.symbol("$accept"), Some(0), "{name}");
        assert_eq!(g.tables.symbol("$end"), Some(1), "{name}");
        assert_eq!(g.tables.symbol("error"), Some(2), "{name}");
        // jison builds `terminals_` before it augments the grammar, so
        // `$end` has no name there and an error at end of input says
        // `got '1'`.
        assert_eq!(g.tables.terminal_name(1), None, "{name}");
        assert_eq!(g.tables.terminal_name(2), Some("error"), "{name}");
        assert_eq!(
            g.tables.production_sizes.len(),
            g.tables.productions.len(),
            "{name}"
        );
        assert_eq!(g.rules.len(), g.tables.lexer.rules.len(), "{name}");
        assert!(g.tables.lexer.conditions.contains_key("INITIAL"), "{name}");
        // Every start condition names rules that exist.
        for (cond, spec) in &g.tables.lexer.conditions {
            for r in &spec.rules {
                assert!(
                    *r < g.rules.len(),
                    "{name}: condition {cond} names rule {r}"
                );
            }
        }
        // Every table cell is a shift, reduce, accept or goto in range.
        for (state, row) in g.tables.table.iter().enumerate() {
            for (symbol, entry) in row {
                match entry {
                    tables::Entry::Goto(s) => {
                        assert!(
                            (*s as usize) < g.tables.table.len(),
                            "{name} state {state} goto {s}"
                        );
                        assert!(
                            g.tables.terminal_name(*symbol).is_none(),
                            "{name}: goto on a terminal"
                        );
                    }
                    tables::Entry::Action(a) => match a.as_slice() {
                        [1, s] => assert!((*s as usize) < g.tables.table.len(), "{name} shift {s}"),
                        [2, p] => assert!(
                            (*p as usize) < g.tables.productions.len(),
                            "{name} reduce {p}"
                        ),
                        [3] => {}
                        other => panic!("{name} state {state}: bad action {other:?}"),
                    },
                }
            }
        }
    }
}

#[test]
fn unknown_grammar_is_an_error() {
    assert!(grammar("pie").is_err());
}
