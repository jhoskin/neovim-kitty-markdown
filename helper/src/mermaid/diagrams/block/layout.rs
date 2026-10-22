//! Port of `diagrams/block/layout.ts` (mermaid 12.2.0): the column grid.
//! `setBlockSizes` grows every block to its siblings' size, `layoutBlocks`
//! places the children of every block row by row, and `findBounds` reports
//! the extent.

use std::collections::BTreeMap;

use super::db::{BlockDb, BlockType, Size};

/// `calculateBlockPosition(columns, position)`: the column and the row of
/// the `position`th slot.
pub fn calculate_block_position(columns: i64, position: i64) -> (i64, i64) {
    if columns < 0 {
        return (position, 0);
    }
    if columns == 1 {
        return (0, position);
    }
    (position % columns, position / columns)
}

/// `getMaxChildSize(block)`: the widest child per column it spans and the
/// tallest child, spaces left out.
fn max_child_size(db: &BlockDb, block: usize) -> (f64, f64) {
    let mut max_width = 0.0;
    let mut max_height = 0.0;
    for &child in &db.block(block).children {
        let child = db.block(child);
        let size = child.size.unwrap_or_default();
        if child.kind == BlockType::Space {
            continue;
        }
        let normalized = size.width / child.width_in_columns() as f64;
        if normalized > max_width {
            max_width = normalized;
        }
        if size.height > max_height {
            max_height = size.height;
        }
    }
    (max_width, max_height)
}

/// `setBlockSizes(block, db, siblingWidth, siblingHeight, padding)`.
fn set_block_sizes(
    db: &mut BlockDb,
    block: usize,
    sibling_width: f64,
    sibling_height: f64,
    padding: f64,
) {
    if !db.block(block).size.is_some_and(|s| s.width != 0.0) {
        db.block_mut(block).size = Some(Size {
            width: sibling_width,
            height: sibling_height,
            x: 0.0,
            y: 0.0,
        });
    }
    let children = db.block(block).children.clone();
    if children.is_empty() {
        return;
    }
    for &child in &children {
        set_block_sizes(db, child, 0.0, 0.0, padding);
    }
    let (max_width, max_height) = max_child_size(db, block);
    for &child in &children {
        let cols = db.block(child).width_in_columns() as f64;
        if db.block(child).size.is_some() {
            db.block_mut(child).size = Some(Size {
                width: max_width * cols + padding * (cols - 1.0),
                height: max_height,
                x: 0.0,
                y: 0.0,
            });
        }
    }
    for &child in &children {
        set_block_sizes(db, child, max_width, max_height, padding);
    }

    let columns = db.block(block).columns.unwrap_or(-1);
    let num_items: i64 = children
        .iter()
        .map(|&c| db.block(c).width_in_columns())
        .sum();
    let mut x_size = children.len() as i64;
    if columns > 0 && columns < num_items {
        x_size = columns;
    }
    let y_size = (num_items as f64 / x_size as f64).ceil();
    let x_size_f = x_size as f64;

    let mut width = x_size_f * (max_width + padding) + padding;
    let mut height = y_size * (max_height + padding) + padding;
    if width < sibling_width {
        width = sibling_width;
        height = sibling_height;
        let child_width = (sibling_width - x_size_f * padding - padding) / x_size_f;
        let child_height = (sibling_height - y_size * padding - padding) / y_size;
        for &child in &children {
            if db.block(child).size.is_some() {
                db.block_mut(child).size = Some(Size {
                    width: child_width,
                    height: child_height,
                    x: 0.0,
                    y: 0.0,
                });
            }
        }
    }
    let own_width = db.block(block).size.map_or(0.0, |s| s.width);
    if width < own_width {
        width = own_width;
        // Grow children to fit.
        let num = if columns > 0 {
            (children.len() as i64).min(columns)
        } else {
            children.len() as i64
        };
        if num > 0 {
            let child_width = (width - num as f64 * padding - padding) / num as f64;
            for &child in &children {
                if let Some(size) = db.block_mut(child).size.as_mut() {
                    size.width = child_width;
                }
            }
        }
    }
    db.block_mut(block).size = Some(Size {
        width,
        height,
        x: 0.0,
        y: 0.0,
    });
}

/// `layoutBlocks(block, db, padding)`: positions the children of `block`
/// inside it, row by row.
fn layout_blocks(db: &mut BlockDb, block: usize, padding: f64) {
    let columns = db.block(block).columns.unwrap_or(-1);
    let children = db.block(block).children.clone();
    if children.is_empty() {
        return;
    }
    let parent_size = db.block(block).size;

    // Per-row maximum heights, so rows of different heights stack right.
    let mut row_heights: BTreeMap<i64, f64> = BTreeMap::new();
    let mut col_pos = 0i64;
    for &child in &children {
        let child = db.block(child);
        let Some(size) = child.size else {
            continue;
        };
        let (_, py) = calculate_block_position(columns, col_pos);
        let current = row_heights.entry(py).or_insert(0.0);
        if size.height > *current {
            *current = size.height;
        }
        let mut filled = child.width_in_columns();
        if columns > 0 {
            filled = filled.min(columns - (col_pos % columns));
        }
        col_pos += filled;
    }
    let mut row_y_offsets: BTreeMap<i64, f64> = BTreeMap::new();
    let mut offset = 0.0;
    for (&row, &height) in &row_heights {
        row_y_offsets.insert(row, offset);
        offset += height + padding;
    }

    // `block?.size?.x ? block.size.x + (-block.size.width / 2 || 0) : -padding`
    let row_start = |parent: Option<Size>| -> f64 {
        match parent {
            Some(s) if s.x != 0.0 => {
                let half = -s.width / 2.0;
                s.x + if half == 0.0 { 0.0 } else { half }
            }
            _ => -padding,
        }
    };

    let mut column_pos = 0i64;
    let mut starting_pos_x = row_start(parent_size);
    let mut row_pos = 0i64;
    for &child in &children {
        let Some(mut size) = db.block(child).size else {
            continue;
        };
        let (_, py) = calculate_block_position(columns, column_pos);
        if py != row_pos {
            row_pos = py;
            starting_pos_x = row_start(parent_size);
        }
        if let Some(parent) = parent_size {
            let half_width = size.width / 2.0;
            size.x = starting_pos_x + padding + half_width;
            starting_pos_x = size.x + half_width;
            let row_y_offset = row_y_offsets.get(&py).copied().unwrap_or(0.0);
            let row_height = row_heights.get(&py).copied().unwrap_or(size.height);
            size.y = parent.y - parent.height / 2.0 + row_y_offset + row_height / 2.0 + padding;
            db.block_mut(child).size = Some(size);
        }
        layout_blocks(db, child, padding);
        let mut columns_filled = db.block(child).width_in_columns();
        if columns > 0 {
            // Make sure overflowing lines do not affect later lines.
            columns_filled = columns_filled.min(columns - (column_pos % columns));
        }
        column_pos += columns_filled;
    }
}

/// `{ minX, minY, maxX, maxY }` of `findBounds`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

/// `findBounds(block, bounds)`: the extent of every positioned block but
/// the root, starting from the zero box as mermaid does.
fn find_bounds(db: &BlockDb, block: usize, mut b: Bounds) -> Bounds {
    let node = db.block(block);
    if let Some(size) = node.size
        && block != db.root()
    {
        b.min_x = b.min_x.min(size.x - size.width / 2.0);
        b.min_y = b.min_y.min(size.y - size.height / 2.0);
        b.max_x = b.max_x.max(size.x + size.width / 2.0);
        b.max_y = b.max_y.max(size.y + size.height / 2.0);
    }
    for &child in &node.children {
        b = find_bounds(db, child, b);
    }
    b
}

/// The `{ x, y, width, height }` `layout` returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// `layout(db)` with `block.padding`.
pub fn layout(db: &mut BlockDb, padding: f64) -> LayoutBounds {
    let root = db.root();
    set_block_sizes(db, root, 0.0, 0.0, padding);
    layout_blocks(db, root, padding);
    let b = find_bounds(
        db,
        root,
        Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 0.0,
        },
    );
    LayoutBounds {
        x: b.min_x,
        y: b.min_y,
        width: b.max_x - b.min_x,
        height: b.max_y - b.min_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::diagrams::block::db::ParsedBlock;

    /// `layout.spec.ts`: `calculateBlockPosition`.
    #[test]
    fn block_positions_follow_the_grid() {
        assert_eq!(calculate_block_position(2, 0), (0, 0));
        assert_eq!(calculate_block_position(2, 1), (1, 0));
        assert_eq!(calculate_block_position(2, 2), (0, 1));
        assert_eq!(calculate_block_position(2, 3), (1, 1));
        assert_eq!(calculate_block_position(2, 4), (0, 2));
        assert_eq!(calculate_block_position(1, 3), (0, 3));
        assert_eq!(calculate_block_position(-1, 3), (3, 0));
    }

    fn leaf(id: &str) -> ParsedBlock {
        ParsedBlock {
            id: id.to_string(),
            kind: BlockType::Square,
            width_in_columns: Some(1),
            ..ParsedBlock::default()
        }
    }

    /// `layout.spec.ts`: two 100 by 50 blocks in two columns span
    /// `2 * (100 + padding) + padding`.
    #[test]
    fn root_width_follows_the_padding() {
        for (padding, expected) in [(4.0, 212.0), (20.0, 260.0)] {
            let mut db = BlockDb::new();
            db.set_hierarchy(vec![
                ParsedBlock {
                    kind: BlockType::ColumnSetting,
                    columns: Some(2),
                    ..ParsedBlock::default()
                },
                leaf("b1"),
                leaf("b2"),
            ])
            .expect("populates");
            for &child in &db.blocks().to_vec() {
                db.block_mut(child).size = Some(Size {
                    width: 100.0,
                    height: 50.0,
                    x: 0.0,
                    y: 0.0,
                });
            }
            let bounds = layout(&mut db, padding);
            let root = db.block(db.root()).size.expect("root sized");
            assert_eq!(root.width, expected);
            assert_eq!(bounds.width, 2.0 * 100.0 + padding);
        }
    }

    /// Children of the root sit in a row starting at `-padding`, centred
    /// on their cells, with `y` measured from the root's top.
    #[test]
    fn children_are_placed_left_to_right() {
        let mut db = BlockDb::new();
        db.set_hierarchy(vec![leaf("a"), leaf("b")])
            .expect("populates");
        for &child in &db.blocks().to_vec() {
            db.block_mut(child).size = Some(Size {
                width: 40.0,
                height: 20.0,
                x: 0.0,
                y: 0.0,
            });
        }
        layout(&mut db, 8.0);
        let a = db.block(db.blocks()[0]).size.expect("sized");
        let b = db.block(db.blocks()[1]).size.expect("sized");
        assert_eq!(a.x, 20.0);
        assert_eq!(b.x, 68.0);
        // root height = 20 + 2 * 8 = 36; y = -18 + 10 + 8 = 0.
        assert_eq!(a.y, 0.0);
        assert_eq!(b.y, 0.0);
    }
}
