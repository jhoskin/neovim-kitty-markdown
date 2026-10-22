//! jison-lex behaviors the driver depends on, checked on small synthetic
//! grammars: start conditions, first match against `%options flex`,
//! `%options case-insensitive`, position bookkeeping and `showPosition`.

use kitty_markdown_helper::mermaid::jison::lexer::{Lexed, Lexer, Loc};
use kitty_markdown_helper::mermaid::jison::{Grammar, LoadError, Token, Yy};

struct NoYy;

impl Yy for NoYy {}

/// A tables file with an empty parse table and the given lexer section.
fn grammar_json(options: &str, conditions: &str, rules: &str) -> String {
    format!(
        r#"{{
  "grammar": "synthetic", "mermaid_version": "12.2.0", "jison_version": "0.4.18",
  "start": "start",
  "symbols_": {{"$accept": 0, "$end": 1, "error": 2, "start": 3}},
  "terminals_": {{"2": "error"}},
  "productions_": [[0, 2], [3, 0]],
  "productions": [{{"lhs": "$accept", "rhs": ["start", "$end"], "action": null}},
                  {{"lhs": "start", "rhs": [], "action": null}}],
  "table": [{{"3": 1}}, {{"1": [3]}}],
  "defaultActions": {{}},
  "lexer": {{
    "options": {options},
    "macros": {{}},
    "action_include": null,
    "conditions": {conditions},
    "rules": {rules}
  }}
}}"#
    )
}

fn load(options: &str, conditions: &str, rules: &str) -> Grammar {
    Grammar::load("synthetic", &grammar_json(options, conditions, rules))
        .expect("synthetic grammar loads")
}

fn rule(index: usize, pattern: &str, conditions: Option<&[&str]>, action: &str) -> String {
    let conditions = match conditions {
        None => "null".to_string(),
        Some(c) => format!(
            "[{}]",
            c.iter()
                .map(|s| format!("{s:?}"))
                .collect::<Vec<_>>()
                .join(",")
        ),
    };
    format!(
        r#"{{"index": {index}, "pattern": {pattern:?}, "conditions": {conditions}, "action": {action:?}}}"#
    )
}

fn tokens(grammar: &Grammar, input: &str) -> Vec<String> {
    tokens_from(&mut Lexer::new(grammar, input))
}

/// `NAME=yytext` for every token until EOF.
fn tokens_from(lexer: &mut Lexer<'_>) -> Vec<String> {
    let mut out = Vec::new();
    loop {
        match lexer.lex(&mut NoYy).expect("lexes") {
            Lexed::Eof => return out,
            Lexed::Token(Token::Name(name)) => out.push(format!("{name}={}", lexer.yytext)),
            Lexed::Token(other) => panic!("unexpected token {other:?}"),
        }
    }
}

const DEFAULT_OPTIONS: &str = r#"{"flex": false, "case_insensitive": false}"#;

#[test]
fn start_conditions_select_rules() {
    let rules = format!(
        "[{},{},{},{},{}]",
        rule(0, "a", None, "return 'A';"),
        rule(1, "<", None, "this.begin('tag'); return 'LT';"),
        rule(2, "[^>]+", Some(&["tag"]), "return 'TAGTEXT';"),
        rule(3, ">", Some(&["tag"]), "this.popState(); return 'GT';"),
        rule(4, "\\s+", None, "/* skip whitespace */"),
    );
    let conditions = r#"{"INITIAL": {"rules": [0, 1, 4], "inclusive": true},
                        "tag": {"rules": [2, 3], "inclusive": false}}"#;
    let g = load(DEFAULT_OPTIONS, conditions, &rules);
    assert_eq!(
        tokens(&g, "a <x y> a"),
        vec!["A=a", "LT=<", "TAGTEXT=x y", "GT=>", "A=a"]
    );
    // Whitespace is not a rule of the exclusive condition.
    let mut lexer = Lexer::new(&g, "< >");
    assert_eq!(
        lexer.lex(&mut NoYy).expect("lexes"),
        Lexed::Token(Token::Name("LT".into()))
    );
    assert_eq!(lexer.top_state(), "tag");
    assert_eq!(
        lexer.lex(&mut NoYy).expect("lexes"),
        Lexed::Token(Token::Name("TAGTEXT".into()))
    );
    assert_eq!(lexer.yytext, " ");
}

#[test]
fn first_match_wins_unless_flex() {
    let rules = format!(
        "[{},{}]",
        rule(0, "ab", None, "return 'AB';"),
        rule(1, "abc", None, "return 'ABC';"),
    );
    let conditions = r#"{"INITIAL": {"rules": [0, 1], "inclusive": true}}"#;
    let g = load(DEFAULT_OPTIONS, conditions, &rules);
    let mut lexer = Lexer::new(&g, "abc");
    assert_eq!(
        lexer.lex(&mut NoYy).expect("lexes"),
        Lexed::Token(Token::Name("AB".into()))
    );
    let err = lexer.lex(&mut NoYy).expect_err("c matches no rule");
    assert_eq!(
        err.to_string(),
        "Lexical error on line 1. Unrecognized text.\nabc\n--^"
    );

    let g = load(
        r#"{"flex": true, "case_insensitive": false}"#,
        conditions,
        &rules,
    );
    assert_eq!(tokens(&g, "abc"), vec!["ABC=abc"]);
}

#[test]
fn case_insensitive_option() {
    let rules = format!("[{}]", rule(0, "gantt", None, "return 'GANTT';"));
    let conditions = r#"{"INITIAL": {"rules": [0], "inclusive": true}}"#;
    let g = load(
        r#"{"flex": false, "case_insensitive": true}"#,
        conditions,
        &rules,
    );
    assert_eq!(tokens(&g, "GanTT"), vec!["GANTT=GanTT"]);
    let g = load(DEFAULT_OPTIONS, conditions, &rules);
    assert!(Lexer::new(&g, "GanTT").lex(&mut NoYy).is_err());
}

#[test]
fn positions_follow_jison_lex() {
    let rules = format!(
        "[{},{}]",
        rule(0, "[a-d]+", None, "return 'W';"),
        rule(1, "\\n", None, "return 'NL';"),
    );
    let conditions = r#"{"INITIAL": {"rules": [0, 1], "inclusive": true}}"#;
    let g = load(DEFAULT_OPTIONS, conditions, &rules);
    let mut lexer = Lexer::new(&g, "ab\ncd");
    lexer.lex(&mut NoYy).expect("W");
    assert_eq!(lexer.yylineno, 0);
    assert_eq!(lexer.yyleng, 2);
    assert_eq!(
        lexer.yylloc,
        Loc {
            first_line: 1,
            last_line: 1,
            first_column: 0,
            last_column: 2
        }
    );
    lexer.lex(&mut NoYy).expect("NL");
    assert_eq!(lexer.yylineno, 1);
    assert_eq!(
        lexer.yylloc,
        Loc {
            first_line: 1,
            last_line: 2,
            first_column: 2,
            last_column: 0
        }
    );
    lexer.lex(&mut NoYy).expect("W");
    assert_eq!(
        lexer.yylloc,
        Loc {
            first_line: 2,
            last_line: 2,
            first_column: 0,
            last_column: 2
        }
    );
    assert_eq!(lexer.lex(&mut NoYy).expect("eof"), Lexed::Eof);
}

#[test]
fn show_position_windows_are_twenty_units() {
    let rules = format!("[{}]", rule(0, "[a-c]+", None, "return 'W';"));
    let conditions = r#"{"INITIAL": {"rules": [0], "inclusive": true}}"#;
    let g = load(DEFAULT_OPTIONS, conditions, &rules);

    let mut lexer = Lexer::new(&g, "abc$");
    lexer.lex(&mut NoYy).expect("W");
    let err = lexer.lex(&mut NoYy).expect_err("$ matches no rule");
    assert_eq!(
        err.to_string(),
        "Lexical error on line 1. Unrecognized text.\nabc$\n---^"
    );

    // Past input is the last 20 units with a `...` prefix that counts
    // toward the caret; upcoming input is cut at 20 units with a suffix.
    let input = format!("{}${}", "a".repeat(30), "b".repeat(30));
    let mut lexer = Lexer::new(&g, &input);
    lexer.lex(&mut NoYy).expect("W");
    let err = lexer.lex(&mut NoYy).expect_err("$ matches no rule");
    assert_eq!(
        err.to_string(),
        format!(
            "Lexical error on line 1. Unrecognized text.\n...{}${}\n{}^",
            "a".repeat(20),
            "b".repeat(19),
            "-".repeat(23)
        )
    );

    // Newlines vanish from both windows.
    let rules = format!(
        "[{},{}]",
        rule(0, "[a-d]+", None, "return 'W';"),
        rule(1, "\\n", None, "return 'NL';"),
    );
    let conditions = r#"{"INITIAL": {"rules": [0, 1], "inclusive": true}}"#;
    let g = load(DEFAULT_OPTIONS, conditions, &rules);
    let mut lexer = Lexer::new(&g, "ab\ncd\n$");
    for _ in 0..4 {
        lexer.lex(&mut NoYy).expect("token");
    }
    let err = lexer.lex(&mut NoYy).expect_err("$ matches no rule");
    assert_eq!(
        err.to_string(),
        "Lexical error on line 3. Unrecognized text.\nabcd$\n----^"
    );
    assert_eq!(err.line, 3);
}

#[test]
fn unput_pushes_the_match_back() {
    let rules = format!(
        "[{},{},{}]",
        rule(0, "[ \\t]+", Some(&["bol"]), "/* ignore */"),
        rule(
            1,
            "[^ \\t\\n]",
            Some(&["bol"]),
            "this.begin('INITIAL'); this.unput(yytext);"
        ),
        rule(2, "[a-z]+", None, "return 'WORD';"),
    );
    let conditions = r#"{"INITIAL": {"rules": [2], "inclusive": true},
                        "bol": {"rules": [0, 1], "inclusive": false}}"#;
    let g = load(DEFAULT_OPTIONS, conditions, &rules);
    let mut lexer = Lexer::new(&g, "  hello");
    lexer.begin("bol").expect("known condition");
    assert_eq!(tokens_from(&mut lexer), vec!["WORD=hello"]);
}

#[test]
fn unknown_start_condition_is_an_error_not_a_panic() {
    let rules = format!(
        "[{}]",
        rule(0, "x", None, "this.begin('nowhere'); return 'X';")
    );
    let conditions = r#"{"INITIAL": {"rules": [0], "inclusive": true}}"#;
    let g = load(DEFAULT_OPTIONS, conditions, &rules);
    let err = Lexer::new(&g, "x")
        .lex(&mut NoYy)
        .expect_err("unknown condition");
    assert!(err.to_string().contains("nowhere"));
}

#[test]
fn loading_fails_loudly_on_an_unsupported_action() {
    let rules = format!(
        "[{},{}]",
        rule(0, "a", None, "return 'A';"),
        rule(1, "b", None, "this.more(); return 'B';"),
    );
    let conditions = r#"{"INITIAL": {"rules": [0, 1], "inclusive": true}}"#;
    let err = Grammar::load(
        "synthetic",
        &grammar_json(DEFAULT_OPTIONS, conditions, &rules),
    )
    .expect_err("more() is unsupported");
    match &err {
        LoadError::Action(u) => {
            assert_eq!(u.grammar, "synthetic");
            assert_eq!(u.rule, 1);
            assert_eq!(u.source, "this.more(); return 'B';");
        }
        other => panic!("unexpected error {other:?}"),
    }
    assert!(err.to_string().contains("synthetic lexer rule 1"));
}

#[test]
fn loading_fails_loudly_on_a_bad_regex() {
    let rules = format!("[{}]", rule(0, "(a", None, "return 'A';"));
    let conditions = r#"{"INITIAL": {"rules": [0], "inclusive": true}}"#;
    let err = Grammar::load(
        "synthetic",
        &grammar_json(DEFAULT_OPTIONS, conditions, &rules),
    )
    .expect_err("unbalanced paren");
    assert!(matches!(err, LoadError::Regex { rule: 0, .. }), "{err:?}");
}
