//! Parse errors and diagram detection against the oracle (R-111, R-113,
//! R-176, R-177, R-178).
//!
//! `Diagram.fromText` in mermaid 12.2.0 parses `encodeEntities(code) + "\n"`,
//! where `code` is the text after front matter, directives, and comments
//! are removed. The appended newline puts an error at the end of the text
//! on the line after the last one, and it sets the excerpt and the caret.
//! Every case below is the oracle's own message for its source, cut of
//! the stack lines mermaid-cli adds; the cases cover an error at the end
//! of the text and one in the middle for each parser family.

use kitty_markdown_helper::error::HelperError;
use kitty_markdown_helper::mermaid::detect::detect_type;
use kitty_markdown_helper::mermaid::raster::Background;
use kitty_markdown_helper::mermaid::{self, RenderRequest};

fn render(source: &str) -> Result<(), HelperError> {
    mermaid::render(&RenderRequest {
        source,
        width_px: 960,
        theme: "default",
        background: Background::default(),
    })
    .map(|_| ())
}

/// `(case, source, oracle message)`: generated with mermaid-cli 12.0.0 and
/// mermaid 12.2.0 from `helper/oracle` (jison types) and
/// `@mermaid-js/parser` 2.0.1 `parse(type, text + "\n")` (langium types).
const ORACLE_ERRORS: &[(&str, &str, &str)] = &[
    (
        "flow_open_square",
        "flowchart LR\n  A[open",
        "Parse error on line 3:\n...owchart LR  A[open\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_open_square_nl",
        "flowchart LR\n  A[open\n",
        "Parse error on line 4:\n...wchart LR  A[open\n--------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_open_round",
        "graph TD\n  A(round",
        "Parse error on line 3:\ngraph TD  A(round\n-----------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_open_rhombus",
        "flowchart TD\n  A --> B{decide",
        "Parse error on line 3:\n...TD  A --> B{decide\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_open_stadium",
        "flowchart TD\n  A([stad",
        "Parse error on line 3:\n...wchart TD  A([stad\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_open_label_pipe",
        "flowchart LR\n  A -->|label",
        "Parse error on line 3:\n...rt LR  A -->|label\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_open_label_pipe_nl",
        "flowchart LR\n  A -->|label\n",
        "Parse error on line 4:\n...t LR  A -->|label\n--------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_open_label_text",
        "flowchart LR\n  A -- text",
        "Parse error on line 3:\n...hart LR  A -- text\n---------------------^\nExpecting 'LINK', 'UNICODE_TEXT', 'EDGE_TEXT', got '1'",
    ),
    (
        "flow_dangling_arrow",
        "flowchart LR\n  A --> B -->\n",
        "Parse error on line 4:\n...t LR  A --> B -->\n--------------------^\nExpecting 'AMP', 'COLON', 'PIPE', 'TESTSTR', 'DOWN', 'DEFAULT', 'NUM', 'COMMA', 'NODE_STRING', 'BRKT', 'MINUS', 'MULT', 'UNICODE_TEXT', got 'EOF'",
    ),
    (
        "flow_dangling_arrow_nonl",
        "flowchart LR\n  A --> B -->",
        "Parse error on line 3:\n...rt LR  A --> B -->\n---------------------^\nExpecting 'AMP', 'COLON', 'PIPE', 'TESTSTR', 'DOWN', 'DEFAULT', 'NUM', 'COMMA', 'NODE_STRING', 'BRKT', 'MINUS', 'MULT', 'UNICODE_TEXT', got 'EOF'",
    ),
    (
        "flow_middle",
        "flowchart LR\n  A[x\n  B --> C\n",
        "Parse error on line 5:\n...LR  A[x  B --> C\n-------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_middle2",
        "flowchart LR\n  A --> B\n  B --> >C\n  C --> D\n",
        "Parse error on line 3:\n...R  A --> B  B --> >C  C --> D\n---------------------^\nExpecting 'AMP', 'COLON', 'PIPE', 'TESTSTR', 'DOWN', 'DEFAULT', 'NUM', 'COMMA', 'NODE_STRING', 'BRKT', 'MINUS', 'MULT', 'UNICODE_TEXT', got 'TAGEND'",
    ),
    (
        "flow_comment_open",
        "flowchart LR\n%% a comment\n  A[open",
        "Parse error on line 3:\n...owchart LR  A[open\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_fm_open",
        "---\ntitle: T\n---\nflowchart LR\n  A[open",
        "Parse error on line 3:\n...owchart LR  A[open\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_dir_open",
        "%%{init: {'theme':'dark'}}%%\nflowchart LR\n  A[open",
        "Parse error on line 3:\n...owchart LR  A[open\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_entity_open",
        "flowchart LR\n  A[#35; x",
        "Parse error on line 3:\n...rt LR  A[\u{fb02}\u{b0}\u{b0}35\u{b6}\u{df} x\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_entity_mid",
        "flowchart LR\n  A[#quot; x\n  B-->C\n",
        "Parse error on line 5:\n...\u{fb02}\u{b0}quot\u{b6}\u{df} x  B-->C\n--------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_style_semicolon",
        "flowchart LR\n  A-->B\n  style A fill:#f9f;\n  C[open",
        "Parse error on line 5:\n... fill:#f9f  C[open\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "flow_crlf_open",
        "flowchart LR\r\n  A[open",
        "Parse error on line 3:\n...owchart LR  A[open\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "class_open_brace",
        "classDiagram\n  class A {",
        "Parse error on line 2:\n...Diagram  class A {\n---------------------^\nExpecting 'STRUCT_STOP', 'MEMBER', got 'EOF_IN_STRUCT'",
    ),
    (
        "class_open_brace_member",
        "classDiagram\n  class A {\n    +int x",
        "Parse error on line 3:\n...lass A {    +int x\n---------------------^\nExpecting 'STRUCT_STOP', 'MEMBER', got 'EOF_IN_STRUCT'",
    ),
    (
        "class_rel_end",
        "classDiagram\n  A <|--",
        "Parse error on line 2:\n...lassDiagram  A <|--\n----------------------^\nExpecting 'STR', 'ALPHA', 'AGGREGATION', 'EXTENSION', 'COMPOSITION', 'DEPENDENCY', 'LOLLIPOP', 'NUM', 'MINUS', 'UNICODE_TEXT', 'BQUOTE_STR', got 'NEWLINE'",
    ),
    (
        "class_rel_end_nl",
        "classDiagram\n  A <|--\n",
        "Parse error on line 2:\n...lassDiagram  A <|--\n----------------------^\nExpecting 'STR', 'ALPHA', 'AGGREGATION', 'EXTENSION', 'COMPOSITION', 'DEPENDENCY', 'LOLLIPOP', 'NUM', 'MINUS', 'UNICODE_TEXT', 'BQUOTE_STR', got 'NEWLINE'",
    ),
    (
        "class_middle",
        "classDiagram\n  A <|-- \n  B --> C\n",
        "Parse error on line 2:\n...lassDiagram  A <|--   B --> C\n----------------------^\nExpecting 'STR', 'ALPHA', 'AGGREGATION', 'EXTENSION', 'COMPOSITION', 'DEPENDENCY', 'LOLLIPOP', 'NUM', 'MINUS', 'UNICODE_TEXT', 'BQUOTE_STR', got 'NEWLINE'",
    ),
    (
        "class_generic_open",
        "classDiagram\n  class A~T",
        "Parse error on line 3:\n...Diagram  class A~T\n---------------------^\nExpecting 'NEWLINE', 'EOF', 'SQS', 'STR', 'LABEL', 'STRUCT_START', 'STRUCT_STOP', 'STYLE_SEPARATOR', 'ANNOTATION_START', 'AGGREGATION', 'EXTENSION', 'COMPOSITION', 'DEPENDENCY', 'LOLLIPOP', 'LINE', 'DOTTED_LINE', 'CALLBACK_NAME', 'HREF', got 'GENERICTYPE'",
    ),
    (
        "seq_end",
        "sequenceDiagram\n  A->>",
        "Parse error on line 2:\n...quenceDiagram  A->>\n----------------------^\nExpecting 'link', '+', '-', '()', 'ACTOR', got 'NEWLINE'",
    ),
    (
        "seq_end2",
        "sequenceDiagram\n  A->>B",
        "Parse error on line 2:\n...uenceDiagram  A->>B\n----------------------^\nExpecting 'TXT', got 'NEWLINE'",
    ),
    (
        "seq_middle",
        "sequenceDiagram\n  A->>B\n  B->>C: hi\n",
        "Parse error on line 2:\n...uenceDiagram  A->>B  B->>C: hi\n----------------------^\nExpecting 'TXT', got 'NEWLINE'",
    ),
    (
        "seq_loop_open",
        "sequenceDiagram\n  loop x\n  A->>B: m",
        "Parse error on line 4:\n...  loop x  A->>B: m\n---------------------^\nExpecting 'SPACE', 'NEWLINE', 'INVALID', 'create', 'box', 'end', 'autonumber', 'activate', 'deactivate', 'title', 'legacy_title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'loop', 'rect', 'opt', 'alt', 'par', 'par_over', 'critical', 'break', 'participant', 'participant_actor', 'destroy', 'note', 'links', 'link', 'properties', 'details', 'ACTOR', got '1'",
    ),
    (
        "state_end",
        "stateDiagram-v2\n  [*] -->",
        "Parse error on line 2:\n...Diagram-v2  [*] -->\n----------------------^\nExpecting 'ID', 'EDGE_STATE', got 'NL'",
    ),
    (
        "state_open_block",
        "stateDiagram-v2\n  state A {\n    B --> C",
        "Parse error on line 3:\n...ate A {    B --> C\n---------------------^\nExpecting 'SPACE', 'NL', 'HIDE_EMPTY', 'scale', 'COMPOSIT_STATE', 'STRUCT_STOP', 'STATE_DESCR', 'ID', 'FORK', 'JOIN', 'CHOICE', 'CONCURRENT', 'note', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'CLICK', 'classDef', 'style', 'class', 'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'EDGE_STATE', got '1'",
    ),
    (
        "er_end",
        "erDiagram\n  A ||--o{",
        "Parse error on line 2:\nerDiagram  A ||--o{\n-------------------^\nExpecting 'UNICODE_TEXT', 'NUM', 'ENTITY_NAME', 'DECIMAL_NUM', 'ENTITY_ONE', 'NON_IDENTIFYING', 'IDENTIFYING', got 'NEWLINE'",
    ),
    (
        "er_open_attr",
        "erDiagram\n  A {\n    string name",
        "Parse error on line 3:\n...A {    string name\n---------------------^\nExpecting 'BLOCK_STOP', 'ATTRIBUTE_WORD', 'ATTRIBUTE_KEY', 'COMMENT', got '1'",
    ),
    (
        "gantt_end",
        "gantt\n  title x\n  section",
        "Parse error on line 3:\n...  title x  section\n---------------------^\nExpecting 'taskData', got 'NL'",
    ),
    (
        "req_end",
        "requirementDiagram\n  requirement r {\n    id: 1",
        "Parse error on line 4:\n...ement r {    id: 1\n---------------------^\nExpecting 'NEWLINE', 'ID', 'TEXT', 'RISK', 'VERIFYMTHD', 'STRUCT_STOP', got 'EOF'",
    ),
    (
        "c4_end",
        "C4Context\n  Person(a, \"A\"",
        "Parse error on line 3:\n...ext  Person(a, \"A\"\n---------------------^\nExpecting 'NEWLINE', 'EOF', 'LBRACE', 'RBRACE', 'STR', 'STR_KEY', 'ATTRIBUTE', 'ATTRIBUTE_EMPTY', got '1'",
    ),
    (
        "xy_end",
        "xychart-beta\n  x-axis [a, b",
        "Parse error on line 2:\n...-beta  x-axis [a, b\n----------------------^\nExpecting 'SQUARE_BRACES_END', 'COMMA', got 'NEWLINE'",
    ),
    (
        "block_end",
        "block-beta\n  a[\"open",
        "Parse error on line 3:\n...lock-beta  a[\"open\n---------------------^\nExpecting 'NODE_DEND', got 'STR'",
    ),
    (
        "mindmap_end",
        "mindmap\n  root((open",
        "Parse error on line 3:\n...indmap  root((open\n---------------------^\nExpecting 'NODE_DEND', got '1'",
    ),
    (
        "kanban_end",
        "kanban\n  todo[open",
        "Parse error on line 3:\nkanban  todo[open\n-----------------^\nExpecting 'NODE_DEND', got '1'",
    ),
    (
        "sankey_end",
        "sankey-beta\na,b",
        "Parse error on line 2:\nsankey-betaa,b\n--------------^\nExpecting 'COMMA', got 'EOF'",
    ),
    (
        "venn_end",
        "venn-beta\n  set A[",
        "Lexical error on line 2. Unrecognized text.\nvenn-beta  set A[\n----------------^",
    ),
    (
        "pie_end",
        "pie\n  \"a\":",
        "Parsing failed:  Parse error on line 2, column 7: Expecting token of type 'NUMBER_PIE' but found `\n`.",
    ),
    (
        "pie_middle",
        "pie\n  \"a\": x\n  \"b\": 2\n",
        "Parsing failed: Lexer error on line 2, column 8: unexpected character: ->x<- at offset: 11, skipped 1 characters. Parse error on line 2, column 9: Expecting token of type 'NUMBER_PIE' but found `\n`.",
    ),
    (
        "packet_end",
        "packet-beta\n  0-1:",
        "Parsing failed:  Parse error on line 2, column 7: Expecting token of type 'STRING' but found `\n`.",
    ),
    (
        "git_end",
        "gitGraph\n  commit id:",
        "Parsing failed:  Parse error on line 2, column 13: Expecting token of type 'STRING' but found `\n`.",
    ),
    (
        "arch_end",
        "architecture-beta\n  service a(",
        "Parsing failed: Lexer error on line 2, column 12: unexpected character: ->(<- at offset: 29, skipped 1 characters. ",
    ),
    (
        "radar_end",
        "radar-beta\n  axis",
        "Parsing failed:  Parse error on line 2, column 7: Expecting token of type 'ID' but found `\n`.",
    ),
    (
        "treemap_end",
        "treemap-beta\n  \"a\":",
        "Parsing failed:  Parse error on line 2, column 6: Expecting end of file but found `:`.",
    ),
    (
        "wardley_end",
        "wardley-beta\n  component",
        "Parsing failed:  Parse error on line 2, column 12: Expecting: one of these possible Token sequences:\n  1. [STRING]\n  2. [ID]\n  3. [NAME_WITH_SPACES]\nbut found: '\n'",
    ),
    (
        "treeview_end",
        "treeView-beta\n  \"a",
        "Parsing failed: Lexer error on line 2, column 3: unexpected character: ->\"<- at offset: 16, skipped 1 characters. ",
    ),
    (
        "em_end",
        "eventmodeling\n  tf 01 scn",
        "Parsing failed:  Parse error on line 2, column 9: Expecting: one of these possible Token sequences:\n  1. [rmo]\n  2. [readmodel]\n  3. [ui]\n  4. [cmd]\n  5. [command]\n  6. [evt]\n  7. [event]\n  8. [pcr]\n  9. [processor]\nbut found: 'scn'",
    ),
    (
        "usecase_end",
        "usecase-beta\n  actor",
        "Error parsing usecase diagram: Expecting: one of these possible Token sequences:\n  1. [IDENTIFIER]\n  2. [PLAIN_STRING]\n  3. [MARKDOWN_STRING]\nbut found: '\n' at line 2, column 8 [20,21)",
    ),
    (
        "swimlane_end",
        "swimlane-beta LR\n  A[open",
        "Parse error on line 3:\n...ne-beta LR  A[open\n---------------------^\nExpecting 'SQE', 'DOUBLECIRCLEEND', 'PE', '-)', 'STADIUMEND', 'SUBROUTINEEND', 'PIPE', 'CYLINDEREND', 'DIAMOND_STOP', 'TAGEND', 'TRAPEND', 'INVTRAPEND', 'UNICODE_TEXT', 'TEXT', 'TAGSTART', got '1'",
    ),
    (
        "pie_bad_value",
        "pie\n  \"a\": x",
        "Parsing failed: Lexer error on line 2, column 8: unexpected character: ->x<- at offset: 11, skipped 1 characters. Parse error on line 2, column 9: Expecting token of type 'NUMBER_PIE' but found `\n`.",
    ),
    (
        "pie_bad_value_nl",
        "pie\n  \"a\": x\n",
        "Parsing failed: Lexer error on line 2, column 8: unexpected character: ->x<- at offset: 11, skipped 1 characters. Parse error on line 2, column 9: Expecting token of type 'NUMBER_PIE' but found `\n`.",
    ),
    (
        "pie_open_string",
        "pie\n  \"a",
        "Parsing failed: Lexer error on line 2, column 3: unexpected character: ->\"<- at offset: 6, skipped 2 characters. ",
    ),
    (
        "pie_mid",
        "pie\n  \"a\" 1\n  \"b\": 2\n",
        "Parsing failed:  Parse error on line 2, column 7: Expecting token of type ':' but found `1`.",
    ),
    (
        "packet_open",
        "packet-beta\n  0-1: \"a",
        "Parsing failed: Lexer error on line 2, column 8: unexpected character: ->\"<- at offset: 19, skipped 2 characters. Parse error on line 2, column 10: Expecting token of type 'STRING' but found `\n`.",
    ),
    (
        "packet_bad",
        "packet-beta\n  0-1 x",
        "Parsing failed: Lexer error on line 2, column 7: unexpected character: ->x<- at offset: 18, skipped 1 characters. Parse error on line 2, column 8: Expecting token of type ':' but found `\n`.",
    ),
    (
        "git_open_str",
        "gitGraph\n  commit id: \"a",
        "Parsing failed: Lexer error on line 2, column 14: unexpected character: ->\"<- at offset: 22, skipped 1 characters. Parse error on line 2, column 15: Expecting token of type 'STRING' but found `a`.",
    ),
    (
        "git_bad",
        "gitGraph\n  commit\n  branch",
        "Parsing failed:  Parse error on line 3, column 9: Expecting: one of these possible Token sequences:\n  1. [REFERENCE]\n  2. [STRING]\nbut found: '\n'",
    ),
    (
        "git_bad_mid",
        "gitGraph\n  commit foo\n  commit\n",
        "Parsing failed:  Parse error on line 2, column 10: Expecting: one of these possible Token sequences:\n  1. [NEWLINE]\n  2. [EOF]\nbut found: 'foo'",
    ),
    (
        "arch_bad",
        "architecture-beta\n  service a(db)[",
        "Parsing failed: Lexer error on line 2, column 16: unexpected character: ->[<- at offset: 33, skipped 1 characters. ",
    ),
    (
        "arch_bad2",
        "architecture-beta\n  group",
        "Parsing failed:  Parse error on line 2, column 8: Expecting token of type 'ID' but found `\n`.",
    ),
    (
        "radar_bad",
        "radar-beta\n  axis A[\"x",
        "Parsing failed: Lexer error on line 2, column 10: unexpected character: ->\"<- at offset: 20, skipped 1 characters. Parse error on line 2, column 11: Expecting token of type 'STRING' but found `x`.",
    ),
    (
        "radar_bad2",
        "radar-beta\n  curve c{1,",
        "Parsing failed:  Parse error on line ?, column ?: Expecting token of type 'NUMBER' but found ``.",
    ),
    (
        "treemap_bad",
        "treemap-beta\n  \"a\": x",
        "Parsing failed:  Parse error on line 2, column 6: Expecting token of type 'EOF' but found `:`.\nParse error on line 2, column 8: Expecting: one of these possible Token sequences:\n  1. [STRING2]\n  2. [CLASS_DEF]\nbut found: 'x'",
    ),
    (
        "treemap_open",
        "treemap-beta\n  \"a",
        "Parsing failed: Lexer error on line 2, column 3: unexpected character: ->\"<- at offset: 15, skipped 1 characters. Parse error on line 2, column 4: Expecting: one of these possible Token sequences:\n  1. [STRING2]\n  2. [CLASS_DEF]\nbut found: 'a'",
    ),
    (
        "wardley_bad",
        "wardley-beta\n  component A [",
        "Parsing failed:  Parse error on line 2, column 16: Expecting token of type 'WARDLEY_NUMBER' but found `\n`.",
    ),
    (
        "cynefin_bad",
        "cynefin-beta\n  complex [",
        "Parsing failed: Lexer error on line 2, column 11: unexpected character: ->[<- at offset: 23, skipped 1 characters. ",
    ),
    (
        "em_bad",
        "eventmodeling\n  tf 01",
        "Parsing failed:  Parse error on line ?, column ?: Expecting: one of these possible Token sequences:\n  1. [rmo]\n  2. [readmodel]\n  3. [ui]\n  4. [cmd]\n  5. [command]\n  6. [evt]\n  7. [event]\n  8. [pcr]\n  9. [processor]\nbut found: ''",
    ),
    (
        "em_bad2",
        "eventmodeling\n%% c\n  tf 01",
        "Parsing failed:  Parse error on line ?, column ?: Expecting: one of these possible Token sequences:\n  1. [rmo]\n  2. [readmodel]\n  3. [ui]\n  4. [cmd]\n  5. [command]\n  6. [evt]\n  7. [event]\n  8. [pcr]\n  9. [processor]\nbut found: ''",
    ),
    (
        "timeline_bad",
        "timeline\n  title x\n  : a :",
        "Parse error on line 3:\n...ine  title x  : a :\n---------------------^\nExpecting 'EOF', 'SPACE', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'period', 'event', got 'INVALID'",
    ),
    (
        "quad_bad",
        "quadrantChart\n  x-axis",
        "Parse error on line 2:\n...adrantChart  x-axis\n----------------------^\nExpecting 'ALPHA', 'NUM', 'COMMA', 'AMP', 'BRKT', 'MULT', 'UNICODE_TEXT', 'STR', 'MD_STR', 'PUNCTUATION', 'PLUS', 'EQUALS', 'DOT', 'UNDERSCORE', got 'NEWLINE'",
    ),
    (
        "quad_bad2",
        "quadrantChart\n  point: [0.1,",
        "Parse error on line 2:\n...hart  point: [0.1,\n---------------------^\nExpecting 'point_y', got '1'",
    ),
    (
        "venn_bad",
        "venn-beta\n  set",
        "Parse error on line 2:\nvenn-beta  set\n--------------^\nExpecting 'IDENTIFIER', 'STRING', got 'NEWLINE'",
    ),
    (
        "usecase_mid",
        "usecase-beta\n  actor\n  actor B\n",
        "Error parsing usecase diagram: Expecting: one of these possible Token sequences:\n  1. [IDENTIFIER]\n  2. [PLAIN_STRING]\n  3. [MARKDOWN_STRING]\nbut found: '\n' at line 2, column 8 [20,21)",
    ),
];

/// Sources that look unfinished but that the oracle renders.
const ORACLE_RENDERS: &[(&str, &str)] = &[
    ("class_member_end", "classDiagram\n  class A\n  A : "),
    ("journey_end", "journey\n  title x\n  section s\n  task: "),
    ("quad_end", "quadrantChart\n  x-axis a -->"),
    ("timeline_end", "timeline\n  title"),
    ("ishikawa_end", "ishikawa-beta\n  Head\n    [open"),
    ("cynefin_end", "cynefin-beta\n  complex"),
    ("treeview_bad", "treeView-beta\n  \"a\" ["),
    ("ishikawa_bad", "ishikawa-beta\n  Head\n  ["),
    ("journey_bad", "journey\n  title x\n  section s\n  task: x"),
    ("class_member_bad", "classDiagram\n  class A\n  A : +x("),
];

/// The jison form names the line in its first line; the failure box takes
/// it from the error's `line` (R-177).
fn jison_line(message: &str) -> Option<u32> {
    message
        .strip_prefix("Parse error on line ")?
        .split(':')
        .next()?
        .parse()
        .ok()
}

#[test]
fn parse_errors_match_the_oracle_text_and_line() {
    let mut failures = Vec::new();
    for (case, source, want) in ORACLE_ERRORS {
        match render(source) {
            Err(HelperError::ParseError { line, message, .. }) => {
                if message != *want {
                    failures.push(format!("{case}: message\n{message}\n--- oracle\n{want}"));
                }
                if let Some(want_line) = jison_line(want)
                    && line != want_line
                {
                    failures.push(format!("{case}: line {line}, oracle {want_line}"));
                }
            }
            other => failures.push(format!("{case}: {other:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn unfinished_looking_sources_the_oracle_renders_also_render() {
    for (case, source) in ORACLE_RENDERS {
        assert!(render(source).is_ok(), "{case}: {:?}", render(source));
    }
}

/// R-113, R-176: mermaid-cli registers `@mermaid-js/mermaid-zenuml`, whose
/// detector is `/^\s*zenuml/` and runs after every built-in one; the text
/// it sees has front matter, directives, and comments removed.
#[test]
fn zenuml_in_every_form_is_an_unsupported_diagram_type() {
    let sources = [
        "zenuml\n  A.method()",
        "   zenuml\n  A.method()",
        "\tzenuml\n  A.method()",
        "---\ntitle: x\n---\nzenuml\n  A.method()",
        "%%{init: {'theme':'dark'}}%%\nzenuml\n  A.method()",
        "%% a comment\nzenuml\n  A.method()",
        "---\nconfig:\n  theme: forest\n---\n%%{init: {'theme':'dark'}}%%\n%% c\n  zenuml\n",
        "zenumlfoo\n  A.method()",
        "zenuml",
    ];
    for source in sources {
        assert_eq!(detect_type(source), Ok("zenuml"), "{source:?}");
        match render(source) {
            Err(HelperError::UnsupportedDiagramType { diagram_type }) => {
                assert_eq!(diagram_type, "zenuml", "{source:?}")
            }
            other => panic!("{source:?}: {other:?}"),
        }
    }
    // The detector is case-sensitive: the oracle finds no diagram type.
    let err = render("ZenUML\n  A.method()").unwrap_err();
    assert_eq!(
        err.to_string(),
        "No diagram type detected matching given configuration for text: ZenUML\n  A.method()"
    );
}

/// R-176: the other types outside the delivered phases, with the same
/// prefixes and leading text as zenuml.
#[test]
fn info_agentflow_and_railroad_are_unsupported_diagram_types() {
    let sources = [
        ("info", "info"),
        ("info", "  info\n"),
        ("info", "info x"),
        ("info", "---\ntitle: x\n---\ninfo"),
        ("agentflow", "agentflow-beta\n  a"),
        ("agentflow", "%%{init: {}}%%\n agentflow-beta"),
        ("railroad", "railroad-beta\n"),
        ("railroad", "Railroad-Beta\n"),
        ("railroadEbnf", "railroad-ebnf-beta\n"),
        ("railroadAbnf", "%% c\nrailroad-abnf-beta\n"),
        ("railroadPeg", "  railroad-peg-beta\n"),
    ];
    for (want, source) in sources {
        match render(source) {
            Err(HelperError::UnsupportedDiagramType { diagram_type }) => {
                assert_eq!(diagram_type, want, "{source:?}")
            }
            other => panic!("{source:?}: {other:?}"),
        }
    }
}
