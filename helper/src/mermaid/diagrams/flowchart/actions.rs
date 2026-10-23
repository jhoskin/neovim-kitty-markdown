//! The grammar actions of `flow.jison` (mermaid 12.2.0), keyed by
//! production index in `helper/grammars/flow.tables.json`. Each arm cites
//! the production and does what its JavaScript action does to the db; a
//! production without an action yields `$1`, as jison's default.
//!
//! The lexer asks `yy.lex.firstGraph()` once per `graph`/`flowchart`
//! keyword, so the db answers that hook through [`Yy`].

use super::db::{DocItem, FlowDb, FlowText, LinkInfo, LinkPos};
use crate::mermaid::jison::{Actions, Lit, Loc, ParseError, Yy};

/// A semantic value on jison's value stack.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Val {
    #[default]
    Undef,
    Str(String),
    Text(FlowText),
    /// A `document`: the statements of a body, already flattened one level
    /// the way `addSubGraph` flattens it with `list.flat()`.
    Doc(Vec<DocItem>),
    /// `node`: the ids of an `A & B` group; also `stylesOpt` and `numList`.
    List(Vec<String>),
    Link(LinkInfo),
    /// `vertexStatement`: `{ stmt, nodes }`.
    Vertex {
        stmt: Vec<String>,
        nodes: Vec<String>,
    },
    /// `direction`: `{ stmt: 'dir', value }`.
    Dir(String),
}

impl Val {
    fn str(&self) -> String {
        match self {
            Val::Str(s) => s.clone(),
            Val::Text(t) => t.text.clone(),
            Val::Dir(d) => d.clone(),
            _ => String::new(),
        }
    }

    fn text(&self) -> Option<FlowText> {
        match self {
            Val::Text(t) => Some(t.clone()),
            _ => None,
        }
    }

    fn list(&self) -> Vec<String> {
        match self {
            Val::List(l) => l.clone(),
            Val::Str(s) => vec![s.clone()],
            _ => Vec::new(),
        }
    }

    fn doc(&self) -> Vec<DocItem> {
        match self {
            Val::Doc(d) => d.clone(),
            _ => Vec::new(),
        }
    }

    fn link(&self) -> LinkInfo {
        match self {
            Val::Link(l) => l.clone(),
            _ => LinkInfo::default(),
        }
    }

    fn vertex(&self) -> (Vec<String>, Vec<String>) {
        match self {
            Val::Vertex { stmt, nodes } => (stmt.clone(), nodes.clone()),
            _ => (Vec::new(), Vec::new()),
        }
    }
}

impl Yy for FlowDb {
    fn yy_call(&mut self, name: &str, _args: &[Lit]) -> Lit {
        if name == "lex.firstGraph" {
            return Lit::Bool(self.first_graph());
        }
        Lit::Undefined
    }
}

/// `$text.text + '' + $token` keeping `$text.type`.
fn concat_text(head: &Val, tail: &Val) -> Val {
    let (text, kind) = match head {
        Val::Text(t) => (t.text.clone(), t.kind.clone()),
        other => (other.str(), "text".to_string()),
    };
    Val::Text(FlowText {
        text: text + &tail.str(),
        kind,
    })
}

/// `linkStyle` positions: `[$DEFAULT]` or the `numList`.
fn positions(v: &Val, default: bool) -> Vec<LinkPos> {
    if default {
        return vec![LinkPos::Default];
    }
    v.list().into_iter().map(LinkPos::Index).collect()
}

impl FlowDb {
    /// `destructStartLink(str)`.
    fn destruct_start_link(s: &str) -> (String, String) {
        let mut s: &str = crate::mermaid::directive::js_trim(s);
        let mut kind = "arrow_open";
        match s.chars().next() {
            Some('<') => {
                kind = "arrow_point";
                s = &s[1..];
            }
            Some('x') => {
                kind = "arrow_cross";
                s = &s[1..];
            }
            Some('o') => {
                kind = "arrow_circle";
                s = &s[1..];
            }
            _ => {}
        }
        let mut stroke = "normal";
        if s.contains('=') {
            stroke = "thick";
        }
        if s.contains('.') {
            stroke = "dotted";
        }
        (kind.to_string(), stroke.to_string())
    }

    /// `destructEndLink(str)`: the head, the stroke, and the length (the
    /// dash count less one, or the dot count).
    fn destruct_end_link(s: &str) -> LinkInfo {
        let s = crate::mermaid::directive::js_trim(s);
        let chars: Vec<char> = s.chars().collect();
        let mut line: Vec<char> = chars[..chars.len().saturating_sub(1)].to_vec();
        let mut kind = "arrow_open".to_string();
        let first = chars.first().copied();
        let double = |head: &str, opener: char, line: &mut Vec<char>| -> String {
            if first == Some(opener) {
                if !line.is_empty() {
                    line.remove(0);
                }
                format!("double_{head}")
            } else {
                head.to_string()
            }
        };
        match chars.last() {
            Some('x') => kind = double("arrow_cross", 'x', &mut line),
            Some('>') => kind = double("arrow_point", '<', &mut line),
            Some('o') => kind = double("arrow_circle", 'o', &mut line),
            _ => {}
        }
        let mut stroke = "normal";
        let mut length = line.len() as f64 - 1.0;
        if line.first() == Some(&'=') {
            stroke = "thick";
        }
        if line.first() == Some(&'~') {
            stroke = "invisible";
        }
        let dots = line.iter().filter(|c| **c == '.').count();
        if dots > 0 {
            stroke = "dotted";
            length = dots as f64;
        }
        LinkInfo {
            kind: Some(kind),
            stroke: Some(stroke.to_string()),
            length: Some(length),
            ..LinkInfo::default()
        }
    }

    /// `destructLink(str, startStr)`.
    pub fn destruct_link(end: &str, start: Option<&str>) -> LinkInfo {
        let info = Self::destruct_end_link(end);
        let Some(start) = start.filter(|s| !s.is_empty()) else {
            return info;
        };
        let invalid = LinkInfo {
            kind: Some("INVALID".to_string()),
            stroke: Some("INVALID".to_string()),
            ..LinkInfo::default()
        };
        let (mut kind, stroke) = Self::destruct_start_link(start);
        if Some(stroke.as_str()) != info.stroke.as_deref() {
            return invalid;
        }
        if kind == "arrow_open" {
            kind = info.kind.clone().unwrap_or_default();
        } else {
            if Some(kind.as_str()) != info.kind.as_deref() {
                return invalid;
            }
            kind = format!("double_{kind}");
        }
        if kind == "double_arrow" {
            kind = "double_arrow_point".to_string();
        }
        LinkInfo {
            kind: Some(kind),
            stroke: Some(stroke),
            length: info.length,
            ..LinkInfo::default()
        }
    }
}

impl Actions for FlowDb {
    type Value = Val;

    fn terminal(&mut self, yytext: &str) -> Val {
        Val::Str(yytext.to_string())
    }

    fn reduce(
        &mut self,
        production: usize,
        rhs: &[Val],
        locs: &[Loc],
        _yytext: &str,
    ) -> Result<Val, ParseError> {
        let at = |i: usize| rhs.get(i).cloned().unwrap_or_default();
        let fail = |message: String| {
            let loc = locs.first().copied().unwrap_or_default();
            ParseError::action(loc.first_line, loc.first_column, message)
        };
        let value = match production {
            // document: /* empty */ { $$ = []; }
            2 => Val::Doc(Vec::new()),
            // document: document line { if (!Array.isArray($line) ||
            // $line.length > 0) $document.push($line); }
            3 => {
                let mut doc = at(0).doc();
                match at(1) {
                    Val::List(ids) => doc.extend(ids.into_iter().map(DocItem::Id)),
                    Val::Dir(d) => doc.push(DocItem::Dir(d)),
                    Val::Str(s) => doc.push(DocItem::Id(s)),
                    Val::Text(t) => doc.push(DocItem::Id(t.text)),
                    _ => {}
                }
                Val::Doc(doc)
            }
            // graphConfig: GRAPH NODIR { yy.setDirection('TB') }
            11 => {
                self.set_direction("TB");
                Val::Str("TB".to_string())
            }
            // graphConfig: GRAPH DIR FirstStmtSeparator { yy.setDirection($DIR) }
            12 => {
                let dir = at(1).str();
                self.set_direction(&dir);
                Val::Str(dir)
            }
            // statement: vertexStatement separator { $$ = $vertexStatement.nodes }
            27 => Val::List(at(0).vertex().1),
            // statement: style/linkStyle/classDef/class/click separator { $$ = [] }
            28..=32 => Val::List(Vec::new()),
            // statement: subgraph SPACE textNoTags SQS text SQE separator
            // document end { yy.addSubGraph($textNoTags, $document, $text) }
            33 => {
                let id = self
                    .add_sub_graph(
                        at(2).text().as_ref(),
                        &at(7).doc(),
                        at(4).text().as_ref(),
                        false,
                    )
                    .map_err(fail)?;
                Val::Str(id)
            }
            // statement: subgraph SPACE textNoTags separator document end
            // { yy.addSubGraph($textNoTags, $document, $textNoTags) }
            34 => {
                let t = at(2).text();
                let id = self
                    .add_sub_graph(t.as_ref(), &at(4).doc(), t.as_ref(), true)
                    .map_err(fail)?;
                Val::Str(id)
            }
            // statement: subgraph separator document end
            // { yy.addSubGraph(undefined, $document, undefined) }
            35 => {
                let id = self
                    .add_sub_graph(None, &at(2).doc(), None, false)
                    .map_err(fail)?;
                Val::Str(id)
            }
            // statement: acc_title acc_title_value
            37 => {
                let v = crate::mermaid::directive::js_trim(&at(1).str()).to_string();
                self.acc_title = v.clone();
                Val::Str(v)
            }
            // statement: acc_descr acc_descr_value
            38 => {
                let v = crate::mermaid::directive::js_trim(&at(1).str()).to_string();
                self.acc_description = v.clone();
                Val::Str(v)
            }
            // statement: acc_descr_multiline_value
            39 => {
                let v = crate::mermaid::directive::js_trim(&at(0).str()).to_string();
                self.acc_description = v.clone();
                Val::Str(v)
            }
            // shapeData: shapeData SHAPE_DATA { $$ = $1 + $2 }
            43 => Val::Str(at(0).str() + &at(1).str()),
            // vertexStatement: vertexStatement link node shapeData
            45 => {
                let node = at(2).list();
                if let Some(last) = node.last() {
                    let data = at(3).str();
                    self.add_vertex(last, None, None, None, None, None, None, Some(&data))
                        .map_err(fail)?;
                }
                let (stmt, nodes) = at(0).vertex();
                self.add_link(&stmt, &node, &at(1).link()).map_err(fail)?;
                let mut all = node.clone();
                all.extend(nodes);
                Val::Vertex {
                    stmt: node,
                    nodes: all,
                }
            }
            // vertexStatement: vertexStatement link node [spaceList]
            46 | 47 => {
                let node = at(2).list();
                let (stmt, nodes) = at(0).vertex();
                self.add_link(&stmt, &node, &at(1).link()).map_err(fail)?;
                let mut all = node.clone();
                all.extend(nodes);
                Val::Vertex {
                    stmt: node,
                    nodes: all,
                }
            }
            // vertexStatement: node spaceList | node
            48 | 50 => {
                let node = at(0).list();
                Val::Vertex {
                    stmt: node.clone(),
                    nodes: node,
                }
            }
            // vertexStatement: node shapeData
            49 => {
                let node = at(0).list();
                if let Some(last) = node.last() {
                    let data = at(1).str();
                    self.add_vertex(last, None, None, None, None, None, None, Some(&data))
                        .map_err(fail)?;
                }
                Val::Vertex {
                    stmt: node.clone(),
                    nodes: node,
                }
            }
            // node: styledVertex { $$ = [$styledVertex] }
            51 => Val::List(vec![at(0).str()]),
            // node: node shapeData spaceList AMP spaceList styledVertex
            52 => {
                let mut node = at(0).list();
                if let Some(last) = node.last() {
                    let data = at(1).str();
                    self.add_vertex(last, None, None, None, None, None, None, Some(&data))
                        .map_err(fail)?;
                }
                node.push(at(5).str());
                Val::List(node)
            }
            // node: node spaceList AMP spaceList styledVertex
            53 => {
                let mut node = at(0).list();
                node.push(at(4).str());
                Val::List(node)
            }
            // styledVertex: vertex STYLE_SEPARATOR idString { yy.setClass($vertex, $idString) }
            55 => {
                let v = at(0).str();
                self.set_class(&v, &at(2).str());
                Val::Str(v)
            }
            // vertex: idString <open> text <close> { yy.addVertex($idString, $text, '<type>') }
            56..=61 | 63..=71 => {
                let (text_index, kind) = match production {
                    56 => (2, "square"),
                    57 => (2, "doublecircle"),
                    58 => (3, "circle"),
                    59 => (2, "ellipse"),
                    60 => (2, "stadium"),
                    61 => (2, "subroutine"),
                    63 => (2, "cylinder"),
                    64 => (2, "round"),
                    65 => (2, "diamond"),
                    66 => (3, "hexagon"),
                    67 => (2, "odd"),
                    68 => (2, "trapezoid"),
                    69 => (2, "inv_trapezoid"),
                    70 => (2, "lean_right"),
                    _ => (2, "lean_left"),
                };
                let id = at(0).str();
                self.add_vertex(
                    &id,
                    at(text_index).text().as_ref(),
                    Some(kind),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .map_err(fail)?;
                Val::Str(id)
            }
            // vertex: idString VERTEX_WITH_PROPS_START NODE_STRING[field]
            // COLON NODE_STRING[value] PIPE text SQE
            62 => {
                let id = at(0).str();
                let mut props = serde_json::Map::new();
                props.insert(at(2).str(), serde_json::Value::String(at(4).str()));
                self.add_vertex(
                    &id,
                    at(6).text().as_ref(),
                    Some("rect"),
                    None,
                    None,
                    None,
                    Some(props),
                    None,
                )
                .map_err(fail)?;
                Val::Str(id)
            }
            // vertex: idString { yy.addVertex($idString) }
            72 => {
                let id = at(0).str();
                self.add_vertex(&id, None, None, None, None, None, None, None)
                    .map_err(fail)?;
                Val::Str(id)
            }
            // link: linkStatement arrowText [SPACE] { $linkStatement.text = $arrowText }
            73 | 75 => {
                let mut link = at(0).link();
                link.text = at(1).text();
                Val::Link(link)
            }
            // link: linkStatement TESTSTR SPACE: the text is a bare string,
            // and `addSingleLink` reads `.text.trim()` off it.
            74 => {
                return Err(fail(
                    "TypeError: Cannot read properties of undefined (reading 'trim')".to_string(),
                ));
            }
            // link: START_LINK edgeText LINK
            77 => {
                let mut link = Self::destruct_link(&at(2).str(), Some(&at(0).str()));
                link.text = at(1).text();
                Val::Link(link)
            }
            // link: LINK_ID START_LINK edgeText LINK
            78 => {
                let mut link = Self::destruct_link(&at(3).str(), Some(&at(1).str()));
                link.text = at(2).text();
                link.id = Some(at(0).str());
                Val::Link(link)
            }
            // edgeText: edgeTextToken { $$ = {text: $1, type: 'text'} }
            79 | 86 | 101 => Val::Text(FlowText::new(&at(0).str(), "text")),
            // edgeText: edgeText edgeTextToken
            80 | 87 | 102 => concat_text(&at(0), &at(1)),
            // edgeText: STR { type: 'string' }
            81 | 88 => Val::Text(FlowText::new(&at(0).str(), "string")),
            // edgeText: MD_STR { type: 'markdown' }
            82 | 89 | 104 => Val::Text(FlowText::new(&at(0).str(), "markdown")),
            // textNoTags: STR { type: 'text' }
            103 => Val::Text(FlowText::new(&at(0).str(), "text")),
            // linkStatement: LINK { yy.destructLink($LINK) }
            83 => Val::Link(Self::destruct_link(&at(0).str(), None)),
            // linkStatement: LINK_ID LINK
            84 => {
                let mut link = Self::destruct_link(&at(1).str(), None);
                link.id = Some(at(0).str());
                Val::Link(link)
            }
            // arrowText: PIPE text PIPE
            85 => at(1),
            // classDefStatement: CLASSDEF SPACE idString SPACE stylesOpt
            105 => {
                self.add_class(&at(2).str(), &at(4).list());
                at(0)
            }
            // classStatement: CLASS SPACE idString SPACE idString
            106 => {
                self.set_class(&at(2).str(), &at(4).str());
                at(0)
            }
            // clickStatement: CLICK CALLBACKNAME ... / CLICK alphaNum ...
            107 | 109 | 115 => {
                self.set_click_event(&at(0).str());
                at(0)
            }
            108 | 110 | 116 => {
                let click = at(0).str();
                self.set_click_event(&click);
                let tooltip = at(rhs.len() - 1).str();
                self.set_tooltip(&click, &tooltip);
                at(0)
            }
            // clickStatement: CLICK HREF STR ...
            111 => {
                self.set_link(&at(0).str(), &at(2).str(), None);
                at(0)
            }
            112 => {
                let click = at(0).str();
                self.set_link(&click, &at(2).str(), None);
                self.set_tooltip(&click, &at(4).str());
                at(0)
            }
            113 => {
                self.set_link(&at(0).str(), &at(2).str(), Some(&at(4).str()));
                at(0)
            }
            114 => {
                let click = at(0).str();
                self.set_link(&click, &at(2).str(), Some(&at(6).str()));
                self.set_tooltip(&click, &at(4).str());
                at(0)
            }
            // clickStatement: CLICK STR ...
            117 => {
                self.set_link(&at(0).str(), &at(1).str(), None);
                at(0)
            }
            118 => {
                let click = at(0).str();
                self.set_link(&click, &at(1).str(), None);
                self.set_tooltip(&click, &at(3).str());
                at(0)
            }
            119 => {
                self.set_link(&at(0).str(), &at(1).str(), Some(&at(3).str()));
                at(0)
            }
            120 => {
                let click = at(0).str();
                self.set_link(&click, &at(1).str(), Some(&at(5).str()));
                self.set_tooltip(&click, &at(3).str());
                at(0)
            }
            // styleStatement: STYLE SPACE idString SPACE stylesOpt
            121 => {
                let styles = at(4).list();
                self.add_vertex(
                    &at(2).str(),
                    None,
                    None,
                    Some(&styles),
                    None,
                    None,
                    None,
                    None,
                )
                .map_err(fail)?;
                at(0)
            }
            // linkStyleStatement: LINKSTYLE SPACE (DEFAULT|numList) SPACE stylesOpt
            122 | 123 => {
                self.update_link(&positions(&at(2), production == 122), &at(4).list())
                    .map_err(fail)?;
                at(0)
            }
            // ... INTERPOLATE SPACE alphaNum SPACE stylesOpt
            124 | 125 => {
                let pos = positions(&at(2), production == 124);
                self.update_link_interpolate(&pos, &at(6).str())
                    .map_err(fail)?;
                self.update_link(&pos, &at(8).list()).map_err(fail)?;
                at(0)
            }
            // ... INTERPOLATE SPACE alphaNum
            126 | 127 => {
                let pos = positions(&at(2), production == 126);
                self.update_link_interpolate(&pos, &at(6).str())
                    .map_err(fail)?;
                at(0)
            }
            // numList: NUM { [$NUM] } | numList COMMA NUM; stylesOpt likewise
            128 | 130 => Val::List(vec![at(0).str()]),
            129 | 131 => {
                let mut list = at(0).list();
                list.push(at(2).str());
                Val::List(list)
            }
            // style: style styleComponent; idString and alphaNum concatenate
            133 | 182 | 184 => Val::Str(at(0).str() + &at(1).str()),
            // direction: direction_xx { $$ = {stmt: 'dir', value: 'XX'} }
            185 => Val::Dir("TB".to_string()),
            186 => Val::Dir("BT".to_string()),
            187 => Val::Dir("RL".to_string()),
            188 => Val::Dir("LR".to_string()),
            189 => Val::Dir("TD".to_string()),
            // Every other production has no action or only `$$ = $1`.
            _ => rhs.first().cloned().unwrap_or_default(),
        };
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(end: &str, start: Option<&str>) -> (String, String, Option<f64>) {
        let l = FlowDb::destruct_link(end, start);
        (
            l.kind.unwrap_or_default(),
            l.stroke.unwrap_or_default(),
            l.length,
        )
    }

    #[test]
    fn destruct_link_reads_heads_strokes_and_lengths() {
        let s = |a: &str, b: &str, n: f64| (a.to_string(), b.to_string(), Some(n));
        assert_eq!(link("-->", None), s("arrow_point", "normal", 1.0));
        assert_eq!(link("---", None), s("arrow_open", "normal", 1.0));
        assert_eq!(link("----->", None), s("arrow_point", "normal", 4.0));
        assert_eq!(link("==>", None), s("arrow_point", "thick", 1.0));
        assert_eq!(link("-.->", None), s("arrow_point", "dotted", 1.0));
        assert_eq!(link("-..->", None), s("arrow_point", "dotted", 2.0));
        assert_eq!(link("~~~", None), s("arrow_open", "invisible", 1.0));
        assert_eq!(link("<-->", None), s("double_arrow_point", "normal", 1.0));
        assert_eq!(link("o--o", None), s("double_arrow_circle", "normal", 1.0));
        assert_eq!(link("x--x", None), s("double_arrow_cross", "normal", 1.0));
        assert_eq!(link("--x", None), s("arrow_cross", "normal", 1.0));
        assert_eq!(
            link("-->", Some("<--")),
            s("double_arrow_point", "normal", 1.0)
        );
        assert_eq!(link("-->", Some("--")), s("arrow_point", "normal", 1.0));
        assert_eq!(link("==>", Some("--")).0, "INVALID");
    }
}
