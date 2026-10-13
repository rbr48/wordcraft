//! File tab (Backstage): new, open, save, export, print, properties; document inspection.

use serde_json::{Value, json};
use wordcraft_doc::{Block, Document, StoryRef};

use super::{pos_json, sel_result};
use crate::{CmdError, CmdResult, CommandSpec, Session, p};

/// Longest user name kept, in characters; longer names are cut, not refused.
pub const MAX_AUTHOR_CHARS: usize = 255;

/// Sets the user name that new tracked changes and comments are recorded under. The name is
/// trimmed, stripped of control characters (they can't go into the saved XML) and cut to
/// [`MAX_AUTHOR_CHARS`]; a blank name is refused so revisions always carry an author.
fn set_author(s: &mut Session, v: &Value) -> CmdResult {
    let name: String = p::req_str(v, "name")?.trim().chars().filter(|c| !c.is_control()).take(MAX_AUTHOR_CHARS).collect();
    let name = name.trim_end();
    if name.is_empty() {
        return Err(CmdError::Params("`name` must not be blank".into()));
    }
    s.author = name.to_string();
    let mut r = sel_result(s)?;
    if let Some(o) = r.as_object_mut() {
        o.insert("name".into(), json!(s.author));
    }
    Ok(r)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("file.new", "New", "File", new).key("Mod+N").params(r#"{"template"?: "blank|sample|letter|resume|report"}"#).pure(),
        CommandSpec::new("file.open", "Open", "File", open)
            .key("Mod+O")
            .params(r#"{"path": string, "data"?: base64 (the file's bytes; `path` then only names it), "password"?: string (for a password-protected Word document), "convert"?: bool (convert a .doc/.dot to .docx/.dotx on opening; default: the Convert on Open preference, on)}"#)
            .pure(),
        CommandSpec::new("file.save", "Save", "File", save)
            .key("Mod+S")
            .params(r#"{"path"?: string, "password"?: string (encrypt this and later saves with it, as file.encrypt does)}"#)
            .pure(),
        CommandSpec::new("file.saveAs", "Save As", "File", save_as).key("F12").params(r#"{"path": string, "password"?: string}"#).pure(),
        // Pure: converting changes where the document is saved, not its content.
        CommandSpec::new("file.convert", "Convert", "File › Info", convert)
            .params(r#"{"write"?: bool (default true: write the converted file now; the original stays as it was)}"#)
            .pure(),
        // Pure: the password must not land in Repeat or a recorded macro, and it isn't an edit
        // to undo; it marks the document as changed itself.
        CommandSpec::new("file.encrypt", "Encrypt with Password", "File › Info › Protect Document", encrypt)
            .params(r#"{"password": string (Word documents are saved encrypted with it) | null or "" (saved unencrypted)}"#)
            .pure(),
        CommandSpec::new("file.exportPdf", "Export PDF", "File › Export", |s, v| {
            let path = p::req_str(v, "path")?;
            let path = if path.to_ascii_lowercase().ends_with(".pdf") { path.to_string() } else { format!("{path}.pdf") };
            crate::io::save_path(std::path::Path::new(&path), &s.doc).map_err(CmdError::Failed)?;
            Ok(json!({"path": path}))
        })
        .params(r#"{"path": string}"#)
        .pure(),
        CommandSpec::new("file.exportPng", "Export Page as PNG", "File › Export", export_png)
            .params(r#"{"path": string, "page"?: n (1-based), "scale"?: px per pt}"#)
            .pure(),
        CommandSpec::new("file.print", "Print", "File", |s, _| {
            s.ui_requests.push(json!({"open": "print"}));
            sel_result(s)
        })
        .key("Mod+P")
        .pure(),
        CommandSpec::new("file.printNative", "Print (Native)", "File", print_native)
            .params(r#"{"path"?: string (a PDF to print; default: the document), "printer"?: string (default: the system default printer), "copies"?: n (1–100)}"#)
            .pure(),
        CommandSpec::new("file.close", "Close", "File", |s, _| {
            s.ui_requests.push(json!({"close": true}));
            sel_result(s)
        })
        .key("Mod+W")
        .pure(),
        CommandSpec::new("file.properties", "Properties", "File › Info", properties)
            .params(r#"{"title"?, "subject"?, "author"?, "keywords"?, "comments"?, "category"?, "custom"?: {name: string | null (removes)}}"#),
        CommandSpec::new("file.info", "Info", "File", info).pure(),
        CommandSpec::new("file.options", "Options", "File", |s, v| {
            let mut remove = Vec::new();
            if let Some(list) = v.get("removeWords").and_then(|x| x.as_array()) {
                for w in list {
                    if let Some(w) = w.as_str() {
                        let w = w.trim();
                        if !w.is_empty() {
                            wordcraft_proof::remove_word(w);
                            remove.push(w.to_string());
                        }
                    }
                }
            }
            if let Some(list) = v.get("addWords").and_then(|x| x.as_array()) {
                for w in list {
                    if let Some(w) = w.as_str() {
                        let w = w.trim();
                        if !w.is_empty() {
                            wordcraft_proof::add_word(w);
                        }
                    }
                }
            }
            if !remove.is_empty() || v.get("addWords").is_some() {
                s.persist_user_dictionary(&remove);
                s.relayout();
            } else {
                s.ui_requests.push(json!({"open": "options"}));
            }
            sel_result(s)
        })
        .params(r#"{"addWords"?: [string], "removeWords"?: [string]}"#)
        .pure(),
        CommandSpec::new("file.setAuthor", "User Name", "File › Options › General", set_author)
            .params(r#"{"name": string (author of new tracked changes and comments)}"#)
            .pure(),
        CommandSpec::new("document.inspect", "Inspect Document", "Agents", inspect).params(r#"{"text"?: bool}"#).pure(),
        CommandSpec::new("document.text", "Document Text", "Agents", |s, v| {
            let story = super::story_param(s, v);
            Ok(json!({"text": s.doc.plain_text(story)}))
        })
        .pure(),
        CommandSpec::new("document.paragraph", "Paragraph Details", "Agents", paragraph).params(r#"{"path": [n], "story"?: Story}"#).pure(),
        CommandSpec::new("document.selection", "Selection", "Agents", |s, _| {
            let mut r = sel_result(s)?;
            if let Some(o) = r.as_object_mut() {
                o.insert("text".into(), json!(s.selected_text()));
                o.insert("format".into(), super::format::state(s));
            }
            Ok(r)
        })
        .pure(),
        CommandSpec::new("document.layout", "Layout Summary", "Agents", layout_summary).pure(),
        CommandSpec::new("document.setText", "Replace Document Text", "Agents", |s, v| {
            let text = p::req_str(v, "text")?;
            let d = Document::from_text(text);
            s.doc.body = d.body;
            s.sel = crate::Selection::caret(s.doc.start_of(StoryRef::Body));
            sel_result(s)
        })
        .params(r#"{"text": string}"#),
    ]
}

fn new(s: &mut Session, v: &Value) -> CmdResult {
    let doc = match p::str(v, "template").unwrap_or("blank") {
        "sample" => crate::sample::sample_document(),
        "letter" => crate::sample::letter(),
        "resume" => crate::sample::resume(),
        "report" => crate::sample::report(),
        _ => Document::new(),
    };
    s.set_document(doc);
    s.path = None;
    sel_result(s)
}

fn open(s: &mut Session, v: &Value) -> CmdResult {
    let Some(path) = p::str(v, "path") else {
        s.ui_requests.push(json!({"open": "openFile"}));
        return sel_result(s);
    };
    let password = p::str(v, "password");
    let (doc, encrypted) = if let Some(data) = p::str(v, "data") {
        let bytes = super::insert::base64_decode(data).ok_or_else(|| CmdError::Params("bad base64".into()))?;
        crate::io::open_bytes_with(path, &bytes, password).map_err(CmdError::Failed)?
    } else {
        crate::io::open_path_with(std::path::Path::new(path), password).map_err(CmdError::Failed)?
    };
    s.set_document(doc);
    s.path = Some(path.into());
    // Like Word, a document opened with a password is saved with it again.
    if encrypted {
        s.password = password.map(crate::io::Password::new);
    }
    let mut out = json!({"path": path, "paragraphs": s.doc.paragraph_count(), "words": s.doc.word_count(), "encrypted": encrypted});
    // A Word 97–2003 file becomes a .docx beside it (bytes from a browser only get the new name).
    if legacy_target(std::path::Path::new(path)).is_some() && p::bool(v, "convert").unwrap_or(s.prefs.convert_legacy) {
        let conv = convert(s, &json!({"write": v.get("data").is_none()}))?;
        if let (Some(o), Some(c)) = (out.as_object_mut(), conv.as_object()) {
            o.extend(c.clone());
        }
    }
    Ok(out)
}

/// The converted file for a Word 97–2003 `path` (`.doc` → `.docx`, `.dot` → `.dotx`), or `None`
/// when `path` isn't one. An existing file of that name is never replaced: the name gets
/// " (converted)", then a number.
fn legacy_target(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let new_ext = match ext.as_str() {
        "doc" => "docx",
        "dot" => "dotx",
        _ => return None,
    };
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let dir = path.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
    let first = dir.join(format!("{stem}.{new_ext}"));
    if !first.exists() {
        return Some(first);
    }
    (1..1000)
        .map(|n| dir.join(if n == 1 { format!("{stem} (converted).{new_ext}") } else { format!("{stem} (converted {n}).{new_ext}") }))
        .find(|p| !p.exists())
}

/// File › Info › Convert: a document opened from a Word 97–2003 file (`.doc`, `.dot`) is saved
/// as a `.docx` (`.dotx`) from now on, beside the original, which is never overwritten. With
/// `write` (the default) the converted file is written at once.
fn convert(s: &mut Session, v: &Value) -> CmdResult {
    let Some(from) = s.path.clone() else {
        return Err(CmdError::Failed("the document has no file to convert; use Save As".into()));
    };
    let Some(to) = legacy_target(&from) else {
        return Ok(json!({"converted": false, "reason": "already in the current format"}));
    };
    let name = |p: &std::path::Path| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    s.path = Some(to.clone());
    let written = if p::bool(v, "write").unwrap_or(true) {
        s.stamp_save();
        let password = s.password.as_ref().map(crate::io::Password::as_str);
        match crate::io::save_path_with(&to, &s.doc, password) {
            Ok(()) => {
                s.dirty = false;
                true
            }
            // The folder may be read-only: the document still saves as .docx (Save asks where).
            Err(e) => {
                s.dirty = true;
                s.status = format!("Converted {} but couldn't write {}: {e}", name(&from), name(&to));
                return Ok(json!({"converted": true, "convertedTo": to.to_string_lossy(), "written": false, "error": e}));
            }
        }
    } else {
        s.dirty = true;
        false
    };
    s.status = format!("Converted {} to {} (the original is unchanged)", name(&from), name(&to));
    Ok(json!({"converted": true, "convertedFrom": from.to_string_lossy(), "convertedTo": to.to_string_lossy(), "written": written}))
}

/// File › Info › Protect Document › Encrypt with Password: the password Word documents are saved
/// with from now on, or none.
fn encrypt(s: &mut Session, v: &Value) -> CmdResult {
    let password = match v.get("password") {
        Some(Value::Null) => None,
        Some(Value::String(pw)) if pw.is_empty() => None,
        Some(Value::String(pw)) => {
            wordcraft_docx::check_password(pw).map_err(|e| CmdError::Params(e.to_string()))?;
            Some(crate::io::Password::new(pw))
        }
        _ => return Err(CmdError::Params("`password` must be a string, or null to remove the password".into())),
    };
    if password != s.password {
        s.password = password;
        s.dirty = true;
    }
    Ok(json!({"encrypted": s.password.is_some()}))
}

fn save(s: &mut Session, v: &Value) -> CmdResult {
    let path = match p::str(v, "path").map(std::path::PathBuf::from).or_else(|| s.path.clone()) {
        Some(p) => p,
        None => {
            s.ui_requests.push(json!({"open": "saveAs"}));
            return Ok(json!({"saved": false}));
        }
    };
    if let Some(pw) = v.get("password").filter(|pw| !pw.is_null()) {
        let Some(pw) = pw.as_str().filter(|pw| !pw.is_empty()) else {
            return Err(CmdError::Params("`password` must be a non-empty string (file.encrypt removes a password)".into()));
        };
        if !crate::io::can_encrypt(&path.to_string_lossy()) {
            return Err(CmdError::Params("only Word documents (.docx, .docm, .dotx, .dotm) can be saved with a password".into()));
        }
        wordcraft_docx::check_password(pw).map_err(|e| CmdError::Params(e.to_string()))?;
        s.password = Some(crate::io::Password::new(pw));
    }
    s.stamp_save();
    let password = s.password.as_ref().map(crate::io::Password::as_str);
    crate::io::save_path_with(&path, &s.doc, password).map_err(CmdError::Failed)?;
    let encrypted = password.is_some() && crate::io::can_encrypt(&path.to_string_lossy());
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if ["docx", "docm", "dotx", "dotm", "odt", "rtf", "json"].contains(&ext.as_str()) {
        s.path = Some(path.clone());
        s.dirty = false;
    }
    Ok(json!({"saved": true, "path": path.to_string_lossy(), "encrypted": encrypted}))
}

fn save_as(s: &mut Session, v: &Value) -> CmdResult {
    if p::str(v, "path").is_none() {
        s.ui_requests.push(json!({"open": "saveAs"}));
        return Ok(json!({"saved": false}));
    }
    save(s, v)
}

fn export_png(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::req_str(v, "path")?;
    let page = p::u64(v, "page").unwrap_or(1).max(1) as usize - 1;
    let scale = p::f32(v, "scale").unwrap_or(2.0).clamp(0.1, 8.0);
    let l = s.export_layout();
    let pg = l.pages.get(page).ok_or_else(|| CmdError::Params(format!("no page {}", page + 1)))?;
    let img = wordcraft_render::render_page(&s.doc, pg, scale, &Default::default());
    let png = img.to_png();
    #[cfg(not(target_arch = "wasm32"))]
    std::fs::write(path, &png).map_err(|e| CmdError::Failed(format!("{path}: {e}")))?;
    Ok(json!({"path": path, "width": img.width, "height": img.height, "bytes": png.len()}))
}

fn properties(s: &mut Session, v: &Value) -> CmdResult {
    let c = &mut s.doc.core;
    let set = |dst: &mut String, k: &str| {
        if let Some(x) = p::str(v, k) {
            *dst = x.to_string();
        }
    };
    set(&mut c.title, "title");
    set(&mut c.subject, "subject");
    set(&mut c.creator, "author");
    set(&mut c.keywords, "keywords");
    set(&mut c.description, "comments");
    set(&mut c.category, "category");
    if let Some(custom) = v.get("custom") {
        let Some(m) = custom.as_object() else {
            return Err(CmdError::Params("`custom` must be an object of name → string or null".into()));
        };
        for (name, val) in m {
            if name.trim().is_empty() || name.chars().count() > MAX_CUSTOM_NAME {
                return Err(CmdError::Params(format!("custom property names are 1–{MAX_CUSTOM_NAME} characters")));
            }
            match val {
                Value::Null => {
                    s.doc.remove_custom_prop(name);
                }
                Value::String(x) if x.len() <= MAX_CUSTOM_VALUE => s.doc.set_custom_prop(name, x),
                _ => return Err(CmdError::Params(format!("custom property `{name}` must be a string (≤ {MAX_CUSTOM_VALUE} bytes) or null"))),
            }
        }
    }
    let mut out = serde_json::to_value(&s.doc.core).unwrap_or(Value::Null);
    if let Some(o) = out.as_object_mut() {
        o.insert("custom".into(), serde_json::to_value(&s.doc.custom_props).unwrap_or(Value::Null));
    }
    Ok(out)
}

/// Longest custom property name (Word's limit).
const MAX_CUSTOM_NAME: usize = 255;
/// Largest custom property value we accept from a command.
const MAX_CUSTOM_VALUE: usize = 1024 * 1024;

fn info(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    Ok(json!({
        "path": s.path.as_ref().map(|p| p.to_string_lossy().to_string()),
        "dirty": s.dirty,
        "pages": l.pages.len(),
        "words": s.doc.word_count(),
        "paragraphs": s.doc.paragraph_count(),
        "properties": s.doc.core,
        "trackChanges": s.doc.settings.track_changes,
        "comments": s.doc.comments.len(),
        "sections": s.doc.sections().len(),
        "encrypted": s.password.is_some(),
        "layoutMs": l.ms,
    }))
}

/// Structure for agents: blocks with text and formatting summary, selection, stats.
fn inspect(s: &mut Session, v: &Value) -> CmdResult {
    let with_text = p::bool(v, "text").unwrap_or(true);
    let mut blocks = Vec::new();
    for (i, b) in s.doc.body.iter().enumerate().take(2000) {
        match &**b {
            Block::Para(p) => {
                let style = p.props.style.clone().unwrap_or_else(|| "Normal".into());
                let mut o = json!({"index": i, "type": "paragraph", "style": style, "len": p.len()});
                if with_text {
                    o["text"] = json!(p.plain_text());
                }
                if let Some(n) = p.props.numbering.filter(|n| n.num != 0) {
                    o["list"] = json!({"num": n.num, "level": n.level});
                }
                if !p.props.is_empty() {
                    o["props"] = serde_json::to_value(&p.props).unwrap_or(Value::Null);
                }
                let runs: Vec<Value> = p
                    .run_ranges()
                    .filter(|(_, c)| !c.is_empty())
                    .map(|(r, c)| json!({"start": r.start, "end": r.end, "props": c}))
                    .take(50)
                    .collect();
                if !runs.is_empty() {
                    o["runs"] = json!(runs);
                }
                if !p.objects.is_empty() {
                    o["objects"] = serde_json::to_value(&p.objects).unwrap_or(Value::Null);
                }
                if p.section.is_some() {
                    o["sectionBreak"] = json!(true);
                }
                blocks.push(o);
            }
            Block::Table(t) => {
                let cells: Vec<Vec<String>> = t
                    .rows
                    .iter()
                    .map(|r| {
                        r.cells
                            .iter()
                            .map(|c| c.blocks.iter().filter_map(|b| b.as_para().map(|p| p.plain_text())).collect::<Vec<_>>().join("\n"))
                            .collect()
                    })
                    .collect();
                blocks.push(json!({"index": i, "type": "table", "rows": t.rows.len(), "cols": t.cols(), "style": t.props.style, "cells": cells}));
            }
        }
    }
    let l = s.layout();
    let parts: Vec<Value> =
        s.doc.parts.iter().map(|(id, p)| json!({"id": id, "kind": p.kind, "text": s.doc.plain_text(StoryRef::Part(*id))})).collect();
    Ok(json!({
        "blocks": blocks,
        "parts": parts,
        "selection": {"anchor": pos_json(&s.sel.anchor), "focus": pos_json(&s.sel.focus), "text": s.selected_text()},
        "pages": l.pages.len(),
        "words": s.doc.word_count(),
        "sections": s.doc.sections().iter().map(|(end, sp)| json!({"endBlock": end, "props": sp})).collect::<Vec<_>>(),
        "comments": s.doc.comments.len(),
        "trackChanges": s.doc.settings.track_changes,
        "properties": s.doc.core,
        "canUndo": s.can_undo(),
        "canRedo": s.can_redo(),
    }))
}

fn paragraph(s: &mut Session, v: &Value) -> CmdResult {
    let path: Vec<u32> = serde_json::from_value(v.get("path").cloned().unwrap_or(json!([0]))).map_err(|e| CmdError::Params(e.to_string()))?;
    let story = super::story_param(s, v);
    let p = s.doc.para(story, &wordcraft_doc::Path(path)).ok_or_else(|| CmdError::Params("no paragraph there".into()))?;
    let resolved = s.doc.styles.resolve_para(&p.props);
    Ok(json!({"paragraph": p, "resolved": resolved}))
}

fn layout_summary(s: &mut Session, _: &Value) -> CmdResult {
    let l = s.layout();
    let pages: Vec<Value> = l
        .pages
        .iter()
        .map(|p| {
            let lines: usize = p.items.iter().map(|it| if let wordcraft_layout::Placed::Lines { l0, l1, .. } = it { l1 - l0 } else { 0 }).sum();
            json!({"number": p.number, "w": p.w, "h": p.h, "section": p.section, "lines": lines, "firstBlock": p.first_block, "header": p.header_story, "footer": p.footer_story})
        })
        .collect();
    let caret = l.caret_on(&s.sel.focus, s.page_hint).map(|c| json!({"page": c.page, "x": c.x, "y": c.top, "h": c.height}));
    Ok(json!({"pages": pages, "caret": caret, "ms": l.ms}))
}

/// The largest PDF `file.printNative` reads from a file or sends to the spooler.
#[cfg(not(target_arch = "wasm32"))]
const PRINT_JOB_MAX_BYTES: u64 = 256 << 20;

/// Print the document (or the PDF at `path`) through the system print spooler (`wordcraft_print`):
/// `copies` copies on `printer`, or on the default printer when none is named.
fn print_native(s: &mut Session, v: &Value) -> CmdResult {
    let copies = u32::try_from(p::u64(v, "copies").unwrap_or(1).clamp(1, 100)).unwrap_or(1);
    #[cfg(target_arch = "wasm32")]
    {
        s.ui_requests.push(json!({"open": "print"}));
        Ok(json!({"wasm": true, "copies": copies}))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let title = if s.doc.core.title.trim().is_empty() { "WordCraft document".to_string() } else { s.doc.core.title.clone() };
        let pdf = match p::str(v, "path") {
            Some(path) => read_print_pdf(path)?,
            None => crate::io::save_bytes_with("print.pdf", &s.doc, None).map_err(CmdError::Failed)?,
        };
        let job = wordcraft_print::Job {
            printer: p::str(v, "printer").filter(|n| !n.is_empty()).map(str::to_string),
            copies,
            title,
            ..wordcraft_print::Job::default()
        };
        let receipt = wordcraft_print::submit(&pdf, &job).map_err(|e| CmdError::Failed(e.to_string()))?;
        s.status = format!("Document sent to the printer ({copies} copy/copies)");
        Ok(json!({"success": true, "copies": copies, "printer": job.printer, "receipt": receipt}))
    }
}

/// The PDF `file.printNative` prints from `path`. Only PDFs are printed: the spooler gets a
/// print-ready PDF, and a Word file would print as the bytes it is.
#[cfg(not(target_arch = "wasm32"))]
fn read_print_pdf(path: &str) -> Result<Vec<u8>, CmdError> {
    let path = std::path::Path::new(path);
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
        return Err(CmdError::Failed(format!("{}: only PDF files can be printed; export the document as PDF first", path.display())));
    }
    let len = std::fs::metadata(path).map_err(|e| CmdError::Failed(format!("{}: {e}", path.display())))?.len();
    if len > PRINT_JOB_MAX_BYTES {
        return Err(CmdError::Failed(format!("{}: the PDF is larger than 256 MB", path.display())));
    }
    std::fs::read(path).map_err(|e| CmdError::Failed(format!("{}: {e}", path.display())))
}
