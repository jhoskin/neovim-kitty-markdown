//! CLI failures (R-146, R-323, R-324): every failed command writes exactly
//! one line `kitty-markdown-helper: <reason>` to stderr, nothing to
//! stdout, and exits with status 1. A multi-line reason (a mermaid parse
//! error, the no-diagram-type message) is folded with `\n` escapes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const PREFIX: &str = "kitty-markdown-helper: ";

/// A fresh scratch folder per test under the system temp folder.
fn scratch() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "kmh-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Runs the helper and returns the one stderr reason of a failure.
fn fail(args: &[&std::ffi::OsStr]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_kitty-markdown-helper"))
        .args(args)
        .output()
        .expect("helper runs");
    let stderr = String::from_utf8(out.stderr).expect("utf-8 stderr");
    assert_eq!(out.status.code(), Some(1), "{args:?}: {stderr}");
    assert!(out.stdout.is_empty(), "{args:?}: stdout {:?}", out.stdout);
    let line = stderr
        .strip_suffix('\n')
        .unwrap_or_else(|| panic!("{args:?}: no line end: {stderr:?}"));
    assert!(
        !line.contains('\n'),
        "{args:?}: more than one line: {stderr:?}"
    );
    line.strip_prefix(PREFIX)
        .unwrap_or_else(|| panic!("{args:?}: no prefix: {stderr:?}"))
        .to_string()
}

fn render_mermaid(dir: &Path, source: &str) -> String {
    let input = dir.join("in.mmd");
    fs::write(&input, source).expect("write source");
    let out = dir.join("out.png");
    let reason = fail(&[
        "render".as_ref(),
        "--mermaid".as_ref(),
        input.as_os_str(),
        "--width".as_ref(),
        "400".as_ref(),
        "-o".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!out.exists(), "a failed render wrote {}", out.display());
    reason
}

/// Undoes the fold, as `printf '%b'` would.
fn unfold(reason: &str) -> String {
    let mut out = String::new();
    let mut chars = reason.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[test]
fn invalid_mermaid_gives_the_folded_oracle_error() {
    let dir = scratch();
    let cases = [
        (
            "flowchart LR\n  A[open",
            "Parse error on line 3:\n...owchart LR  A[open\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
        ),
        (
            "classDiagram\n  A <|--\n",
            "Parse error on line 2:\n...lassDiagram  A <|--\n----------------------^\nExpecting 'STR', 'ALPHA', 'AGGREGATION', 'EXTENSION', 'COMPOSITION', 'DEPENDENCY', 'LOLLIPOP', 'NUM', 'MINUS', 'UNICODE_TEXT', 'BQUOTE_STR', got 'NEWLINE'",
        ),
        (
            "pie\n  \"a\":",
            "Parsing failed:  Parse error on line 2, column 7: Expecting token of type 'NUMBER_PIE' but found `\n`.",
        ),
    ];
    for (source, want) in cases {
        let reason = render_mermaid(&dir, source);
        assert_eq!(unfold(&reason), want, "{source:?}");
        assert!(reason.contains("\\n"), "{reason}");
    }
    // An unclosed front matter block and invalid front matter YAML.
    let reason = render_mermaid(&dir, "---\ntitle: x\n\ngraph TD\n  A-->B");
    assert!(
        reason.starts_with("Diagrams beginning with --- are not valid."),
        "{reason}"
    );
    render_mermaid(&dir, "---\ntitle: [x\n---\ngraph TD\n  A-->B");
}

#[test]
fn text_that_is_no_diagram_type_gives_one_line() {
    let dir = scratch();
    let reason = render_mermaid(&dir, "foo bar\nbaz\n\n  qux");
    assert_eq!(
        unfold(&reason),
        "No diagram type detected matching given configuration for text: foo bar\nbaz\n\n  qux"
    );
    let reason = render_mermaid(&dir, "");
    assert!(reason.starts_with("No diagram type detected"), "{reason}");
    let reason = render_mermaid(&dir, "C:\\path\\x\nmore");
    assert!(reason.ends_with("C:\\\\path\\\\x\\nmore"), "{reason}");
}

#[test]
fn unsupported_mermaid_gives_one_line() {
    let dir = scratch();
    assert_eq!(
        render_mermaid(&dir, "zenuml\n  A.method()"),
        "unsupported diagram type: zenuml"
    );
    assert_eq!(
        render_mermaid(&dir, "%%{init: {'look': 'handDrawn'}}%%\ngraph TD\n  A-->B"),
        "unsupported feature: look: handDrawn"
    );
}

#[test]
fn unreadable_inputs_and_outputs_give_one_line() {
    let dir = scratch();
    let missing = dir.join("missing.mmd");
    let out = dir.join("out.png");
    for flag in ["--mermaid", "--image", "--svg"] {
        fail(&[
            "render".as_ref(),
            flag.as_ref(),
            missing.as_os_str(),
            "--width".as_ref(),
            "100".as_ref(),
            "-o".as_ref(),
            out.as_os_str(),
        ]);
    }
    let text = dir.join("not-an-image.png");
    fs::write(&text, "multi\nline\ntext\n").expect("write");
    for flag in ["--image", "--svg"] {
        fail(&[
            "render".as_ref(),
            flag.as_ref(),
            text.as_os_str(),
            "--width".as_ref(),
            "100".as_ref(),
            "-o".as_ref(),
            out.as_os_str(),
        ]);
    }
    let input = dir.join("ok.mmd");
    fs::write(&input, "graph TD\n  A-->B\n").expect("write");
    let nowhere = dir.join("no-such-dir").join("out.png");
    let reason = fail(&[
        "render".as_ref(),
        "--mermaid".as_ref(),
        input.as_os_str(),
        "--width".as_ref(),
        "200".as_ref(),
        "-o".as_ref(),
        nowhere.as_os_str(),
    ]);
    let want = format!("cannot write {}: ", nowhere.display());
    assert!(reason.starts_with(&want), "{reason}");
}

#[test]
fn compare_failures_give_one_line() {
    let dir = scratch();
    let a = dir.join("a.png");
    let b = dir.join("b.png");
    image::RgbaImage::new(10, 10).save(&a).expect("png a");
    image::RgbaImage::new(13, 10).save(&b).expect("png b");
    assert_eq!(
        fail(&["compare".as_ref(), a.as_os_str(), b.as_os_str()]),
        "image sizes differ: 10x10 vs 13x10"
    );
    let missing = dir.join("missing.png");
    fail(&["compare".as_ref(), a.as_os_str(), missing.as_os_str()]);
    let text = dir.join("text.png");
    fs::write(&text, "not\na png").expect("write");
    fail(&["compare".as_ref(), text.as_os_str(), a.as_os_str()]);
}
