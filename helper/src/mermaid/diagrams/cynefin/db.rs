//! Port of `cynefinDb.ts`, `cynefinParser.ts` (`populate`), and the parts
//! of `commonDb.ts` they use.

use super::parser::{Ast, DomainName};

/// `CynefinTransition`.
#[derive(Debug, Clone, PartialEq)]
pub struct CynefinTransition {
    pub from: DomainName,
    pub to: DomainName,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CynefinDb {
    /// `data.domains`, a `Map` keyed by name: a repeated block replaces the
    /// items of the first and keeps its position.
    domains: Vec<(DomainName, Vec<String>)>,
    transitions: Vec<CynefinTransition>,
    title: String,
    acc_title: String,
    acc_descr: String,
}

impl CynefinDb {
    pub fn new() -> CynefinDb {
        CynefinDb::default()
    }

    /// `setDiagramTitle`: `sanitizeText` with DOMPurify, which leaves plain
    /// text as it is; markup in a title is out of scope here.
    pub fn set_diagram_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn diagram_title(&self) -> &str {
        &self.title
    }

    pub fn acc_title(&self) -> &str {
        &self.acc_title
    }

    pub fn acc_descr(&self) -> &str {
        &self.acc_descr
    }

    /// `getDomains().get(name)?.items`.
    pub fn items(&self, name: DomainName) -> Option<&[String]> {
        self.domains
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, items)| items.as_slice())
    }

    pub fn transitions(&self) -> &[CynefinTransition] {
        &self.transitions
    }

    /// `populate`: `populateCommonDb` (an empty title or accessibility
    /// text leaves the current one), then `setDomains` and
    /// `setTransitions`.
    pub fn populate(&mut self, ast: &Ast) {
        if let Some(d) = ast.acc_descr.as_deref().filter(|d| !d.is_empty()) {
            self.acc_descr = d.to_string();
        }
        if let Some(t) = ast.acc_title.as_deref().filter(|t| !t.is_empty()) {
            self.acc_title = t.to_string();
        }
        if let Some(t) = ast.title.as_deref().filter(|t| !t.is_empty()) {
            self.set_diagram_title(t);
        }
        for block in &ast.domains {
            match self.domains.iter_mut().find(|(n, _)| *n == block.domain) {
                Some((_, items)) => *items = block.items.clone(),
                None => self.domains.push((block.domain, block.items.clone())),
            }
        }
        // `setTransitions`: a self-loop is skipped with a warning, and an
        // empty label is `undefined`.
        self.transitions = ast
            .transitions
            .iter()
            .filter(|t| {
                if t.from == t.to {
                    log::warn!(
                        "Cynefin: self-loop transition on domain \"{}\" is not meaningful and will be skipped.",
                        t.from.as_str()
                    );
                    return false;
                }
                true
            })
            .map(|t| CynefinTransition {
                from: t.from,
                to: t.to,
                label: t.label.clone().filter(|l| !l.is_empty()),
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;

    fn db(text: &str) -> CynefinDb {
        let mut db = CynefinDb::new();
        db.populate(&parse(text).unwrap());
        db
    }

    #[test]
    fn a_repeated_domain_replaces_its_items() {
        let db = db("cynefin-beta\ncomplex\n\"a\"\nclear\ncomplex\n\"b\"\n\"c\"\n");
        assert_eq!(
            db.items(DomainName::Complex),
            Some(&["b".to_string(), "c".to_string()][..])
        );
        assert_eq!(db.items(DomainName::Clear), Some(&[][..]));
        assert_eq!(db.items(DomainName::Chaotic), None);
        assert_eq!(db.domains[0].0, DomainName::Complex);
    }

    #[test]
    fn self_loops_and_empty_labels() {
        let db = db(
            "cynefin-beta\ncomplex --> complex : \"x\"\nclear --> chaotic : \"\"\nchaotic --> complex : \"up\"\n",
        );
        assert_eq!(
            db.transitions(),
            &[
                CynefinTransition {
                    from: DomainName::Clear,
                    to: DomainName::Chaotic,
                    label: None
                },
                CynefinTransition {
                    from: DomainName::Chaotic,
                    to: DomainName::Complex,
                    label: Some("up".to_string())
                },
            ]
        );
    }

    #[test]
    fn an_empty_title_keeps_the_front_matter_title() {
        let mut db = CynefinDb::new();
        db.set_diagram_title("Front");
        db.populate(&parse("cynefin-beta\ntitle\naccTitle: A\naccDescr: D\n").unwrap());
        assert_eq!(db.diagram_title(), "Front");
        assert_eq!(db.acc_title(), "A");
        assert_eq!(db.acc_descr(), "D");
    }
}
