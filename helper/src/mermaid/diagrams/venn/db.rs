//! Port of `diagrams/venn/vennDB.ts` (mermaid 12.2.0) and the title half
//! of `commonDb.ts`.

use indexmap::IndexMap;

use super::layout::js_str_cmp;
use crate::mermaid::jison::lexer::js_trim;

/// `VennData`: one `set` or `union` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct VennData {
    pub sets: Vec<String>,
    pub size: f64,
    pub label: Option<String>,
}

/// `VennTextData`: one `text` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct VennTextData {
    pub sets: Vec<String>,
    pub id: String,
    pub label: Option<String>,
}

/// `VennStyleData`: one `style` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct VennStyleData {
    pub targets: Vec<String>,
    pub styles: IndexMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct VennDb {
    subsets: Vec<VennData>,
    text_nodes: Vec<VennTextData>,
    style_entries: Vec<VennStyleData>,
    known_sets: Vec<String>,
    current_sets: Option<Vec<String>>,
    indent_mode: bool,
    /// `yy.consumeIndentText`, the property the lexer sets on the db.
    pub(super) consume_indent_text: bool,
    title: Option<String>,
}

/// `normalizeText`: trimmed, one pair of surrounding double quotes removed.
pub fn normalize_text(text: &str) -> String {
    let trimmed = js_trim(text);
    let utf16_len = trimmed.encode_utf16().count();
    if utf16_len >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        return trimmed[1..trimmed.len() - 1].to_string();
    }
    trimmed.to_string()
}

fn normalize_identifier_list(list: &[String]) -> Vec<String> {
    list.iter().map(|s| normalize_text(s)).collect()
}

/// `normalizeIdentifierList(list).sort()`.
fn sorted_identifiers(list: &[String]) -> Vec<String> {
    let mut sets = normalize_identifier_list(list);
    sets.sort_by(|a, b| js_str_cmp(a, b));
    sets
}

impl VennDb {
    pub fn new() -> VennDb {
        VennDb::default()
    }

    /// `addSubsetData(identifierList, label, size)`.
    pub fn add_subset_data(&mut self, list: &[String], label: Option<&str>, size: Option<f64>) {
        let sets = sorted_identifiers(list);
        let n = list.len() as f64;
        let resolved_size = size.unwrap_or(10.0 / n.powi(2));
        self.current_sets = Some(sets.clone());
        if sets.len() == 1 && !self.known_sets.contains(&sets[0]) {
            self.known_sets.push(sets[0].clone());
        }
        self.subsets.push(VennData {
            sets,
            size: resolved_size,
            label: label.filter(|l| !l.is_empty()).map(normalize_text),
        });
    }

    /// `addTextData(identifierList, id, label)`.
    pub fn add_text_data(&mut self, list: &[String], id: &str, label: Option<&str>) {
        self.text_nodes.push(VennTextData {
            sets: sorted_identifiers(list),
            id: normalize_text(id),
            label: label.filter(|l| !l.is_empty()).map(normalize_text),
        });
    }

    /// `addStyleData(identifierList, data)`.
    pub fn add_style_data(&mut self, list: &[String], data: &[(String, String)]) {
        let targets = sorted_identifiers(list);
        let mut styles = IndexMap::new();
        for (key, value) in data {
            // `normalizeStyleValue(value) ?? value`: an empty value stays.
            let v = if value.is_empty() {
                value.clone()
            } else {
                normalize_text(value)
            };
            styles.insert(key.clone(), v);
        }
        self.style_entries.push(VennStyleData { targets, styles });
    }

    /// `validateUnionIdentifiers(identifierList)`.
    pub fn validate_union_identifiers(&self, list: &[String]) -> Result<(), String> {
        let unknown: Vec<String> = normalize_identifier_list(list)
            .into_iter()
            .filter(|id| !self.known_sets.contains(id))
            .collect();
        if unknown.is_empty() {
            Ok(())
        } else {
            Err(format!("unknown set identifier: {}", unknown.join(", ")))
        }
    }

    pub fn subset_data(&self) -> &[VennData] {
        &self.subsets
    }

    pub fn text_data(&self) -> &[VennTextData] {
        &self.text_nodes
    }

    pub fn style_data(&self) -> &[VennStyleData] {
        &self.style_entries
    }

    pub fn current_sets(&self) -> Option<&[String]> {
        self.current_sets.as_deref()
    }

    pub fn indent_mode(&self) -> bool {
        self.indent_mode
    }

    pub fn set_indent_mode(&mut self, enabled: bool) {
        self.indent_mode = enabled;
    }

    /// `commonDb.setDiagramTitle`. `sanitizeText` leaves plain text as it
    /// is; HTML in a title is not sanitized here.
    pub fn set_diagram_title(&mut self, title: &str) {
        self.title = Some(title.to_string());
    }

    /// `getDiagramTitle()` as `draw` tests it: a non-empty string.
    pub fn diagram_title(&self) -> Option<&str> {
        self.title.as_deref().filter(|t| !t.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn subsets_sort_their_sets_and_default_the_size() {
        let mut db = VennDb::new();
        db.add_subset_data(&ids(&["B"]), None, None);
        db.add_subset_data(&ids(&["A"]), Some("Alpha"), Some(20.0));
        db.add_subset_data(&ids(&["B", "A"]), None, None);
        db.add_subset_data(&ids(&["C", "\"A\"", "B"]), Some(""), None);
        let s = db.subset_data();
        assert_eq!(s[0].size, 10.0);
        assert_eq!(s[1].label.as_deref(), Some("Alpha"));
        assert_eq!(s[2].sets, ids(&["A", "B"]));
        assert_eq!(s[2].size, 2.5);
        assert_eq!(s[3].sets, ids(&["A", "B", "C"]));
        assert_eq!(s[3].size, 10.0 / 9.0);
        assert_eq!(s[3].label, None);
        assert_eq!(db.current_sets(), Some(&ids(&["A", "B", "C"])[..]));
    }

    #[test]
    fn unions_of_unknown_sets_are_rejected() {
        let mut db = VennDb::new();
        db.add_subset_data(&ids(&["A"]), None, None);
        assert_eq!(
            db.validate_union_identifiers(&ids(&["A", "X", "\"Y\""])),
            Err("unknown set identifier: X, Y".to_string())
        );
    }
}
