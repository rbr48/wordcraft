//! Table Design and Table Layout tabs.

use serde_json::{Value, json};
use wordcraft_doc::props::{Align, Border, BorderStyle, Borders, HeightRule, Rgb, TextDirection, VAlign, VMerge};
use wordcraft_doc::{Block, Paragraph, Path, Pos, Table, para_block};

use super::sel_result;
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

fn in_table(s: &Session) -> Option<&'static str> {
    if s.sel.focus.path.cell().is_some() { None } else { Some("the cursor isn't in a table") }
}

fn parse_path(p_str: &str) -> Result<Path, CmdError> {
    let parts: Result<Vec<u32>, _> = p_str.split(['.', '/']).map(str::parse).collect();
    parts.map(Path).map_err(|_| CmdError::Params("invalid table path".into()))
}

pub fn specs() -> Vec<CommandSpec> {
    let t = |c: CommandSpec| c.when(in_table);
    vec![
        t(CommandSpec::new("table.insertRowAbove", "Insert Above", "Table Layout › Rows & Columns", |s, _| row(s, false))),
        t(CommandSpec::new("table.insertRowBelow", "Insert Below", "Table Layout › Rows & Columns", |s, _| row(s, true))),
        t(CommandSpec::new("table.insertColumnLeft", "Insert Left", "Table Layout › Rows & Columns", |s, _| col(s, false))),
        t(CommandSpec::new("table.insertColumnRight", "Insert Right", "Table Layout › Rows & Columns", |s, _| col(s, true))),
        t(CommandSpec::new("table.deleteRow", "Delete Rows", "Table Layout › Rows & Columns › Delete", del_row)),
        t(CommandSpec::new("table.deleteColumn", "Delete Columns", "Table Layout › Rows & Columns › Delete", del_col)),
        t(CommandSpec::new("table.deleteTable", "Delete Table", "Table Layout › Rows & Columns › Delete", del_table)),
        t(CommandSpec::new("table.deleteCells", "Delete Cells", "Table Layout › Rows & Columns › Delete", delete_cells)
            .params(r#"{"shift"?: "left|up"}"#)),
        t(CommandSpec::new("table.merge", "Merge Cells", "Table Layout › Merge", merge)),
        t(CommandSpec::new("table.split", "Split Cells", "Table Layout › Merge", split_cells)
            .params(r#"{"columns"?: n, "rows"?: n, "mergeFirst"?: bool}"#)),
        t(CommandSpec::new("table.splitTable", "Split Table", "Table Layout › Merge", split_table)),
        t(CommandSpec::new("table.style", "Table Styles", "Table Design › Table Styles", |s, v| {
            let st = p::req_str(v, "style")?;
            let id = super::table_style::table_style_id(s, st).ok_or_else(|| CmdError::Params(format!("no table style `{st}`")))?;
            with_table(s, |t| t.props.style = Some(id.clone()))
        })
        .params(r#"{"style": string}"#)),
        t(CommandSpec::new("table.look", "Table Style Options", "Table Design › Table Style Options", |s, v| {
            let set = |k: &str, cur: bool| p::bool(v, k).unwrap_or(cur);
            with_table(s, |t| {
                let l = &mut t.props.look;
                l.header_row = set("headerRow", l.header_row);
                l.total_row = set("totalRow", l.total_row);
                l.banded_rows = set("bandedRows", l.banded_rows);
                l.first_column = set("firstColumn", l.first_column);
                l.last_column = set("lastColumn", l.last_column);
                l.banded_columns = set("bandedColumns", l.banded_columns);
            })
        })
        .params(
            r#"{"headerRow"?: bool, "totalRow"?: bool, "bandedRows"?: bool, "firstColumn"?: bool, "lastColumn"?: bool, "bandedColumns"?: bool}"#,
        )),
        t(CommandSpec::new("table.shading", "Shading", "Table Design › Table Styles", |s, v| {
            let c = match p::str(v, "color") {
                None | Some("none") => None,
                Some(hex) => Some(Rgb::parse(hex).ok_or_else(|| CmdError::Params(format!("bad colour `{hex}`")))?),
            };
            with_cells(s, |cl| cl.props.shading = c)
        })
        .params(r#"{"color": "RRGGBB" | null}"#)),
        t(CommandSpec::new("table.borders", "Borders", "Table Design › Borders", borders)
            .params(r#"{"kind": "all|outside|inside|none|top|bottom|left|right", "style"?: "single|double|dotted|dashed|thick|triple|dotDash|wave", "width"?: pt, "color"?: "RRGGBB|auto"}"#)),
        t(CommandSpec::new("table.cellAlign", "Alignment", "Table Layout › Alignment", |s, v| {
            let (h, va): (Option<Align>, VAlign) = match p::str(v, "value").unwrap_or("topLeft") {
                "topLeft" => (Some(Align::Left), VAlign::Top),
                "topCenter" => (Some(Align::Center), VAlign::Top),
                "topRight" => (Some(Align::Right), VAlign::Top),
                "centerLeft" => (Some(Align::Left), VAlign::Center),
                "center" => (Some(Align::Center), VAlign::Center),
                "centerRight" => (Some(Align::Right), VAlign::Center),
                "bottomLeft" => (Some(Align::Left), VAlign::Bottom),
                "bottomCenter" => (Some(Align::Center), VAlign::Bottom),
                "bottomRight" => (Some(Align::Right), VAlign::Bottom),
                "top" => (None, VAlign::Top),
                "vcenter" => (None, VAlign::Center),
                "bottom" => (None, VAlign::Bottom),
                x => return Err(CmdError::Params(format!("unknown alignment `{x}`"))),
            };
            with_cells(s, |cl| {
                cl.props.valign = va;
                if let Some(h) = h {
                    for b in &mut cl.blocks {
                        if let Block::Para(p) = std::sync::Arc::make_mut(b) {
                            p.props.align = Some(h);
                            p.touch();
                        }
                    }
                }
            })
        })
        .params(r#"{"value": "topLeft|topCenter|…|bottomRight|top|vcenter|bottom"}"#)),
        t(CommandSpec::new("table.textDirection", "Text Direction", "Table Layout › Alignment", text_direction)
            .params(r#"{"value"?: "horizontal|down|up"} (default: the next direction after the caret cell's, like Word's button)"#)),
        t(CommandSpec::new("table.autofit", "AutoFit", "Table Layout › Cell Size", autofit).params(r#"{"mode": "contents|window|fixed"}"#)),
        t(CommandSpec::new("table.distributeColumns", "Distribute Columns", "Table Layout › Cell Size", distribute_selected_columns)),
        t(CommandSpec::new("table.distributeRows", "Distribute Rows", "Table Layout › Cell Size", distribute_selected_rows)),
        CommandSpec::new("table.columnWidth", "Table Column Width", "Table Layout › Cell Size", |s, v| {
            let w = p::req_f32(v, "width")?.clamp(6.0, 1584.0);
            let adjust_next = p::bool(v, "adjustNext").unwrap_or(false);
            let story = s.sel.focus.story;
            let (tp, r, c) = if let Some(p_str) = p::str(v, "path") {
                let path = parse_path(p_str)?;
                let row_idx = p::u64(v, "row").unwrap_or(0) as usize;
                let col_idx = p::u64(v, "col").unwrap_or(0) as usize;
                (path, row_idx, col_idx)
            } else {
                cell(s)?
            };
            let t = s.doc.table_mut(story, &tp)?;
            set_column_width(t, r, c, w, adjust_next);
            touch_cells(s, &tp)?;
            sel_result(s)
        })
        .params(r#"{"width": pt, "adjustNext"?: bool, "path"?: "0.1", "row"?: n, "col"?: n}"#),
        CommandSpec::new("table.rowHeight", "Table Row Height", "Table Layout › Cell Size", |s, v| {
            let h = p::req_f32(v, "height")?.clamp(1.0, 1584.0);
            let rule = match p::str(v, "rule") {
                None | Some("atLeast") => HeightRule::AtLeast,
                Some("exact") => HeightRule::Exact,
                Some(x) => return Err(CmdError::Params(format!("unknown height rule `{x}`"))),
            };
            let story = s.sel.focus.story;
            let (tp, r, _) = if let Some(p_str) = p::str(v, "path") {
                let path = parse_path(p_str)?;
                let row_idx = p::u64(v, "row").unwrap_or(0) as usize;
                (path, row_idx, 0)
            } else {
                cell(s)?
            };
            let t = s.doc.table_mut(story, &tp)?;
            if let Some(row) = t.rows.get_mut(r) {
                row.props.height = Some(h);
                row.props.height_rule = rule;
            }
            touch_cells(s, &tp)?;
            sel_result(s)
        })
        .params(r#"{"height": pt, "rule"?: "atLeast|exact", "path"?: "0.1", "row"?: n}"#),
        CommandSpec::new("table.resizeTable", "Resize Whole Table", "Table Layout › Table", resize_table)
            .params(r#"{"path"?: "0.1", "width"?: pt, "height"?: pt}"#),
        t(CommandSpec::new("table.repeatHeader", "Repeat Header Rows", "Table Layout › Data", |s, v| {
            let (_, r, _) = cell(s)?;
            let on = p::bool(v, "value");
            with_table(s, |t| {
                let current_header_count = t.rows.iter().take_while(|row| row.props.header).count();
                let on_val = on.unwrap_or(r >= current_header_count);
                if on_val {
                    for row in t.rows.iter_mut().take(r + 1) {
                        row.props.header = true;
                    }
                } else {
                    for row in t.rows.iter_mut().skip(r) {
                        row.props.header = false;
                    }
                }
            })
        })),
        t(CommandSpec::new("table.sort", "Sort", "Table Layout › Data", sort_table)
            .params(r#"{"column"?: n (grid column, 0 = first), "descending"?: bool, "header"?: bool, "type"?: "number|text"}"#)),
        t(CommandSpec::new("table.toText", "Convert to Text", "Table Layout › Data", to_text).params(r#"{"separator"?: "tab|comma|paragraph|other", "char"?: "x"}"#)),
        t(CommandSpec::new("table.formula", "Formula", "Table Layout › Data", formula).params(r#"{"formula"?: "=SUM(ABOVE)"}"#)),
        CommandSpec::new("table.selectTable", "Select Table", "Table Layout › Table › Select", |s, v| {
            let tp = if let Some(p_str) = p::str(v, "path") {
                parse_path(p_str)?
            } else {
                cell(s)?.0
            };
            select_cells(s, &tp, None)
        })
        .params(r#"{"path"?: "0.1"}"#)
        .pure(),
        CommandSpec::new("table.selectRow", "Select Row", "Table Layout › Table › Select", |s, v| {
            let (tp, r) = if let Some(p_str) = p::str(v, "path") {
                let path = parse_path(p_str)?;
                let row_idx = p::u64(v, "row").unwrap_or(0) as usize;
                (path, row_idx)
            } else {
                let (path, row_idx, _) = cell(s)?;
                (path, row_idx)
            };
            select_cells(s, &tp, Some((r, None)))
        })
        .params(r#"{"path"?: "0.1", "row"?: n}"#)
        .pure(),
        CommandSpec::new("table.selectColumn", "Select Column", "Table Layout › Table › Select", |s, v| {
            let (tp, g) = if let Some(p_str) = p::str(v, "path") {
                let path = parse_path(p_str)?;
                let col_idx = p::u64(v, "col").unwrap_or(0) as usize;
                (path, col_idx)
            } else {
                let (path, row_idx, col_idx) = cell(s)?;
                let story = s.sel.focus.story;
                let g = s.doc.table(story, &path).map(|t| t.grid_col(row_idx, col_idx)).unwrap_or(col_idx);
                (path, g)
            };
            select_column_cells(s, &tp, g)
        })
        .params(r#"{"path"?: "0.1", "col"?: n}"#)
        .pure(),
        t(CommandSpec::new("table.selectCell", "Select Cell", "Table Layout › Table › Select", |s, _| {
            let (tp, r, c) = cell(s)?;
            select_cells(s, &tp, Some((r, Some(c))))
        })
        .pure()),
        CommandSpec::new("table.selectRange", "Select Cell Range", "Table Layout › Table › Select", |s, v| {
            let (tp, r0, c0) = if let Some(p_str) = p::str(v, "path") {
                let path = parse_path(p_str)?;
                let r = p::u64(v, "r0").unwrap_or(0) as usize;
                let c = p::u64(v, "c0").unwrap_or(0) as usize;
                (path, r, c)
            } else {
                let (path, r, c) = cell(s)?;
                (path, r, c)
            };
            let r1 = p::u64(v, "r1").unwrap_or(r0 as u64) as usize;
            let c1 = p::u64(v, "c1").unwrap_or(c0 as u64) as usize;
            select_cell_range(s, &tp, r0, c0, r1, c1)
        })
        .params(r#"{"path"?: "0.1", "r0"?: n, "c0"?: n, "r1"?: n, "c1"?: n}"#)
        .pure(),
        t(CommandSpec::new("table.properties", "Properties", "Table Layout › Table", properties).params(
            r#"{"align"?: "left|center|right", "width"?: pt|null, "widthPct"?: percent, "indent"?: pt, "cellSpacing"?: pt|null, "altText"?: string, "rowHeight"?: pt|null, "rowHeightRule"?: "atLeast|exact", "allowBreak"?: bool, "headerRow"?: bool, "columnWidth"?: pt, "cellWidth"?: pt|null, "valign"?: "top|center|bottom"} (row, column and cell settings apply to the caret's; without settings it returns the current ones)"#,
        )),
        CommandSpec::new("table.position", "Table Positioning", "Table Layout › Table", position)
            .params(r#"{"floating"?: bool, "hRel"?: "column|margin|page", "hAlign"?: "left|center|right", "x"?: pt, "vRel"?: "paragraph|margin|page", "vAlign"?: "top|center|bottom", "y"?: pt, "dist"?: [left, top, right, bottom] pt, "overlap"?: bool} (floating: false puts the table back in the text)"#),
        CommandSpec::new("table.fromText", "Convert Text to Table", "Insert › Tables", from_text).params(r#"{"separator"?: "tab|comma|paragraph|other", "char"?: "x"}"#),
        CommandSpec::new("table.quick", "Quick Tables", "Insert › Tables", quick_table).params(r#"{"kind"?: "calendar|tabular|matrix"}"#),
    ]
}

/// Table Layout › AutoFit: Contents sizes the columns to their text, Window spreads them across
/// the text column, Fixed keeps the widths as they are.
fn autofit(s: &mut Session, v: &Value) -> CmdResult {
    let mode = p::str(v, "mode").unwrap_or("window");
    let (tp, _, _) = cell(s)?;
    let story = s.sel.focus.story;
    let tw = super::page::sect(s).text_width();
    match mode {
        "fixed" => with_table(s, |t| t.props.fixed = true),
        "contents" => {
            let t = s.doc.table(story, &tp).ok_or_else(|| CmdError::Disabled("the cursor isn't in a table".into()))?;
            let avail = (tw - t.props.indent.filter(|i| i.is_finite()).unwrap_or(0.0).max(0.0)).max(72.0);
            let measured = wordcraft_layout::measure_table_columns(&s.doc, t)
                .ok_or_else(|| CmdError::Failed("the table is too large to fit to its contents".into()))?;
            let widths = wordcraft_layout::autofit_widths(&measured, avail);
            with_table(s, |t| {
                t.props.fixed = false;
                t.props.width = None;
                t.props.width_pct = None;
                t.grid = widths.iter().map(|w| w.max(6.0)).collect();
                sync_cell_widths(t);
            })
        }
        _ => with_table(s, |t| {
            // The columns keep their proportions and fill the text column.
            t.props.fixed = false;
            t.props.width_pct = Some(100.0);
            scale_grid(t, tw);
        }),
    }
}

/// Give every cell the width of the grid columns it spans, so the cells' preferred widths
/// (saved to .docx) agree with the grid.
pub(crate) fn sync_cell_widths(t: &mut Table) {
    let grid = t.grid.clone();
    for row in &mut t.rows {
        let mut g = 0usize;
        for c in &mut row.cells {
            let span = c.span();
            let w: f32 = grid.get(g..g.saturating_add(span).min(grid.len())).unwrap_or(&[]).iter().sum();
            if w > 0.0 {
                c.props.width = Some(w);
            }
            g += span;
        }
    }
}

/// Set the width of the grid column cell `c` of row `r` starts in. The table's preferred width
/// goes, so the layout doesn't scale the column back.
fn set_column_width(t: &mut Table, r: usize, c: usize, w: f32, adjust_next: bool) {
    let g = t.grid_col(r, c);
    let span = t.rows.get(r).and_then(|row| row.cells.get(c)).map(|cell| cell.span()).unwrap_or(1);
    let boundary_g = g.saturating_add(span).saturating_sub(1);
    if let Some(x) = t.grid.get_mut(boundary_g) {
        let old_w = *x;
        let delta = w - old_w;
        *x = w;
        if adjust_next && boundary_g + 1 < t.grid.len() {
            if let Some(next_x) = t.grid.get_mut(boundary_g + 1) {
                *next_x = (*next_x - delta).max(6.0);
            }
        } else {
            t.props.width = None;
            t.props.width_pct = None;
        }
        sync_cell_widths(t);
    }
}

/// Scales the grid so its columns total `target` points (a preferred table width), then gives every
/// cell the width of its columns.
fn scale_grid(t: &mut Table, target: f32) {
    let total: f32 = t.grid.iter().sum();
    let n = t.grid.len().max(1) as f32;
    for g in &mut t.grid {
        *g = if total > 0.0 { (*g * target / total).max(6.0) } else { (target / n).max(6.0) };
    }
    sync_cell_widths(t);
}

/// Gives the columns cell (`r`, `c`) spans the preferred width `w`: they keep their proportions.
fn set_span_width(t: &mut Table, r: usize, c: usize, w: f32) {
    let g = t.grid_col(r, c);
    let span = t.rows.get(r).and_then(|row| row.cells.get(c)).map_or(1, wordcraft_doc::Cell::span);
    let end = (g + span).min(t.grid.len());
    if g >= end {
        return;
    }
    let current: f32 = t.grid.get(g..end).unwrap_or(&[]).iter().sum();
    let k = if current > 0.0 { w / current } else { w / (end - g) as f32 };
    for x in t.grid.get_mut(g..end).unwrap_or(&mut []) {
        *x = (if current > 0.0 { *x * k } else { k }).max(6.0);
    }
    sync_cell_widths(t);
}

/// Table Layout › Table › Positioning: makes the table float at a position (or puts it back in the
/// text when `floating` is false). Text wraps around a floating table.
fn position(s: &mut Session, v: &Value) -> CmdResult {
    use wordcraft_doc::para::{Anchor, FloatAlign};
    use wordcraft_doc::props::TableFloat;
    let (tp, _, _) = cell(s)?;
    let story = s.sel.focus.story;
    let t = s.doc.table_mut(story, &tp)?;
    if !p::bool(v, "floating").unwrap_or(true) {
        t.props.float = None;
    } else {
        let h_rel = match p::str(v, "hRel").unwrap_or("column") {
            "column" => Anchor::Column,
            "margin" => Anchor::Margin,
            "page" => Anchor::Page,
            x => return Err(CmdError::Params(format!("unknown horizontal relation `{x}`"))),
        };
        let v_rel = match p::str(v, "vRel").unwrap_or("paragraph") {
            "paragraph" => Anchor::Paragraph,
            "margin" => Anchor::Margin,
            "page" => Anchor::Page,
            x => return Err(CmdError::Params(format!("unknown vertical relation `{x}`"))),
        };
        let h_align = match p::str(v, "hAlign") {
            None => None,
            Some("left") => Some(FloatAlign::Start),
            Some("center") => Some(FloatAlign::Center),
            Some("right") => Some(FloatAlign::End),
            Some(x) => return Err(CmdError::Params(format!("unknown horizontal alignment `{x}`"))),
        };
        let v_align = match p::str(v, "vAlign") {
            None => None,
            Some("top") => Some(FloatAlign::Start),
            Some("center") => Some(FloatAlign::Center),
            Some("bottom") => Some(FloatAlign::End),
            Some(x) => return Err(CmdError::Params(format!("unknown vertical alignment `{x}`"))),
        };
        let current = t.props.float.unwrap_or_default();
        let mut dist = current.dist;
        if let Some(arr) = v.get("dist").and_then(Value::as_array) {
            for (slot, x) in dist.iter_mut().zip(arr) {
                *slot = x.as_f64().map_or(*slot, |f| f as f32).clamp(0.0, 1584.0);
            }
        }
        t.props.float = Some(TableFloat {
            h_rel,
            v_rel,
            x: p::f32(v, "x").unwrap_or(0.0).clamp(-1584.0, 1584.0),
            y: p::f32(v, "y").unwrap_or(0.0).clamp(-1584.0, 1584.0),
            h_align,
            v_align,
            dist,
            overlap: p::bool(v, "overlap").unwrap_or(current.overlap),
        });
    }
    super::table::touch_cells(s, &tp)?;
    sel_result(s)
}

/// Table Properties: table, row, column and cell settings in one undoable step. Row, column and
/// cell settings apply to the caret's. Without any setting it returns the current values.
fn properties(s: &mut Session, v: &Value) -> CmdResult {
    let (tp, r, c) = cell(s)?;
    let keys = [
        "align",
        "width",
        "widthPct",
        "indent",
        "cellSpacing",
        "altText",
        "rowHeight",
        "rowHeightRule",
        "allowBreak",
        "headerRow",
        "columnWidth",
        "cellWidth",
        "valign",
    ];
    if !keys.iter().any(|k| v.get(*k).is_some()) {
        let t = s.doc.table(s.sel.focus.story, &tp).cloned().unwrap_or_default();
        let mut out = serde_json::to_value(&t.props).unwrap_or(Value::Null);
        let row = t.rows.get(r);
        let cl = row.and_then(|x| x.cells.get(c));
        if let Some(o) = out.as_object_mut() {
            o.insert("row".into(), row.map(|x| serde_json::to_value(&x.props).unwrap_or(Value::Null)).unwrap_or(Value::Null));
            o.insert("columnWidth".into(), t.grid.get(t.grid_col(r, c)).map_or(Value::Null, |w| json!(w)));
            o.insert("cell".into(), cl.map(|x| serde_json::to_value(&x.props).unwrap_or(Value::Null)).unwrap_or(Value::Null));
        }
        return Ok(out);
    }
    let align = match p::str(v, "align") {
        None => None,
        Some("left") => Some(Align::Left),
        Some("center") => Some(Align::Center),
        Some("right") => Some(Align::Right),
        Some(x) => return Err(CmdError::Params(format!("unknown table alignment `{x}`"))),
    };
    let rule = match p::str(v, "rowHeightRule") {
        None => None,
        Some("atLeast") => Some(HeightRule::AtLeast),
        Some("exact") => Some(HeightRule::Exact),
        Some(x) => return Err(CmdError::Params(format!("unknown height rule `{x}`"))),
    };
    let valign = match p::str(v, "valign") {
        None => None,
        Some("top") => Some(VAlign::Top),
        Some("center") => Some(VAlign::Center),
        Some("bottom") => Some(VAlign::Bottom),
        Some(x) => return Err(CmdError::Params(format!("unknown vertical alignment `{x}`"))),
    };
    // `null` clears a preferred size (automatic); a number sets it.
    let size = |k: &str, lo: f32, hi: f32| -> Option<Option<f32>> {
        match v.get(k)? {
            Value::Null => Some(None),
            x => x.as_f64().map(|f| f as f32).filter(|f| f.is_finite()).map(|f| Some(f.clamp(lo, hi))),
        }
    };
    let width = size("width", 6.0, 1584.0);
    let cell_spacing = size("cellSpacing", 0.0, 100.0);
    // The alternative text a screen reader gives for the table; empty clears it.
    let alt_text = v.get("altText").and_then(Value::as_str).map(|a| a.trim().to_string());
    let width_pct = p::f32(v, "widthPct").map(|x| x.clamp(1.0, 100.0));
    let indent = p::f32(v, "indent").map(|x| x.clamp(-1584.0, 1584.0));
    let row_height = size("rowHeight", 1.0, 1584.0);
    let allow_break = p::bool(v, "allowBreak");
    let header = p::bool(v, "headerRow");
    let column_width = p::f32(v, "columnWidth").map(|x| x.clamp(6.0, 1584.0));
    let cell_width = size("cellWidth", 6.0, 1584.0);
    let text_width = super::page::sect(s).text_width();
    with_table(s, |t| {
        if let Some(a) = align {
            t.props.align = Some(a);
        }
        if let Some(i) = indent {
            t.props.indent = Some(i);
        }
        if let Some(sp) = cell_spacing {
            t.props.cell_spacing = sp;
        }
        if let Some(a) = &alt_text {
            t.props.caption = (!a.is_empty()).then(|| a.clone());
        }
        if let Some(cw) = column_width {
            set_column_width(t, r, c, cw, false);
        }
        if let Some(w) = width {
            t.props.width = w;
            t.props.width_pct = None;
            // A preferred width is the grid's total: the columns scale to it.
            if let Some(w) = w {
                scale_grid(t, w);
            }
        }
        if let Some(pct) = width_pct {
            t.props.width_pct = Some(pct);
            scale_grid(t, text_width * pct / 100.0);
        }
        if let Some(row) = t.rows.get_mut(r) {
            if let Some(h) = row_height {
                row.props.height = h;
                row.props.height_rule = if h.is_some() { rule.unwrap_or(HeightRule::AtLeast) } else { HeightRule::Auto };
            } else if let Some(rule) = rule
                && row.props.height.is_some()
            {
                row.props.height_rule = rule;
            }
            if let Some(b) = allow_break {
                row.props.cant_split = !b;
            }
            if let Some(h) = header {
                row.props.header = h;
            }
            if let Some(cl) = row.cells.get_mut(c)
                && let Some(va) = valign
            {
                cl.props.valign = va;
            }
        }
        // The cell's preferred width is the width of the columns it spans (the grid is what is
        // drawn), so it changes the columns, not a number the layout never reads.
        if let Some(Some(w)) = cell_width {
            set_span_width(t, r, c, w);
        }
    })
}

fn cell(s: &Session) -> Result<(Path, usize, usize), CmdError> {
    s.sel.focus.path.cell().ok_or_else(|| CmdError::Disabled("the cursor isn't in a table".into()))
}

fn with_table(s: &mut Session, f: impl Fn(&mut Table)) -> CmdResult {
    let (tp, _, _) = cell(s)?;
    let t = s.doc.table_mut(s.sel.focus.story, &tp)?;
    f(t);
    touch_cells(s, &tp)?;
    sel_result(s)
}

/// Selected cells: (row, cell) pairs covered by the selection (at least the caret's cell).
fn selected_cells(s: &Session) -> Vec<(usize, usize)> {
    if let Some(col_block) = &s.column {
        let mut v = Vec::new();
        for (a, _) in &col_block.segments {
            if let Some((_, r, c)) = a.path.cell() {
                v.push((r, c));
            }
        }
        if !v.is_empty() {
            return v;
        }
    }
    let (a, b) = s.sel.ordered();
    let (Some((ta, ra, ca)), Some((tb, rb, cb))) = (a.path.cell(), b.path.cell()) else { return Vec::new() };
    if ta != tb {
        return vec![(rb, cb)];
    }
    let (r0, r1) = (ra.min(rb), ra.max(rb));
    if let Some(t) = s.doc.table(s.sel.focus.story, &ta) {
        let span_a = t.rows.get(ra).and_then(|r| r.cells.get(ca)).map(|c| c.span()).unwrap_or(1);
        let span_b = t.rows.get(rb).and_then(|r| r.cells.get(cb)).map(|c| c.span()).unwrap_or(1);
        let ga = t.grid_col(ra, ca);
        let gb = t.grid_col(rb, cb);
        let g0 = ga.min(gb);
        let g1 = (ga + span_a - 1).max(gb + span_b - 1);
        let mut v = Vec::new();
        for r in r0..=r1 {
            let mut seen_in_row = std::collections::BTreeSet::new();
            for g in g0..=g1 {
                if let Some(c) = t.cell_at_grid(r, g) {
                    seen_in_row.insert(c);
                }
            }
            for c in seen_in_row {
                v.push((r, c));
            }
        }
        if !v.is_empty() {
            return v;
        }
    }
    let (c0, c1) = (ca.min(cb), ca.max(cb));
    let mut v = Vec::new();
    for r in r0..=r1 {
        for c in c0..=c1 {
            v.push((r, c));
        }
    }
    v
}

fn with_cells(s: &mut Session, f: impl Fn(&mut wordcraft_doc::Cell)) -> CmdResult {
    let (tp, _, _) = cell(s)?;
    let cells = selected_cells(s);
    let t = s.doc.table_mut(s.sel.focus.story, &tp)?;
    for (r, c) in cells {
        if let Some(cl) = t.rows.get_mut(r).and_then(|row| row.cells.get_mut(c)) {
            f(cl);
        }
    }
    touch_cells(s, &tp)?;
    sel_result(s)
}

/// Table Layout › Text Direction: turn the selected cells' text. Without a value it cycles the
/// caret cell's direction (horizontal → down → up) and gives every selected cell the result.
fn text_direction(s: &mut Session, v: &Value) -> CmdResult {
    let (tp, r, c) = cell(s)?;
    let dir = match p::str(v, "value") {
        Some("horizontal" | "lrTb") => TextDirection::Horizontal,
        Some("down" | "tbRl") => TextDirection::Down,
        Some("up" | "btLr") => TextDirection::Up,
        Some(x) => return Err(CmdError::Params(format!("unknown text direction `{x}`"))),
        None => {
            let t = s.doc.table(s.sel.focus.story, &tp).ok_or_else(|| CmdError::Disabled("the cursor isn't in a table".into()))?;
            t.rows.get(r).and_then(|row| row.cells.get(c)).map(|cl| cl.props.text_direction).unwrap_or_default().next()
        }
    };
    with_cells(s, |cl| cl.props.text_direction = dir)
}

/// Bump the revision of every paragraph in a table (table style changes alter their layout).
pub(crate) fn touch_cells(s: &mut Session, tp: &Path) -> Result<(), CmdError> {
    let story = s.sel.focus.story;
    let paths: Vec<Path> = s.doc.para_paths(story).into_iter().filter(|p| p.0.len() > tp.0.len() && p.0.starts_with(&tp.0)).collect();
    for p in paths {
        s.doc.para_mut(story, &p)?.touch();
    }
    Ok(())
}

fn fix_caret(s: &mut Session, tp: &Path, r: usize, c: usize) -> CmdResult {
    let story = s.sel.focus.story;
    let Some(t) = s.doc.table(story, tp) else {
        s.sel = Selection::caret(s.doc.clamp(&Pos { story, path: tp.clone(), off: 0 }));
        return sel_result(s);
    };
    let r = r.min(t.rows.len().saturating_sub(1));
    let c = c.min(t.rows.get(r).map(|x| x.cells.len().saturating_sub(1)).unwrap_or(0));
    let mut path = tp.0.clone();
    path.extend([r as u32, c as u32, 0]);
    s.sel = Selection::caret(Pos { story, path: Path(path), off: 0 });
    sel_result(s)
}

/// Insert Above / Below: one new row per selected row, above the first selected row or below
/// the last one (Word's rule, whichever cell the selection's focus is in).
fn row(s: &mut Session, below: bool) -> CmdResult {
    let (tp, r, c) = cell(s)?;
    let rows: std::collections::BTreeSet<usize> = selected_cells(s).into_iter().map(|x| x.0).collect();
    let first = rows.first().copied().unwrap_or(r);
    let last = rows.last().copied().unwrap_or(r);
    let n = rows.len().max(1);
    let (at, like) = if below { (last + 1, last) } else { (first, first) };
    let story = s.sel.focus.story;
    let t = s.doc.table_mut(story, &tp)?;
    refuse_rows_in_merged_table(t)?;
    if t.rows.len() + n > wordcraft_doc::table::MAX_ROWS {
        return Err(CmdError::Failed(format!("a table can't have more than {} rows", wordcraft_doc::table::MAX_ROWS)));
    }
    // A row inserted above a header row is a header too (Word keeps the heading on both rows).
    let header = !below && t.rows.get(first).is_some_and(|x| x.props.header);
    for _ in 0..n {
        t.insert_row(at, like);
        if header && let Some(x) = t.rows.get_mut(at) {
            x.props.header = true;
        }
    }
    fix_caret(s, &tp, at, c)
}

/// Word refuses to insert or delete rows in a table with vertically merged cells ("Cannot access
/// individual rows in this collection because the table has vertically merged cells").
fn refuse_rows_in_merged_table(t: &Table) -> Result<(), CmdError> {
    if t.rows.iter().any(|row| row.cells.iter().any(|c| c.props.vmerge != VMerge::None)) {
        return Err(CmdError::Failed("rows can't be inserted or deleted in a table with vertically merged cells; unmerge them first".into()));
    }
    Ok(())
}

/// Insert Left / Right: one new column per selected grid column, beside the selection's outer
/// edge. The table keeps its total width, so the other columns give up the room.
fn col(s: &mut Session, right: bool) -> CmdResult {
    let (tp, r, c) = cell(s)?;
    let story = s.sel.focus.story;
    let spots = selected_cells(s);
    let spots = if spots.is_empty() { vec![(r, c)] } else { spots };
    let t = s.doc.table_mut(story, &tp)?;
    let (lo, hi) = spots.iter().fold((usize::MAX, 0usize), |(lo, hi), &(ri, ci)| {
        let g = t.grid_col(ri, ci);
        let span = t.rows.get(ri).and_then(|x| x.cells.get(ci)).map(|x| x.span()).unwrap_or(1);
        (lo.min(g), hi.max(g + span))
    });
    let (lo, hi) = if lo == usize::MAX { (0, 0) } else { (lo, hi) };
    let n = hi.saturating_sub(lo).max(1);
    if t.grid.len() + n > wordcraft_doc::table::MAX_COLS {
        return Err(CmdError::Failed(format!("a table can't have more than {} columns", wordcraft_doc::table::MAX_COLS)));
    }
    let at = if right { hi } else { lo };
    let before: f32 = t.grid.iter().sum();
    for _ in 0..n {
        t.insert_col(at);
    }
    // Keep the table within its width: the columns share the room the new ones take.
    let after: f32 = t.grid.iter().sum();
    if before > 0.0 && after > 0.0 {
        let k = before / after;
        t.grid.iter_mut().for_each(|w| *w *= k);
    }
    sync_cell_widths(t);
    fix_caret(s, &tp, r, if right { c + 1 } else { c })
}

fn del_row(s: &mut Session, _: &Value) -> CmdResult {
    let (tp, r, _) = cell(s)?;
    let rows: std::collections::BTreeSet<usize> = selected_cells(s).into_iter().map(|x| x.0).collect();
    let story = s.sel.focus.story;
    let t = s.doc.table_mut(story, &tp)?;
    if rows.len() >= t.rows.len() {
        return del_table(s, &Value::Null);
    }
    refuse_rows_in_merged_table(t)?;
    for rr in rows.iter().rev() {
        t.delete_row(*rr);
    }
    fix_caret(s, &tp, r, 0)
}

fn del_col(s: &mut Session, _: &Value) -> CmdResult {
    let (tp, r, c) = cell(s)?;
    let story = s.sel.focus.story;
    let cells = selected_cells(s);
    let t = s.doc.table_mut(story, &tp)?;
    if t.grid.len() <= 1 {
        return del_table(s, &Value::Null);
    }
    let mut gc_set = std::collections::BTreeSet::new();
    for (ri, ci) in &cells {
        gc_set.insert(t.grid_col(*ri, *ci));
    }
    let mut grid_cols: Vec<usize> = gc_set.into_iter().collect();
    grid_cols.sort_unstable_by(|a, b| b.cmp(a));
    if grid_cols.is_empty() {
        let g = t.grid_col(r, c);
        grid_cols.push(g);
    }
    if grid_cols.len() >= t.grid.len() {
        return del_table(s, &Value::Null);
    }
    for g in grid_cols {
        if t.grid.len() > 1 {
            t.delete_col(g);
        }
    }
    // Rows left without cells are dropped; if that was every row, no table is left.
    if t.rows.is_empty() {
        return del_table(s, &Value::Null);
    }
    fix_caret(s, &tp, r, c.saturating_sub(1))
}

fn del_table(s: &mut Session, _: &Value) -> CmdResult {
    let (tp, _, _) = cell(s)?;
    let story = s.sel.focus.story;
    s.doc.remove_block(story, &tp)?;
    s.sel = Selection::caret(s.doc.clamp(&Pos { story, path: tp, off: 0 }));
    sel_result(s)
}

fn merge(s: &mut Session, _: &Value) -> CmdResult {
    let (tp, _, _) = cell(s)?;
    let cells = selected_cells(s);
    if cells.len() < 2 {
        return Err(CmdError::Failed("select two or more cells to merge".into()));
    }
    let story = s.sel.focus.story;
    let (r0, r1) = (cells.iter().map(|x| x.0).min().unwrap_or(0), cells.iter().map(|x| x.0).max().unwrap_or(0));
    let t = s.doc.table_mut(story, &tp)?;
    let c0 = cells.iter().map(|x| x.1).min().unwrap_or(0);
    let c1 = cells.iter().map(|x| x.1).max().unwrap_or(0);
    let g0 = t.grid_col(r0, c0);
    let g1 = t.grid_col(r0, c1) + t.rows.get(r0).and_then(|r| r.cells.get(c1)).map(|c| c.span()).unwrap_or(1) - 1;
    t.merge(r0, r1, g0, g1);
    fix_caret(s, &tp, r0, c0)
}

fn split_table(s: &mut Session, _: &Value) -> CmdResult {
    let (tp, r, _) = cell(s)?;
    if r == 0 {
        return Err(CmdError::Failed("can't split at the first row".into()));
    }
    let story = s.sel.focus.story;
    let t = s.doc.table_mut(story, &tp)?;
    let tail_rows = if r <= t.rows.len() { t.rows.split_off(r) } else { vec![] };
    let tail = Table { props: t.props.clone(), grid: t.grid.clone(), rows: tail_rows };
    let after = tp.with_last(tp.last() + 1);
    s.doc.insert_block(story, &after, Block::Para(Paragraph::new()))?;
    s.doc.insert_block(story, &after.with_last(after.last() + 1), Block::Table(tail))?;
    s.sel = Selection::caret(Pos { story, path: after, off: 0 });
    sel_result(s)
}

/// The border a pen describes: `style` (a line style), `width` in points and `color` (`RRGGBB`,
/// or `auto` / nothing for the automatic colour). Table Design › Borders uses it, and so does the
/// Border Painter.
pub(crate) fn pen_border(v: &Value) -> Result<Border, CmdError> {
    let style = match p::str(v, "style").unwrap_or("single") {
        "single" => BorderStyle::Single,
        "double" => BorderStyle::Double,
        "dotted" => BorderStyle::Dotted,
        "dashed" => BorderStyle::Dashed,
        "thick" => BorderStyle::Thick,
        "triple" => BorderStyle::Triple,
        "dotDash" => BorderStyle::DotDash,
        "wave" => BorderStyle::Wave,
        x => return Err(CmdError::Params(format!("unknown line style `{x}`"))),
    };
    let width = p::f32(v, "width").unwrap_or(0.5).clamp(0.25, 6.0);
    let color = match p::str(v, "color") {
        None | Some("auto") => None,
        Some(hex) => Some(Rgb::parse(hex).ok_or_else(|| CmdError::Params(format!("bad colour `{hex}`")))?),
    };
    Ok(Border { style, width, color, space: 0.0 })
}

/// One side of a cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

const SIDES: [Side; 4] = [Side::Top, Side::Bottom, Side::Left, Side::Right];

fn opposite(side: Side) -> Side {
    match side {
        Side::Top => Side::Bottom,
        Side::Bottom => Side::Top,
        Side::Left => Side::Right,
        Side::Right => Side::Left,
    }
}

/// The cell across `side` from cell (`r`, `ci`): the cell covering the same grid column in the
/// row above or below, or the neighbour in the row. `None` at the table's edge.
fn neighbour(t: &Table, r: usize, ci: usize, side: Side) -> Option<(usize, usize)> {
    match side {
        Side::Top => {
            let nr = r.checked_sub(1)?;
            Some((nr, t.cell_at_grid(nr, t.grid_col(r, ci))?))
        }
        Side::Bottom => {
            let nr = r + 1;
            if nr >= t.rows.len() {
                return None;
            }
            Some((nr, t.cell_at_grid(nr, t.grid_col(r, ci))?))
        }
        Side::Left => Some((r, ci.checked_sub(1)?)),
        Side::Right => {
            let nci = ci + 1;
            (nci < t.rows.get(r)?.cells.len()).then_some((r, nci))
        }
    }
}

fn set_cell_side(t: &mut Table, r: usize, ci: usize, side: Side, b: Border) {
    if let Some(cl) = t.rows.get_mut(r).and_then(|row| row.cells.get_mut(ci)) {
        let borders = cl.props.borders.get_or_insert_with(Borders::default);
        let slot = match side {
            Side::Top => &mut borders.top,
            Side::Bottom => &mut borders.bottom,
            Side::Left => &mut borders.left,
            Side::Right => &mut borders.right,
        };
        *slot = Some(b);
    }
}

/// Set one side of a cell and the matching side of the cell across it, so a shared line is
/// one line in both cells.
pub(crate) fn set_side(t: &mut Table, r: usize, ci: usize, side: Side, b: Border) {
    let across = neighbour(t, r, ci, side);
    set_cell_side(t, r, ci, side, b);
    if let Some((nr, nci)) = across {
        set_cell_side(t, nr, nci, opposite(side), b);
    }
}

/// Table Design › Borders. With the caret in the table, or a selection covering the whole table,
/// the table's borders are set. With a range of cells selected, only those cells change: `all` sets every side of them, `outside` their
/// outer edges, `inside` the lines between them, `none` clears their sides, and `top`, `bottom`,
/// `left` and `right` one outer edge. Shared lines are set in both cells.
fn borders(s: &mut Session, v: &Value) -> CmdResult {
    let kind = p::str(v, "kind").unwrap_or("all");
    if !matches!(kind, "all" | "outside" | "inside" | "none" | "top" | "bottom" | "left" | "right") {
        return Err(CmdError::Params(format!("unknown border kind `{kind}`")));
    }
    let pen = pen_border(v)?;
    let none = Border { style: BorderStyle::None, width: 0.0, color: None, space: 0.0 };
    let b = if kind == "none" { none } else { pen };
    let (tp, _, _) = cell(s)?;
    let story = s.sel.focus.story;
    let ranged = s.column_segments().is_some();
    let spots = selected_cells(s);
    let t = s.doc.table_mut(story, &tp)?;
    let total: usize = t.rows.iter().map(|r| r.cells.len()).sum();
    let chosen: std::collections::BTreeSet<(usize, usize)> = spots.iter().copied().collect();
    if !ranged || chosen.len() >= total {
        let mut cur = t.props.borders.unwrap_or_default();
        match kind {
            "all" => cur = Borders::all(b),
            "none" => cur = Borders::all(none),
            "outside" => {
                cur.top = Some(b);
                cur.bottom = Some(b);
                cur.left = Some(b);
                cur.right = Some(b);
                // The cleared inside sides must not fall back to the table style (Word writes nil).
                cur.between = Some(none);
                cur.inside_v = Some(none);
            }
            "inside" => {
                cur.between = Some(b);
                cur.inside_v = Some(b);
                // The cleared outer sides must not fall back to the table style (Word writes nil).
                cur.top = Some(none);
                cur.bottom = Some(none);
                cur.left = Some(none);
                cur.right = Some(none);
            }
            "top" => cur.top = Some(b),
            "bottom" => cur.bottom = Some(b),
            "left" => cur.left = Some(b),
            _ => cur.right = Some(b),
        }
        t.props.borders = Some(cur);
        for r in &mut t.rows {
            for c in &mut r.cells {
                c.props.borders = None;
            }
        }
    } else {
        for &(r, ci) in &chosen {
            for side in SIDES {
                let inner = neighbour(t, r, ci, side).is_some_and(|n| chosen.contains(&n));
                let wanted = match (kind, side) {
                    ("all" | "none", _) => true,
                    ("outside", _) => !inner,
                    ("inside", _) => inner,
                    ("top", Side::Top) | ("bottom", Side::Bottom) | ("left", Side::Left) | ("right", Side::Right) => !inner,
                    _ => false,
                };
                if wanted {
                    set_side(t, r, ci, side, b);
                }
            }
        }
    }
    touch_cells(s, &tp)?;
    sel_result(s)
}

pub(crate) fn cell_text(c: &wordcraft_doc::Cell) -> String {
    c.blocks.iter().filter_map(|b| b.as_para().map(|p| p.plain_text())).collect::<Vec<_>>().join("\n")
}

/// The key a row sorts by: a number (numbers sort first, by value) or the text in lower case.
#[derive(Clone, Debug, PartialEq)]
enum SortKey {
    Number(f64),
    Text(String),
}

fn sort_key(text: &str, text_only: bool) -> SortKey {
    if !text_only && let Some(x) = super::table_formula::parse_number(text) {
        return SortKey::Number(x);
    }
    SortKey::Text(text.trim().to_lowercase())
}

/// A total order on keys: numbers by value (IEEE total order), then text by code point.
fn compare_keys(a: &SortKey, b: &SortKey) -> std::cmp::Ordering {
    use std::cmp::Ordering::{Greater, Less};
    match (a, b) {
        (SortKey::Number(x), SortKey::Number(y)) => x.total_cmp(y),
        (SortKey::Number(_), SortKey::Text(_)) => Less,
        (SortKey::Text(_), SortKey::Number(_)) => Greater,
        (SortKey::Text(x), SortKey::Text(y)) => x.cmp(y),
    }
}

/// Table Layout › Data › Sort: reorders the rows below the header by the cell in grid column
/// `column` (0 is the first column). `type` is `number` (numbers first, by value, then text) or
/// `text`. Rows with a vertically merged cell can't move with their rows, so they are refused.
fn sort_table(s: &mut Session, v: &Value) -> CmdResult {
    let col = p::u64(v, "column").unwrap_or(0) as usize;
    let desc = p::bool(v, "descending").unwrap_or(false);
    let header = p::bool(v, "header");
    let text_only = match p::str(v, "type").unwrap_or("number") {
        "text" => true,
        "number" => false,
        x => return Err(CmdError::Params(format!("unknown sort type `{x}`"))),
    };
    let (tp, _, _) = cell(s)?;
    let story = s.sel.focus.story;
    let (skip, order) = {
        let t = s.doc.table(story, &tp).ok_or_else(|| CmdError::Failed("no table".into()))?;
        let skip = usize::from(header.unwrap_or(t.rows.first().is_some_and(|r| r.props.header))).min(t.rows.len());
        let body = t.rows.get(skip..).unwrap_or(&[]);
        // Word refuses to sort any table with merged cells, horizontal spans included.
        if t.rows.iter().any(|row| row.cells.iter().any(|c| c.props.vmerge != VMerge::None || c.span() > 1)) {
            return Err(CmdError::Failed("a table with merged cells can't be sorted; unmerge them first".into()));
        }
        // One key per row, from the cell covering the grid column (rows may differ in their cells).
        let mut keyed: Vec<(SortKey, usize)> = body.iter().enumerate().map(|(i, row)| (sort_key(&row_text_at(row, col), text_only), i)).collect();
        keyed.sort_by(|a, b| {
            let o = compare_keys(&a.0, &b.0);
            if desc { o.reverse() } else { o }
        });
        (skip, keyed.into_iter().map(|(_, i)| i).collect::<Vec<usize>>())
    };
    let t = s.doc.table_mut(story, &tp)?;
    let mut rows: Vec<Option<wordcraft_doc::Row>> = t.rows.drain(skip..).map(Some).collect();
    for i in order {
        if let Some(row) = rows.get_mut(i).and_then(Option::take) {
            t.rows.push(row);
        }
    }
    touch_cells(s, &tp)?;
    sel_result(s)
}

/// The text of the cell covering grid column `g` in a row (empty when the row has no such cell).
fn row_text_at(row: &wordcraft_doc::Row, g: usize) -> String {
    let mut x = 0usize;
    for c in &row.cells {
        if g < x + c.span() {
            return cell_text(c);
        }
        x += c.span();
    }
    String::new()
}

/// What separates the cells' text when a table becomes text.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Separator {
    /// Cells are joined with this character.
    Text(char),
    /// Each cell becomes its own paragraph.
    Paragraphs,
}

/// The separator a `separator` parameter names: `Some(char)` for a character, `None` for
/// paragraph marks.
fn separator_param(v: &Value, default: &str) -> Result<Option<char>, CmdError> {
    match p::str(v, "separator").unwrap_or(default) {
        "tab" => Ok(Some('\t')),
        "comma" => Ok(Some(',')),
        "paragraph" | "paragraphs" => Ok(None),
        "other" => match p::str(v, "char").and_then(|c| c.chars().next()) {
            Some(c) => Ok(Some(c)),
            None => Err(CmdError::Params("separator `other` needs a `char`".into())),
        },
        x => Err(CmdError::Params(format!("unknown separator `{x}`"))),
    }
}

/// Table Layout › Data › Convert to Text: each row becomes a paragraph, its cells joined by the
/// separator. Runs, fields and the paragraphs' formatting survive the conversion.
fn to_text(s: &mut Session, v: &Value) -> CmdResult {
    let (tp, _, _) = cell(s)?;
    let sep = match separator_param(v, "tab")? {
        Some(c) => Separator::Text(c),
        None => Separator::Paragraphs,
    };
    let story = s.sel.focus.story;
    let t = s.doc.table(story, &tp).cloned().ok_or_else(|| CmdError::Failed("no table".into()))?;
    let mut lines = table_lines(&t, sep);
    if lines.is_empty() {
        lines.push(Paragraph::new());
    }
    s.doc.remove_block(story, &tp)?;
    for (k, line) in lines.into_iter().enumerate() {
        s.doc.insert_block(story, &tp.with_last(tp.last() + k), Block::Para(line))?;
    }
    s.sel = Selection::caret(Pos { story, path: tp, off: 0 });
    sel_result(s)
}

/// The table's rows as paragraphs (see [`to_text`]). A nested table's rows follow the text of
/// the cell that holds it, as paragraphs of their own.
fn table_lines(t: &Table, sep: Separator) -> Vec<Paragraph> {
    let mut lines = Vec::new();
    for row in &t.rows {
        let mut line = Paragraph::new();
        let mut fresh = true;
        for (ci, cl) in row.cells.iter().enumerate() {
            if ci > 0 {
                match sep {
                    Separator::Text(c) => line.append(Paragraph::with_text(&c.to_string(), Default::default())),
                    Separator::Paragraphs => {
                        if !fresh {
                            lines.push(std::mem::replace(&mut line, Paragraph::new()));
                        }
                        fresh = true;
                    }
                }
            }
            for (bi, block) in cl.blocks.iter().enumerate() {
                match &**block {
                    Block::Para(p) => {
                        if bi > 0 && !fresh && matches!(sep, Separator::Text(_)) {
                            // A cell's second paragraph starts a line of its own.
                            lines.push(std::mem::replace(&mut line, Paragraph::new()));
                            fresh = true;
                        }
                        if fresh {
                            line.props = p.props.clone();
                            fresh = false;
                        }
                        line.append(p.clone());
                        line.mark = p.mark.clone();
                    }
                    Block::Table(inner) => {
                        if !fresh {
                            lines.push(std::mem::replace(&mut line, Paragraph::new()));
                        }
                        lines.extend(table_lines(inner, sep));
                        line = Paragraph::new();
                        fresh = true;
                    }
                }
            }
        }
        if !fresh {
            lines.push(line);
        }
    }
    lines
}

/// `=SUM(ABOVE)`, `=SUM(LEFT)`, `=SUM(RIGHT)`, `=SUM(BELOW)`, `=AVERAGE(...)`, `=COUNT(...)`, `=MAX`, `=MIN`, `=PRODUCT`.
/// Table Layout › Data › Formula: inserts a formula field at the caret, evaluated now and again
/// by Update Field. What a formula can use is described in [`super::table_formula`].
fn formula(s: &mut Session, v: &Value) -> CmdResult {
    let (tp, r, c) = cell(s)?;
    let raw = p::str(v, "formula").unwrap_or("=SUM(ABOVE)");
    let story = s.sel.focus.story;
    let t = s.doc.table(story, &tp).ok_or_else(|| CmdError::Failed("no table".into()))?;
    let instr = auto_formula_instr(t, r, c, raw);
    let text = super::table_formula::evaluate_field(t, r, c, &instr)?;
    let props = s.typing_props();
    let at = super::delete_selection(s)?;
    let end = s.doc.insert_object(&at, wordcraft_doc::InlineObject::Field { instr: instr.clone(), result: text.clone(), locked: false }, &props)?;
    s.sel = Selection::caret(end);
    Ok(json!({"result": text, "formula": instr}))
}

/// A formula as a field stores it: upper case, with one leading `=`. A `\#` numeric picture is
/// kept as typed.
pub(crate) fn formula_instr(raw: &str) -> String {
    let body = raw.trim();
    let body = body.strip_prefix('=').unwrap_or(body);
    match super::table_formula::split_switch(body) {
        (formula, Some(switch)) => format!("={} {switch}", formula.trim().to_ascii_uppercase()),
        (formula, None) => format!("={}", formula.trim().to_ascii_uppercase()),
    }
}

/// Resolve auto-direction if formula is a bare function name like `=SUM`.
fn auto_formula_instr(t: &Table, r: usize, c: usize, raw: &str) -> String {
    let instr = formula_instr(raw);
    let body = instr.strip_prefix('=').unwrap_or(&instr);
    if !body.contains('(') && matches!(body, "SUM" | "AVERAGE" | "COUNT" | "MAX" | "MIN" | "PRODUCT") {
        let g = t.grid_col(r, c);
        let has_above = (0..r)
            .rev()
            .any(|row| t.cell_at_grid(row, g).and_then(|ci| t.rows.get(row)?.cells.get(ci)).and_then(super::table_formula::cell_number).is_some());
        let dir = if has_above { "ABOVE" } else { "LEFT" };
        format!("={body}({dir})")
    } else {
        instr
    }
}

/// Table Layout › Rows & Columns › Delete › Delete Cells. `shift` `left` (the default) removes the
/// selected cells and moves the rest of their rows left. `up` moves the cells below them up in
/// their columns and removes the bottom cell of each column, which must end its row (Word's
/// shape; a cell in the middle of a row can't be removed without a gap).
fn delete_cells(s: &mut Session, v: &Value) -> CmdResult {
    let shift = p::str(v, "shift").unwrap_or("left");
    let (tp, r, c) = cell(s)?;
    let story = s.sel.focus.story;
    let spots = selected_cells(s);
    let t = s.doc.table_mut(story, &tp)?;
    match shift {
        "left" => {
            let mut cells: Vec<(usize, usize)> = spots;
            cells.sort_unstable();
            cells.dedup();
            for &(ri, ci) in cells.iter().rev() {
                let Some(row) = t.rows.get_mut(ri) else { continue };
                if row.cells.len() <= 1 || ci >= row.cells.len() {
                    continue;
                }
                if row.cells.get(ci).is_some_and(|x| x.props.vmerge != VMerge::None) {
                    return Err(CmdError::Failed("a merged cell can't be deleted by shifting cells; unmerge it first".into()));
                }
                row.cells.remove(ci);
            }
        }
        "up" => {
            // Per grid column: the top row of the selection and the cell's width.
            let mut groups: std::collections::BTreeMap<usize, (usize, usize)> = std::collections::BTreeMap::new();
            for &(ri, ci) in &spots {
                let g = t.grid_col(ri, ci);
                let span = t.rows.get(ri).and_then(|row| row.cells.get(ci)).map_or(1, wordcraft_doc::Cell::span);
                let e = groups.entry(g).or_insert((span, ri));
                if e.0 != span {
                    return Err(CmdError::Failed("the selected cells don't line up in their columns".into()));
                }
                e.1 = e.1.min(ri);
            }
            let last = t.rows.len().saturating_sub(1);
            for (&g, &(span, top)) in &groups {
                if top == last {
                    return Err(CmdError::Failed("no cells below to shift up".into()));
                }
                for ro in top..=last {
                    let ci = starting_at(t, ro, g).ok_or_else(|| CmdError::Failed("the cells below don't line up for shifting up".into()))?;
                    let plain = t.rows.get(ro).and_then(|row| row.cells.get(ci)).is_some_and(|x| x.span() == span && x.props.vmerge == VMerge::None);
                    if !plain {
                        return Err(CmdError::Failed("shifting cells up needs plain cells of the same width below".into()));
                    }
                }
                let bottom_ci = starting_at(t, last, g).unwrap_or(0);
                let ends_row = t.rows.get(last).is_some_and(|row| bottom_ci + 1 == row.cells.len());
                if !ends_row {
                    return Err(CmdError::Failed(
                        "shifting cells up is only possible where the bottom cell ends its row; delete the row or shift left instead".into(),
                    ));
                }
            }
            for (&g, &(_, top)) in &groups {
                for ro in top..last {
                    let here = starting_at(t, ro, g).unwrap_or(0);
                    let below = starting_at(t, ro + 1, g).unwrap_or(0);
                    let moved =
                        t.rows.get_mut(ro + 1).and_then(|row| row.cells.get_mut(below)).map(|x| std::mem::take(&mut x.blocks)).unwrap_or_default();
                    if let Some(x) = t.rows.get_mut(ro).and_then(|row| row.cells.get_mut(here)) {
                        x.blocks = if moved.is_empty() { vec![para_block(Paragraph::new())] } else { moved };
                    }
                }
                let bottom_ci = starting_at(t, last, g).unwrap_or(0);
                if let Some(row) = t.rows.get_mut(last) {
                    row.cells.remove(bottom_ci);
                }
            }
            t.repair_vmerge();
        }
        x => return Err(CmdError::Params(format!("unknown shift `{x}`; use left or up"))),
    }
    fix_caret(s, &tp, r, c)
}

/// The index of the cell in row `ro` that starts at grid column `g`.
pub(crate) fn starting_at(t: &Table, ro: usize, g: usize) -> Option<usize> {
    let mut x = 0usize;
    for (ci, cl) in t.rows.get(ro)?.cells.iter().enumerate() {
        if x == g {
            return Some(ci);
        }
        x += cl.span();
    }
    None
}

/// Table Layout › Merge › Split Cells: splits the caret's cell into `columns` cells side by side
/// and `rows` cells one above the other. With `mergeFirst`, a selection of several cells is merged
/// first and the merged cell is split.
fn split_cells(s: &mut Session, v: &Value) -> CmdResult {
    let cols = p::u64(v, "columns").unwrap_or(2).clamp(1, wordcraft_doc::table::MAX_COLS as u64) as usize;
    let rows = p::u64(v, "rows").unwrap_or(1).clamp(1, 100) as usize;
    if p::bool(v, "mergeFirst").unwrap_or(false) && selected_cells(s).len() > 1 {
        merge(s, &Value::Null)?;
    }
    let (tp, r, c) = cell(s)?;
    let story = s.sel.focus.story;
    let t = s.doc.table_mut(story, &tp)?;
    split_cell_into(t, r, c, cols, rows)?;
    fix_caret(s, &tp, r, c)
}

/// Splits cell (`r`, `c`) into `cols` cells across and `rows` cells down. The cells across are
/// made first; then `rows - 1` rows go in below row `r`, where the cells of the split run down
/// as they are, and the other cells of row `r` run down through the new rows (vertical merges).
/// A cell whose row holds merged cells can't be split this way.
fn split_cell_into(t: &mut Table, r: usize, c: usize, cols: usize, rows: usize) -> Result<(), CmdError> {
    let Some(row) = t.rows.get(r) else { return Err(CmdError::Failed("the cell is no longer there".into())) };
    if row.cells.get(c).is_none_or(|x| x.props.vmerge != VMerge::None) {
        return Err(CmdError::Failed("unmerge the cell before splitting it".into()));
    }
    if rows > 1 && row.cells.iter().any(|x| x.props.vmerge != VMerge::None) {
        return Err(CmdError::Failed("unmerge the cells in that row before splitting a cell into rows".into()));
    }
    if t.rows.len() + rows - 1 > wordcraft_doc::table::MAX_ROWS {
        return Err(CmdError::Failed("the table would have too many rows".into()));
    }
    if cols > 1 {
        t.split_cell(r, c, cols);
    }
    if rows > 1 {
        // The row's shape (spans, and which cells are the split's own) and its height style.
        let (shape, props) = {
            let Some(row) = t.rows.get_mut(r) else { return Err(CmdError::Failed("the cell is no longer there".into())) };
            let pieces = c..c + cols;
            let mut shape = Vec::with_capacity(row.cells.len());
            for (ci, x) in row.cells.iter_mut().enumerate() {
                let own = pieces.contains(&ci);
                shape.push((x.span(), own));
                if !own {
                    x.props.vmerge = VMerge::Restart;
                }
            }
            (shape, row.props.clone())
        };
        for k in 1..rows {
            let cells = shape
                .iter()
                .map(|&(span, own)| {
                    let mut cl = wordcraft_doc::Cell::empty();
                    cl.props.span = span as u32;
                    cl.props.vmerge = if own { VMerge::None } else { VMerge::Continue };
                    cl
                })
                .collect();
            let row_props = wordcraft_doc::props::RowProps { header: false, ..props.clone() };
            t.rows.insert(r + k, wordcraft_doc::Row { props: row_props, cells });
        }
    }
    t.repair_vmerge();
    sync_cell_widths(t);
    Ok(())
}

/// Table Layout › Cell Size › Distribute Columns: the selected columns (the whole table without a
/// selection) get equal widths, keeping their total.
fn distribute_selected_columns(s: &mut Session, _: &Value) -> CmdResult {
    let (tp, _, _) = cell(s)?;
    let ranged = s.column_segments().is_some();
    let spots = selected_cells(s);
    let story = s.sel.focus.story;
    let t = s.doc.table_mut(story, &tp)?;
    let (lo, hi) = if ranged {
        spots.iter().fold((usize::MAX, 0), |(lo, hi), &(ri, ci)| {
            let g = t.grid_col(ri, ci);
            let span = t.rows.get(ri).and_then(|row| row.cells.get(ci)).map_or(1, wordcraft_doc::Cell::span);
            (lo.min(g), hi.max(g + span))
        })
    } else {
        (0, t.grid.len())
    };
    let (lo, hi) = if lo >= hi { (0, t.grid.len()) } else { (lo, hi.min(t.grid.len())) };
    let n = hi.saturating_sub(lo).max(1) as f32;
    let total: f32 = t.grid.get(lo..hi).unwrap_or(&[]).iter().sum();
    for w in t.grid.get_mut(lo..hi).unwrap_or(&mut []) {
        *w = total / n;
    }
    sync_cell_widths(t);
    touch_cells(s, &tp)?;
    sel_result(s)
}

/// Table Layout › Cell Size › Distribute Rows: the selected rows (the whole table without a
/// selection) get the average of their heights, keeping their total.
fn distribute_selected_rows(s: &mut Session, _: &Value) -> CmdResult {
    let (tp, _, _) = cell(s)?;
    let ranged = s.column_segments().is_some();
    let spots = selected_cells(s);
    let story = s.sel.focus.story;
    let t = s.doc.table_mut(story, &tp)?;
    let rows: std::collections::BTreeSet<usize> = if ranged { spots.iter().map(|x| x.0).collect() } else { (0..t.rows.len()).collect() };
    let heights: Vec<f32> =
        rows.iter().filter_map(|&ri| t.rows.get(ri)).map(|row| row.props.height.filter(|h| h.is_finite() && *h > 0.0).unwrap_or(18.0)).collect();
    if heights.is_empty() {
        return sel_result(s);
    }
    let avg = (heights.iter().sum::<f32>() / heights.len() as f32).max(4.0);
    for ri in rows {
        if let Some(row) = t.rows.get_mut(ri) {
            row.props.height = Some(avg);
        }
    }
    touch_cells(s, &tp)?;
    sel_result(s)
}

/// Table Layout › Table › Resize Whole Table (the table's corner, dragged): the grid scales to the
/// width and the rows to the height, and the table's stated width is the grid's total.
fn resize_table(s: &mut Session, v: &Value) -> CmdResult {
    let story = s.sel.focus.story;
    let tp = match p::str(v, "path") {
        Some(p_str) => parse_path(p_str)?,
        None => cell(s)?.0,
    };
    let target_w = p::f32(v, "width").filter(|w| w.is_finite()).map(|w| w.clamp(20.0, 3000.0));
    let target_h = p::f32(v, "height").filter(|h| h.is_finite()).map(|h| h.clamp(10.0, 5000.0));
    let t = s.doc.table_mut(story, &tp)?;
    if let Some(new_w) = target_w {
        let current: f32 = t.grid.iter().sum();
        if current > 0.0 && !t.grid.is_empty() {
            let k = new_w / current;
            for g in &mut t.grid {
                *g = (*g * k).max(6.0);
            }
        } else {
            let n = t.grid.len().max(1) as f32;
            t.grid.iter_mut().for_each(|g| *g = (new_w / n).max(6.0));
        }
        t.props.width = Some(t.grid.iter().sum());
        t.props.width_pct = None;
        sync_cell_widths(t);
    }
    if let Some(new_h) = target_h {
        let n = t.rows.len().max(1) as f32;
        let all_set = t.rows.iter().all(|r| r.props.height.is_some_and(|h| h.is_finite() && h > 0.0));
        let current: f32 = t.rows.iter().filter_map(|r| r.props.height).sum();
        if all_set && current > 0.0 {
            let k = new_h / current;
            for row in &mut t.rows {
                row.props.height = Some(((row.props.height.unwrap_or(4.0)) * k).max(4.0));
            }
        } else {
            let each = (new_h / n).max(4.0);
            for row in &mut t.rows {
                row.props.height = Some(each);
            }
        }
    }
    touch_cells(s, &tp)?;
    sel_result(s)
}

/// The selected cells as a table of their own: the grid columns they cover, each row's selected
/// cells in order, with the gaps between them filled by empty cells. Cells keep their text,
/// formatting and merges (a merge reaching outside the selection is repaired by the grid).
pub(crate) fn selection_table(t: &Table, cells: &[(usize, usize)]) -> Option<Table> {
    let mut lo = usize::MAX;
    let mut hi = 0usize;
    let mut r0 = usize::MAX;
    let mut r1 = 0usize;
    for &(r, c) in cells {
        let g = t.grid_col(r, c);
        let span = t.rows.get(r).and_then(|row| row.cells.get(c)).map_or(1, wordcraft_doc::Cell::span);
        lo = lo.min(g);
        hi = hi.max(g + span);
        r0 = r0.min(r);
        r1 = r1.max(r);
    }
    if lo >= hi || r0 > r1 {
        return None;
    }
    let mut rows = Vec::with_capacity(r1 - r0 + 1);
    for r in r0..=r1 {
        let mut picked: Vec<(usize, &wordcraft_doc::Cell)> =
            cells.iter().filter(|&&(rr, _)| rr == r).filter_map(|&(rr, c)| Some((t.grid_col(rr, c), t.rows.get(rr)?.cells.get(c)?))).collect();
        picked.sort_by_key(|(g, _)| *g);
        let mut out: Vec<wordcraft_doc::Cell> = Vec::new();
        let mut x = lo;
        for (g, cl) in picked {
            if g < x {
                continue;
            }
            if g > x {
                out.push(filler(g - x));
            }
            out.push(cl.clone());
            x = g + cl.span();
        }
        if x < hi {
            out.push(filler(hi - x));
        }
        let mut props = t.rows.get(r).map(|row| row.props.clone()).unwrap_or_default();
        props.header = false;
        rows.push(wordcraft_doc::Row { props, cells: out });
    }
    let grid: Vec<f32> = (lo..hi).map(|g| t.grid.get(g).copied().unwrap_or(72.0)).collect();
    let props = wordcraft_doc::props::TableProps {
        width: None,
        width_pct: None,
        indent: None,
        float: None,
        caption: None,
        fmt_change: None,
        ..t.props.clone()
    };
    let mut out = Table { props, grid, rows };
    out.repair_vmerge();
    sync_cell_widths(&mut out);
    Some(out)
}

/// An empty cell `span` grid columns wide.
fn filler(span: usize) -> wordcraft_doc::Cell {
    let mut c = wordcraft_doc::Cell::empty();
    c.props.span = span.clamp(1, wordcraft_doc::table::MAX_COLS) as u32;
    c
}

/// A table as clipboard text: one line per row, cells separated by tabs, a cell's paragraphs
/// joined by spaces so that every row stays one line.
pub(crate) fn table_text(t: &Table) -> String {
    let rows: Vec<String> = t
        .rows
        .iter()
        .map(|row| {
            let cells: Vec<String> =
                row.cells.iter().map(|c| c.blocks.iter().filter_map(|b| b.as_para().map(|p| p.plain_text())).collect::<Vec<_>>().join(" ")).collect();
            cells.join("\t")
        })
        .collect();
    rows.join("\n")
}

fn no_table() -> CmdError {
    CmdError::Params("the path doesn't name a table".into())
}

/// Selects the cells of table `tp` that cover grid columns `min_g..=max_g` in rows
/// `min_r..=max_r`: a rectangle, so a merged cell is selected whole. The selection is a cell
/// range, which the table commands act on, and which copy, cut and delete act on too. A row or
/// column outside the table is an error, not a silent change of selection.
pub fn select_grid_range(s: &mut Session, tp: &Path, min_r: usize, min_g: usize, max_r: usize, max_g: usize) -> CmdResult {
    let story = s.sel.focus.story;
    let t = s.doc.table(story, tp).ok_or_else(no_table)?;
    let rows = t.rows.len();
    let (r0, r1) = (min_r.min(max_r), min_r.max(max_r));
    if r0 >= rows {
        return Err(CmdError::Params(format!("the table has no row {r0}")));
    }
    let r1 = r1.min(rows - 1);
    let cols = t.grid.len();
    let (g0, g1) = (min_g.min(max_g), min_g.max(max_g));
    if g0 >= cols {
        return Err(CmdError::Params(format!("the table has no column {g0}")));
    }
    let g1 = g1.min(cols - 1);
    // The cells covering the rectangle, row by row, in document order.
    let mut chosen: Vec<(usize, usize)> = Vec::new();
    for r in r0..=r1 {
        let mut seen = std::collections::BTreeSet::new();
        for g in g0..=g1 {
            if let Some(c) = t.cell_at_grid(r, g) {
                seen.insert(c);
            }
        }
        chosen.extend(seen.into_iter().map(|c| (r, c)));
    }
    if chosen.is_empty() {
        return Err(CmdError::Failed("no cells there to select".into()));
    }
    // Where each cell's text ends: the last paragraph inside it, nested tables included. One pass
    // over the story's paragraphs, not one per selected cell.
    let all_paths = s.doc.para_paths(story);
    let depth = tp.0.len();
    let mut ends: std::collections::HashMap<(usize, usize), Path> = std::collections::HashMap::new();
    for q in &all_paths {
        if q.0.len() > depth + 1
            && q.0.starts_with(&tp.0)
            && let (Some(&r), Some(&c)) = (q.0.get(depth), q.0.get(depth + 1))
        {
            ends.insert((r as usize, c as usize), q.clone());
        }
    }
    let mut segments = Vec::with_capacity(chosen.len());
    for (r, c) in chosen {
        let mut a = tp.0.clone();
        a.extend([r as u32, c as u32, 0]);
        let end_path = ends.get(&(r, c)).cloned().unwrap_or_else(|| {
            let mut x = tp.0.clone();
            x.extend([r as u32, c as u32, 0]);
            Path(x)
        });
        let off = s.doc.para(story, &end_path).map(|x| x.len()).unwrap_or(0);
        segments.push((Pos { story, path: Path(a), off: 0 }, Pos { story, path: end_path, off }));
    }
    if let (Some((first_a, _)), Some((_, last_b))) = (segments.first(), segments.last()) {
        s.sel = Selection { anchor: last_b.clone(), focus: first_a.clone() };
        let l = s.layout();
        let xa = l.caret_on(first_a, s.page_hint).map(|k| k.x).unwrap_or(0.0);
        let xb = l.caret_on(last_b, s.page_hint).map(|k| k.x + 50.0).unwrap_or(100.0);
        s.column = Some(crate::session::ColumnBlock { sel: s.sel.clone(), anchor_x: xa, focus_x: xb, segments });
    }
    sel_result(s)
}

/// Selects the cell range from (`r0`, `c0`) to (`r1`, `c1`), cell indices in their rows. `c1` of
/// `usize::MAX` runs to the last column.
pub fn select_cell_range(s: &mut Session, tp: &Path, r0: usize, c0: usize, r1: usize, c1: usize) -> CmdResult {
    let story = s.sel.focus.story;
    let t = s.doc.table(story, tp).ok_or_else(no_table)?;
    let g0 = t
        .rows
        .get(r0)
        .and_then(|row| row.cells.get(c0))
        .map(|_| t.grid_col(r0, c0))
        .ok_or_else(|| CmdError::Params(format!("the table has no cell at row {r0}, column {c0}")))?;
    let g1 = if c1 == usize::MAX {
        t.grid.len().saturating_sub(1)
    } else {
        let cell = t
            .rows
            .get(r1)
            .and_then(|row| row.cells.get(c1))
            .ok_or_else(|| CmdError::Params(format!("the table has no cell at row {r1}, column {c1}")))?;
        t.grid_col(r1, c1) + cell.span() - 1
    };
    select_grid_range(s, tp, r0.min(r1), g0.min(g1), r0.max(r1), g0.max(g1))
}

fn select_cells(s: &mut Session, tp: &Path, which: Option<(usize, Option<usize>)>) -> CmdResult {
    let story = s.sel.focus.story;
    let t = s.doc.table(story, tp).ok_or_else(no_table)?;
    let max_g = t.grid.len().saturating_sub(1);
    let last_row = t.rows.len().saturating_sub(1);
    match which {
        None => select_grid_range(s, tp, 0, 0, last_row, max_g),
        Some((r, None)) => select_grid_range(s, tp, r, 0, r, max_g),
        Some((r, Some(c))) => select_cell_range(s, tp, r, c, r, c),
    }
}

fn select_column_cells(s: &mut Session, tp: &Path, col: usize) -> CmdResult {
    let story = s.sel.focus.story;
    let t = s.doc.table(story, tp).ok_or_else(no_table)?;
    let last_row = t.rows.len().saturating_sub(1);
    select_grid_range(s, tp, 0, col, last_row, col)
}

/// Table Layout › Insert › Convert Text to Table: each selected paragraph becomes a row, and the
/// separator splits it into cells. Formatting carries over. Text beyond the table's limits is
/// refused, not dropped.
fn from_text(s: &mut Session, v: &Value) -> CmdResult {
    let sep = separator_param(v, "tab")?;
    let (a, b) = s.sel.ordered();
    if a.path.parent() != b.path.parent() || a.story != b.story {
        return Err(CmdError::Failed("select paragraphs to convert".into()));
    }
    let story = a.story;
    let mut source = Vec::new();
    for i in a.path.last()..=b.path.last() {
        let para = s.doc.para(story, &a.path.with_last(i)).cloned().ok_or_else(|| CmdError::Failed("select paragraphs to convert".into()))?;
        source.push(para);
    }
    let mut rows: Vec<Vec<Paragraph>> = Vec::with_capacity(source.len());
    for para in source {
        rows.push(split_paragraph_cells(para, sep)?);
    }
    let cols = rows.iter().map(Vec::len).max().unwrap_or(1);
    if cols > wordcraft_doc::table::MAX_COLS {
        return Err(CmdError::Failed(format!(
            "the text has {cols} columns, but a table can have at most {}; use fewer separators",
            wordcraft_doc::table::MAX_COLS
        )));
    }
    if rows.len() > wordcraft_doc::table::MAX_ROWS {
        return Err(CmdError::Failed(format!("the text has {} rows, but a table can have at most {}", rows.len(), wordcraft_doc::table::MAX_ROWS)));
    }
    let width = super::page::sect(s).text_width();
    let mut t = Table::new(rows.len().max(1), cols.max(1), width);
    for (r, cells) in rows.into_iter().enumerate() {
        for (c, para) in cells.into_iter().enumerate() {
            if let Some(cl) = t.rows.get_mut(r).and_then(|row| row.cells.get_mut(c)) {
                cl.blocks = vec![para_block(para)];
            }
        }
    }
    for i in (a.path.last()..=b.path.last()).rev() {
        s.doc.remove_block(story, &a.path.with_last(i))?;
    }
    s.doc.insert_block(story, &a.path, Block::Table(t))?;
    let mut first = a.path.0.clone();
    first.extend([0, 0, 0]);
    s.sel = Selection::caret(Pos { story, path: Path(first), off: 0 });
    sel_result(s)
}

/// Split one paragraph at each separator into cell paragraphs. Each part keeps the formatting of
/// its characters, and the spaces and tabs at its ends are trimmed. With no separator (`None`)
/// the paragraph is a single cell.
fn split_paragraph_cells(mut rest: Paragraph, sep: Option<char>) -> Result<Vec<Paragraph>, CmdError> {
    let Some(sep) = sep else {
        trim_paragraph(&mut rest)?;
        return Ok(vec![rest]);
    };
    let mut out = Vec::new();
    while let Some(i) = rest.text.find(sep) {
        let mut tail = rest.split_off(i)?;
        tail.delete(0, sep.len_utf8())?;
        trim_paragraph(&mut rest)?;
        out.push(std::mem::replace(&mut rest, tail));
    }
    trim_paragraph(&mut rest)?;
    out.push(rest);
    Ok(out)
}

/// Remove the spaces and tabs at either end of a paragraph's text.
fn trim_paragraph(p: &mut Paragraph) -> Result<(), CmdError> {
    let trailing = p.text.len() - p.text.trim_end_matches([' ', '\t']).len();
    if trailing > 0 {
        let end = p.text.len();
        p.delete(end - trailing, end)?;
    }
    let leading = p.text.len() - p.text.trim_start_matches([' ', '\t']).len();
    if leading > 0 {
        p.delete(0, leading)?;
    }
    Ok(())
}

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// Today as days since 1970-01-01 (UTC).
fn today_days() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| (d.as_secs() / 86_400) as i64).unwrap_or(0)
}

/// (year, month 1–12, day 1–31) of a day count from 1970-01-01 (proleptic Gregorian calendar).
pub(crate) fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The day count of a date from 1970-01-01.
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The calendar of the month that holds day `day`: its name across the top, the weekdays (Sunday
/// first, as Word's English calendars start), then the dates in weeks.
pub(crate) fn calendar_rows(day: i64) -> Vec<Vec<String>> {
    let (y, m, _) = civil_from_days(day);
    // 1970-01-01 was a Thursday: day 4 of the week counting Sunday as 0.
    let first_weekday = (days_from_civil(y, m, 1) + 4).rem_euclid(7) as usize;
    let name = MONTHS.get((m - 1).clamp(0, 11) as usize).copied().unwrap_or("");
    let mut rows = vec![vec![format!("{name} {y}")], WEEKDAYS.iter().map(|w| w.to_string()).collect()];
    let mut cells: Vec<String> = vec![String::new(); first_weekday];
    cells.extend((1..=days_in_month(y, m)).map(|d| d.to_string()));
    while !cells.len().is_multiple_of(7) {
        cells.push(String::new());
    }
    rows.extend(cells.chunks(7).map(<[String]>::to_vec));
    rows
}

/// Insert › Tables › Quick Tables. The calendar is the current month, with its name across the top
/// and the weekdays under it; the other kinds are sample text to type over.
fn quick_table(s: &mut Session, v: &Value) -> CmdResult {
    let kind = p::str(v, "kind").unwrap_or("tabular");
    let (data, style, calendar): (Vec<Vec<String>>, &str, bool) = match kind {
        "calendar" => (calendar_rows(today_days()), "GridTable1Light", true),
        "matrix" => (
            [vec!["", "Column A", "Column B", "Column C"], vec!["Row 1", "1", "2", "3"], vec!["Row 2", "4", "5", "6"], vec!["Row 3", "7", "8", "9"]]
                .iter()
                .map(|row| row.iter().map(|x| x.to_string()).collect())
                .collect(),
            "GridTable4AccentBlue",
            false,
        ),
        "tabular" => (
            [
                vec!["Item", "Needed"],
                vec!["Books", "1"],
                vec!["Magazines", "3"],
                vec!["Notebooks", "1"],
                vec!["Paper pads", "1"],
                vec!["Pens", "3"],
                vec!["Pencils", "2"],
            ]
            .iter()
            .map(|row| row.iter().map(|x| x.to_string()).collect())
            .collect(),
            "GridTable4AccentBlue",
            false,
        ),
        x => return Err(CmdError::Params(format!("unknown quick table `{x}`"))),
    };
    let cols = if calendar { 7 } else { data.iter().map(Vec::len).max().unwrap_or(1) };
    let width = super::page::sect(s).text_width();
    let mut t = Table::new(data.len(), cols, width);
    t.props.style = Some(style.into());
    for (r, row) in data.iter().enumerate() {
        for (c, txt) in row.iter().enumerate() {
            if let Some(cl) = t.rows.get_mut(r).and_then(|x| x.cells.get_mut(c)) {
                let mut para = Paragraph::with_text(txt, Default::default());
                if calendar && r == 0 {
                    para.props.align = Some(Align::Center);
                }
                cl.blocks = vec![para_block(para)];
            }
        }
    }
    if calendar {
        // The month's name spans the week.
        t.merge_right(0, 0, cols - 1);
        for r in t.rows.iter_mut().take(2) {
            r.props.header = true;
        }
    } else if let Some(r) = t.rows.first_mut() {
        r.props.header = true;
    }
    let story = s.sel.focus.story;
    let at = super::delete_selection(s)?;
    let new = s.doc.split_paragraph(&at)?;
    s.doc.insert_block(story, &new.path, Block::Table(t))?;
    let mut first = new.path.0.clone();
    first.extend([0, 0, 0]);
    s.sel = Selection::caret(Pos { story, path: Path(first), off: 0 });
    sel_result(s)
}
