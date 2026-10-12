//! Tables: a column grid, rows and cells (cells hold block lists). Merges follow OOXML: a cell
//! spans `span` grid columns; vertical merges are `Restart` + `Continue` cells.

use serde::{Deserialize, Serialize};

use crate::props::{CellProps, RowProps, TableProps, VMerge};
use crate::{Blocks, Paragraph, para_block};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Cell {
    #[serde(default)]
    pub props: CellProps,
    pub blocks: Blocks,
}

impl Cell {
    pub fn empty() -> Cell {
        Cell { props: CellProps { span: 1, ..Default::default() }, blocks: vec![para_block(Paragraph::new())] }
    }
    pub fn with_text(s: &str) -> Cell {
        Cell { props: CellProps { span: 1, ..Default::default() }, blocks: vec![para_block(Paragraph::with_text(s, Default::default()))] }
    }
    pub fn span(&self) -> usize {
        self.props.span.clamp(1, 63) as usize
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Row {
    #[serde(default)]
    pub props: RowProps,
    pub cells: Vec<Cell>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Table {
    #[serde(default)]
    pub props: TableProps,
    /// Grid column widths, points.
    pub grid: Vec<f32>,
    pub rows: Vec<Row>,
}

/// Hard limits that keep hostile input from exhausting memory.
pub const MAX_ROWS: usize = 32_767;
pub const MAX_COLS: usize = 63;

impl Table {
    /// A `rows × cols` table spanning `width` points, "Table Grid" style.
    pub fn new(rows: usize, cols: usize, width: f32) -> Table {
        let rows = rows.clamp(1, MAX_ROWS);
        let cols = cols.clamp(1, MAX_COLS);
        let w = (width / cols as f32).max(6.0);
        Table {
            props: TableProps { style: Some("TableGrid".into()), ..Default::default() },
            grid: vec![w; cols],
            rows: (0..rows)
                .map(|_| Row {
                    props: RowProps::default(),
                    cells: (0..cols)
                        .map(|_| {
                            let mut c = Cell::empty();
                            c.props.width = Some(w);
                            c
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    pub fn cols(&self) -> usize {
        self.grid.len().max(self.rows.iter().map(|r| r.cells.iter().map(Cell::span).sum::<usize>()).max().unwrap_or(0))
    }

    /// Grid column where cell `c` of row `r` starts.
    pub fn grid_col(&self, r: usize, c: usize) -> usize {
        self.rows.get(r).map(|row| row.cells.iter().take(c).map(Cell::span).sum()).unwrap_or(0)
    }

    /// Cell index in row `r` that covers grid column `g`.
    pub fn cell_at_grid(&self, r: usize, g: usize) -> Option<usize> {
        let row = self.rows.get(r)?;
        let mut x = 0;
        for (i, c) in row.cells.iter().enumerate() {
            if g < x + c.span() {
                return Some(i);
            }
            x += c.span();
        }
        None
    }

    /// Insert a row copied (formatting, empty text) from row `like` at index `at`.
    pub fn insert_row(&mut self, at: usize, like: usize) {
        let Some(src) = self.rows.get(like.min(self.rows.len().saturating_sub(1))) else { return };
        if self.rows.len() >= MAX_ROWS {
            return;
        }
        let mut row = Row { props: src.props.clone(), cells: Vec::with_capacity(src.cells.len()) };
        row.props.header = false;
        for c in &src.cells {
            let mut nc = Cell::empty();
            nc.props = c.props.clone();
            if nc.props.vmerge == VMerge::Restart {
                nc.props.vmerge = VMerge::None;
            }
            row.cells.push(nc);
        }
        self.rows.insert(at.min(self.rows.len()), row);
        self.repair_vmerge();
    }

    pub fn delete_row(&mut self, r: usize) {
        if r < self.rows.len() {
            self.rows.remove(r);
        }
        self.repair_vmerge();
    }

    /// Insert a grid column at grid index `g` (cells spanning it widen instead).
    pub fn insert_col(&mut self, g: usize) {
        if self.grid.len() >= MAX_COLS {
            return;
        }
        let g = g.min(self.grid.len());
        let w = self.grid.get(g.min(self.grid.len().saturating_sub(1))).copied().unwrap_or(72.0);
        self.grid.insert(g, w);
        for r in 0..self.rows.len() {
            let mut x = 0;
            let mut idx = None;
            if let Some(row) = self.rows.get(r) {
                for (i, c) in row.cells.iter().enumerate() {
                    if g <= x {
                        idx = Some((i, false));
                        break;
                    }
                    if g < x + c.span() {
                        idx = Some((i, true));
                        break;
                    }
                    x += c.span();
                }
            }
            if let Some(row) = self.rows.get_mut(r) {
                match idx {
                    Some((i, true)) => {
                        if let Some(c) = row.cells.get_mut(i) {
                            c.props.span += 1;
                        }
                    }
                    Some((i, false)) => {
                        let mut c = Cell::empty();
                        c.props.width = Some(w);
                        row.cells.insert(i, c);
                    }
                    None => {
                        let mut c = Cell::empty();
                        c.props.width = Some(w);
                        row.cells.push(c);
                    }
                }
            }
        }
        self.repair_vmerge();
    }

    /// Delete grid column `g` (spanning cells shrink).
    pub fn delete_col(&mut self, g: usize) {
        if g >= self.grid.len() || self.grid.len() <= 1 {
            return;
        }
        self.grid.remove(g);
        for row in &mut self.rows {
            let mut x = 0;
            let mut rm = None;
            for (i, c) in row.cells.iter_mut().enumerate() {
                if g < x + c.span() {
                    if c.span() > 1 {
                        c.props.span -= 1;
                    } else {
                        rm = Some(i);
                    }
                    break;
                }
                x += c.span();
            }
            if let Some(i) = rm {
                row.cells.remove(i);
            }
        }
        self.rows.retain(|r| !r.cells.is_empty());
        self.repair_vmerge();
    }

    /// Merge cells `c0..=c1` of row `r` horizontally (content is concatenated).
    pub fn merge_right(&mut self, r: usize, c0: usize, c1: usize) {
        let Some(row) = self.rows.get_mut(r) else { return };
        if c1 <= c0 || c1 >= row.cells.len() {
            return;
        }
        let removed: Vec<Cell> = row.cells.drain(c0 + 1..=c1).collect();
        if let Some(first) = row.cells.get_mut(c0) {
            for c in removed {
                first.props.span += c.props.span.max(1);
                let nonempty: Vec<_> = c.blocks.into_iter().filter(|b| b.as_para().is_none_or(|p| !p.is_empty())).collect();
                first.blocks.extend(nonempty);
            }
            if first.blocks.len() > 1 && first.blocks.first().and_then(|b| b.as_para()).is_some_and(Paragraph::is_empty) {
                first.blocks.remove(0);
            }
        }
    }

    /// Merge a rectangle of cells (rows r0..=r1, grid columns g0..=g1).
    pub fn merge(&mut self, r0: usize, r1: usize, g0: usize, g1: usize) {
        for r in r0..=r1.min(self.rows.len().saturating_sub(1)) {
            let (Some(a), Some(b)) = (self.cell_at_grid(r, g0), self.cell_at_grid(r, g1)) else { continue };
            self.merge_right(r, a, b);
            if let Some(c) = self.rows.get_mut(r).and_then(|row| row.cells.get_mut(a)) {
                c.props.vmerge = if r1 == r0 {
                    VMerge::None
                } else if r == r0 {
                    VMerge::Restart
                } else {
                    VMerge::Continue
                };
            }
        }
        // Move content of continued cells into the restart cell.
        if r1 > r0
            && let Some(a) = self.cell_at_grid(r0, g0)
        {
            let mut moved = Vec::new();
            for r in r0 + 1..=r1.min(self.rows.len().saturating_sub(1)) {
                if let Some(c) = self.cell_at_grid(r, g0).and_then(|ci| self.rows.get_mut(r).and_then(|row| row.cells.get_mut(ci))) {
                    let blocks = std::mem::replace(&mut c.blocks, vec![para_block(Paragraph::new())]);
                    moved.extend(blocks.into_iter().filter(|b| b.as_para().is_none_or(|p| !p.is_empty())));
                }
            }
            if let Some(c) = self.rows.get_mut(r0).and_then(|row| row.cells.get_mut(a)) {
                c.blocks.extend(moved);
                if c.blocks.len() > 1 && c.blocks.first().and_then(|b| b.as_para()).is_some_and(Paragraph::is_empty) {
                    c.blocks.remove(0);
                }
            }
        }
        self.repair_vmerge();
    }

    /// Split cell `c` of row `r` into `n` cells horizontally.
    pub fn split_cell(&mut self, r: usize, c: usize, n: usize) {
        let n = n.clamp(1, MAX_COLS);
        let Some(row) = self.rows.get(r) else { return };
        let Some(cell) = row.cells.get(c) else { return };
        let span = cell.span();
        if n <= 1 {
            return;
        }
        let g = self.grid_col(r, c);

        if span >= n {
            // Enough grid columns: just re-span
            let sub = span / n;
            let rem = span % n;
            let first_span = {
                let Some(row) = self.rows.get_mut(r) else { return };
                let Some(first) = row.cells.get_mut(c) else { return };
                first.props.span = (sub + if rem > 0 { 1 } else { 0 }) as u32;
                first.props.width = self.grid.get(g).copied();
                first.span()
            };
            let mut g_off = g + first_span;
            for k in 1..n {
                let this_span = sub + if k < rem { 1 } else { 0 };
                let this_span = this_span.max(1);
                let w = self.grid.get(g_off).copied();
                let mut nc = Cell::empty();
                nc.props.span = this_span as u32;
                nc.props.width = w;
                if let Some(row) = self.rows.get_mut(r) {
                    row.cells.insert(c + k, nc);
                }
                g_off += this_span;
            }
        } else {
            // Need to subdivide grid columns: split the last column of the span into (n - span + 1) parts
            let extra = n - span;
            if self.grid.len() + extra > MAX_COLS {
                return;
            }
            let last_g = g + span.saturating_sub(1);
            let old_w = self.grid.get(last_g).copied().unwrap_or(72.0);
            let sub_w = (old_w / (extra + 1) as f32).max(6.0);
            // Replace grid[last_g] with (extra+1) sub-columns
            if let Some(entry) = self.grid.get_mut(last_g) {
                *entry = sub_w;
            }
            for _ in 0..extra {
                self.grid.insert(last_g + 1, sub_w);
            }
            // Widen spans in other rows that cover last_g
            for ri in 0..self.rows.len() {
                if ri == r {
                    continue;
                }
                if let Some(ci) = self.cell_at_grid(ri, last_g) {
                    let cell_start_g = self.grid_col(ri, ci);
                    if let Some(cell) = self.rows.get_mut(ri).and_then(|row| row.cells.get_mut(ci)) {
                        let cell_end_g = cell_start_g + cell.span().saturating_sub(1);
                        if cell_end_g >= last_g {
                            cell.props.span += extra as u32;
                        }
                    }
                }
            }
            // Now target row has `span` columns, need `n` cells of span 1 each
            let total_new_span = span + extra;
            let _ = total_new_span;
            let w = self.grid.get(g).copied().unwrap_or(6.0);
            if let Some(row) = self.rows.get_mut(r) {
                let mut orig = if c < row.cells.len() { row.cells.remove(c) } else { Cell::empty() };
                orig.props.span = 1;
                orig.props.width = Some(w);
                row.cells.insert(c, orig);
                for k in 1..n {
                    let mut nc = Cell::empty();
                    nc.props.span = 1;
                    nc.props.width = Some(w);
                    row.cells.insert(c + k, nc);
                }
            }
        }
    }

    /// Repair vertical merge chains across all grid columns.
    pub fn repair_vmerge(&mut self) {
        for g in 0..self.grid.len() {
            let mut active_restart: Option<(usize, usize, usize)> = None;
            let mut prev_ci: Option<usize> = None;
            let mut continue_count = 0;

            for r in 0..self.rows.len() {
                let Some(ci) = self.cell_at_grid(r, g) else {
                    if let Some((restart_r, restart_ci, _)) = active_restart.take() {
                        if continue_count == 0 {
                            if let Some(c) = self.rows.get_mut(restart_r).and_then(|row| row.cells.get_mut(restart_ci)) {
                                c.props.vmerge = VMerge::None;
                            }
                        }
                    }
                    prev_ci = None;
                    continue;
                };

                let cell_vmerge = self.rows.get(r).and_then(|row| row.cells.get(ci)).map(|c| c.props.vmerge).unwrap_or(VMerge::None);
                let cell_span = self.rows.get(r).and_then(|row| row.cells.get(ci)).map(|c| c.span()).unwrap_or(1);

                match cell_vmerge {
                    VMerge::Restart => {
                        if let Some((restart_r, restart_ci, _)) = active_restart.take() {
                            if continue_count == 0 {
                                if let Some(c) = self.rows.get_mut(restart_r).and_then(|row| row.cells.get_mut(restart_ci)) {
                                    c.props.vmerge = VMerge::None;
                                }
                            }
                        }
                        active_restart = Some((r, ci, cell_span));
                        prev_ci = Some(ci);
                        continue_count = 0;
                    }
                    VMerge::Continue => {
                        let is_valid = if let Some((_, _, restart_span)) = active_restart {
                            prev_ci == Some(ci) && cell_span == restart_span
                        } else {
                            false
                        };

                        if is_valid {
                            continue_count += 1;
                            prev_ci = Some(ci);
                        } else {
                            if let Some((restart_r, restart_ci, _)) = active_restart.take() {
                                if continue_count == 0 {
                                    if let Some(c) = self.rows.get_mut(restart_r).and_then(|row| row.cells.get_mut(restart_ci)) {
                                        c.props.vmerge = VMerge::None;
                                    }
                                }
                            }
                            if let Some(c) = self.rows.get_mut(r).and_then(|row| row.cells.get_mut(ci)) {
                                c.props.vmerge = VMerge::None;
                            }
                            prev_ci = Some(ci);
                        }
                    }
                    VMerge::None => {
                        if let Some((restart_r, restart_ci, _)) = active_restart.take() {
                            if continue_count == 0 {
                                if let Some(c) = self.rows.get_mut(restart_r).and_then(|row| row.cells.get_mut(restart_ci)) {
                                    c.props.vmerge = VMerge::None;
                                }
                            }
                        }
                        prev_ci = Some(ci);
                    }
                }
            }

            if let Some((restart_r, restart_ci, _)) = active_restart.take() {
                if continue_count == 0 {
                    if let Some(c) = self.rows.get_mut(restart_r).and_then(|row| row.cells.get_mut(restart_ci)) {
                        c.props.vmerge = VMerge::None;
                    }
                }
            }
        }
    }

    pub fn distribute_rows(&mut self) {
        let total: f32 = self.rows.iter().filter_map(|r| r.props.height).sum::<f32>();
        let count = self.rows.len().max(1);
        let h = if total > 0.0 { total / count as f32 } else { 18.0 }.max(4.0);
        for r in &mut self.rows {
            r.props.height = Some(h);
        }
    }

    pub fn total_width(&self) -> f32 {
        self.grid.iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_table_shape() {
        let t = Table::new(3, 4, 468.0);
        assert_eq!(t.rows.len(), 3);
        assert_eq!(t.cols(), 4);
        assert_eq!(t.total_width(), 468.0);
        let huge = Table::new(1_000_000, 1000, 100.0);
        assert_eq!(huge.rows.len(), MAX_ROWS);
        assert_eq!(huge.cols(), MAX_COLS);
    }

    #[test]
    fn row_col_ops() {
        let mut t = Table::new(2, 2, 200.0);
        t.insert_row(1, 0);
        assert_eq!(t.rows.len(), 3);
        t.insert_col(1);
        assert_eq!(t.cols(), 3);
        assert!(t.rows.iter().all(|r| r.cells.len() == 3));
        t.delete_col(0);
        assert_eq!(t.cols(), 2);
        t.delete_row(0);
        assert_eq!(t.rows.len(), 2);
        t.delete_row(99);
        t.delete_col(99);
    }

    #[test]
    fn merge_and_split() {
        let mut t = Table::new(2, 3, 300.0);
        t.merge_right(0, 0, 1);
        assert_eq!(t.rows[0].cells.len(), 2);
        assert_eq!(t.rows[0].cells[0].span(), 2);
        assert_eq!(t.cell_at_grid(0, 1), Some(0));
        assert_eq!(t.grid_col(0, 1), 2);
        t.insert_col(1);
        assert_eq!(t.rows[0].cells[0].span(), 3);
        t.split_cell(0, 0, 3);
        assert_eq!(t.rows[0].cells.len(), 4);
        let mut v = Table::new(3, 2, 100.0);
        v.merge(0, 2, 0, 0);
        assert_eq!(v.rows[0].cells[0].props.vmerge, VMerge::Restart);
        assert_eq!(v.rows[2].cells[0].props.vmerge, VMerge::Continue);
    }

    #[test]
    fn vmerge_repair_removes_orphan_continues() {
        let mut t = Table::new(3, 2, 200.0);
        t.merge(0, 1, 0, 0);
        assert_eq!(t.rows[0].cells[0].props.vmerge, VMerge::Restart);
        assert_eq!(t.rows[1].cells[0].props.vmerge, VMerge::Continue);
        t.delete_row(0);
        let c0 = t.cell_at_grid(0, 0).and_then(|ci| t.rows.get(0).and_then(|r| r.cells.get(ci))).unwrap();
        assert_ne!(c0.props.vmerge, VMerge::Continue);
    }

    #[test]
    fn split_cell_does_not_widen_table() {
        let mut t = Table::new(2, 2, 200.0);
        assert_eq!(t.total_width(), 200.0);
        t.split_cell(0, 0, 2);
        assert_eq!(t.total_width(), 200.0);
    }

    #[test]
    fn distribute_rows_averages_height() {
        let mut t = Table::new(3, 1, 100.0);
        t.rows[0].props.height = Some(10.0);
        t.rows[1].props.height = Some(20.0);
        t.rows[2].props.height = Some(30.0);
        t.distribute_rows();
        assert_eq!(t.rows[0].props.height, Some(20.0));
        assert_eq!(t.rows[1].props.height, Some(20.0));
        assert_eq!(t.rows[2].props.height, Some(20.0));
    }
}
