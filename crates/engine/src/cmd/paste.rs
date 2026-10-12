//! Paste Special: paste the clipboard in a chosen format (formatted, RTF, HTML, unformatted).
//!
//! The clipboard can hold several representations of the same content. WordCraft's own copy keeps
//! a rich fragment; the system clipboard gives text, and callers (agents, the CLI, a front end
//! with a richer clipboard) may pass `html` or `rtf` too. Text that is itself RTF or HTML source
//! can also be pasted as that format.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use wordcraft_doc::edit::Fragment;
use wordcraft_doc::props::VMerge;
use wordcraft_doc::table::MAX_ROWS;
use wordcraft_doc::{Block, Document, Paragraph, Path, Pos, Row, Table};

use super::{delete_selection, sel_result};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

/// Largest clipboard payload Paste Special accepts, in bytes.
pub const MAX_PASTE_BYTES: usize = 16 << 20;
/// Nesting depth we follow when adjusting pasted tables.
const MAX_DEPTH: usize = 64;

/// The formats, in the order the dialog lists them: (id, label).
pub const FORMATS: [(&str, &str); 4] =
    [("formatted", "Formatted Text"), ("rtf", "Rich Text Format (RTF)"), ("html", "HTML Format"), ("text", "Unformatted Text")];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("edit.pasteSpecial", "Paste Special", "Home › Clipboard › Paste", paste_special)
            .key("Mod+Alt+V")
            .params(r#"{"as"?: "formatted"|"rtf"|"html"|"text", "text"?: string, "html"?: string, "rtf"?: string}"#),
    ]
}

/// Clipboard text from the params (system clipboard), else WordCraft's own copy.
fn clip_text(s: &Session, v: &Value) -> Option<String> {
    match p::str(v, "text") {
        Some(t) => Some(t.replace("\r\n", "\n")),
        None => s.clipboard.as_ref().map(Fragment::plain_text),
    }
}

fn looks_like_rtf(t: &str) -> bool {
    t.trim_start().starts_with("{\\rtf")
}

fn looks_like_html(t: &str) -> bool {
    let t = t.trim_start();
    t.starts_with('<') && (t.contains("</") || t.contains("/>"))
}

/// The source for a format, if the clipboard has it.
fn source(s: &Session, v: &Value, fmt: &str) -> Option<String> {
    let non_empty = |t: String| if t.is_empty() { None } else { Some(t) };
    match fmt {
        "formatted" => {
            let f = s.clipboard.as_ref()?;
            // System clipboard text that isn't our own copy means someone else copied since.
            match p::str(v, "text") {
                Some(t) if f.plain_text() != t.replace("\r\n", "\n") => None,
                _ => Some(String::new()),
            }
        }
        "rtf" => p::str(v, "rtf").map(str::to_string).or_else(|| clip_text(s, v).filter(|t| looks_like_rtf(t))).and_then(non_empty),
        "html" => p::str(v, "html").map(str::to_string).or_else(|| clip_text(s, v).filter(|t| looks_like_html(t))).and_then(non_empty),
        "text" => clip_text(s, v).and_then(non_empty),
        _ => None,
    }
}

/// Formats the clipboard can be pasted as, best first.
pub fn available(s: &Session, v: &Value) -> Vec<&'static str> {
    FORMATS.iter().map(|(id, _)| *id).filter(|id| source(s, v, id).is_some()).collect()
}

fn formats_json(ids: &[&str]) -> Value {
    json!(ids.iter().map(|id| json!({"id": id, "label": FORMATS.iter().find(|(f, _)| f == id).map(|(_, l)| *l).unwrap_or(*id)})).collect::<Vec<_>>())
}

fn paste_special(s: &mut Session, v: &Value) -> CmdResult {
    for k in ["text", "html", "rtf"] {
        if p::str(v, k).is_some_and(|t| t.len() > MAX_PASTE_BYTES) {
            return Err(CmdError::Params(format!("`{k}` is larger than {} MB", MAX_PASTE_BYTES >> 20)));
        }
    }
    let ids = available(s, v);
    let Some(fmt) = p::str(v, "as") else {
        // No format chosen: list them and ask a front end (if any) to show the dialog.
        let mut req = json!({"open": "pasteSpecial", "formats": ids});
        for k in ["text", "html", "rtf"] {
            if let Some(t) = p::str(v, k) {
                req[k] = json!(t);
            }
        }
        s.ui_requests.push(req);
        return Ok(json!({"formats": formats_json(&ids)}));
    };
    let fmt = match fmt.to_ascii_lowercase().as_str() {
        "formatted" | "keep" | "wordcraft" => "formatted",
        "rtf" => "rtf",
        "html" => "html",
        "text" | "unformatted" | "plain" => "text",
        other => return Err(CmdError::Params(format!("unknown format `{other}` (formatted, rtf, html or text)"))),
    };
    let Some(src) = source(s, v, fmt) else {
        return Err(CmdError::Failed(if ids.is_empty() {
            "the clipboard is empty".into()
        } else {
            format!("the clipboard has no {fmt} content (available: {})", ids.join(", "))
        }));
    };
    let frag = match fmt {
        "text" => {
            super::type_text(s, &src.replace('\n', "\r"))?;
            let mut r = sel_result(s)?;
            r["pastedAs"] = json!(fmt);
            return Ok(r);
        }
        "formatted" => s.clipboard.clone().ok_or_else(|| CmdError::Failed("the clipboard is empty".into()))?,
        "html" => adopt(s, wordcraft_formats::html::import(&src)),
        _ => adopt(s, wordcraft_formats::rtf::import(src.as_bytes()).map_err(|e| CmdError::Failed(format!("bad RTF: {e}")))?),
    };
    insert(s, frag)?;
    let mut r = sel_result(s)?;
    r["pastedAs"] = json!(fmt);
    Ok(r)
}

/// Replace the selection with `frag`, marking it as an insertion under Track Changes.
pub fn insert(s: &mut Session, mut frag: Fragment) -> Result<(), CmdError> {
    // A copied table has its own rules for where it goes (see `paste_table`). A copy of one cell
    // pastes that cell's text, unless it is pasted over selected cells.
    if let [Block::Table(src)] = frag.blocks.as_slice() {
        let single = src.rows.len() == 1 && src.rows.first().is_some_and(|row| row.cells.len() == 1);
        let over_cells = s.column_segments().is_some();
        if !single || over_cells {
            if paste_table(s, src)? {
                return Ok(());
            }
        } else if let Some(cell) = src.rows.first().and_then(|row| row.cells.first()) {
            frag.blocks = cell.blocks.iter().map(|b| (**b).clone()).collect();
        }
    }
    let at = delete_selection(s)?;
    if s.doc.settings.track_changes {
        let rid = super::new_revision(s, wordcraft_doc::RevisionKind::Insert);
        for b in &mut frag.blocks {
            each_para(b, 0, &mut |p| {
                for r in &mut p.runs {
                    r.props.ins = Some(rid);
                    r.props.del = None;
                    r.props.fmt_change = None;
                }
            });
        }
    }
    let end = s.doc.insert_fragment(&at, &frag)?;
    s.sel = Selection::caret(end);
    Ok(())
}

/// Pastes a copied table. Over a selection of cells, the copy's cells replace the selected cells
/// from the top-left one, and rows are added at the foot of the table when the copy is taller.
/// With the caret in a table cell (and nothing selected), a copy with the table's column count
/// becomes rows above the caret's row; any other copy is left to the normal path, which nests it
/// in the cell. Outside a table it is inserted as a table of its own. Returns whether it was pasted.
fn paste_table(s: &mut Session, src: &Table) -> Result<bool, CmdError> {
    let story = s.sel.focus.story;
    let cells: Vec<(Path, usize, usize)> =
        s.column_segments().map(|segs| segs.iter().filter_map(|(a, _)| a.path.cell()).collect()).unwrap_or_default();
    if let Some((tp, _, _)) = cells.first().cloned() {
        let dest: Vec<(usize, usize)> = cells.iter().filter(|x| x.0 == tp).map(|x| (x.1, x.2)).collect();
        let Some((r0, g0)) = dest.iter().try_fold((usize::MAX, usize::MAX), |(r, g), &(row, cell)| {
            let t = s.doc.table(story, &tp)?;
            Some((r.min(row), g.min(t.grid_col(row, cell))))
        }) else {
            return Ok(false);
        };
        let t = s.doc.table_mut(story, &tp)?;
        overwrite_cells(t, r0, g0, src);
        let ci = super::table::starting_at(t, r0, g0).unwrap_or(0);
        let mut path = tp.0.clone();
        path.extend([r0 as u32, ci as u32, 0]);
        s.sel = Selection::caret(Pos { story, path: Path(path), off: 0 });
        return Ok(true);
    }
    let Some((tp, r, c)) = s.sel.focus.path.cell() else { return Ok(false) };
    if !s.sel.is_collapsed() {
        return Ok(false);
    }
    let t = s.doc.table(story, &tp).ok_or_else(|| CmdError::Failed("the table isn't there any more".into()))?;
    if src.cols() != t.cols() {
        return Ok(false);
    }
    let t = s.doc.table_mut(story, &tp)?;
    for (k, row) in src.rows.iter().enumerate() {
        if t.rows.len() >= MAX_ROWS {
            break;
        }
        let mut props = row.props.clone();
        props.header = false;
        let cells: Vec<wordcraft_doc::Cell> = row
            .cells
            .iter()
            .map(|cl| {
                let mut nc = cl.clone();
                nc.props.vmerge = VMerge::None;
                nc
            })
            .collect();
        t.rows.insert((r + k).min(t.rows.len()), Row { props, cells });
    }
    t.repair_vmerge();
    super::table::sync_cell_widths(t);
    let caret_cell = c.min(t.rows.get(r).map_or(0, |row| row.cells.len().saturating_sub(1)));
    let mut path = tp.0.clone();
    path.extend([r as u32, caret_cell as u32, 0]);
    s.sel = Selection::caret(Pos { story, path: Path(path), off: 0 });
    Ok(true)
}

/// Writes the copy's cells into the table from grid column `g0` of row `r0`, replacing the text of
/// each cell they land on. Where the two tables' columns don't line up, a copied cell goes into
/// the cell covering its column, after any text this paste already put there, so none is lost.
/// Rows are added below the table when the copy is taller than the rest.
fn overwrite_cells(t: &mut Table, r0: usize, g0: usize, src: &Table) {
    let mut written = std::collections::BTreeSet::new();
    for (i, row) in src.rows.iter().enumerate() {
        while r0 + i >= t.rows.len() && t.rows.len() < MAX_ROWS {
            let last = t.rows.len().saturating_sub(1);
            t.insert_row(t.rows.len(), last);
        }
        let dest = r0 + i;
        if dest >= t.rows.len() {
            break;
        }
        for (j, cell) in row.cells.iter().enumerate() {
            let g = g0 + src.grid_col(i, j);
            let Some(ci) = t.cell_at_grid(dest, g) else { continue };
            if let Some(d) = t.rows.get_mut(dest).and_then(|x| x.cells.get_mut(ci)) {
                if written.insert((dest, ci)) {
                    d.blocks = cell.blocks.clone();
                } else {
                    d.blocks.extend(cell.blocks.iter().filter(|b| b.as_para().is_none_or(|p| !p.is_empty())).cloned());
                }
            }
        }
    }
}

/// Every paragraph in a block, tables included (bounded depth).
pub(crate) fn each_para(b: &mut Block, depth: usize, f: &mut dyn FnMut(&mut Paragraph)) {
    if depth > MAX_DEPTH {
        return;
    }
    match b {
        Block::Para(p) => f(p),
        Block::Table(t) => {
            for row in &mut t.rows {
                for cell in &mut row.cells {
                    for cb in &mut cell.blocks {
                        each_para(Arc::make_mut(cb), depth + 1, f);
                    }
                }
            }
        }
    }
}

/// Turn an imported document into a fragment of the session's document: bring over the media,
/// lists and styles it refers to (renumbered so they can't collide with ours).
fn adopt(s: &mut Session, src: Document) -> Fragment {
    let mut media = BTreeMap::new();
    for (key, data) in &src.media {
        let ext = key.rsplit_once('.').map(|(_, e)| e).unwrap_or("png");
        media.insert(key.clone(), s.doc.add_media(data.to_vec(), ext));
    }
    let next_abs = s.doc.numbering.abstracts.iter().map(|a| a.id.saturating_add(1)).max().unwrap_or(0);
    let mut abs_map = BTreeMap::new();
    for (i, a) in src.numbering.abstracts.iter().enumerate() {
        let id = next_abs.saturating_add(i as u32);
        abs_map.insert(a.id, id);
        s.doc.numbering.abstracts.push(wordcraft_doc::numbering::AbstractNum { id, ..a.clone() });
    }
    let next_num = s.doc.numbering.nums.iter().map(|n| n.id.saturating_add(1)).max().unwrap_or(1).max(1);
    let mut num_map = BTreeMap::new();
    for (i, n) in src.numbering.nums.iter().enumerate() {
        let id = next_num.saturating_add(i as u32);
        num_map.insert(n.id, id);
        let abstract_id = abs_map.get(&n.abstract_id).copied().unwrap_or(n.abstract_id);
        s.doc.numbering.nums.push(wordcraft_doc::numbering::Num { id, abstract_id, ..n.clone() });
    }
    for st in &src.styles.styles {
        if s.doc.styles.get(&st.id).is_none() {
            s.doc.styles.upsert(st.clone());
        }
    }
    let mut blocks: Vec<Block> = src.body.iter().map(|b| (**b).clone()).collect();
    // Pasted pictures fit our text width, like Insert › Pictures (layout keeps their size).
    let max_w = crate::io::text_width(&s.doc);
    for b in &mut blocks {
        crate::io::fit_block_pictures(b, max_w);
    }
    for b in &mut blocks {
        each_para(b, 0, &mut |p| {
            if let Some(n) = p.props.numbering.as_mut()
                && n.num != 0
            {
                n.num = num_map.get(&n.num).copied().unwrap_or(0);
            }
            for o in &mut p.objects {
                if let wordcraft_doc::InlineObject::Image { media: key, .. } = o
                    && let Some(k) = media.get(key.as_str())
                {
                    *key = k.clone();
                }
            }
        });
    }
    // Text box stories the imported shapes show; pasting gives each a new story of its own.
    let parts = src.parts.into_iter().filter(|(_, p)| p.kind == wordcraft_doc::PartKind::TextBox).collect();
    Fragment { blocks, parts }
}
