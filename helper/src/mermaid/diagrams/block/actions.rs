//! The grammar actions of `block.jison` (mermaid 12.2.0), keyed by
//! production index in `helper/grammars/block.tables.json`. The lexer
//! actions only call `yy.getLogger()`, which the translator drops, so the
//! default `Yy` is enough.

use super::db::{
    self, BlockType, ParsedBlock, edge_str_to_edge_data, edge_str_to_edge_start_data,
    edge_str_to_pattern, edge_str_to_thickness, type_str_to_type,
};
use crate::mermaid::jison::lexer::js_trim;
use crate::mermaid::jison::{Actions, Loc, ParseError, Yy};

/// A `node` production value: `{ id, label, typeStr, directions }`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodeVal {
    pub id: String,
    pub label: Option<String>,
    pub type_str: Option<String>,
    pub directions: Vec<String>,
}

/// A semantic value on jison's `vstack`.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Nil,
    /// A token's `yytext`, or a string a production returned.
    Text(String),
    /// A single statement object.
    Block(Box<ParsedBlock>),
    /// A `document`, or the three objects of `nodeStatement link node`.
    Blocks(Vec<ParsedBlock>),
    /// `link`: `{ edgeTypeStr, label }`.
    Link {
        edge_type_str: String,
        label: String,
    },
    Node(NodeVal),
    /// `nodeShapeNLabel`: `{ typeStr, label, directions }`.
    ShapeLabel {
        type_str: String,
        label: String,
        directions: Vec<String>,
    },
    Dirs(Vec<String>),
}

impl Val {
    fn text(&self) -> &str {
        match self {
            Val::Text(s) => s,
            _ => "",
        }
    }

    /// `typeof $1.length === 'number' ? $1 : [$1]`, with the one level of
    /// `.flat()` that `populateBlockDatabase` applies.
    fn into_blocks(self) -> Vec<ParsedBlock> {
        match self {
            Val::Block(b) => vec![*b],
            Val::Blocks(v) => v,
            _ => Vec::new(),
        }
    }

    fn node(&self) -> NodeVal {
        match self {
            Val::Node(n) => n.clone(),
            _ => NodeVal::default(),
        }
    }

    fn dirs(&self) -> Vec<String> {
        match self {
            Val::Dirs(d) => d.clone(),
            _ => Vec::new(),
        }
    }
}

/// `parseInt(s)`: the leading integer, or 0 when there is none (mermaid
/// would carry `NaN`, which the layout treats as 0 in every sum).
fn parse_int(s: &str) -> i64 {
    let t = s.trim();
    let (sign, digits) = match t.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, t.strip_prefix('+').unwrap_or(t)),
    };
    let end = digits
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit())
        .map_or(digits.len(), |(i, _)| i);
    digits[..end].parse::<i64>().map_or(0, |v| sign * v)
}

/// The actions and the hierarchy `yy.setHierarchy` receives.
#[derive(Debug, Default)]
pub struct BlockActions {
    pub hierarchy: Vec<ParsedBlock>,
    /// `cnt` of `generateId`, shared with the db afterwards.
    cnt: usize,
}

impl BlockActions {
    /// `generateId()` without the random part, so ids are stable.
    fn generate_id(&mut self) -> String {
        self.cnt += 1;
        format!("id-{}", self.cnt)
    }

    /// `nodeStatement: node` and `node SIZE` share this object shape.
    fn node_block(node: NodeVal, width_in_columns: i64) -> ParsedBlock {
        ParsedBlock {
            id: node.id,
            label: node.label,
            kind: type_str_to_type(node.type_str.as_deref()),
            directions: node.directions,
            width_in_columns: Some(width_in_columns),
            ..ParsedBlock::default()
        }
    }
}

impl Yy for BlockActions {}

impl Actions for BlockActions {
    type Value = Val;

    fn terminal(&mut self, yytext: &str) -> Val {
        Val::Text(yytext.to_string())
    }

    fn reduce(
        &mut self,
        production: usize,
        rhs: &[Val],
        locs: &[Loc],
        _yytext: &str,
    ) -> Result<Val, ParseError> {
        let at = |i: usize| rhs.get(i).cloned().unwrap_or(Val::Nil);
        let value = match production {
            // start: BLOCK_DIAGRAM_KEY document EOF { yy.setHierarchy($2); }
            7 => {
                self.hierarchy = at(1).into_blocks();
                Val::Nil
            }
            // spaceLines, separator, stop: logging only.
            1..=6 | 8..=11 => Val::Nil,
            // document: statement
            12 => Val::Blocks(at(0).into_blocks()),
            // document: statement document { $$ = [$1].concat($2); }
            13 => {
                let mut blocks = at(0).into_blocks();
                blocks.extend(at(1).into_blocks());
                Val::Blocks(blocks)
            }
            // link: LINK { $$ = {edgeTypeStr: $1, label: ''}; }
            14 => Val::Link {
                edge_type_str: at(0).text().to_string(),
                label: String::new(),
            },
            // link: START_LINK LINK_LABEL STR LINK { $$ = {edgeTypeStr: $4, label: $3}; }
            15 => Val::Link {
                edge_type_str: at(3).text().to_string(),
                label: at(2).text().to_string(),
            },
            // statement: nodeStatement | columnsStatement | blockStatement |
            // classDefStatement | cssClassStatement | styleStatement
            16 | 17 | 19..=22 => at(0),
            // statement: SPACE_BLOCK
            18 => {
                let num = parse_int(at(0).text());
                let id = self.generate_id();
                Val::Block(Box::new(ParsedBlock {
                    id,
                    kind: BlockType::Space,
                    label: Some(String::new()),
                    width: Some(num),
                    ..ParsedBlock::default()
                }))
            }
            // nodeStatement: nodeStatement link node
            23 => {
                let (edge_type_str, label) = match at(1) {
                    Val::Link {
                        edge_type_str,
                        label,
                    } => (edge_type_str, label),
                    _ => (String::new(), String::new()),
                };
                // `$1.id` of an array is `undefined` in mermaid; a chain of
                // three nodes breaks there too, so the first object keeps
                // whatever the left side carried.
                let first = match at(0) {
                    Val::Block(b) => *b,
                    Val::Blocks(mut v) if !v.is_empty() => v.remove(0),
                    _ => ParsedBlock::default(),
                };
                let node = at(2).node();
                let edge = ParsedBlock {
                    id: format!("{}-{}", first.id, node.id),
                    start: Some(first.id.clone()),
                    end: Some(node.id.clone()),
                    label: Some(label),
                    kind: BlockType::Edge,
                    thickness: Some(edge_str_to_thickness(&edge_type_str).to_string()),
                    pattern: Some(edge_str_to_pattern(&edge_type_str).to_string()),
                    directions: node.directions.clone(),
                    arrow_type_end: Some(edge_str_to_edge_data(&edge_type_str).to_string()),
                    arrow_type_start: Some(edge_str_to_edge_start_data(&edge_type_str).to_string()),
                    ..ParsedBlock::default()
                };
                let left = ParsedBlock {
                    id: first.id,
                    label: first.label,
                    kind: first.kind,
                    directions: first.directions,
                    ..ParsedBlock::default()
                };
                let right = ParsedBlock {
                    id: node.id,
                    label: node.label,
                    kind: type_str_to_type(node.type_str.as_deref()),
                    directions: node.directions,
                    ..ParsedBlock::default()
                };
                Val::Blocks(vec![left, edge, right])
            }
            // nodeStatement: node SIZE
            24 => Val::Block(Box::new(BlockActions::node_block(
                at(0).node(),
                parse_int(at(1).text()),
            ))),
            // nodeStatement: node
            25 => Val::Block(Box::new(BlockActions::node_block(at(0).node(), 1))),
            // columnsStatement: COLUMNS
            26 => {
                let text = at(0).text().to_string();
                let columns = if text == "auto" { -1 } else { parse_int(&text) };
                Val::Block(Box::new(ParsedBlock {
                    kind: BlockType::ColumnSetting,
                    columns: Some(columns),
                    ..ParsedBlock::default()
                }))
            }
            // blockStatement: id-block nodeStatement document end
            27 => {
                let mut block = match at(1) {
                    Val::Block(b) => *b,
                    Val::Blocks(mut v) if !v.is_empty() => v.remove(0),
                    _ => ParsedBlock::default(),
                };
                let _ = self.generate_id();
                block.kind = BlockType::Composite;
                block.children = at(2).into_blocks();
                Val::Block(Box::new(block))
            }
            // blockStatement: BLOCK_DIAGRAM_KEY document end
            28 => {
                let id = self.generate_id();
                Val::Block(Box::new(ParsedBlock {
                    id,
                    kind: BlockType::Composite,
                    label: Some(String::new()),
                    children: at(1).into_blocks(),
                    ..ParsedBlock::default()
                }))
            }
            // node: NODE_ID
            29 => Val::Node(NodeVal {
                id: at(0).text().to_string(),
                ..NodeVal::default()
            }),
            // node: NODE_ID nodeShapeNLabel
            30 => {
                let (type_str, label, directions) = match at(1) {
                    Val::ShapeLabel {
                        type_str,
                        label,
                        directions,
                    } => (Some(type_str), Some(label), directions),
                    _ => (None, None, Vec::new()),
                };
                Val::Node(NodeVal {
                    id: at(0).text().to_string(),
                    label,
                    type_str,
                    directions,
                })
            }
            // dirList: DIR
            31 => Val::Dirs(vec![at(0).text().to_string()]),
            // dirList: DIR dirList
            32 => {
                let mut dirs = vec![at(0).text().to_string()];
                dirs.extend(at(1).dirs());
                Val::Dirs(dirs)
            }
            // nodeShapeNLabel: NODE_DSTART STR NODE_DEND
            33 => Val::ShapeLabel {
                type_str: format!("{}{}", at(0).text(), at(2).text()),
                label: at(1).text().to_string(),
                directions: Vec::new(),
            },
            // nodeShapeNLabel: BLOCK_ARROW_START STR dirList BLOCK_ARROW_END
            34 => Val::ShapeLabel {
                type_str: format!("{}{}", at(0).text(), at(3).text()),
                label: at(1).text().to_string(),
                directions: at(2).dirs(),
            },
            // classDefStatement: classDef CLASSDEF_ID CLASSDEF_STYLEOPTS (and DEFAULT)
            35 | 36 => Val::Block(Box::new(ParsedBlock {
                kind: BlockType::ClassDef,
                id: js_trim(at(1).text()).to_string(),
                css: Some(js_trim(at(2).text()).to_string()),
                ..ParsedBlock::default()
            })),
            // cssClassStatement: class CLASSENTITY_IDS STYLECLASS
            37 => Val::Block(Box::new(ParsedBlock {
                kind: BlockType::ApplyClass,
                id: js_trim(at(1).text()).to_string(),
                style_class: Some(js_trim(at(2).text()).to_string()),
                ..ParsedBlock::default()
            })),
            // styleStatement: style STYLE_ENTITY_IDS STYLE_DEFINITION_DATA
            38 => Val::Block(Box::new(ParsedBlock {
                kind: BlockType::ApplyStyles,
                id: js_trim(at(1).text()).to_string(),
                styles_str: Some(js_trim(at(2).text()).to_string()),
                ..ParsedBlock::default()
            })),
            other => {
                let loc = locs.first().copied().unwrap_or_default();
                return Err(ParseError::action(
                    loc.first_line,
                    loc.first_column,
                    format!("block grammar has no production {other}"),
                ));
            }
        };
        Ok(value)
    }
}

impl BlockActions {
    /// Moves the parse result into a fresh database, as `setHierarchy` does.
    pub fn into_db(self) -> Result<db::BlockDb, db::DbError> {
        let mut database = db::BlockDb::new();
        database.set_hierarchy(self.hierarchy)?;
        Ok(database)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_int_reads_a_leading_integer() {
        assert_eq!(parse_int("3"), 3);
        assert_eq!(parse_int("-1"), -1);
        assert_eq!(parse_int("2px"), 2);
        assert_eq!(parse_int("auto"), 0);
    }
}
