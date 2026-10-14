//! Layout tab: page setup (margins, orientation, size, columns), breaks, line numbers, hyphenation.

use serde_json::{Value, json};
use wordcraft_doc::props::{Border, Borders};
use wordcraft_doc::section::{Columns, LineNumbering, MAX_NOTE_START, SectionProps, SectionStart};
use wordcraft_doc::{Block, Pos};

use super::sel_result;
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("layout.margins", "Margins", "Layout › Page Setup", margins).params(
            r#"{"preset"?: "normal|narrow|moderate|wide|mirrored|office2003", "top"?: pt, "bottom"?: pt, "left"?: pt, "right"?: pt, "gutter"?: pt}"#,
        ),
        CommandSpec::new("layout.orientation", "Orientation", "Layout › Page Setup", |s, v| {
            let land = match p::str(v, "value") {
                Some("landscape") => true,
                Some("portrait") => false,
                _ => !sect(s).landscape,
            };
            with_sect(s, |x| x.set_landscape(land))
        })
        .params(r#"{"value": "portrait|landscape"}"#),
        CommandSpec::new("layout.size", "Size", "Layout › Page Setup", size).params(r#"{"name"?: "Letter|Legal|A4|…", "width"?: pt, "height"?: pt}"#),
        CommandSpec::new("layout.columns", "Columns", "Layout › Page Setup", columns)
            .params(
                r#"{"count": 1-12, "space"?: pt, "separator"?: bool, "preset"?: "left|right", "widths"?: [[width pt, space after pt], …] (unequal columns; sets the count), "apply"?: "section|document|forward"}"#,
            ),
        CommandSpec::new("layout.break", "Breaks", "Layout › Page Setup", breaks)
            .params(r#"{"kind": "page|column|textWrapping|nextPage|continuous|evenPage|oddPage"}"#),
        CommandSpec::new("layout.lineNumbers", "Line Numbers", "Layout › Page Setup", |s, v| {
            let mode = p::str(v, "value").unwrap_or("continuous");
            let enabled = p::bool(v, "enabled").unwrap_or(mode != "none");
            let start = p::u64(v, "start").unwrap_or(1).clamp(1, 100_000) as u32;
            let count_by = p::u64(v, "countBy").or_else(|| p::u64(v, "count_by")).unwrap_or(1).clamp(1, 1000) as u32;
            let distance = p::f32(v, "distance").unwrap_or(0.0).max(0.0);
            with_sect(s, |x| {
                if !enabled || mode == "none" {
                    x.line_numbers = None;
                } else {
                    let restart = match mode {
                        "restartPage" | "page" => wordcraft_doc::section::LineNumberRestart::Page,
                        "restartSection" | "section" => wordcraft_doc::section::LineNumberRestart::Section,
                        _ => wordcraft_doc::section::LineNumberRestart::Continuous,
                    };
                    x.line_numbers = Some(LineNumbering {
                        count_by,
                        start,
                        distance,
                        restart,
                    });
                }
            })
        })
        .params(r#"{"value"?: "none|continuous|restartPage|restartSection", "enabled"?: bool, "start"?: u32, "countBy"?: u32, "distance"?: pt}"#),
        CommandSpec::new("layout.hyphenation", "Hyphenation", "Layout › Page Setup", |s, v| {
            if let Some(on) = p::bool(v, "auto").or_else(|| p::bool(v, "value")) {
                s.doc.settings.auto_hyphenation = on;
            } else if v.as_object().is_none_or(|m| m.is_empty()) {
                s.doc.settings.auto_hyphenation = !s.doc.settings.auto_hyphenation;
            }
            if let Some(caps) = p::bool(v, "caps") {
                s.doc.settings.hyphenate_caps = caps;
            }
            if let Some(zone) = p::f32(v, "zone") {
                s.doc.settings.hyphenation_zone = zone.max(0.0);
            }
            if let Some(lim) = p::u64(v, "limit") {
                s.doc.settings.consecutive_hyphen_limit = lim as u32;
            }
            Ok(json!({
                "value": s.doc.settings.auto_hyphenation,
                "auto": s.doc.settings.auto_hyphenation,
                "caps": s.doc.settings.hyphenate_caps,
                "zone": s.doc.settings.hyphenation_zone,
                "limit": s.doc.settings.consecutive_hyphen_limit,
            }))
        })
        .params(r#"{"value"?: bool, "auto"?: bool, "caps"?: bool, "zone"?: pt, "limit"?: u32}"#),
        CommandSpec::new("layout.pageSetup", "Page Setup", "Layout › Page Setup", |s, v| {
            if let Some(props) = v.get("section") {
                let mut new: SectionProps = serde_json::from_value(props.clone()).map_err(|e| CmdError::Params(e.to_string()))?;
                validate_section(&mut new)?;
                return with_sect(s, |x| *x = new.clone());
            }
            s.ui_requests.push(json!({"open": "pageSetup"}));
            Ok(serde_json::to_value(sect(s)).unwrap_or(Value::Null))
        })
        .params(r#"{"section"?: SectionProps} (replaces the whole section, headers and footers included; checked like Size, Margins, Columns, Line Numbers, Footnotes and Page Borders: non-finite numbers and margins that leave no room for text are refused; page sides 7.2–1584 pt, 1–12 columns with 0–300 pt gaps, header/footer/gutter 0–1584 pt, page border width 0.25–6 pt and distance 0–31 pt, note start 1–32767, line numbers start 1–32767, countBy 1–100, distance 0–1584 pt are clamped)"#),
        CommandSpec::new("layout.verticalAlign", "Vertical Alignment", "Layout › Page Setup › Layout", |s, v| {
            let va = match p::str(v, "value") {
                Some("center") => wordcraft_doc::props::VAlign::Center,
                Some("bottom") => wordcraft_doc::props::VAlign::Bottom,
                _ => wordcraft_doc::props::VAlign::Top,
            };
            with_sect(s, |x| x.valign = va)
        }),
        CommandSpec::new("layout.differentFirstPage", "Different First Page", "Header & Footer › Options", |s, v| {
            let on = p::bool(v, "value").unwrap_or(!sect(s).title_page);
            with_sect(s, |x| x.title_page = on)
        }),
        CommandSpec::new("layout.differentOddEven", "Different Odd & Even Pages", "Header & Footer › Options", |s, v| {
            s.doc.settings.even_odd_headers = p::bool(v, "value").unwrap_or(!s.doc.settings.even_odd_headers);
            sel_result(s)
        }),
        CommandSpec::new("layout.pageNumberFormat", "Format Page Numbers", "Insert › Header & Footer › Page Number", |s, v| {
            if v.as_object().is_none_or(|m| m.is_empty()) {
                s.ui_requests.push(json!({"open": "pageNumbers"}));
                return Ok(json!({
                    "format": sect(s).page_num_format.ooxml(),
                    "start": sect(s).page_num_start,
                    "continue": sect(s).page_num_start.is_none(),
                }));
            }
            let fmt = p::str(v, "format").map(wordcraft_doc::section::NumFormat::from_ooxml);
            let start = if p::bool(v, "continue").unwrap_or(false) {
                None
            } else {
                p::u64(v, "start").map(|x| x.min(100_000) as u32)
            };
            with_sect(s, |x| {
                if let Some(f) = fmt {
                    x.page_num_format = f;
                }
                x.page_num_start = start;
            })
        })
        .params(r#"{"format"?: "decimal|lowerRoman|upperRoman|lowerLetter|upperLetter", "start"?: n, "continue"?: bool}"#),
        CommandSpec::new("layout.section", "Section Properties", "Layout › Page Setup", |s, _| {
            Ok(serde_json::to_value(sect(s)).unwrap_or(Value::Null))
        })
        .pure(),
        CommandSpec::new("layout.sectionDirection", "Section Direction", "Layout › Page Setup", |s, v| {
            let rtl = match p::str(v, "value") {
                Some("rtl") | Some("rightToLeft") => true,
                Some("ltr") | Some("leftToRight") => false,
                _ => p::bool(v, "rtl").unwrap_or(!sect(s).rtl),
            };
            with_sect(s, |x| x.rtl = rtl)
        })
        .params(r#"{"value"?: "ltr|rtl", "rtl"?: bool}"#),
    ]
}

fn block_of(s: &Session) -> usize {
    s.sel.focus.path.0.first().copied().unwrap_or(0) as usize
}

pub fn sect(s: &Session) -> SectionProps {
    let i = s.doc.section_index_of(block_of(s));
    s.doc.sections().get(i).map(|(_, x)| (*x).clone()).unwrap_or_default()
}

/// Apply to the section(s) of the selection.
fn with_sect(s: &mut Session, f: impl Fn(&mut SectionProps)) -> CmdResult {
    let (a, b) = s.sel.ordered();
    let (i0, i1) = (a.path.0.first().copied().unwrap_or(0) as usize, b.path.0.first().copied().unwrap_or(0) as usize);
    let ends: Vec<usize> = s.doc.sections().iter().map(|(e, _)| *e).collect();
    let first = ends.iter().position(|e| i0 <= *e).unwrap_or(0);
    let last = ends.iter().position(|e| i1 <= *e).unwrap_or(first);
    for k in first..=last {
        let block = ends.get(k).copied().unwrap_or(0);
        f(s.doc.section_mut(block));
    }
    Ok(serde_json::to_value(sect(s)).unwrap_or(Value::Null))
}

/// Page sides, in points: 0.1" to 22" (the range Size uses).
pub const MIN_PAGE_SIDE: f32 = 7.2;
pub const MAX_PAGE_SIDE: f32 = 1584.0;
/// The room the text keeps between the margins, in points.
pub const MIN_TEXT_ROOM: f32 = 36.0;

/// Checks a whole section handed to `layout.pageSetup` by the rules of Size, Margins, Columns,
/// Line Numbers, Footnotes and Page Borders: a non-finite number or margins that leave no room
/// for text is an error; the rest is clamped to the ranges those commands use.
fn validate_section(x: &mut SectionProps) -> Result<(), CmdError> {
    let bad = |what: &str| CmdError::Params(format!("section: {what} must be a finite number"));
    let finite = [x.page_w, x.page_h, x.margin_top, x.margin_bottom, x.margin_left, x.margin_right, x.header, x.footer, x.gutter, x.columns.space];
    if finite.iter().any(|n| !n.is_finite()) {
        return Err(bad("sizes, margins, distances and column gaps"));
    }
    if x.columns.widths.iter().any(|(w, sp)| !w.is_finite() || !sp.is_finite()) {
        return Err(bad("column widths"));
    }
    if x.line_numbers.as_ref().is_some_and(|l| !l.distance.is_finite()) {
        return Err(bad("the line number distance"));
    }
    if x.page_borders.as_ref().is_some_and(|b| border_sides(b).any(|d| !d.width.is_finite() || !d.space.is_finite())) {
        return Err(bad("page border widths and distances"));
    }
    x.page_w = x.page_w.clamp(MIN_PAGE_SIDE, MAX_PAGE_SIDE);
    x.page_h = x.page_h.clamp(MIN_PAGE_SIDE, MAX_PAGE_SIDE);
    // Like Size, the orientation follows the sides.
    if x.page_w != x.page_h {
        x.landscape = x.page_w > x.page_h;
    }
    x.margin_top = x.margin_top.max(0.0);
    x.margin_bottom = x.margin_bottom.max(0.0);
    x.margin_left = x.margin_left.max(0.0);
    x.margin_right = x.margin_right.max(0.0);
    x.header = x.header.clamp(0.0, x.page_h);
    x.footer = x.footer.clamp(0.0, x.page_h);
    x.gutter = x.gutter.clamp(0.0, MAX_PAGE_SIDE);
    if x.margin_left + x.margin_right + x.gutter > x.page_w - MIN_TEXT_ROOM || x.margin_top + x.margin_bottom > x.page_h - MIN_TEXT_ROOM {
        return Err(CmdError::Params("margins leave no room for text".into()));
    }
    x.columns.space = x.columns.space.clamp(0.0, 300.0);
    x.columns.widths.truncate(12);
    for (w, sp) in &mut x.columns.widths {
        *w = w.clamp(18.0, MAX_PAGE_SIDE);
        *sp = sp.clamp(0.0, 300.0);
    }
    // As in Columns: custom widths set the count; a single width is no custom layout.
    if x.columns.widths.len() > 1 {
        x.columns.count = x.columns.widths.len() as u32;
    } else {
        x.columns.widths.clear();
        x.columns.count = x.columns.count.clamp(1, 12);
    }
    if let Some(l) = &mut x.line_numbers {
        l.start = l.start.clamp(1, 32_767);
        l.count_by = l.count_by.clamp(1, 100);
        l.distance = l.distance.clamp(0.0, 1584.0);
    }
    x.page_num_start = x.page_num_start.map(|n| n.min(100_000));
    x.footnote_num_start = x.footnote_num_start.map(|v| v.clamp(1, MAX_NOTE_START));
    x.endnote_num_start = x.endnote_num_start.map(|v| v.clamp(1, MAX_NOTE_START));
    // Page Borders: width 0.25–6 pt, distance from the text 0–31 pt.
    if let Some(b) = &mut x.page_borders {
        for d in border_sides_mut(b) {
            d.width = d.width.clamp(0.25, 6.0);
            d.space = d.space.clamp(0.0, 31.0);
        }
    }
    Ok(())
}

fn border_sides(b: &Borders) -> impl Iterator<Item = &Border> {
    [&b.top, &b.left, &b.bottom, &b.right, &b.between, &b.inside_v].into_iter().flatten()
}

fn border_sides_mut(b: &mut Borders) -> impl Iterator<Item = &mut Border> {
    [&mut b.top, &mut b.left, &mut b.bottom, &mut b.right, &mut b.between, &mut b.inside_v].into_iter().flatten()
}

fn margins(s: &mut Session, v: &Value) -> CmdResult {
    let preset = p::str(v, "preset");
    let (t, b, l, r) = match preset {
        Some("normal") => (72.0, 72.0, 72.0, 72.0),
        Some("narrow") => (36.0, 36.0, 36.0, 36.0),
        Some("moderate") => (72.0, 72.0, 54.0, 54.0),
        Some("wide") => (72.0, 72.0, 144.0, 144.0),
        Some("mirrored") => (72.0, 72.0, 90.0, 72.0),
        Some("office2003") => (72.0, 72.0, 90.0, 90.0),
        Some(x) => return Err(CmdError::Params(format!("unknown preset `{x}`"))),
        None => {
            let c = sect(s);
            (
                p::f32(v, "top").unwrap_or(c.margin_top),
                p::f32(v, "bottom").unwrap_or(c.margin_bottom),
                p::f32(v, "left").unwrap_or(c.margin_left),
                p::f32(v, "right").unwrap_or(c.margin_right),
            )
        }
    };
    let gutter = p::f32(v, "gutter");
    let cur = sect(s);
    if l + r + gutter.unwrap_or(cur.gutter) > cur.page_w - 36.0 || t + b > cur.page_h - 36.0 {
        return Err(CmdError::Params("margins leave no room for text".into()));
    }
    // A preset picks mirrored or not; plain numbers (a ruler drag, an agent) keep the setting.
    if preset.is_some() {
        s.doc.settings.mirror_margins = preset == Some("mirrored");
    }
    with_sect(s, |x| {
        x.margin_top = t.max(0.0);
        x.margin_bottom = b.max(0.0);
        x.margin_left = l.max(0.0);
        x.margin_right = r.max(0.0);
        if let Some(g) = gutter {
            x.gutter = g.max(0.0);
        }
    })
}

fn size(s: &mut Session, v: &Value) -> CmdResult {
    let (w, h) = if let Some(n) = p::str(v, "name") {
        let (_, w, h) = wordcraft_geom::PAPER_SIZES
            .iter()
            .find(|(name, _, _)| name.eq_ignore_ascii_case(n))
            .ok_or_else(|| CmdError::Params(format!("unknown paper `{n}`")))?;
        (*w, *h)
    } else {
        (p::req_f32(v, "width")?, p::req_f32(v, "height")?)
    };
    if !(72.0..=1584.0).contains(&w) || !(72.0..=1584.0).contains(&h) {
        return Err(CmdError::Params("page size must be 1\"–22\"".into()));
    }
    with_sect(s, |x| {
        let (pw, ph) = if x.landscape { (w.max(h), w.min(h)) } else { (w.min(h), w.max(h)) };
        x.page_w = pw;
        x.page_h = ph;
    })
}

fn columns(s: &mut Session, v: &Value) -> CmdResult {
    let preset = p::str(v, "preset");
    let space = p::f32(v, "space").unwrap_or(36.0).clamp(0.0, 300.0);
    let sep = p::bool(v, "separator").unwrap_or(false);
    let tw = sect(s).text_width();
    // Unequal columns: (width, space after) each, as the Columns dialog sends them.
    let custom = match v.get("widths").filter(|w| !w.is_null()) {
        Some(w) => Some(column_widths(w, tw)?),
        None => None,
    };
    let count = match &custom {
        Some(c) => c.len() as u64,
        None => p::u64(v, "count").unwrap_or(match preset {
            Some("left") | Some("right") => 2,
            _ => 1,
        }),
    };
    if !(1..=12).contains(&count) {
        return Err(CmdError::Params("columns must be 1–12".into()));
    }
    let widths = match (custom, preset) {
        (Some(c), _) if c.len() > 1 => c,
        (Some(_), _) => Vec::new(),
        (None, Some("left")) => vec![((tw - space) / 3.0, space), ((tw - space) * 2.0 / 3.0, 0.0)],
        (None, Some("right")) => vec![((tw - space) * 2.0 / 3.0, space), ((tw - space) / 3.0, 0.0)],
        _ => Vec::new(),
    };
    // Equal columns keep one spacing; unequal ones remember the first gap for a later switch back.
    let space = widths.first().map(|w| w.1).unwrap_or(space);
    let cols = Columns { count: count as u32, space, separator: sep, widths };
    match p::str(v, "apply").unwrap_or("section") {
        "section" => with_sect(s, |x| x.columns = cols.clone()),
        "document" => {
            let ends: Vec<usize> = s.doc.sections().iter().map(|(e, _)| *e).collect();
            for e in ends {
                s.doc.section_mut(e).columns = cols.clone();
            }
            Ok(serde_json::to_value(sect(s)).unwrap_or(Value::Null))
        }
        // From the caret on: a continuous section break first, then the new section's columns.
        "forward" => {
            let start = s.sel.ordered().0;
            s.sel = Selection::caret(start);
            breaks(s, &json!({"kind": "continuous"}))?;
            with_sect(s, |x| x.columns = cols.clone())
        }
        x => Err(CmdError::Params(format!("unknown apply `{x}` (section, document or forward)"))),
    }
}

/// `[[width, space after], …]` in points: 1–12 columns, each at least 0.25", that fit the text
/// width (`tw`).
fn column_widths(v: &Value, tw: f32) -> Result<Vec<(f32, f32)>, CmdError> {
    let bad = || CmdError::Params("widths: [[width, space after], …] in points, 1–12 columns".into());
    let a = v.as_array().filter(|a| (1..=12).contains(&a.len())).ok_or_else(bad)?;
    let mut out = Vec::with_capacity(a.len());
    for (i, c) in a.iter().enumerate() {
        let num = |k: usize| c.get(k).and_then(Value::as_f64).map(|x| x as f32).filter(|x| x.is_finite());
        let w = num(0).ok_or_else(bad)?;
        // The last column has no space after it.
        let sp = if i + 1 == a.len() { 0.0 } else { num(1).unwrap_or(0.0) };
        if w < 18.0 || !(0.0..=300.0).contains(&sp) {
            return Err(CmdError::Params("each column must be at least 0.25\" wide, with 0–300 pt after it".into()));
        }
        out.push((w, sp));
    }
    let total: f32 = out.iter().map(|(w, sp)| w + sp).sum();
    if total > tw + 1.0 {
        return Err(CmdError::Params(format!("the columns ({total:.0} pt) are wider than the text ({tw:.0} pt)")));
    }
    Ok(out)
}

fn breaks(s: &mut Session, v: &Value) -> CmdResult {
    let kind = p::str(v, "kind").unwrap_or("page");
    let start = match kind {
        "page" => return super::text::specs().iter().find(|c| c.id == "text.pageBreak").map(|c| (c.run)(s, v)).unwrap_or_else(|| sel_result(s)),
        "column" => return super::text::specs().iter().find(|c| c.id == "text.columnBreak").map(|c| (c.run)(s, v)).unwrap_or_else(|| sel_result(s)),
        "textWrapping" => {
            return super::text::specs().iter().find(|c| c.id == "text.lineBreak").map(|c| (c.run)(s, v)).unwrap_or_else(|| sel_result(s));
        }
        "nextPage" => SectionStart::NextPage,
        "continuous" => SectionStart::Continuous,
        "evenPage" => SectionStart::EvenPage,
        "oddPage" => SectionStart::OddPage,
        x => return Err(CmdError::Params(format!("unknown break `{x}`"))),
    };
    if s.sel.focus.story != wordcraft_doc::StoryRef::Body || s.sel.focus.path.depth() > 0 {
        return Err(CmdError::Failed("section breaks go in the main text".into()));
    }
    // Section break: the paragraph before the caret ends a section with the current props; the
    // new section (after) takes them too and starts as requested.
    let at = super::delete_selection(s)?;
    let cur = sect(s);
    let new = s.doc.split_paragraph(&at)?;
    let ended = at.path.clone();
    let para = s.doc.para_mut(at.story, &ended)?;
    let mut props = cur;
    // The start type belongs to the section that begins after the break.
    let next_block = new.path.last();
    para.section = Some(Box::new(props.clone()));
    props.start = start;
    *s.doc.section_mut(next_block) = props;
    s.sel = Selection::caret(Pos { off: 0, ..new });
    let _ = Block::Para;
    sel_result(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Session {
        Session::new(wordcraft_doc::Document::from_text("one\ntwo\nthree"))
    }

    /// More Columns: unequal widths with a line between, as one undo step.
    #[test]
    fn unequal_columns_with_a_line_between() {
        let mut s = doc();
        let tw = sect(&s).text_width();
        s.run("layout.columns", &json!({"widths": [[150.0, 24.0], [tw - 174.0, 0.0]], "separator": true})).unwrap();
        let c = sect(&s).columns;
        assert_eq!((c.count, c.separator, c.widths.len()), (2, true, 2));
        assert_eq!(c.widths.first(), Some(&(150.0, 24.0)));
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(sect(&s).columns, Columns::default());
        // Too wide, too narrow, too many, or nonsense: refused.
        for bad in [json!([[tw, 36.0], [100.0, 0.0]]), json!([[5.0, 0.0], [100.0, 0.0]]), json!(vec![[40.0, 0.0]; 13]), json!("x")] {
            assert!(s.run("layout.columns", &json!({"widths": bad})).is_err(), "{bad}");
        }
    }

    /// Apply to: the whole document, or from the caret on (a continuous section break first).
    #[test]
    fn columns_apply_to_the_document_or_from_the_caret_on() {
        let mut s = doc();
        s.run("layout.break", &json!({"kind": "nextPage"})).unwrap();
        s.run("layout.columns", &json!({"count": 3, "apply": "document"})).unwrap();
        assert!(s.doc.sections().iter().all(|(_, x)| x.columns.count == 3));
        let mut s = doc();
        s.run("caret.docEnd", &json!({})).unwrap();
        s.run("layout.columns", &json!({"count": 2, "apply": "forward"})).unwrap();
        let counts: Vec<u32> = s.doc.sections().iter().map(|(_, x)| x.columns.count).collect();
        assert_eq!(counts, vec![1, 2]);
        assert_eq!(s.doc.last_section.start, SectionStart::Continuous);
        assert!(s.run("layout.columns", &json!({"count": 2, "apply": "elsewhere"})).is_err());
    }

    fn with_section(f: impl Fn(&mut Value)) -> Value {
        let mut v = serde_json::to_value(SectionProps::default()).unwrap();
        f(&mut v);
        json!({"section": v})
    }

    /// `layout.pageSetup {section}` checks the values like the dedicated commands do.
    #[test]
    fn page_setup_section_is_validated() {
        let mut s = doc();
        let before = sect(&s);
        // Refused with an error, nothing changes: non-finite (overflowing), margins that leave no text room.
        for (what, p) in [
            ("inf width", with_section(|v| v["pageW"] = json!(1e300))),
            ("inf margin", with_section(|v| v["marginTop"] = json!(1e300))),
            ("inf header", with_section(|v| v["header"] = json!(1e300))),
            ("inf column space", with_section(|v| v["columns"]["space"] = json!(1e300))),
            ("inf column width", with_section(|v| v["columns"]["widths"] = json!([[1e300, 0.0], [100.0, 0.0]]))),
            ("inf line number distance", with_section(|v| v["lineNumbers"] = json!({"distance": 1e300}))),
            ("inf border space", with_section(|v| v["pageBorders"] = json!({"top": {"style": "single", "width": 1.0, "space": 1e300}}))),
            ("inf border width", with_section(|v| v["pageBorders"] = json!({"top": {"style": "single", "width": 1e300, "space": 1.0}}))),
            ("margins > page", with_section(|v| v["marginLeft"] = json!(5000.0))),
            ("margins eat the height", with_section(|v| v["marginTop"] = json!(700.0))),
            ("gutter eats the width", with_section(|v| v["gutter"] = json!(600.0))),
            ("negative page", with_section(|v| v["pageW"] = json!(-5.0))),
            ("zero page height", with_section(|v| v["pageH"] = json!(0.0))),
        ] {
            assert!(s.run("layout.pageSetup", &p).is_err(), "{what}");
            assert_eq!(sect(&s), before, "{what}");
        }
        // Clamped.
        s.run(
            "layout.pageSetup",
            &with_section(|v| {
                v["pageW"] = json!(100.0);
                v["pageH"] = json!(100.0);
                v["marginTop"] = json!(-10.0);
                v["marginBottom"] = json!(0.0);
                v["marginLeft"] = json!(0.0);
                v["marginRight"] = json!(0.0);
                v["header"] = json!(-3.0);
                v["footer"] = json!(1e6);
                v["columns"]["count"] = json!(1000);
                v["columns"]["space"] = json!(-4.0);
                v["lineNumbers"] = json!({"countBy": 0, "start": 0, "distance": 1e6, "restart": "page"});
                v["pageNumStart"] = json!(4_000_000_000u64);
                v["footnoteNumStart"] = json!(4_000_000_000u64);
                v["endnoteNumStart"] = json!(0);
                v["pageBorders"] =
                    json!({"top": {"style": "single", "width": 1e6, "space": -5.0}, "left": {"style": "single", "width": 0.0, "space": 1e6}});
            }),
        )
        .unwrap();
        let x = sect(&s);
        assert_eq!((x.page_w, x.page_h), (100.0, 100.0));
        assert_eq!(x.margin_top, 0.0);
        assert_eq!(x.header, 0.0);
        assert_eq!(x.footer, x.page_h, "a footer distance stays on the page");
        assert_eq!(x.columns.count, 12);
        assert_eq!(x.columns.space, 0.0);
        let l = x.line_numbers.unwrap();
        assert_eq!((l.count_by, l.start, l.distance), (1, 1, 1584.0));
        assert_eq!(x.page_num_start, Some(100_000));
        assert_eq!(x.footnote_num_start, Some(wordcraft_doc::section::MAX_NOTE_START));
        assert_eq!(x.endnote_num_start, Some(1));
        let b = x.page_borders.unwrap();
        let (t, lf) = (b.top.unwrap(), b.left.unwrap());
        assert_eq!((t.width, t.space), (6.0, 0.0));
        assert_eq!((lf.width, lf.space), (0.25, 31.0));
        // Huge pages clamp; columns and widths agree; orientation follows the sides.
        let mut s = doc();
        s.run(
            "layout.pageSetup",
            &with_section(|v| {
                v["pageW"] = json!(1e5);
                v["pageH"] = json!(792.0);
                v["landscape"] = json!(false);
                v["columns"]["count"] = json!(3);
                v["columns"]["widths"] = json!(vec![json!([1.0, 1e5]); 40]);
            }),
        )
        .unwrap();
        let x = sect(&s);
        assert_eq!(x.page_w, 1584.0);
        assert!(x.landscape, "wider than tall is landscape");
        assert_eq!(x.columns.widths.len(), 12);
        assert_eq!(x.columns.count, 12, "the count follows the widths");
        assert!(x.columns.widths.iter().all(|(w, sp)| *w >= 18.0 && (0.0..=300.0).contains(sp)));
        // One width is no custom layout: equal columns.
        s.run("layout.pageSetup", &with_section(|v| v["columns"]["widths"] = json!([[200.0, 0.0]]))).unwrap();
        assert!(sect(&s).columns.widths.is_empty());
    }

    /// A valid section still applies as given.
    #[test]
    fn page_setup_section_applies_a_valid_one() {
        let mut s = doc();
        s.run(
            "layout.pageSetup",
            &with_section(|v| {
                v["pageW"] = json!(595.0);
                v["pageH"] = json!(842.0);
                v["marginLeft"] = json!(50.0);
                v["columns"]["count"] = json!(2);
            }),
        )
        .unwrap();
        let x = sect(&s);
        assert_eq!((x.page_w, x.page_h, x.margin_left, x.columns.count), (595.0, 842.0, 50.0, 2));
    }
}
