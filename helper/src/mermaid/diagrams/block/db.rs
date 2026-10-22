//! Port of `diagrams/block/blockDB.ts` and `blockTypes.ts` (mermaid 12.2.0).
//!
//! mermaid keeps the block tree and a `Map` of the same objects side by
//! side, so a `size` written through either is seen through both. Here the
//! blocks live in one arena and the tree and the id map hold indices into
//! it.

use std::collections::HashMap;

use indexmap::IndexMap;

/// `BlockType` of `blockTypes.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockType {
    #[default]
    Na,
    ColumnSetting,
    Edge,
    Round,
    BlockArrow,
    Space,
    Square,
    Diamond,
    Hexagon,
    Odd,
    LeanRight,
    LeanLeft,
    Trapezoid,
    InvTrapezoid,
    RectLeftInvArrow,
    OddRight,
    Circle,
    Ellipse,
    Stadium,
    Subroutine,
    Cylinder,
    Group,
    DoubleCircle,
    ClassDef,
    ApplyClass,
    ApplyStyles,
    Composite,
}

/// `typeStr2Type(typeStr)`: the bracket pair of a node to its type.
pub fn type_str_to_type(type_str: Option<&str>) -> BlockType {
    match type_str {
        Some("[]") => BlockType::Square,
        Some("()") => BlockType::Round,
        Some("(())") => BlockType::Circle,
        Some(">]") => BlockType::RectLeftInvArrow,
        Some("{}") => BlockType::Diamond,
        Some("{{}}") => BlockType::Hexagon,
        Some("([])") => BlockType::Stadium,
        Some("[[]]") => BlockType::Subroutine,
        Some("[()]") => BlockType::Cylinder,
        Some("((()))") => BlockType::DoubleCircle,
        Some("[//]") => BlockType::LeanRight,
        Some("[\\\\]") => BlockType::LeanLeft,
        Some("[/\\]") => BlockType::Trapezoid,
        Some("[\\/]") => BlockType::InvTrapezoid,
        Some("<[]>") => BlockType::BlockArrow,
        _ => BlockType::Na,
    }
}

/// `edgeStrToEdgeData(typeStr)`: the arrow head at the end of a link.
pub fn edge_str_to_edge_data(type_str: &str) -> &'static str {
    match type_str.trim().chars().last() {
        Some('x') => "arrow_cross",
        Some('o') => "arrow_circle",
        Some('>') => "arrow_point",
        _ => "",
    }
}

/// `edgeStrToEdgeStartData(typeStr)`: the arrow head at the start.
pub fn edge_str_to_edge_start_data(type_str: &str) -> &'static str {
    match type_str.trim().chars().next() {
        Some('x') => "arrow_cross",
        Some('o') => "arrow_circle",
        Some('<') => "arrow_point",
        _ => "arrow_open",
    }
}

/// `edgeStrToThickness(typeStr)`.
pub fn edge_str_to_thickness(type_str: &str) -> &'static str {
    if type_str.contains("==") {
        "thick"
    } else {
        "normal"
    }
}

/// `edgeStrToPattern(typeStr)`.
pub fn edge_str_to_pattern(type_str: &str) -> &'static str {
    if type_str.contains(".-") {
        "dotted"
    } else {
        "solid"
    }
}

/// A block as the grammar actions build it, before `populateBlockDatabase`
/// (`Block` of `blockTypes.ts` with the parser-only fields).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParsedBlock {
    pub id: String,
    pub label: Option<String>,
    pub kind: BlockType,
    pub children: Vec<ParsedBlock>,
    /// `widthInColumns` of a node statement.
    pub width_in_columns: Option<i64>,
    /// `width` of a `space` statement.
    pub width: Option<i64>,
    /// `columns` of a column setting.
    pub columns: Option<i64>,
    pub directions: Vec<String>,
    /// `css` of a `classDef`.
    pub css: Option<String>,
    /// `styleClass` of an `applyClass`.
    pub style_class: Option<String>,
    /// `stylesStr` of an `applyStyles`.
    pub styles_str: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub thickness: Option<String>,
    pub pattern: Option<String>,
    pub arrow_type_end: Option<String>,
    pub arrow_type_start: Option<String>,
}

/// `size` of a block after `calculateBlockSizes` and `layout`: a centre and
/// a size.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
}

/// A block in the database.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Block {
    pub id: String,
    pub label: String,
    pub kind: BlockType,
    /// Indices into the arena.
    pub children: Vec<usize>,
    pub size: Option<Size>,
    pub columns: Option<i64>,
    pub classes: Vec<String>,
    pub directions: Vec<String>,
    /// `styles`: the `style` statement's declarations, split on commas.
    pub styles: Vec<String>,
    pub width_in_columns: Option<i64>,
    /// `width` of a space block.
    pub width: Option<i64>,
}

impl Block {
    pub fn width_in_columns(&self) -> i64 {
        self.width_in_columns.unwrap_or(1)
    }
}

/// An edge statement, `edgeList` of `blockDB.ts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EdgeBlock {
    pub id: String,
    pub start: String,
    pub end: String,
    pub label: String,
    pub thickness: String,
    pub pattern: String,
    pub arrow_type_end: String,
    pub arrow_type_start: String,
}

/// `ClassDef` of `blockTypes.ts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClassDef {
    pub id: String,
    pub text_styles: Vec<String>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DbError {
    /// `addStyle2Node` dereferences `blockDatabase.get(id)!`; mermaid throws
    /// a `TypeError` when the block does not exist.
    UnknownStyleTarget(String),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::UnknownStyleTarget(id) => write!(
                f,
                "TypeError: Cannot set properties of undefined (setting 'styles') for block {id}"
            ),
        }
    }
}

impl std::error::Error for DbError {}

pub const ROOT_ID: &str = "root";

/// The database after `setHierarchy`.
#[derive(Debug, Clone, Default)]
pub struct BlockDb {
    arena: Vec<Block>,
    /// `blockDatabase`: id to arena index, in insertion order.
    by_id: IndexMap<String, usize>,
    edges: Vec<EdgeBlock>,
    edge_count: HashMap<String, i64>,
    classes: IndexMap<String, ClassDef>,
}

impl BlockDb {
    pub fn new() -> BlockDb {
        let mut db = BlockDb::default();
        db.insert(Block {
            id: ROOT_ID.to_string(),
            kind: BlockType::Composite,
            columns: Some(-1),
            ..Block::default()
        });
        db
    }

    fn insert(&mut self, block: Block) -> usize {
        let index = self.arena.len();
        self.by_id.insert(block.id.clone(), index);
        self.arena.push(block);
        index
    }

    pub fn root(&self) -> usize {
        0
    }

    pub fn block(&self, index: usize) -> &Block {
        &self.arena[index]
    }

    pub fn block_mut(&mut self, index: usize) -> &mut Block {
        &mut self.arena[index]
    }

    /// `getBlock(id)`.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    /// `getBlocks()`: the children of the root.
    pub fn blocks(&self) -> &[usize] {
        &self.arena[0].children
    }

    /// `getBlocksFlat()`: every block in the database, root included.
    pub fn blocks_flat(&self) -> impl Iterator<Item = usize> + '_ {
        self.by_id.values().copied()
    }

    /// `getEdges()`.
    pub fn edges(&self) -> &[EdgeBlock] {
        &self.edges
    }

    /// `getClasses()`.
    pub fn classes(&self) -> &IndexMap<String, ClassDef> {
        &self.classes
    }

    /// `setHierarchy(block)`: fills the database from the parse tree.
    pub fn set_hierarchy(&mut self, blocks: Vec<ParsedBlock>) -> Result<(), DbError> {
        let root = self.root();
        self.populate(blocks, root)
    }

    /// `addStyleClass(id, styleAttributes)`.
    pub fn add_style_class(&mut self, id: &str, style_attributes: &str) {
        let class = self
            .classes
            .entry(id.to_string())
            .or_insert_with(|| ClassDef {
                id: id.to_string(),
                ..ClassDef::default()
            });
        for attrib in style_attributes.split(',') {
            // `attrib.replace(/([^;]*);/, '$1').trim()`: drops the first `;`.
            let fixed = attrib.replacen(';', "", 1).trim().to_string();
            if attrib.contains("color") {
                let text_style = fixed.replacen("fill", "bgFill", 1);
                let text_style = text_style.replacen("color", "fill", 1);
                class.text_styles.push(text_style);
            }
            class.styles.push(fixed);
        }
    }

    /// `addStyle2Node(id, styles)`.
    pub fn add_style_to_node(&mut self, id: &str, styles: &str) -> Result<(), DbError> {
        let index = self
            .index_of(id)
            .ok_or_else(|| DbError::UnknownStyleTarget(id.to_string()))?;
        self.arena[index].styles = styles.split(',').map(str::to_string).collect();
        Ok(())
    }

    /// `setCssClass(itemIds, cssClassName)`.
    pub fn set_css_class(&mut self, item_ids: &str, class_name: &str) {
        for id in item_ids.split(',') {
            let index = match self.index_of(id) {
                Some(index) => index,
                None => self.insert(Block {
                    id: id.trim().to_string(),
                    kind: BlockType::Na,
                    ..Block::default()
                }),
            };
            self.arena[index].classes.push(class_name.to_string());
        }
    }

    /// `populateBlockDatabase(blockList, parent)`.
    fn populate(&mut self, block_list: Vec<ParsedBlock>, parent: usize) -> Result<(), DbError> {
        let mut children: Vec<usize> = Vec::new();
        for block in block_list {
            match block.kind {
                BlockType::ClassDef => {
                    self.add_style_class(&block.id, block.css.as_deref().unwrap_or(""));
                    continue;
                }
                BlockType::ApplyClass => {
                    self.set_css_class(&block.id, block.style_class.as_deref().unwrap_or(""));
                    continue;
                }
                BlockType::ApplyStyles => {
                    if let Some(styles) = block.styles_str.as_deref().filter(|s| !s.is_empty()) {
                        self.add_style_to_node(&block.id, styles)?;
                    }
                    continue;
                }
                BlockType::ColumnSetting => {
                    self.arena[parent].columns = Some(block.columns.unwrap_or(-1));
                }
                BlockType::Edge => {
                    let count = self.edge_count.get(&block.id).copied().unwrap_or(0) + 1;
                    self.edge_count.insert(block.id.clone(), count);
                    self.edges.push(EdgeBlock {
                        id: format!("{count}-{}", block.id),
                        start: block.start.unwrap_or_default(),
                        end: block.end.unwrap_or_default(),
                        label: block.label.unwrap_or_default(),
                        thickness: block.thickness.unwrap_or_default(),
                        pattern: block.pattern.unwrap_or_default(),
                        arrow_type_end: block.arrow_type_end.unwrap_or_default(),
                        arrow_type_start: block.arrow_type_start.unwrap_or_default(),
                    });
                }
                _ => {
                    let label = match block.label.filter(|l| !l.is_empty()) {
                        Some(label) => label,
                        None if block.kind == BlockType::Composite => String::new(),
                        None => block.id.clone(),
                    };
                    let existing = self.index_of(&block.id);
                    let index = match existing {
                        None => self.insert(Block {
                            id: block.id.clone(),
                            label: label.clone(),
                            kind: block.kind,
                            columns: block.columns,
                            directions: block.directions.clone(),
                            width_in_columns: block.width_in_columns,
                            width: block.width,
                            ..Block::default()
                        }),
                        Some(index) => {
                            // Add newer relevant data to the aggregated node.
                            if block.kind != BlockType::Na {
                                self.arena[index].kind = block.kind;
                            }
                            if label != block.id {
                                self.arena[index].label = label.clone();
                            }
                            index
                        }
                    };
                    // mermaid recurses into the parsed block's children even
                    // when an earlier block with the same id already exists;
                    // those children are registered but belong to nothing.
                    let target = if existing.is_some() {
                        self.arena.push(Block {
                            id: block.id.clone(),
                            kind: BlockType::Na,
                            ..Block::default()
                        });
                        self.arena.len() - 1
                    } else {
                        index
                    };
                    self.populate(block.children, target)?;
                    if block.kind == BlockType::Space {
                        let w = block.width.unwrap_or(1);
                        for j in 0..w {
                            let mut clone = self.arena[index].clone();
                            clone.id = format!("{}-{j}", clone.id);
                            let clone_index = self.insert(clone);
                            children.push(clone_index);
                        }
                    } else if existing.is_none() {
                        children.push(index);
                    }
                }
            }
        }
        self.arena[parent].children = children;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_strings_map_like_mermaid() {
        assert_eq!(type_str_to_type(Some("()")), BlockType::Round);
        assert_eq!(type_str_to_type(Some("[\\\\]")), BlockType::LeanLeft);
        assert_eq!(type_str_to_type(Some("<[]>")), BlockType::BlockArrow);
        assert_eq!(type_str_to_type(None), BlockType::Na);
        assert_eq!(edge_str_to_edge_data(" --> "), "arrow_point");
        assert_eq!(edge_str_to_edge_start_data("x--"), "arrow_cross");
        assert_eq!(edge_str_to_edge_start_data("-->"), "arrow_open");
        assert_eq!(edge_str_to_thickness("==>"), "thick");
        assert_eq!(edge_str_to_pattern("-.->"), "dotted");
    }

    #[test]
    fn spaces_expand_and_classes_resolve() {
        let mut db = BlockDb::new();
        db.set_hierarchy(vec![
            ParsedBlock {
                id: "a".into(),
                kind: BlockType::Na,
                width_in_columns: Some(1),
                ..ParsedBlock::default()
            },
            ParsedBlock {
                id: "id-1".into(),
                kind: BlockType::Space,
                width: Some(2),
                label: Some(String::new()),
                ..ParsedBlock::default()
            },
            ParsedBlock {
                id: "blue".into(),
                kind: BlockType::ClassDef,
                css: Some("fill:#6e6ce6, color:#fff;".into()),
                ..ParsedBlock::default()
            },
            ParsedBlock {
                id: "a".into(),
                kind: BlockType::ApplyClass,
                style_class: Some("blue".into()),
                ..ParsedBlock::default()
            },
        ])
        .expect("populates");
        let children = db.blocks();
        assert_eq!(children.len(), 3);
        assert_eq!(db.block(children[0]).label, "a");
        assert_eq!(db.block(children[1]).id, "id-1-0");
        assert_eq!(db.block(children[2]).id, "id-1-1");
        assert_eq!(db.block(children[0]).classes, vec!["blue".to_string()]);
        let blue = &db.classes()["blue"];
        assert_eq!(blue.styles, vec!["fill:#6e6ce6", "color:#fff"]);
        assert_eq!(blue.text_styles, vec!["fill:#fff"]);
    }
}
