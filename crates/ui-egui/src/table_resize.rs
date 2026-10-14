//! Interactive mouse dragging to resize and select table rows, columns, and the whole table.
//!
//! - Hovering above a column header: cursor becomes pointing hand with a downward arrow (↓),
//!   clicking selects the entire column (`table.selectColumn`).
//! - Hovering in the left margin beside a row: cursor becomes pointing hand with a rightward arrow (→),
//!   clicking selects the entire row (`table.selectRow`).
//! - Hovering near the top-left handle (✥): clicking selects the entire table (`table.selectTable`).
//! - Hovering over a column border: shows `ResizeHorizontal` cursor (⬄),
//!   dragging resizes that column (`table.columnWidth`).
//! - Hovering over a row border: shows `ResizeVertical` cursor (⇳),
//!   dragging resizes that row (`table.rowHeight`).
//! - Hovering over the bottom-right corner of the table: shows `ResizeNwSe` cursor (⤡),
//!   dragging resizes the whole table (`table.resizeTable`).

use egui::{Color32, Pos2, Rect, Stroke, Ui, pos2, vec2};
use serde_json::json;
use wordcraft_doc::Path;
use wordcraft_layout::{DocLayout, Placed};

use crate::WordApp;
use crate::theme::Tokens;

/// Hover detection threshold for border dividers in screen pixels.
const BORDER_THRESHOLD: f32 = 4.0;
/// Corner handle hit threshold in screen pixels.
const CORNER_THRESHOLD: f32 = 8.0;

/// What part of a table is being grabbed / hovered.
#[derive(Clone, Debug, PartialEq)]
pub enum TableGrab {
    Column { path: Path, row: usize, col: usize, start_w: f32, start_x: f32, border_x: f32, y0: f32, y1: f32, is_last_col: bool },
    Row { path: Path, row: usize, start_h: f32, start_y: f32, border_y: f32, x0: f32, x1: f32 },
    Corner { path: Path, start_w: f32, start_h: f32, start_pos: Pos2, corner_pos: Pos2, min_screen: Pos2 },
    SelectColumn { path: Path, col: usize, col_x0: f32, col_x1: f32, top_y: f32 },
    SelectRow { path: Path, row: usize, left_x: f32, row_y0: f32, row_y1: f32 },
    SelectTable { path: Path, handle_pos: Pos2 },
}

impl TableGrab {
    pub fn cursor(&self) -> egui::CursorIcon {
        match self {
            TableGrab::Column { .. } => egui::CursorIcon::ResizeHorizontal,
            TableGrab::Row { .. } => egui::CursorIcon::ResizeVertical,
            TableGrab::Corner { .. } => egui::CursorIcon::ResizeNwSe,
            TableGrab::SelectColumn { .. } | TableGrab::SelectRow { .. } => egui::CursorIcon::PointingHand,
            TableGrab::SelectTable { .. } => egui::CursorIcon::Move,
        }
    }
}

/// Active drag state for resizing table rows/columns/whole table.
#[derive(Clone, Debug, PartialEq)]
pub struct TableDrag {
    pub grab: TableGrab,
    pub page: usize,
    pub current_pos: Pos2,
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

        // Find table bounds on this page to hit-test top-left and bottom-right handles.
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

        // 1. Check top-left table select handle (✥).
        for tb in &tables {
            let top_left = pos2(pr.min.x + tb.min_x * page_scale, pr.min.y + tb.min_y * page_scale);
            let select_handle_pos = top_left - vec2(10.0, 10.0);
            if at.distance(select_handle_pos) <= 10.0 {
                return Some((TableGrab::SelectTable { path: tb.path.clone(), handle_pos: select_handle_pos }, page_idx));
            }
        }

        // 2. Check bottom-right table resize handle (◻).
        for tb in &tables {
            let top_left = pos2(pr.min.x + tb.min_x * page_scale, pr.min.y + tb.min_y * page_scale);
            let corner_screen = pos2(pr.min.x + tb.max_x * page_scale, pr.min.y + tb.max_y * page_scale);
            if at.distance(corner_screen) <= CORNER_THRESHOLD {
                return Some((
                    TableGrab::Corner {
                        path: tb.path.clone(),
                        start_w: (tb.max_x - tb.min_x).max(20.0),
                        start_h: (tb.max_y - tb.min_y).max(10.0),
                        start_pos: at,
                        corner_pos: corner_screen,
                        min_screen: top_left,
                    },
                    page_idx,
                ));
            }
        }

        // 3. Check column header selection band (zone just above top row).
        for item in &page.items {
            if let Placed::Cell { rect, table, row, cell, story: cell_story, .. } = item {
                if *cell_story != story || *row != 0 {
                    continue;
                }
                let cx0 = pr.min.x + rect.x * page_scale;
                let cx1 = pr.min.x + rect.right() * page_scale;
                let cy0 = pr.min.y + rect.y * page_scale;
                if at.y >= cy0 - 14.0 && at.y <= cy0 && at.x >= cx0 && at.x <= cx1 {
                    return Some((TableGrab::SelectColumn { path: table.clone(), col: *cell, col_x0: cx0, col_x1: cx1, top_y: cy0 }, page_idx));
                }
            }
        }

        // 4. Check row left margin selection band (zone just outside column 0 left border).
        for item in &page.items {
            if let Placed::Cell { rect, table, row, cell, story: cell_story, .. } = item {
                if *cell_story != story || *cell != 0 {
                    continue;
                }
                let cx0 = pr.min.x + rect.x * page_scale;
                let cy0 = pr.min.y + rect.y * page_scale;
                let cy1 = pr.min.y + rect.bottom() * page_scale;
                if at.x >= cx0 - 18.0 && at.x <= cx0 - 1.0 && at.y >= cy0 && at.y <= cy1 {
                    return Some((TableGrab::SelectRow { path: table.clone(), row: *row, left_x: cx0, row_y0: cy0, row_y1: cy1 }, page_idx));
                }
            }
        }

        // 5. Check cell resize borders (column right & row bottom dividers).
        let mut best: Option<(TableGrab, f32)> = None;

        for item in &page.items {
            if let Placed::Cell { rect, table, row, cell, story: cell_story, .. } = item {
                if *cell_story != story {
                    continue;
                }
                let tb_match = tables.iter().find(|t| t.path == *table);
                let (tb_min_x, tb_max_x, tb_min_y, tb_max_y) = match tb_match {
                    Some(t) => (t.min_x, t.max_x, t.min_y, t.max_y),
                    None => (rect.x, rect.right(), rect.y, rect.bottom()),
                };
                let table_y0 = pr.min.y + tb_min_y * page_scale;
                let table_y1 = pr.min.y + tb_max_y * page_scale;
                let table_x0 = pr.min.x + tb_min_x * page_scale;
                let table_x1 = pr.min.x + tb_max_x * page_scale;

                let cx0 = pr.min.x + rect.x * page_scale;
                let cx1 = pr.min.x + rect.right() * page_scale;
                let cy0 = pr.min.y + rect.y * page_scale;
                let cy1 = pr.min.y + rect.bottom() * page_scale;

                // Column right border.
                let dist_x = (at.x - cx1).abs();
                if dist_x <= BORDER_THRESHOLD && at.y >= cy0 - 2.0 && at.y <= cy1 + 2.0 && best.as_ref().is_none_or(|(_, d)| dist_x < *d) {
                    let is_last = (cx1 - table_x1).abs() <= 2.0;
                    best = Some((
                        TableGrab::Column {
                            path: table.clone(),
                            row: *row,
                            col: *cell,
                            start_w: rect.w,
                            start_x: at.x,
                            border_x: cx1,
                            y0: table_y0,
                            y1: table_y1,
                            is_last_col: is_last,
                        },
                        dist_x,
                    ));
                }

                // Row bottom border (skip vertically merged continuation cells whose bottom is not an actual cell bottom).
                let is_vmerge_cont = app
                    .session
                    .doc
                    .table(story, table)
                    .and_then(|t| t.rows.get(*row))
                    .and_then(|r| r.cells.get(*cell))
                    .is_some_and(|c| c.props.vmerge == wordcraft_doc::props::VMerge::Continue);
                let dist_y = (at.y - cy1).abs();
                if !is_vmerge_cont
                    && dist_y <= BORDER_THRESHOLD
                    && at.x >= cx0 - 2.0
                    && at.x <= cx1 + 2.0
                    && best.as_ref().is_none_or(|(_, d)| dist_y < *d)
                {
                    best = Some((
                        TableGrab::Row { path: table.clone(), row: *row, start_h: rect.h, start_y: at.y, border_y: cy1, x0: table_x0, x1: table_x1 },
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

/// Pointer interaction for table resizing and header selection. Returns true if handled.
pub fn pointer(app: &mut WordApp, ui: &Ui, resp: &egui::Response, pages: &[Rect], layout: &DocLayout, scale: f32, at: Pos2) -> bool {
    if let Some(d) = app.canvas.table_drag.as_mut() {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.canvas.table_drag = None;
            return true;
        }

        d.current_pos = at;

        if ui.input(|i| i.pointer.primary_down()) {
            let Some(page) = layout.pages.get(d.page) else {
                app.canvas.table_drag = None;
                return true;
            };
            let pr = pages.get(d.page).copied().unwrap_or(Rect::NOTHING);
            let page_scale = crate::canvas::page_screen_scale(pr, page, scale).max(0.01);

            let run_cmd = match &d.grab {
                TableGrab::Column { path, row, col, start_w, start_x, is_last_col, .. } => {
                    let dx = (at.x - start_x) / page_scale;
                    let new_w = (*start_w + dx).clamp(10.0, 1500.0);
                    let shift = ui.input(|i| i.modifiers.shift);
                    let adjust_next = !shift && !*is_last_col;
                    Some((
                        "table.columnWidth",
                        json!({
                            "width": new_w,
                            "adjustNext": adjust_next,
                            "path": path.to_string(),
                            "row": row,
                            "col": col,
                        }),
                    ))
                }
                TableGrab::Row { path, row, start_h, start_y, .. } => {
                    let dy = (at.y - start_y) / page_scale;
                    let new_h = (*start_h + dy).clamp(8.0, 1500.0);
                    Some((
                        "table.rowHeight",
                        json!({
                            "height": new_h,
                            "rule": "atLeast",
                            "path": path.to_string(),
                            "row": row,
                        }),
                    ))
                }
                TableGrab::Corner { path, start_w, start_h, start_pos, .. } => {
                    let dx = (at.x - start_pos.x) / page_scale;
                    let dy = (at.y - start_pos.y) / page_scale;
                    let new_w = (*start_w + dx).clamp(20.0, 3000.0);
                    let new_h = (*start_h + dy).clamp(10.0, 5000.0);
                    Some((
                        "table.resizeTable",
                        json!({
                            "width": new_w,
                            "height": new_h,
                            "path": path.to_string(),
                        }),
                    ))
                }
                _ => None,
            };
            if let Some((cmd, params)) = run_cmd {
                app.session.join_next_undo();
                let _ = app.run(cmd, params);
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
        match grab {
            TableGrab::SelectColumn { path, col, .. } => {
                let _ = app.run("table.selectColumn", json!({ "path": path.to_string(), "col": col }));
                return true;
            }
            TableGrab::SelectRow { path, row, .. } => {
                let _ = app.run("table.selectRow", json!({ "path": path.to_string(), "row": row }));
                return true;
            }
            TableGrab::SelectTable { path, .. } => {
                let _ = app.run("table.selectTable", json!({ "path": path.to_string() }));
                return true;
            }
            resize_grab => {
                app.canvas.table_drag = Some(TableDrag { grab: resize_grab, page, current_pos: at });
                return true;
            }
        }
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

/// Paint table resize handles, select arrows, and drag guidelines.
pub fn paint(app: &WordApp, painter: &egui::Painter, t: &Tokens, layout: &DocLayout, pages: &[Rect], scale: f32, hover: Option<Pos2>) {
    let story = app.session.sel.focus.story;
    let in_table_path = app.session.sel.focus.path.cell().map(|(tp, _, _)| tp);

    // 1. Paint top-left select handle (✥) and bottom-right resize handle (◻) for active or hovered table.
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
            let is_hovered = hover.is_some_and(|hp| {
                let table_screen_rect = Rect::from_min_max(
                    pos2(pr.min.x + tb.min_x * page_scale - 18.0, pr.min.y + tb.min_y * page_scale - 18.0),
                    pos2(pr.min.x + tb.max_x * page_scale + 12.0, pr.min.y + tb.max_y * page_scale + 12.0),
                );
                table_screen_rect.contains(hp)
            });
            let top_left = pos2(pr.min.x + tb.min_x * page_scale, pr.min.y + tb.min_y * page_scale);
            let corner_screen = pos2(pr.min.x + tb.max_x * page_scale, pr.min.y + tb.max_y * page_scale);

            // Draw MS Word-style top-left select table handle (✥) when active or hovered.
            if is_active || is_hovered || app.canvas.table_drag.is_some() {
                let select_handle_pos = top_left - vec2(10.0, 10.0);
                let sr = Rect::from_center_size(select_handle_pos, vec2(12.0, 12.0));
                painter.rect_filled(sr, 1.0, Color32::WHITE);
                painter.rect_stroke(sr, 1.0, Stroke::new(1.0, t.accent), egui::StrokeKind::Middle);
                let c = sr.center();
                let icon_color = t.accent_text;
                painter.line_segment([pos2(c.x - 3.5, c.y), pos2(c.x + 3.5, c.y)], Stroke::new(1.2, icon_color));
                painter.line_segment([pos2(c.x, c.y - 3.5), pos2(c.x, c.y + 3.5)], Stroke::new(1.2, icon_color));
            }

            // Draw bottom-right resize square handle (◻).
            if is_active || is_hovered || app.canvas.table_drag.is_some() {
                let size = 6.0;
                let hr = Rect::from_center_size(corner_screen, vec2(size, size));
                painter.rect_filled(hr, 1.0, Color32::WHITE);
                painter.rect_stroke(hr, 1.0, Stroke::new(1.0, t.accent), egui::StrokeKind::Middle);
            }
        }
    }

    // 2. Paint selection hover arrows (downward arrow for column, rightward arrow for row).
    if let Some(p) = hover
        && let Some((grab, _)) = grab_at(app, layout, pages, scale, p)
    {
        match grab {
            TableGrab::SelectColumn { col_x0, col_x1, top_y, .. } => {
                let cx = (col_x0 + col_x1) * 0.5;
                let cy = top_y - 7.0;
                let arr_color = Color32::from_rgb(0x18, 0x18, 0x1B);
                painter.line_segment([pos2(cx, cy - 5.0), pos2(cx, cy + 3.0)], Stroke::new(2.0, arr_color));
                painter.line_segment([pos2(cx - 3.5, cy), pos2(cx, cy + 3.5)], Stroke::new(2.0, arr_color));
                painter.line_segment([pos2(cx + 3.5, cy), pos2(cx, cy + 3.5)], Stroke::new(2.0, arr_color));
            }
            TableGrab::SelectRow { left_x, row_y0, row_y1, .. } => {
                let cx = left_x - 8.0;
                let cy = (row_y0 + row_y1) * 0.5;
                let arr_color = Color32::from_rgb(0x18, 0x18, 0x1B);
                painter.line_segment([pos2(cx - 4.0, cy), pos2(cx + 4.0, cy)], Stroke::new(2.0, arr_color));
                painter.line_segment([pos2(cx + 1.0, cy - 3.5), pos2(cx + 4.5, cy)], Stroke::new(2.0, arr_color));
                painter.line_segment([pos2(cx + 1.0, cy + 3.5), pos2(cx + 4.5, cy)], Stroke::new(2.0, arr_color));
            }
            _ => {}
        }
    }

    // 3. Paint ghost guide line or box during active drag.
    if let Some(d) = &app.canvas.table_drag {
        let stroke = Stroke::new(1.5, t.accent.gamma_multiply(0.85));
        let p = d.current_pos;
        match &d.grab {
            TableGrab::Column { y0, y1, .. } => {
                // Live guide line follows current mouse x across the full height of the table.
                painter.line_segment([pos2(p.x, *y0), pos2(p.x, *y1)], stroke);
            }
            TableGrab::Row { x0, x1, .. } => {
                // Live guide line follows current mouse y across the full width of the table.
                painter.line_segment([pos2(*x0, p.y), pos2(*x1, p.y)], stroke);
            }
            TableGrab::Corner { min_screen, .. } => {
                // Live ghost outline bounding box of the resizing table.
                let ghost_rect = Rect::from_min_max(*min_screen, pos2(p.x.max(min_screen.x + 20.0), p.y.max(min_screen.y + 10.0)));
                painter.rect_stroke(ghost_rect, 0.0, stroke, egui::StrokeKind::Middle);
            }
            _ => {}
        }
    }
}
