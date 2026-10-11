//! Interactive mouse dragging to resize table rows, columns, and the whole table.
//!
//! Hovering over a column border shows `ResizeHorizontal` cursor (⬄),
//! dragging resizes that column (`table.columnWidth`).
//! Hovering over a row border shows `ResizeVertical` cursor (⇳),
//! dragging resizes that row (`table.rowHeight`).
//! Hovering over the bottom-right corner of the table shows `ResizeNorthWestSouthEast` cursor (⤡),
//! dragging resizes the whole table (`table.resizeTable`).

use egui::{Color32, Pos2, Rect, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_doc::Path;
use wordcraft_layout::{DocLayout, Placed};

use crate::WordApp;
use crate::theme::Tokens;

/// Hover detection threshold in screen pixels.
const BORDER_THRESHOLD: f32 = 4.0;
/// Corner handle hit threshold in screen pixels.
const CORNER_THRESHOLD: f32 = 8.0;

/// What part of a table is being grabbed / hovered.
#[derive(Clone, Debug, PartialEq)]
pub enum TableGrab {
    Column { path: Path, row: usize, col: usize, start_w: f32, start_x: f32, border_x: f32, y0: f32, y1: f32 },
    Row { path: Path, row: usize, start_h: f32, start_y: f32, border_y: f32, x0: f32, x1: f32 },
    Corner { path: Path, start_w: f32, start_h: f32, start_pos: Pos2, corner_pos: Pos2 },
}

impl TableGrab {
    pub fn cursor(&self) -> egui::CursorIcon {
        match self {
            TableGrab::Column { .. } => egui::CursorIcon::ResizeHorizontal,
            TableGrab::Row { .. } => egui::CursorIcon::ResizeVertical,
            TableGrab::Corner { .. } => egui::CursorIcon::ResizeNwSe,
        }
    }
}

/// Active drag state for resizing table rows/columns/whole table.
#[derive(Clone, Debug, PartialEq)]
pub struct TableDrag {
    pub grab: TableGrab,
    pub page: usize,
}

/// Bounding info for a table on a given page.
struct TableBounds {
    path: Path,
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
}

/// Detect what table element is under screen point `at`.
fn grab_at(app: &WordApp, layout: &DocLayout, pages: &[Rect], scale: f32, at: Pos2) -> Option<(TableGrab, usize)> {
    if app.canvas.table_tool.is_some() {
        return None;
    }
    let story = app.session.sel.focus.story;

    for (page_idx, pr) in pages.iter().enumerate() {
        let Some(page) = layout.pages.get(page_idx) else { continue };
        let page_scale = crate::canvas::page_screen_scale(*pr, page, scale);

        // Find table bounds on this page to hit-test bottom-right corner.
        let mut tables: Vec<TableBounds> = Vec::new();
        for item in &page.items {
            if let Placed::Cell { rect, table, story: cell_story, .. } = item {
                if *cell_story != story {
                    continue;
                }
                if let Some(tb) = tables.iter_mut().find(|t| t.path == *table) {
                    tb.min_x = tb.min_x.min(rect.x);
                    tb.max_x = tb.max_x.max(rect.right());
                    tb.min_y = tb.min_y.min(rect.y);
                    tb.max_y = tb.max_y.max(rect.bottom());
                } else {
                    tables.push(TableBounds { path: table.clone(), min_x: rect.x, max_x: rect.right(), min_y: rect.y, max_y: rect.bottom() });
                }
            }
        }

        // Check corner handles first.
        for tb in &tables {
            let corner_screen = pos2(pr.min.x + tb.max_x * page_scale, pr.min.y + tb.max_y * page_scale);
            if at.distance(corner_screen) <= CORNER_THRESHOLD {
                return Some((
                    TableGrab::Corner {
                        path: tb.path.clone(),
                        start_w: (tb.max_x - tb.min_x).max(20.0),
                        start_h: (tb.max_y - tb.min_y).max(10.0),
                        start_pos: at,
                        corner_pos: corner_screen,
                    },
                    page_idx,
                ));
            }
        }

        // Check cell borders (columns & rows).
        let mut best: Option<(TableGrab, f32)> = None;

        for item in &page.items {
            if let Placed::Cell { rect, table, row, cell, story: cell_story } = item {
                if *cell_story != story {
                    continue;
                }
                let cx0 = pr.min.x + rect.x * page_scale;
                let cx1 = pr.min.x + rect.right() * page_scale;
                let cy0 = pr.min.y + rect.y * page_scale;
                let cy1 = pr.min.y + rect.bottom() * page_scale;

                // Column right border.
                let dist_x = (at.x - cx1).abs();
                if dist_x <= BORDER_THRESHOLD && at.y >= cy0 - 2.0 && at.y <= cy1 + 2.0 && best.as_ref().is_none_or(|(_, d)| dist_x < *d) {
                    best = Some((
                        TableGrab::Column {
                            path: table.clone(),
                            row: *row,
                            col: *cell,
                            start_w: rect.w,
                            start_x: at.x,
                            border_x: cx1,
                            y0: cy0,
                            y1: cy1,
                        },
                        dist_x,
                    ));
                }

                // Row bottom border.
                let dist_y = (at.y - cy1).abs();
                if dist_y <= BORDER_THRESHOLD && at.x >= cx0 - 2.0 && at.x <= cx1 + 2.0 && best.as_ref().is_none_or(|(_, d)| dist_y < *d) {
                    best = Some((
                        TableGrab::Row { path: table.clone(), row: *row, start_h: rect.h, start_y: at.y, border_y: cy1, x0: cx0, x1: cx1 },
                        dist_y,
                    ));
                }
            }
        }

        if let Some((grab, _)) = best {
            return Some((grab, page_idx));
        }
    }

    None
}

/// Pointer interaction for table resizing. Returns true if handled.
pub fn pointer(app: &mut WordApp, ui: &Ui, resp: &egui::Response, pages: &[Rect], layout: &DocLayout, scale: f32, at: Pos2) -> bool {
    if let Some(d) = app.canvas.table_drag.as_ref() {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.canvas.table_drag = None;
            return true;
        }

        if ui.input(|i| i.pointer.primary_down()) {
            let Some(page) = layout.pages.get(d.page) else {
                app.canvas.table_drag = None;
                return true;
            };
            let pr = pages.get(d.page).copied().unwrap_or(Rect::NOTHING);
            let page_scale = crate::canvas::page_screen_scale(pr, page, scale).max(0.01);

            match &d.grab {
                TableGrab::Column { path, row, col, start_w, start_x, .. } => {
                    let dx = (at.x - start_x) / page_scale;
                    let new_w = (*start_w + dx).clamp(10.0, 1500.0);
                    app.session.join_next_undo();
                    let _ = app.run(
                        "table.columnWidth",
                        json!({
                            "width": new_w,
                            "path": path.to_string(),
                            "row": row,
                            "col": col,
                        }),
                    );
                }
                TableGrab::Row { path, row, start_h, start_y, .. } => {
                    let dy = (at.y - start_y) / page_scale;
                    let new_h = (*start_h + dy).clamp(8.0, 1500.0);
                    app.session.join_next_undo();
                    let _ = app.run(
                        "table.rowHeight",
                        json!({
                            "height": new_h,
                            "rule": "atLeast",
                            "path": path.to_string(),
                            "row": row,
                        }),
                    );
                }
                TableGrab::Corner { path, start_w, start_h, start_pos, .. } => {
                    let dx = (at.x - start_pos.x) / page_scale;
                    let dy = (at.y - start_pos.y) / page_scale;
                    let new_w = (*start_w + dx).clamp(20.0, 3000.0);
                    let new_h = (*start_h + dy).clamp(10.0, 5000.0);
                    app.session.join_next_undo();
                    let _ = app.run(
                        "table.resizeTable",
                        json!({
                            "width": new_w,
                            "height": new_h,
                            "path": path.to_string(),
                        }),
                    );
                }
            }
            return true;
        } else {
            // Released.
            app.canvas.table_drag = None;
            return true;
        }
    }

    let pressed = ui.input(|i| i.pointer.primary_pressed()) && resp.contains_pointer();
    if !pressed {
        return false;
    }

    if let Some((grab, page)) = grab_at(app, layout, pages, scale, at) {
        app.canvas.table_drag = Some(TableDrag { grab, page });
        return true;
    }

    false
}

/// The cursor icon when hovering over table borders or handles.
pub fn cursor(app: &WordApp, layout: &DocLayout, pages: &[Rect], scale: f32, at: Option<Pos2>) -> Option<egui::CursorIcon> {
    if let Some(d) = &app.canvas.table_drag {
        return Some(d.grab.cursor());
    }
    let at = at?;
    grab_at(app, layout, pages, scale, at).map(|(g, _)| g.cursor())
}

/// Paint table resize handles and drag guidelines.
pub fn paint(app: &WordApp, painter: &egui::Painter, t: &Tokens, layout: &DocLayout, pages: &[Rect], scale: f32) {
    let story = app.session.sel.focus.story;
    let in_table_path = app.session.sel.focus.path.cell().map(|(tp, _, _)| tp);

    // 1. Paint bottom-right corner resize handle for active or hovered table.
    for (page_idx, pr) in pages.iter().enumerate() {
        let Some(page) = layout.pages.get(page_idx) else { continue };
        let page_scale = crate::canvas::page_screen_scale(*pr, page, scale);

        let mut tables: Vec<TableBounds> = Vec::new();
        for item in &page.items {
            if let Placed::Cell { rect, table, story: cell_story, .. } = item {
                if *cell_story != story {
                    continue;
                }
                if let Some(tb) = tables.iter_mut().find(|t| t.path == *table) {
                    tb.min_x = tb.min_x.min(rect.x);
                    tb.max_x = tb.max_x.max(rect.right());
                    tb.min_y = tb.min_y.min(rect.y);
                    tb.max_y = tb.max_y.max(rect.bottom());
                } else {
                    tables.push(TableBounds { path: table.clone(), min_x: rect.x, max_x: rect.right(), min_y: rect.y, max_y: rect.bottom() });
                }
            }
        }

        for tb in &tables {
            let is_active = in_table_path.as_ref() == Some(&tb.path);
            let corner_screen = pos2(pr.min.x + tb.max_x * page_scale, pr.min.y + tb.max_y * page_scale);

            // Draw Word-like square corner handle when table is active or during drag.
            if is_active || app.canvas.table_drag.is_some() {
                let size = 6.0;
                let hr = Rect::from_center_size(corner_screen, vec2(size, size));
                painter.rect_filled(hr, 1.0, Color32::WHITE);
                painter.rect_stroke(hr, 1.0, Stroke::new(1.0, t.accent), egui::StrokeKind::Middle);
            }
        }
    }

    // 2. Paint ghost guide line during active drag.
    if let Some(d) = &app.canvas.table_drag {
        let stroke = Stroke::new(1.5, t.accent.gamma_multiply(0.85));
        match &d.grab {
            TableGrab::Column { border_x, y0, y1, .. } => {
                painter.line_segment([pos2(*border_x, *y0), pos2(*border_x, *y1)], stroke);
            }
            TableGrab::Row { border_y, x0, x1, .. } => {
                painter.line_segment([pos2(*x0, *border_y), pos2(*x1, *border_y)], stroke);
            }
            TableGrab::Corner { corner_pos, .. } => {
                let hr = Rect::from_center_size(*corner_pos, vec2(8.0, 8.0));
                painter.rect_filled(hr, 1.0, t.accent);
            }
        }
    }
}
