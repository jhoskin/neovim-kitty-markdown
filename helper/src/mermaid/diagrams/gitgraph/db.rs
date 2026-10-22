//! Port of `gitGraphAst.ts` with the `populate` step of `gitGraphParser.ts`
//! and the parts of `commonDb.ts` the diagram reads.

use indexmap::IndexMap;

use super::parser::{Ast, CommitKind, Direction, Statement, StatementKind};

/// `commitType` in `gitGraphTypes.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitType {
    Normal,
    Reverse,
    Highlight,
    Merge,
    CherryPick,
}

impl From<CommitKind> for CommitType {
    fn from(kind: CommitKind) -> CommitType {
        match kind {
            CommitKind::Normal => CommitType::Normal,
            CommitKind::Reverse => CommitType::Reverse,
            CommitKind::Highlight => CommitType::Highlight,
        }
    }
}

/// `Commit` in `gitGraphTypes.ts`.
#[derive(Debug, Clone, PartialEq)]
pub struct Commit {
    pub id: String,
    pub message: String,
    pub seq: u32,
    pub kind: CommitType,
    pub tags: Vec<String>,
    pub parents: Vec<String>,
    pub branch: String,
    /// The `type:` of a merge, which decides the symbol drawn.
    pub custom_type: Option<CommitType>,
    /// A merge with an `id:` shows a commit label.
    pub custom_id: bool,
}

impl Commit {
    /// `commit.customType ?? commit.type`.
    pub fn symbol_type(&self) -> CommitType {
        self.custom_type.unwrap_or(self.kind)
    }
}

/// An error the db throws while a statement runs, with the statement's
/// 1-based source line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbError {
    pub message: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq)]
struct BranchConfig {
    name: String,
    order: Option<f64>,
}

/// `GitGraphState`.
#[derive(Debug, Clone, PartialEq)]
pub struct GitGraphDb {
    commits: IndexMap<String, Commit>,
    head: Option<String>,
    branch_config: IndexMap<String, BranchConfig>,
    /// Branch name to the id of its head commit, `None` before the first.
    branches: IndexMap<String, Option<String>>,
    curr_branch: String,
    direction: Direction,
    seq: u32,
    title: String,
    acc_title: String,
    acc_descr: String,
}

impl GitGraphDb {
    /// The state after `ImperativeState` initializes it from the config.
    pub fn new(main_branch_name: &str, main_branch_order: Option<f64>) -> GitGraphDb {
        let mut branch_config = IndexMap::new();
        branch_config.insert(
            main_branch_name.to_string(),
            BranchConfig {
                name: main_branch_name.to_string(),
                order: main_branch_order,
            },
        );
        let mut branches = IndexMap::new();
        branches.insert(main_branch_name.to_string(), None);
        GitGraphDb {
            commits: IndexMap::new(),
            head: None,
            branch_config,
            branches,
            curr_branch: main_branch_name.to_string(),
            direction: Direction::LR,
            seq: 0,
            title: String::new(),
            acc_title: String::new(),
            acc_descr: String::new(),
        }
    }

    /// `setDiagramTitle`: `sanitizeText` leaves plain text as it is.
    pub fn set_diagram_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn diagram_title(&self) -> &str {
        &self.title
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// `getCommits`: the commits in insertion order.
    pub fn commits(&self) -> &IndexMap<String, Commit> {
        &self.commits
    }

    /// `getCommitsArray`: the commits sorted by `seq`.
    pub fn commits_array(&self) -> Vec<&Commit> {
        let mut all: Vec<&Commit> = self.commits.values().collect();
        all.sort_by_key(|c| c.seq);
        all
    }

    /// `getBranchesAsObjArray`: branches sorted by order, a branch without
    /// one taking `0.<index>`; the sort is stable like `Array.sort`.
    pub fn branches_as_obj_array(&self) -> Vec<String> {
        let mut with_order: Vec<(f64, &str)> = self
            .branch_config
            .values()
            .enumerate()
            .map(|(i, b)| {
                let order = b
                    .order
                    .unwrap_or_else(|| format!("0.{i}").parse().unwrap_or(0.0));
                (order, b.name.as_str())
            })
            .collect();
        with_order.sort_by(|a, b| a.0.total_cmp(&b.0));
        with_order.into_iter().map(|(_, n)| n.to_string()).collect()
    }

    /// `getID()`: mermaid draws seven random hex digits. The id is drawn as
    /// the commit label, so the helper picks seven digits from the sequence
    /// number instead and the output stays stable across renders.
    fn generated_id(&self) -> String {
        let mut x: u32 = self.seq.wrapping_mul(0x9E37_79B9).wrapping_add(0x7F4A_7C15);
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^= x >> 12;
        format!("{}-{:07x}", self.seq, x & 0x0FFF_FFFF)
    }

    /// `commit(commitDB)`.
    pub fn commit(
        &mut self,
        id: Option<&str>,
        msg: &str,
        kind: Option<CommitType>,
        tags: Vec<String>,
    ) {
        let id = match id {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => self.generated_id(),
        };
        let new_commit = Commit {
            id: id.clone(),
            message: msg.to_string(),
            seq: self.seq,
            kind: kind.unwrap_or(CommitType::Normal),
            tags,
            parents: self.head.iter().cloned().collect(),
            branch: self.curr_branch.clone(),
            custom_type: None,
            custom_id: false,
        };
        self.seq += 1;
        self.head = Some(id.clone());
        self.commits.insert(id.clone(), new_commit);
        self.branches.insert(self.curr_branch.clone(), Some(id));
    }

    /// `branch(branchDB)`.
    pub fn branch(&mut self, name: &str, order: Option<f64>, line: u32) -> Result<(), DbError> {
        if self.branches.contains_key(name) {
            return Err(DbError {
                message: format!(
                    "Trying to create an existing branch. (Help: Either use a new name if you want create a new branch or try using \"checkout {name}\")"
                ),
                line,
            });
        }
        self.branches.insert(name.to_string(), self.head.clone());
        self.branch_config.insert(
            name.to_string(),
            BranchConfig {
                name: name.to_string(),
                order,
            },
        );
        self.checkout(name, line)
    }

    /// `merge(mergeDB)`.
    pub fn merge(
        &mut self,
        other_branch: &str,
        custom_id: Option<&str>,
        override_type: Option<CommitType>,
        custom_tags: Vec<String>,
        line: u32,
    ) -> Result<(), DbError> {
        let fail = |message: String| DbError { message, line };
        let custom_id = custom_id.filter(|id| !id.is_empty());
        let current_check = self.branches.get(&self.curr_branch).cloned().flatten();
        let other_check = self.branches.get(other_branch).cloned().flatten();
        let current_commit = current_check.as_ref().and_then(|id| self.commits.get(id));
        let other_commit = other_check.as_ref().and_then(|id| self.commits.get(id));
        if let (Some(current), Some(_)) = (current_commit, other_commit)
            && current.branch == other_branch
        {
            return Err(fail(format!(
                "Cannot merge branch '{other_branch}' into itself."
            )));
        }
        if self.curr_branch == other_branch {
            return Err(fail(
                "Incorrect usage of \"merge\". Cannot merge a branch to itself".to_string(),
            ));
        }
        let Some(current_commit) = current_commit else {
            return Err(fail(format!(
                "Incorrect usage of \"merge\". Current branch ({})has no commits",
                self.curr_branch
            )));
        };
        if !self.branches.contains_key(other_branch) {
            return Err(fail(format!(
                "Incorrect usage of \"merge\". Branch to be merged ({other_branch}) does not exist"
            )));
        }
        let Some(other_commit) = other_commit else {
            return Err(fail(format!(
                "Incorrect usage of \"merge\". Branch to be merged ({other_branch}) has no commits"
            )));
        };
        if current_commit.id == other_commit.id {
            return Err(fail(
                "Incorrect usage of \"merge\". Both branches have same head".to_string(),
            ));
        }
        if let Some(id) = custom_id
            && self.commits.contains_key(id)
        {
            return Err(fail(format!(
                "Incorrect usage of \"merge\". Commit with id:{id} already exists, use different custom id"
            )));
        }
        let verified_branch = other_check.unwrap_or_default();
        let id = custom_id
            .map(str::to_string)
            .unwrap_or_else(|| self.generated_id());
        let parents = match &self.head {
            None => Vec::new(),
            Some(head) => vec![head.clone(), verified_branch],
        };
        let commit = Commit {
            id: id.clone(),
            message: format!("merged branch {other_branch} into {}", self.curr_branch),
            seq: self.seq,
            parents,
            branch: self.curr_branch.clone(),
            kind: CommitType::Merge,
            custom_type: override_type,
            custom_id: custom_id.is_some(),
            tags: custom_tags,
        };
        self.seq += 1;
        self.head = Some(id.clone());
        self.commits.insert(id.clone(), commit);
        self.branches.insert(self.curr_branch.clone(), Some(id));
        Ok(())
    }

    /// `cherryPick(cherryPickDB)` with the empty `targetId` the parser
    /// always passes.
    pub fn cherry_pick(
        &mut self,
        source_id: Option<&str>,
        parent_id: Option<&str>,
        tags: Option<Vec<String>>,
        line: u32,
    ) -> Result<(), DbError> {
        let fail = |message: &str| DbError {
            message: message.to_string(),
            line,
        };
        let source = source_id
            .filter(|id| !id.is_empty())
            .and_then(|id| self.commits.get(id))
            .cloned()
            .ok_or_else(|| {
                fail(
                    "Incorrect usage of \"cherryPick\". Source commit id should exist and provided",
                )
            })?;
        let parent_id = parent_id.filter(|p| !p.is_empty());
        if let Some(parent) = parent_id
            && !source.parents.iter().any(|p| p == parent)
        {
            return Err(fail(
                "Invalid operation: The specified parent commit is not an immediate parent of the cherry-picked commit.",
            ));
        }
        if source.kind == CommitType::Merge && parent_id.is_none() {
            return Err(fail(
                "Incorrect usage of cherry-pick: If the source commit is a merge commit, an immediate parent commit must be specified.",
            ));
        }
        if source.branch == self.curr_branch {
            return Err(fail(
                "Incorrect usage of \"cherryPick\". Source commit is already on current branch",
            ));
        }
        let current_id = self.branches.get(&self.curr_branch).cloned().flatten();
        if current_id
            .as_ref()
            .and_then(|id| self.commits.get(id))
            .is_none()
        {
            return Err(DbError {
                message: format!(
                    "Incorrect usage of \"cherry-pick\". Current branch ({})has no commits",
                    self.curr_branch
                ),
                line,
            });
        }
        let tags = match tags {
            Some(tags) => tags.into_iter().filter(|t| !t.is_empty()).collect(),
            None => {
                let parent_part = if source.kind == CommitType::Merge {
                    format!("|parent:{}", parent_id.unwrap_or(""))
                } else {
                    String::new()
                };
                vec![format!("cherry-pick:{}{parent_part}", source.id)]
            }
        };
        let id = self.generated_id();
        let parents = match &self.head {
            None => Vec::new(),
            Some(head) => vec![head.clone(), source.id.clone()],
        };
        let commit = Commit {
            id: id.clone(),
            message: format!("cherry-picked {} into {}", source.message, self.curr_branch),
            seq: self.seq,
            parents,
            branch: self.curr_branch.clone(),
            kind: CommitType::CherryPick,
            tags,
            custom_type: None,
            custom_id: false,
        };
        self.seq += 1;
        self.head = Some(id.clone());
        self.commits.insert(id.clone(), commit);
        self.branches.insert(self.curr_branch.clone(), Some(id));
        Ok(())
    }

    /// `checkout(branch)`.
    pub fn checkout(&mut self, branch: &str, line: u32) -> Result<(), DbError> {
        let Some(head) = self.branches.get(branch) else {
            return Err(DbError {
                message: format!(
                    "Trying to checkout branch which is not yet created. (Help try using \"branch {branch}\")"
                ),
                line,
            });
        };
        self.curr_branch = branch.to_string();
        self.head = head
            .as_ref()
            .filter(|id| self.commits.contains_key(*id))
            .cloned();
        Ok(())
    }

    /// `gitGraphParser.populate`: `populateCommonDb`, the direction, then
    /// every statement through its `parse*` adapter.
    pub fn populate(&mut self, ast: &Ast) -> Result<(), DbError> {
        if let Some(d) = ast.acc_descr.as_deref().filter(|s| !s.is_empty()) {
            self.acc_descr = d.to_string();
        }
        if let Some(t) = ast.acc_title.as_deref().filter(|s| !s.is_empty()) {
            self.acc_title = t.to_string();
        }
        if let Some(t) = ast.title.as_deref().filter(|s| !s.is_empty()) {
            self.set_diagram_title(t);
        }
        if let Some(dir) = ast.dir {
            self.direction = dir;
        }
        for statement in &ast.statements {
            self.statement(statement)?;
        }
        Ok(())
    }

    fn statement(&mut self, statement: &Statement) -> Result<(), DbError> {
        let line = statement.line;
        match &statement.kind {
            StatementKind::Commit {
                id,
                message,
                tags,
                kind,
            } => {
                // `parseCommit`: `tags ?? undefined`, and an empty list is
                // still a list, so it replaces the default.
                self.commit(
                    id.as_deref(),
                    message.as_deref().unwrap_or(""),
                    Some(kind.map(CommitType::from).unwrap_or(CommitType::Normal)),
                    tags.clone(),
                );
                Ok(())
            }
            // `parseBranch`: `order ?? 0`.
            StatementKind::Branch { name, order } => {
                self.branch(name, Some(order.unwrap_or(0.0)), line)
            }
            StatementKind::Merge {
                branch,
                id,
                tags,
                kind,
            } => self.merge(
                branch,
                id.as_deref(),
                kind.map(CommitType::from),
                tags.clone(),
                line,
            ),
            StatementKind::Checkout { branch } => self.checkout(branch, line),
            // `parseCherryPicking`: an empty tag list is `undefined`.
            StatementKind::CherryPick { id, parent, tags } => self.cherry_pick(
                id.as_deref(),
                parent.as_deref(),
                (!tags.is_empty()).then(|| tags.clone()),
                line,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::gitgraph::parser::parse;

    fn db_for(text: &str) -> GitGraphDb {
        let mut db = GitGraphDb::new("main", Some(0.0));
        db.populate(&parse(text).unwrap()).unwrap();
        db
    }

    fn err_for(text: &str) -> DbError {
        let mut db = GitGraphDb::new("main", Some(0.0));
        db.populate(&parse(text).unwrap()).unwrap_err()
    }

    #[test]
    fn commits_branches_and_merges_build_the_graph() {
        let db = db_for(
            "gitGraph\n commit id: \"A\"\n branch develop\n commit id: \"B\"\n checkout main\n commit id: \"C\"\n merge develop id: \"M\" tag: \"t\"\n",
        );
        let ids: Vec<&str> = db.commits_array().iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["A", "B", "C", "M"]);
        let m = &db.commits()["M"];
        assert_eq!(m.parents, vec!["C".to_string(), "B".to_string()]);
        assert_eq!(m.kind, CommitType::Merge);
        assert!(m.custom_id);
        assert_eq!(m.message, "merged branch develop into main");
        assert_eq!(db.commits()["B"].parents, vec!["A".to_string()]);
        assert_eq!(db.commits()["B"].branch, "develop");
        assert_eq!(db.branches_as_obj_array(), vec!["main", "develop"]);
    }

    #[test]
    fn generated_ids_are_stable_and_shaped_like_the_oracle() {
        let db = db_for("gitGraph\n commit\n commit\n");
        let ids: Vec<&str> = db.commits_array().iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids[0].starts_with("0-") && ids[0].len() == 9);
        assert!(ids[1].starts_with("1-") && ids[1].len() == 9);
        assert_eq!(
            ids,
            db_for("gitGraph\n commit\n commit\n")
                .commits_array()
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn branch_order_sorts_like_the_docs() {
        let db = db_for(
            "gitGraph\n commit\n branch test1 order: 3\n branch test2\n branch test3\n branch test4 order: 1\n",
        );
        assert_eq!(
            db.branches_as_obj_array(),
            vec!["main", "test2", "test3", "test4", "test1"]
        );
        let mut db = GitGraphDb::new("main", Some(2.0));
        db.populate(&parse("gitGraph\n commit\n branch test1 order: 3\n branch test2\n branch test3\n branch test4 order: 1\n").unwrap()).unwrap();
        assert_eq!(
            db.branches_as_obj_array(),
            vec!["test2", "test3", "test4", "main", "test1"]
        );
    }

    #[test]
    fn cherry_pick_tags_and_parents() {
        let db = db_for(
            "gitGraph\n commit id: \"ZERO\"\n branch develop\n branch release\n commit id: \"A\"\n checkout main\n commit id: \"ONE\"\n checkout develop\n commit id: \"B\"\n checkout main\n merge develop id: \"MERGE\"\n commit id: \"TWO\"\n checkout release\n cherry-pick id: \"MERGE\" parent: \"B\"\n commit id: \"THREE\"\n",
        );
        let picked = db
            .commits_array()
            .into_iter()
            .find(|c| c.kind == CommitType::CherryPick)
            .unwrap();
        assert_eq!(picked.tags, vec!["cherry-pick:MERGE|parent:B".to_string()]);
        assert_eq!(picked.parents, vec!["A".to_string(), "MERGE".to_string()]);
        assert_eq!(
            picked.message,
            "cherry-picked merged branch develop into main into release"
        );
    }

    #[test]
    fn errors_carry_the_oracle_text_and_the_line() {
        let e = err_for("gitGraph\n\n merge main\n");
        assert_eq!(
            e.message,
            "Incorrect usage of \"merge\". Cannot merge a branch to itself"
        );
        assert_eq!(e.line, 3);
        let e = err_for("gitGraph\n branch dev\n checkout main\n commit\n merge dev\n");
        assert_eq!(
            e.message,
            "Incorrect usage of \"merge\". Branch to be merged (dev) has no commits"
        );
        let e = err_for("gitGraph\n commit\n branch dev\n checkout main\n merge dev\n");
        assert_eq!(
            e.message,
            "Incorrect usage of \"merge\". Both branches have same head"
        );
        let e = err_for("gitGraph\n commit\n branch dev\n merge main\n");
        assert_eq!(e.message, "Cannot merge branch 'main' into itself.");
        let e = err_for("gitGraph\n commit\n merge nope\n");
        assert_eq!(
            e.message,
            "Incorrect usage of \"merge\". Branch to be merged (nope) does not exist"
        );
        let e = err_for("gitGraph\n checkout dev\n");
        assert_eq!(
            e.message,
            "Trying to checkout branch which is not yet created. (Help try using \"branch dev\")"
        );
        let e = err_for("gitGraph\n branch main\n");
        assert!(
            e.message
                .starts_with("Trying to create an existing branch.")
        );
        let e = err_for("gitGraph\n commit id: \"A\"\n cherry-pick id: \"A\"\n");
        assert_eq!(
            e.message,
            "Incorrect usage of \"cherryPick\". Source commit is already on current branch"
        );
        let e = err_for("gitGraph\n merge dev\n");
        assert_eq!(
            e.message,
            "Incorrect usage of \"merge\". Current branch (main)has no commits"
        );
    }
}
