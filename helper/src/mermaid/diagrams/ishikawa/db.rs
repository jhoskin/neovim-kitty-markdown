//! Port of `ishikawaDb.ts` (mermaid 12.2.0): the effect is the first node,
//! every later line is a cause whose depth comes from its indentation
//! relative to the first cause.

/// `IshikawaNode`: a label and its causes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IshikawaNode {
    pub text: String,
    pub children: Vec<IshikawaNode>,
}

/// `{ level, node }` of the JS stack; the node is the path of child
/// indexes from the root.
#[derive(Debug, Clone)]
struct StackEntry {
    level: usize,
    path: Vec<usize>,
}

#[derive(Debug, Default)]
pub struct IshikawaDb {
    root: Option<IshikawaNode>,
    stack: Vec<StackEntry>,
    base_level: Option<usize>,
    diagram_title: Option<String>,
}

impl IshikawaDb {
    pub fn new() -> IshikawaDb {
        IshikawaDb::default()
    }

    pub fn root(&self) -> Option<&IshikawaNode> {
        self.root.as_ref()
    }

    /// `setDiagramTitle(label)` runs for the effect; the renderer never
    /// draws it.
    pub fn diagram_title(&self) -> Option<&str> {
        self.diagram_title.as_deref()
    }

    pub fn set_diagram_title(&mut self, title: &str) {
        self.diagram_title = Some(title.to_string());
    }

    fn node_mut(&mut self, path: &[usize]) -> Option<&mut IshikawaNode> {
        let mut node = self.root.as_mut()?;
        for &i in path {
            node = node.children.get_mut(i)?;
        }
        Some(node)
    }

    /// `addNode(rawLevel, text)`.
    pub fn add_node(&mut self, raw_level: usize, text: &str) {
        let label = sanitize_text(text);
        if self.root.is_none() {
            self.root = Some(IshikawaNode {
                text: label.clone(),
                children: Vec::new(),
            });
            self.stack = vec![StackEntry {
                level: 0,
                path: Vec::new(),
            }];
            self.set_diagram_title(&label);
            return;
        }

        // `this.baseLevel ??= rawLevel`: the first cause sets the base, so
        // an effect indented deeper than its causes still nests them.
        let base = *self.base_level.get_or_insert(raw_level);
        // `level = rawLevel - baseLevel + 1`, at least 1.
        let level = (raw_level as isize - base as isize + 1).max(1) as usize;

        while self.stack.len() > 1 && self.stack.last().is_some_and(|e| e.level >= level) {
            self.stack.pop();
        }
        let parent_path = self
            .stack
            .last()
            .map(|e| e.path.clone())
            .unwrap_or_default();
        let Some(parent) = self.node_mut(&parent_path) else {
            return;
        };
        parent.children.push(IshikawaNode {
            text: label,
            children: Vec::new(),
        });
        let mut path = parent_path;
        path.push(parent.children.len() - 1);
        self.stack.push(StackEntry { level, path });
    }
}

/// `common.sanitizeText(text, config)` under `securityLevel: strict`:
/// DOMPurify parses the text as HTML and serializes it back, so a bare `&`,
/// `<`, or `>` comes out as an entity, which `.text()` then shows as
/// written. A `<br>` tag in any spelling comes back as `<br>`, which
/// `splitLines` then splits on.
pub fn sanitize_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c == '<'
            && let Some(len) = br_tag_len(rest)
        {
            out.push_str("<br>");
            rest = &rest[len..];
            continue;
        }
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// The byte length of a `<br>`, `<br/>`, or `<br />` tag at the start of
/// `text`, matched without case as `splitLines` does.
fn br_tag_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.len() < 4 || !bytes[1..3].eq_ignore_ascii_case(b"br") {
        return None;
    }
    let mut i = 3;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if bytes.get(i) == Some(&b'/') {
        i += 1;
    }
    (bytes.get(i) == Some(&b'>')).then_some(i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn causes_nest_by_indentation_relative_to_the_first_cause() {
        let mut db = IshikawaDb::new();
        db.add_node(4, "Problem");
        db.add_node(0, "Cause A");
        db.add_node(2, "Subcause A1");
        db.add_node(0, "Cause B");
        let root = db.root().expect("root");
        assert_eq!(root.text, "Problem");
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].children[0].text, "Subcause A1");
        assert_eq!(root.children[1].text, "Cause B");
        assert_eq!(db.diagram_title(), Some("Problem"));
    }

    #[test]
    fn a_shallower_line_than_the_base_is_a_top_level_cause() {
        let mut db = IshikawaDb::new();
        db.add_node(0, "E");
        db.add_node(4, "A");
        db.add_node(8, "A1");
        db.add_node(2, "B");
        let root = db.root().expect("root");
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[1].text, "B");
    }

    #[test]
    fn sanitize_escapes_markup_but_keeps_line_breaks() {
        assert_eq!(sanitize_text("a & b < c"), "a &amp; b &lt; c");
        assert_eq!(sanitize_text("one<br/>two<BR >x"), "one<br>two<br>x");
    }
}
