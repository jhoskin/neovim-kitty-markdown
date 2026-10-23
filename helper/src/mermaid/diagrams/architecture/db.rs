//! `ArchitectureDB` (`architectureDb.ts`) and `populateDb`
//! (`architectureParser.ts`) of mermaid 12.2.0, with the direction helpers
//! of `architectureTypes.ts`.
//!
//! `populateDb` adds every group first, then the services, the junctions,
//! the edges, and the `align` hints, whatever their order in the text. The
//! errors the db throws keep their JS text; the renderer reports them at
//! the line of the statement that caused them.

use indexmap::{IndexMap, IndexSet};

use super::parser::{self, Ast};

/// A side of a node: `L`, `R`, `T`, or `B`.
pub type Dir = char;

pub fn is_x(d: Dir) -> bool {
    d == 'L' || d == 'R'
}

pub fn is_y(d: Dir) -> bool {
    d == 'T' || d == 'B'
}

/// `isArchitectureDirectionXY`: one side horizontal, the other vertical.
pub fn is_xy(a: Dir, b: Dir) -> bool {
    (is_x(a) && is_y(b)) || (is_y(a) && is_x(b))
}

/// `getOppositeArchitectureDirection`.
pub fn opposite(d: Dir) -> Dir {
    if is_x(d) {
        if d == 'L' { 'R' } else { 'L' }
    } else if d == 'T' {
        'B'
    } else {
        'T'
    }
}

/// `getArchitectureDirectionPair`: `None` for `LL`, `RR`, `TT`, and `BB`.
pub fn direction_pair(source: Dir, target: Dir) -> Option<(Dir, Dir)> {
    (source != target).then_some((source, target))
}

/// `shiftPositionByArchitectureDirectionPair`.
pub fn shift_position((x, y): (i64, i64), (lhs, rhs): (Dir, Dir)) -> (i64, i64) {
    if is_x(lhs) {
        let dx = if lhs == 'L' { -1 } else { 1 };
        if is_y(rhs) {
            (x + dx, y + if rhs == 'T' { 1 } else { -1 })
        } else {
            (x + dx, y)
        }
    } else {
        let dy = if lhs == 'T' { 1 } else { -1 };
        if is_x(rhs) {
            (x + if rhs == 'L' { 1 } else { -1 }, y + dy)
        } else {
            (x, y + dy)
        }
    }
}

/// `getArchitectureDirectionXYFactors`.
pub fn xy_factors(pair: (Dir, Dir)) -> (f64, f64) {
    match pair {
        ('L', 'T') | ('T', 'L') => (1.0, 1.0),
        ('B', 'L') | ('L', 'B') => (1.0, -1.0),
        ('B', 'R') | ('R', 'B') => (-1.0, -1.0),
        _ => (-1.0, 1.0),
    }
}

/// `getArchitectureDirectionAlignment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Bend,
    Horizontal,
    Vertical,
}

pub fn alignment(a: Dir, b: Dir) -> Alignment {
    if is_xy(a, b) {
        Alignment::Bend
    } else if is_x(a) {
        Alignment::Horizontal
    } else {
        Alignment::Vertical
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Service,
    Junction,
}

/// An `ArchitectureService` or `ArchitectureJunction`.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub kind: NodeType,
    pub icon: Option<String>,
    pub icon_text: Option<String>,
    pub title: Option<String>,
    /// Indices into [`ArchitectureDb::edges`].
    pub edges: Vec<usize>,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub id: String,
    pub icon: Option<String>,
    pub title: Option<String>,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub lhs_id: String,
    pub lhs_dir: Dir,
    pub lhs_into: bool,
    pub lhs_group: bool,
    pub rhs_id: String,
    pub rhs_dir: Dir,
    pub rhs_into: bool,
    pub rhs_group: bool,
    pub title: Option<String>,
}

/// An `align row|column` hint.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutHint {
    pub row: bool,
    pub members: Vec<String>,
}

/// An error `populateDb` throws, with the line of its statement.
#[derive(Debug, Clone, PartialEq)]
pub struct DbError {
    pub line: u32,
    pub message: String,
}

/// `getDataStructures()`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DataStructures {
    /// Per node: the neighbour in each direction pair.
    pub adj_list: IndexMap<String, IndexMap<(Dir, Dir), String>>,
    /// One grid per connected component, node id to grid cell.
    pub spatial_maps: Vec<IndexMap<String, (i64, i64)>>,
    /// Sorted group pair to the alignment of an edge between them.
    pub group_alignments: IndexMap<(String, String), Alignment>,
}

#[derive(Debug, Clone, Default)]
pub struct ArchitectureDb {
    pub nodes: IndexMap<String, Node>,
    pub groups: IndexMap<String, Group>,
    pub edges: Vec<Edge>,
    pub layout_hints: Vec<LayoutHint>,
    /// Id to `"node"` or `"group"`.
    registered: IndexMap<String, &'static str>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
}

impl ArchitectureDb {
    pub fn new() -> ArchitectureDb {
        ArchitectureDb::default()
    }

    pub fn set_diagram_title(&mut self, title: &str) {
        self.title = Some(title.to_string());
    }

    /// The `in` checks `addService`, `addJunction`, and `addGroup` share.
    fn check_parent(&self, what: &str, id: &str, parent: &Option<String>) -> Result<(), String> {
        let Some(parent) = parent else {
            return Ok(());
        };
        if id == parent {
            return Err(format!("The {what} [{id}] cannot be placed within itself"));
        }
        match self.registered.get(parent) {
            None => Err(format!(
                "The {what} [{id}]'s parent does not exist. Please make sure the parent is created before this {what}"
            )),
            Some(&"node") => Err(format!("The {what} [{id}]'s parent is not a group")),
            Some(_) => Ok(()),
        }
    }

    fn check_id(&self, what: &str, id: &str) -> Result<(), String> {
        match self.registered.get(id) {
            Some(kind) => Err(format!(
                "The {what} id [{id}] is already in use by another {kind}"
            )),
            None => Ok(()),
        }
    }

    pub fn add_group(&mut self, g: &parser::Group) -> Result<(), String> {
        self.check_id("group", &g.id)?;
        self.check_parent("group", &g.id, &g.parent)?;
        self.registered.insert(g.id.clone(), "group");
        self.groups.insert(
            g.id.clone(),
            Group {
                id: g.id.clone(),
                icon: g.icon.clone(),
                title: g.title.clone(),
                parent: g.parent.clone(),
            },
        );
        Ok(())
    }

    pub fn add_service(&mut self, s: &parser::Service) -> Result<(), String> {
        self.check_id("service", &s.id)?;
        self.check_parent("service", &s.id, &s.parent)?;
        self.registered.insert(s.id.clone(), "node");
        self.nodes.insert(
            s.id.clone(),
            Node {
                id: s.id.clone(),
                kind: NodeType::Service,
                icon: s.icon.clone(),
                icon_text: s.icon_text.clone(),
                title: s.title.clone(),
                edges: Vec::new(),
                parent: s.parent.clone(),
            },
        );
        Ok(())
    }

    pub fn add_junction(&mut self, j: &parser::Junction) -> Result<(), String> {
        self.check_id("junction", &j.id)?;
        self.check_parent("junction", &j.id, &j.parent)?;
        self.registered.insert(j.id.clone(), "node");
        self.nodes.insert(
            j.id.clone(),
            Node {
                id: j.id.clone(),
                kind: NodeType::Junction,
                icon: None,
                icon_text: None,
                title: None,
                edges: Vec::new(),
                parent: j.parent.clone(),
            },
        );
        Ok(())
    }

    pub fn add_edge(&mut self, e: &parser::Edge) -> Result<(), String> {
        let valid = |d: Dir| matches!(d, 'L' | 'R' | 'T' | 'B');
        if !valid(e.lhs_dir) {
            return Err(format!(
                "Invalid direction given for left hand side of edge {}--{}. Expected (L,R,T,B) got {}",
                e.lhs_id, e.rhs_id, e.lhs_dir
            ));
        }
        if !valid(e.rhs_dir) {
            return Err(format!(
                "Invalid direction given for right hand side of edge {}--{}. Expected (L,R,T,B) got {}",
                e.lhs_id, e.rhs_id, e.rhs_dir
            ));
        }
        let known = |id: &str| self.nodes.contains_key(id) || self.groups.contains_key(id);
        if !known(&e.lhs_id) {
            return Err(format!(
                "The left-hand id [{}] does not yet exist. Please create the service/group before declaring an edge to it.",
                e.lhs_id
            ));
        }
        if !known(&e.rhs_id) {
            return Err(format!(
                "The right-hand id [{}] does not yet exist. Please create the service/group before declaring an edge to it.",
                e.rhs_id
            ));
        }
        // `this.nodes.get(lhsId).in` on a group id reads `undefined.in`.
        let parent_of = |id: &str| {
            self.nodes
                .get(id)
                .map(|n| n.parent.clone())
                .ok_or_else(|| "Cannot read properties of undefined (reading 'in')".to_string())
        };
        let lhs_group_id = parent_of(&e.lhs_id)?;
        let rhs_group_id = parent_of(&e.rhs_id)?;
        let same_group = matches!((&lhs_group_id, &rhs_group_id), (Some(a), Some(b)) if a == b);
        if e.lhs_group && same_group {
            return Err(format!(
                "The left-hand id [{}] is modified to traverse the group boundary, but the edge does not pass through two groups.",
                e.lhs_id
            ));
        }
        if e.rhs_group && same_group {
            return Err(format!(
                "The right-hand id [{}] is modified to traverse the group boundary, but the edge does not pass through two groups.",
                e.rhs_id
            ));
        }
        self.edges.push(Edge {
            lhs_id: e.lhs_id.clone(),
            lhs_dir: e.lhs_dir,
            lhs_into: e.lhs_into,
            lhs_group: e.lhs_group,
            rhs_id: e.rhs_id.clone(),
            rhs_dir: e.rhs_dir,
            rhs_into: e.rhs_into,
            rhs_group: e.rhs_group,
            title: e.title.clone(),
        });
        let index = self.edges.len() - 1;
        if self.nodes.contains_key(&e.lhs_id) && self.nodes.contains_key(&e.rhs_id) {
            for id in [&e.lhs_id, &e.rhs_id] {
                if let Some(n) = self.nodes.get_mut(id) {
                    n.edges.push(index);
                }
            }
        }
        Ok(())
    }

    pub fn add_layout_hint(&mut self, a: &parser::Alignment) -> Result<(), String> {
        if a.members.len() < 2 {
            return Err(format!(
                "An align directive requires at least two members; got {}",
                a.members.len()
            ));
        }
        let mut seen = IndexSet::new();
        for id in &a.members {
            if self.registered.get(id) != Some(&"node") {
                return Err(format!(
                    "align {} references [{id}], which is not a service or junction",
                    a.direction
                ));
            }
            if !seen.insert(id.clone()) {
                return Err(format!("align {} lists [{id}] more than once", a.direction));
            }
        }
        self.layout_hints.push(LayoutHint {
            row: a.direction == "row",
            members: a.members.clone(),
        });
        Ok(())
    }

    /// `populateCommonDb` then `populateDb`.
    pub fn populate(&mut self, ast: &Ast) -> Result<(), DbError> {
        if let Some(t) = &ast.acc_title {
            self.acc_title = Some(t.clone());
        }
        if let Some(t) = &ast.acc_descr {
            self.acc_descr = Some(t.clone());
        }
        if let Some(t) = &ast.title {
            self.title = Some(t.clone());
        }
        let err = |line: u32| move |message: String| DbError { line, message };
        for g in &ast.groups {
            self.add_group(g).map_err(err(g.line))?;
        }
        for s in &ast.services {
            self.add_service(s).map_err(err(s.line))?;
        }
        for j in &ast.junctions {
            self.add_junction(j).map_err(err(j.line))?;
        }
        for e in &ast.edges {
            self.add_edge(e).map_err(err(e.line))?;
        }
        for a in &ast.alignments {
            self.add_layout_hint(a).map_err(err(a.line))?;
        }
        Ok(())
    }

    /// `getDataStructures()`: the adjacency lists, the spatial maps from a
    /// BFS over them, and the group alignments.
    pub fn data_structures(&self) -> DataStructures {
        let mut group_alignments = IndexMap::new();
        let mut adj_list: IndexMap<String, IndexMap<(Dir, Dir), String>> = IndexMap::new();
        for (id, service) in &self.nodes {
            let mut direction_map = IndexMap::new();
            for &e in &service.edges {
                let edge = &self.edges[e];
                let lhs_group = self.nodes.get(&edge.lhs_id).and_then(|n| n.parent.clone());
                let rhs_group = self.nodes.get(&edge.rhs_id).and_then(|n| n.parent.clone());
                if let (Some(a), Some(b)) = (&lhs_group, &rhs_group)
                    && a != b
                {
                    let al = alignment(edge.lhs_dir, edge.rhs_dir);
                    if al != Alignment::Bend {
                        group_alignments.insert(group_key(a, b), al);
                    }
                }
                if &edge.lhs_id == id {
                    if let Some(pair) = direction_pair(edge.lhs_dir, edge.rhs_dir) {
                        direction_map.insert(pair, edge.rhs_id.clone());
                    }
                } else if let Some(pair) = direction_pair(edge.rhs_dir, edge.lhs_dir) {
                    direction_map.insert(pair, edge.lhs_id.clone());
                }
            }
            adj_list.insert(id.clone(), direction_map);
        }

        let mut visited: IndexSet<String> = IndexSet::new();
        let mut not_visited: IndexSet<String> = adj_list.keys().cloned().collect();
        let mut spatial_maps = Vec::new();
        while let Some(first) = not_visited.first().cloned() {
            let mut spatial: IndexMap<String, (i64, i64)> = IndexMap::new();
            spatial.insert(first.clone(), (0, 0));
            let mut queue = std::collections::VecDeque::from([first]);
            while let Some(id) = queue.pop_front() {
                visited.insert(id.clone());
                not_visited.shift_remove(&id);
                let pos = spatial.get(&id).copied().unwrap_or((0, 0));
                if let Some(adj) = adj_list.get(&id) {
                    for (dir, rhs) in adj {
                        if !visited.contains(rhs) {
                            spatial.insert(rhs.clone(), shift_position(pos, *dir));
                            queue.push_back(rhs.clone());
                        }
                    }
                }
            }
            spatial_maps.push(spatial);
        }
        DataStructures {
            adj_list,
            spatial_maps,
            group_alignments,
        }
    }
}

/// `architectureGroupAlignmentKey`: the two group ids sorted.
pub fn group_key(a: &str, b: &str) -> (String, String) {
    if utf16_less(b, a) {
        (b.to_string(), a.to_string())
    } else {
        (a.to_string(), b.to_string())
    }
}

/// `Array.prototype.sort`'s default order: UTF-16 code units.
fn utf16_less(a: &str, b: &str) -> bool {
    a.encode_utf16().lt(b.encode_utf16())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn populate(text: &str) -> Result<ArchitectureDb, DbError> {
        let ast = parser::parse(text).unwrap_or_else(|e| panic!("{}", e.message));
        let mut db = ArchitectureDb::new();
        db.populate(&ast).map(|_| db)
    }

    fn error(text: &str) -> (u32, String) {
        let e = populate(text).expect_err("the db rejects it");
        (e.line, e.message)
    }

    #[test]
    fn groups_are_added_before_services_whatever_the_order() {
        let db = populate("architecture-beta\nservice a(server)[A] in g\ngroup g\n").unwrap();
        assert_eq!(db.nodes["a"].parent.as_deref(), Some("g"));
    }

    #[test]
    fn errors_keep_the_mermaid_text_and_the_statement_line() {
        assert_eq!(
            error("architecture-beta\nservice a\nservice a\n"),
            (
                3,
                "The service id [a] is already in use by another node".to_string()
            )
        );
        assert_eq!(
            error("architecture-beta\nservice a in g\n"),
            (
                2,
                "The service [a]'s parent does not exist. Please make sure the parent is created before this service"
                    .to_string()
            )
        );
        assert_eq!(
            error("architecture-beta\nservice a\njunction j in a\n").1,
            "The junction [j]'s parent is not a group"
        );
        assert_eq!(
            error("architecture-beta\ngroup g\nservice a\na:L -- R:g\n").1,
            "Cannot read properties of undefined (reading 'in')"
        );
        assert_eq!(
            error(
                "architecture-beta\ngroup g\nservice a in g\nservice b in g\na{group}:L -- R:b\n"
            )
            .1,
            "The left-hand id [a] is modified to traverse the group boundary, but the edge does not pass through two groups."
        );
        assert_eq!(
            error("architecture-beta\nservice a\nservice b\nalign row a b a\n").1,
            "align row lists [a] more than once"
        );
        assert_eq!(
            error("architecture-beta\ngroup g\nservice a\nalign column a g\n").1,
            "align column references [g], which is not a service or junction"
        );
    }

    #[test]
    fn the_bfs_grid_follows_the_edge_sides() {
        let db = populate(concat!(
            "architecture-beta\n",
            "service db\nservice disk1\nservice disk2\nservice server\n",
            "db:L -- R:server\ndisk1:T -- B:server\ndisk2:T -- B:db\n",
        ))
        .unwrap();
        let ds = db.data_structures();
        assert_eq!(ds.spatial_maps.len(), 1);
        let grid: Vec<(&str, (i64, i64))> = ds.spatial_maps[0]
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        assert_eq!(
            grid,
            [
                ("db", (0, 0)),
                ("server", (-1, 0)),
                ("disk2", (0, -1)),
                ("disk1", (-1, -1)),
            ]
        );
    }
}
