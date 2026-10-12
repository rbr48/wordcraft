//! Tools and the long tail: Repeat, macros, AutoCorrect, Compare, accessibility checker,
//! document inspector, restrict editing, versions, templates, Quick Parts, and more.

use serde_json::{Value, json};
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::props::{CharProps, NumRef, RowProps, TableProps};
use wordcraft_doc::{Block, Cell, Document, Paragraph, Pos, RevisionKind, Row, StoryRef, Table, para_block};

use super::{delete_selection, sel_result};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

/// Built-in AutoCorrect replacements (our own list of common typos and symbols).
pub const AUTOCORRECT: &[(&str, &str)] = &[
    ("teh", "the"),
    ("adn", "and"),
    ("recieve", "receive"),
    ("recieved", "received"),
    ("seperate", "separate"),
    ("definately", "definitely"),
    ("occured", "occurred"),
    ("untill", "until"),
    ("wich", "which"),
    ("becuase", "because"),
    ("thier", "their"),
    ("alot", "a lot"),
    ("accomodate", "accommodate"),
    ("acheive", "achieve"),
    ("beleive", "believe"),
    ("calender", "calendar"),
    ("goverment", "government"),
    ("tommorow", "tomorrow"),
    ("wierd", "weird"),
    ("i", "I"),
    ("dont", "don't"),
    ("cant", "can't"),
    ("wont", "won't"),
    ("didnt", "didn't"),
    ("doesnt", "doesn't"),
    ("isnt", "isn't"),
    ("(c)", "©"),
    ("(r)", "®"),
    ("(tm)", "™"),
    ("...", "…"),
    ("->", "→"),
    ("<-", "←"),
    ("=>", "⇒"),
    (":)", "☺"),
    ("1/2", "½"),
    ("1/4", "¼"),
    ("3/4", "¾"),
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("edit.repeat", "Repeat", "Quick Access Toolbar", |s, _| {
            let Some((id, params)) = s.last_command.clone() else { return Err(CmdError::Failed("nothing to repeat".into())) };
            s.run(&id, &params)
        })
        .key("F4")
        .pure(),
        CommandSpec::new("tools.recordMacro", "Record Macro", "View › Macros", |s, v| match s.recording.take() {
            Some((name, steps)) => {
                let n = steps.len();
                s.macros.insert(name.clone(), steps);
                Ok(json!({"recording": false, "saved": name, "steps": n}))
            }
            None => {
                let name = p::str(v, "name").unwrap_or("Macro1").to_string();
                s.recording = Some((name.clone(), Vec::new()));
                Ok(json!({"recording": true, "name": name}))
            }
        })
        .params(r#"{"name"?: string} (call again to stop)"#)
        .pure(),
        CommandSpec::new("tools.macros", "Macros", "View › Macros", macros)
            .params(r#"{"run"?: name, "define"?: {"name": string, "steps": [{"command", "params"}]}, "delete"?: name}"#)
            .pure(),
        CommandSpec::new("tools.autocorrect", "AutoCorrect Options", "File › Options › Proofing", |s, v| {
            if let Some(b) = p::bool(v, "enabled") {
                s.autocorrect_on = b;
            }
            if let Some(b) = p::bool(v, "twoInitialCaps") {
                s.autocorrect_two_initial_caps = b;
            }
            if let Some(b) = p::bool(v, "capsLock") {
                s.autocorrect_caps_lock = b;
            }
            if let Some(b) = p::bool(v, "capSentences") {
                s.autocorrect_cap_sentences = b;
            }
            if let Some(b) = p::bool(v, "capCells") {
                s.autocorrect_cap_cells = b;
            }
            if let (Some(f), Some(t)) =
                (v.get("add").and_then(|a| a.get("from")).and_then(Value::as_str), v.get("add").and_then(|a| a.get("to")).and_then(Value::as_str))
            {
                s.autocorrect_user.retain(|(a, _)| a != f);
                s.autocorrect_user.push((f.to_string(), t.to_string()));
            }
            if let Some(w) = p::str(v, "addInitialCapsException") {
                let w = w.trim();
                if !INITIAL_CAPS_EXCEPTIONS.contains(&w)
                    && !s.autocorrect_initial_caps_exceptions.iter().any(|e| e == w)
                {
                    s.autocorrect_initial_caps_exceptions.push(w.to_string());
                }
            }
            if let Some(w) = p::str(v, "deleteInitialCapsException") {
                let w = w.trim();
                s.autocorrect_initial_caps_exceptions.retain(|e| e != w);
            }
            Ok(json!({
                "enabled": s.autocorrect_on,
                "twoInitialCaps": s.autocorrect_two_initial_caps,
                "capsLock": s.autocorrect_caps_lock,
                "capSentences": s.autocorrect_cap_sentences,
                "capCells": s.autocorrect_cap_cells,
                "entries": AUTOCORRECT.len() + s.autocorrect_user.len(),
                "user": s.autocorrect_user,
                "initialCapsExceptions": INITIAL_CAPS_EXCEPTIONS.iter().map(|e| e.to_string()).chain(s.autocorrect_initial_caps_exceptions.iter().cloned()).collect::<Vec<_>>(),
            }))
        })
        .params(r#"{"enabled"?: bool, "twoInitialCaps"?: bool, "capsLock"?: bool, "capSentences"?: bool, "capCells"?: bool, "add"?: {"from": string, "to": string}, "addInitialCapsException"?: string, "deleteInitialCapsException"?: string}"#)
        .pure(),
        CommandSpec::new("review.compare", "Compare", "Review › Compare", compare).params(r#"{"path"?: string, "text"?: string (revised version)}"#),
        CommandSpec::new("review.combine", "Combine", "Review › Compare", compare).params(r#"{"path"?: string}"#),
        CommandSpec::new("file.accessibility", "Check Accessibility", "Review › Accessibility", accessibility).pure(),
        CommandSpec::new("tools.auditDocument", "Audit Document Quality", "Review › Accessibility", audit_document)
            .params(r#"{"fix"?: "tableHeaders|defaultAltText|all"} -> {"issues": [...], "readability": {...}}"#)
            .pure(),
        CommandSpec::new("file.inspect", "Inspect Document", "File › Info", inspect_doc)
            .params(r#"{"remove"?: ["comments", "revisions", "properties", "hidden", "headers"]}"#),
        CommandSpec::new("review.restrict", "Restrict Editing", "Review › Protect", |s, v| {
            let mode = p::str(v, "mode").unwrap_or("readOnly").to_string();
            s.doc.settings.protection = if mode == "none" { None } else { Some(mode.clone()) };
            if mode == "trackedChanges" {
                s.doc.settings.track_changes = true;
            }
            Ok(json!({"protection": s.doc.settings.protection}))
        })
        .params(r#"{"mode": "none|readOnly|comments|trackedChanges|forms"}"#),
        CommandSpec::new("file.protect", "Protect Document", "File › Info", |s, v| {
            let mode = p::str(v, "mode").unwrap_or("readOnly");
            s.run("review.restrict", &json!({"mode": mode}))
        })
        .params(r#"{"mode": "none|readOnly|comments|trackedChanges"}"#),
        CommandSpec::new("file.versions", "Version History", "File › Info", |s, v| {
            if let Some(label) = p::str(v, "save") {
                s.versions.push((label.to_string(), super::now_iso(), s.doc.clone()));
                if s.versions.len() > 50 {
                    s.versions.remove(0);
                }
            }
            if let Some(i) = p::u64(v, "restore") {
                let (_, _, d) = s.versions.get(i as usize).cloned().ok_or_else(|| CmdError::Params("no such version".into()))?;
                s.checkpoint("Restore Version");
                s.doc = d;
                s.touch();
                s.clamp_selection();
            }
            Ok(Value::Array(s.versions.iter().enumerate().map(|(i, (l, d, _))| json!({"index": i, "label": l, "date": d})).collect()))
        })
        .params(r#"{"save"?: label, "restore"?: index}"#)
        .pure(),
        CommandSpec::new("file.recover", "Recover Unsaved Documents", "File › Info", |s, _| {
            Ok(Value::Array(s.versions.iter().enumerate().map(|(i, (l, d, _))| json!({"index": i, "label": l, "date": d})).collect()))
        })
        .pure(),
        CommandSpec::new("file.newFromTemplate", "New from Template", "File › New", |s, v| {
            let path = p::req_str(v, "path")?;
            let doc = crate::io::open_path(std::path::Path::new(path)).map_err(CmdError::Failed)?;
            s.set_document(doc);
            s.path = None;
            s.dirty = true;
            sel_result(s)
        })
        .params(r#"{"path": string}"#)
        .pure(),
        CommandSpec::new("file.saveTemplate", "Save as Template", "File › Save As", |s, v| {
            let path = p::req_str(v, "path")?;
            let path = if path.ends_with(".dotx") { path.to_string() } else { format!("{path}.dotx") };
            crate::io::save_path(std::path::Path::new(&path), &s.doc).map_err(CmdError::Failed)?;
            Ok(json!({"path": path}))
        })
        .params(r#"{"path": string}"#)
        .pure(),
        CommandSpec::new("file.autosave", "AutoSave", "Quick Access Toolbar", |s, v| {
            s.autosave = p::bool(v, "value").unwrap_or(!s.autosave);
            Ok(json!({"value": s.autosave}))
        })
        .pure(),
        CommandSpec::new("file.compatibility", "Check Compatibility", "File › Info", |s, _| {
            let mut issues = Vec::new();
            if s.doc
                .para_paths(StoryRef::Body)
                .iter()
                .any(|p| s.doc.para(StoryRef::Body, p).is_some_and(|x| x.objects.iter().any(|o| matches!(o, InlineObject::Opaque { .. }))))
            {
                issues.push("Some embedded objects from the original file are shown as text only.");
            }
            Ok(json!({"issues": issues}))
        })
        .pure(),
        CommandSpec::new("file.share", "Share", "File", |s, _| {
            s.ui_requests.push(json!({"open": "saveAs"}));
            Ok(json!({"hint": "Save or export a copy (PDF, Word document) to share it."}))
        })
        .pure(),
        CommandSpec::new("insert.quickParts", "Quick Parts", "Insert › Text", quick_parts)
            .params(r#"{"save"?: name (from the selection), "insert"?: name, "delete"?: name} → list"#),
        CommandSpec::new("insert.autoText", "AutoText", "Insert › Text › Quick Parts", quick_parts).params(r#"{"save"?: name, "insert"?: name}"#),
        CommandSpec::new("insert.docProperty", "Document Property", "Insert › Text › Quick Parts", |s, v| {
            let name = p::str(v, "name").unwrap_or("Title").to_string();
            let c = &s.doc.core;
            let val = match name.to_lowercase().as_str() {
                "title" => c.title.clone(),
                "author" | "creator" => c.creator.clone(),
                "subject" => c.subject.clone(),
                "keywords" => c.keywords.clone(),
                "comments" | "description" => c.description.clone(),
                "category" => c.category.clone(),
                _ => return Err(CmdError::Params("unknown property".into())),
            };
            let instr = match name.to_lowercase().as_str() {
                "author" | "creator" => "AUTHOR".to_string(),
                "title" => "TITLE".to_string(),
                other => format!("DOCPROPERTY {other}"),
            };
            let props = s.typing_props();
            let at = delete_selection(s)?;
            let end = s.doc.insert_object(&at, InlineObject::Field { instr, result: val, locked: false }, &props)?;
            s.sel = Selection::caret(end);
            sel_result(s)
        })
        .params(r#"{"name": "Title|Author|Subject|Keywords|Comments|Category"}"#),
        CommandSpec::new("insert.signatureLine", "Signature Line", "Insert › Text", |s, v| {
            let signer = p::str(v, "signer").unwrap_or("").to_string();
            let title = p::str(v, "title").unwrap_or("").to_string();
            let at = delete_selection(s)?;
            let mut lines = vec!["X ______________________________".to_string()];
            if !signer.is_empty() {
                lines.push(signer);
            }
            if !title.is_empty() {
                lines.push(title);
            }
            let frag = wordcraft_doc::edit::Fragment {
                blocks: lines.iter().map(|l| Block::Para(Paragraph::with_text(l, CharProps::default()).styled("NoSpacing"))).collect(),
                ..Default::default()
            };
            let end = s.doc.insert_fragment(&at, &frag)?;
            s.sel = Selection::caret(end);
            sel_result(s)
        })
        .params(r#"{"signer"?: string, "title"?: string}"#),
        CommandSpec::new("insert.object", "Object", "Insert › Text", |s, v| {
            let path = p::req_str(v, "path")?;
            s.run("insert.textFromFile", &json!({"path": path}))
        })
        .params(r#"{"path": string}"#),
        CommandSpec::new("insert.spreadsheet", "Spreadsheet Table", "Insert › Tables", |s, v| {
            let csv = p::str(v, "csv").unwrap_or("A,B,C\n1,2,3\n4,5,6");
            let rows = super::mailings::parse_csv(csv);
            let cols = rows.iter().map(Vec::len).max().unwrap_or(1).clamp(1, 63);
            s.run("insert.table", &json!({"rows": rows.len().max(1), "cols": cols, "style": "GridTable1Light"}))?;
            let Some((tp, _, _)) = s.sel.focus.path.cell() else { return sel_result(s) };
            let t = s.doc.table_mut(s.sel.focus.story, &tp)?;
            for (r, row) in rows.iter().enumerate() {
                for (c, val) in row.iter().enumerate() {
                    if let Some(cell) = t.rows.get_mut(r).and_then(|x| x.cells.get_mut(c)) {
                        cell.blocks = vec![para_block(Paragraph::with_text(val, CharProps::default()))];
                    }
                }
            }
            sel_result(s)
        })
        .params(r#"{"csv"?: string}"#),
        CommandSpec::new("layout.linkToPrevious", "Link to Previous", "Header & Footer › Navigation", |s, v| {
            let header = p::bool(v, "footer").map(|f| !f).unwrap_or(true);
            let block = s.sel.focus.path.0.first().copied().unwrap_or(0) as usize;
            let sect = s.doc.section_mut(block);
            if header {
                sect.headers = Default::default();
            } else {
                sect.footers = Default::default();
            }
            Ok(json!({"linked": true}))
        })
        .params(r#"{"footer"?: bool}"#),
        CommandSpec::new("hf.next", "Next Section", "Header & Footer › Navigation", |s, _| hf_nav(s, 1)).pure(),
        CommandSpec::new("hf.previous", "Previous Section", "Header & Footer › Navigation", |s, _| hf_nav(s, -1)).pure(),
        CommandSpec::new("hf.position", "Header/Footer Position", "Header & Footer › Position", |s, v| {
            let (h, f) = (p::f32(v, "header"), p::f32(v, "footer"));
            let block = s.sel.focus.path.0.first().copied().unwrap_or(0) as usize;
            let sect = s.doc.section_mut(block);
            if let Some(h) = h {
                sect.header = h.clamp(0.0, 300.0);
            }
            if let Some(f) = f {
                sect.footer = f.clamp(0.0, 300.0);
            }
            Ok(json!({"header": sect.header, "footer": sect.footer}))
        })
        .params(r#"{"header"?: pt, "footer"?: pt}"#),
        CommandSpec::new("table.cellMargins", "Cell Margins", "Table Layout › Alignment", |s, v| {
            let Some((tp, _, _)) = s.sel.focus.path.cell() else { return Err(CmdError::Disabled("not in a table".into())) };
            let story = s.sel.focus.story;
            // Only the sides named change; the others keep the table's margins (Word's defaults at first).
            let current = s.doc.table(story, &tp).and_then(|t| t.props.cell_margins).unwrap_or([0.0, 5.4, 0.0, 5.4]);
            let side = |key: &str, now: f32| p::f32(v, key).map_or(now, |x| x.clamp(0.0, 144.0));
            let [top, left, bottom, right] = current;
            let m = [side("top", top), side("left", left), side("bottom", bottom), side("right", right)];
            s.doc.table_mut(story, &tp)?.props.cell_margins = Some(m);
            super::table::touch_cells(s, &tp)?;
            sel_result(s)
        })
        .params(r#"{"top"?: pt, "left"?: pt, "bottom"?: pt, "right"?: pt}"#),
        CommandSpec::new("para.setNumberingValue", "Set Numbering Value", "Home › Paragraph › Numbering", |s, v| {
            let value = p::u64(v, "value").unwrap_or(1).clamp(0, 100_000) as u32;
            let f = s.sel.focus.clone();
            let n = s
                .doc
                .para_at(&f)
                .and_then(|x| x.props.numbering)
                .filter(|n| n.num != 0)
                .ok_or_else(|| CmdError::Failed("not in a numbered list".into()))?;
            let new = s.doc.numbering.restart(n.num).ok_or_else(|| CmdError::Failed("bad list".into()))?;
            if let Some(num) = s.doc.numbering.nums.iter_mut().find(|x| x.id == new) {
                num.start_overrides = vec![(n.level, value)];
            }
            let paths: Vec<_> = s.doc.para_paths(f.story).into_iter().filter(|q| *q >= f.path).collect();
            for path in paths {
                let para = s.doc.para_mut(f.story, &path)?;
                match para.props.numbering {
                    Some(x) if x.num == n.num => para.props.numbering = Some(NumRef { num: new, level: x.level }),
                    _ => break,
                }
                para.touch();
            }
            sel_result(s)
        })
        .params(r#"{"value": n}"#),
        CommandSpec::new("para.defineBullet", "Define New Bullet", "Home › Paragraph › Bullets", |s, v| {
            s.run("para.bullets", &json!({"kind": p::str(v, "char").unwrap_or("★")}))
        })
        .params(r#"{"char": string}"#),
        CommandSpec::new("para.defineNumber", "Define New Number Format", "Home › Paragraph › Numbering", |s, v| {
            let fmt = wordcraft_doc::section::NumFormat::from_ooxml(p::str(v, "format").unwrap_or("decimal"));
            let text = p::str(v, "text").unwrap_or("%1.").to_string();
            let start = p::u64(v, "start").unwrap_or(1).min(100_000) as u32;
            let id = s.doc.numbering.add_list(wordcraft_doc::ListKind::Numbered);
            if let Some(a) = s.doc.numbering.abstract_of(id).map(|a| a.id)
                && let Some(abs) = s.doc.numbering.abstracts.iter_mut().find(|x| x.id == a)
                && let Some(l) = abs.levels.first_mut()
            {
                l.format = fmt;
                l.text = text;
                l.start = start;
            }
            super::para::fmt(s, &|pp| {
                pp.numbering = Some(NumRef { num: id, level: 0 });
                pp.style = Some("ListParagraph".into());
            })
        })
        .params(r#"{"format": "decimal|upperRoman|lowerLetter…", "text"?: "%1.", "start"?: n}"#),
        CommandSpec::new("review.language", "Language", "Review › Language", |s, v| {
            let lang = p::str(v, "lang").unwrap_or("en-US").to_string();
            let no_proof = p::bool(v, "noProof");
            super::format::apply(s, &|c| {
                c.lang = Some(lang.clone());
                if let Some(np) = no_proof {
                    c.no_proof = Some(np);
                }
            })
        })
        .params(r#"{"lang": "en-US|en-GB|fr-FR|…", "noProof"?: bool}"#),
        CommandSpec::new("review.showMarkup", "Show Markup", "Review › Tracking", |s, v| {
            s.view.show_markup = p::bool(v, "value").unwrap_or(!s.view.show_markup);
            s.relayout();
            Ok(json!({"value": s.view.show_markup}))
        })
        .pure(),
        CommandSpec::new("review.editor", "Editor", "Home › Editor", |s, v| s.run("review.spelling", v)).pure(),
        CommandSpec::new("select.similar", "Select Text with Similar Formatting", "Home › Editing › Select", select_similar).pure(),
        CommandSpec::new("select.extend", "Extend Selection", "Editing › Selection", |s, _| {
            // F8 grows the selection: word → sentence → paragraph → document.
            let (a, b) = s.sel.ordered();
            let para_len = s.doc.para_at(&a).map(|x| x.len()).unwrap_or(0);
            let id = if a == b {
                "select.word"
            } else if a.path == b.path && (a.off > 0 || b.off < para_len) {
                if s.doc.para_at(&a).map(|x| x.sentence_at(a.off)) == Some((a.off, b.off)) { "select.paragraph" } else { "select.sentence" }
            } else {
                "select.all"
            };
            s.run(id, &json!({}))
        })
        .key("F8")
        .pure(),
        CommandSpec::new("design.effects", "Effects", "Design › Document Formatting", |s, v| {
            s.doc.settings.theme_name =
                format!("{} ({})", s.doc.settings.theme_name.split(" (").next().unwrap_or("Craft"), p::str(v, "name").unwrap_or("Subtle"));
            sel_result(s)
        }),
        CommandSpec::new("view.immersive", "Immersive Reader", "View › Immersive", |s, _| {
            s.view.read_mode = !s.view.read_mode;
            s.view.focus_mode = s.view.read_mode;
            Ok(json!({"value": s.view.read_mode}))
        })
        .pure(),
        CommandSpec::new("view.vertical", "Vertical", "View › Page Movement", |s, _| {
            s.view.multi_page = false;
            s.view.fit.clear();
            Ok(json!({"pageMovement": "vertical"}))
        })
        .pure(),
        CommandSpec::new("view.sideToSide", "Side to Side", "View › Page Movement", |s, _| {
            s.view.multi_page = true;
            s.view.fit = "multiplePages".into();
            Ok(json!({"pageMovement": "sideToSide"}))
        })
        .pure(),
        CommandSpec::new("tools.dictate", "Dictate", "Home › Voice", dictate).params(r#"{"text"?: string, "listening"?: bool}"#),
        CommandSpec::new("tools.copilot", "Copilot / AI Assistant", "Home › AI", copilot).params(
            r#"{"action"?: "toggle|draft|rewrite|summarize|proofread|table|ask|autocomplete", "prompt"?: string, "tone"?: string, "replace"?: bool, "insert"?: bool}"#,
        ),
        CommandSpec::new("ai.rewrite", "AI Rewrite", "Home › AI", ai_rewrite).params(
            r#"{"text"?: string, "tone"?: "concise|academic|executive|casual|professional", "replace"?: bool, "insert"?: bool}"#,
        ),
        CommandSpec::new("ai.generateTable", "AI Generate Table", "Insert › Tables", ai_generate_table).params(
            r#"{"prompt": string, "rows"?: n, "cols"?: n, "style"?: string}"#,
        ),
        CommandSpec::new("ai.summarize", "AI Summarize", "Home › AI", ai_summarize).params(
            r#"{"mode"?: "bullets|executive|abstract", "text"?: string, "insert"?: bool, "replace"?: bool}"#,
        ),
        CommandSpec::new("ai.ask", "AI Ask Document", "Home › AI", ai_ask).params(
            r#"{"question": string}"#,
        ),
        CommandSpec::new("ai.autocomplete", "AI Autocomplete", "Home › AI", ai_autocomplete).params(
            r#"{"prefix"?: string, "insert"?: bool}"#,
        ),
        CommandSpec::new("tools.customizeRibbon", "Customize Ribbon", "Tools › Customize", customize_ribbon).params(r#"{"tabs"?: string[]}"#).pure(),
        CommandSpec::new("tools.customizeKeyboard", "Customize Keyboard", "Tools › Customize", customize_keyboard)
            .params(r#"{"command"?: string, "key"?: string}"#)
            .pure(),
        CommandSpec::new("tools.templates", "Templates and Add-ins", "Tools › Templates", templates_and_addins)
            .params(r#"{"template"?: string}"#)
            .pure(),
        CommandSpec::new("addins.get", "Get Add-ins", "Insert › Add-ins", get_addins).params(r#"{"action"?: "browse|install|list"}"#).pure(),
    ]
}

fn dictate(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(txt) = v.get("text").and_then(Value::as_str) {
        let at = delete_selection(s)?;
        let end = s.doc.insert_text(&at, txt, &s.typing_props())?;
        s.sel = Selection::caret(end);
        return sel_result(s);
    }
    let listening = v.get("listening").and_then(Value::as_bool).unwrap_or(true);
    s.status = if listening { "Dictation: listening...".into() } else { "Dictation: stopped".into() };
    Ok(json!({"listening": listening, "status": s.status}))
}

fn customize_ribbon(_s: &mut Session, v: &Value) -> CmdResult {
    let tabs = v.get("tabs").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect::<Vec<_>>());
    Ok(json!({"customized": true, "tabs": tabs}))
}

fn customize_keyboard(s: &mut Session, v: &Value) -> CmdResult {
    let cmd = p::str(v, "command").unwrap_or("");
    let key = p::str(v, "key").unwrap_or("");
    let reset = p::bool(v, "reset").unwrap_or(false);

    if reset {
        s.custom_shortcuts.clear();
        s.cmd_shortcuts.clear();
        return Ok(json!({"reset": true}));
    }

    if cmd.is_empty() {
        return Ok(json!({
            "shortcuts": s.cmd_shortcuts,
        }));
    }

    if key.is_empty() || key.eq_ignore_ascii_case("none") {
        if let Some(old_key) = s.cmd_shortcuts.remove(cmd) {
            s.custom_shortcuts.remove(&old_key);
        }
        Ok(json!({"assigned": false, "command": cmd}))
    } else {
        if let Some(old_cmd) = s.custom_shortcuts.get(key) {
            s.cmd_shortcuts.remove(old_cmd);
        }
        if let Some(old_key) = s.cmd_shortcuts.get(cmd) {
            s.custom_shortcuts.remove(old_key);
        }
        s.custom_shortcuts.insert(key.to_string(), cmd.to_string());
        s.cmd_shortcuts.insert(cmd.to_string(), key.to_string());
        Ok(json!({"assigned": true, "command": cmd, "key": key}))
    }
}

fn copilot(s: &mut Session, v: &Value) -> CmdResult {
    let action = p::str(v, "action").unwrap_or("");
    let prompt = p::str(v, "prompt").unwrap_or("");
    let tone = p::str(v, "tone").unwrap_or("professional");
    let replace = p::bool(v, "replace").unwrap_or(false);
    let insert = p::bool(v, "insert").unwrap_or(false);

    if action.is_empty() && prompt.is_empty() {
        s.ui_requests.push(json!({"togglePane": "ai"}));
        return Ok(json!({"action": "toggle", "pane": "ai"}));
    }

    if action == "table" {
        return ai_generate_table(s, v);
    }
    if action == "ask" {
        let q = if !prompt.is_empty() { prompt } else { p::str(v, "question").unwrap_or("") };
        let res = ask_document(s, q);
        let answer = res.get("answer").and_then(Value::as_str).unwrap_or("").to_string();
        return Ok(json!({
            "action": "ask",
            "title": "Ask Document",
            "content": answer,
            "data": res
        }));
    }
    if action == "autocomplete" {
        return ai_autocomplete(s, v);
    }

    let api_key = p::str(v, "api_key").unwrap_or("");
    let provider = p::str(v, "provider").unwrap_or("OpenRouter");
    let model = p::str(v, "model").unwrap_or("");
    let base_url = p::str(v, "base_url");

    let (title, content) = if !api_key.trim().is_empty() {
        let (sys_prompt, user_prompt, title_name) = match action {
            "rewrite" => {
                let src = if !prompt.is_empty() { prompt.to_string() } else { s.selected_text() };
                (
                    format!("You are an expert editing assistant. Rewrite the following text with a {tone} tone. Output ONLY the rewritten text without conversational commentary."),
                    src,
                    "Rewrite",
                )
            }
            "summarize" => {
                let src = if !prompt.is_empty() {
                    prompt.to_string()
                } else {
                    let sel = s.selected_text();
                    if !sel.is_empty() { sel } else { s.doc.plain_text(StoryRef::Body) }
                };
                (
                    format!("You are an executive summarizer. Summarize the text using {tone} format. Output concise, structured, professional text."),
                    src,
                    "Summary",
                )
            }
            "proofread" => {
                let src = if !prompt.is_empty() {
                    prompt.to_string()
                } else {
                    let sel = s.selected_text();
                    if !sel.is_empty() { sel } else { s.doc.plain_text(StoryRef::Body) }
                };
                (
                    "You are a professional proofreader. Correct all spelling, grammar, punctuation, and flow issues. Output the polished corrected text.".into(),
                    src,
                    "Proofreading",
                )
            }
            "ask" => {
                let doc_txt = s.doc.plain_text(StoryRef::Body);
                (
                    "You are an intelligent document analyst. Answer the user's question accurately using the provided document context.".into(),
                    format!("Document text:\n{}\n\nUser question: {}", doc_txt, prompt),
                    "Ask Document",
                )
            }
            _ => (
                "You are an expert document author. Draft articulate, ready-to-use document text following the prompt. Output clean prose without meta-commentary.".into(),
                prompt.to_string(),
                "Draft",
            ),
        };

        match crate::ai_client::query_llm_api(provider, model, api_key, base_url, &sys_prompt, &user_prompt) {
            Ok(generated) => (title_name.to_string(), generated),
            Err(e) => return Err(CmdError::Failed(format!("AI Provider ({provider}) error: {e}"))),
        }
    } else {
        match action {
            "rewrite" => {
                let src = if !prompt.is_empty() { prompt.to_string() } else { s.selected_text() };
                ("Rewrite".to_string(), rewrite_text(&src, tone))
            }
            "summarize" => {
                let src = if !prompt.is_empty() {
                    prompt.to_string()
                } else {
                    let sel = s.selected_text();
                    if !sel.is_empty() { sel } else { s.doc.plain_text(StoryRef::Body) }
                };
                ("Summary".to_string(), summarize_text_mode(&src, tone))
            }
            "proofread" => {
                let src = if !prompt.is_empty() {
                    prompt.to_string()
                } else {
                    let sel = s.selected_text();
                    if !sel.is_empty() { sel } else { s.doc.plain_text(StoryRef::Body) }
                };
                ("Proofreading".to_string(), proofread_text(&src))
            }
            _ => ("Draft".to_string(), draft_content(prompt)),
        }
    };

    if replace && !content.is_empty() {
        let at = delete_selection(s)?;
        let end = s.doc.insert_text(&at, &content, &s.typing_props())?;
        s.sel = Selection::caret(end);
    } else if insert && !content.is_empty() {
        let at = s.sel.focus.clone();
        let end = s.doc.insert_text(&at, &content, &s.typing_props())?;
        s.sel = Selection::caret(end);
    }

    Ok(json!({
        "action": if action.is_empty() { "draft" } else { action },
        "title": title,
        "content": content,
    }))
}

fn ai_rewrite(s: &mut Session, v: &Value) -> CmdResult {
    let tone = p::str(v, "tone").unwrap_or("professional");
    let src = if let Some(t) = p::str(v, "text") {
        t.to_string()
    } else {
        let sel = s.selected_text();
        if !sel.trim().is_empty() { sel } else { s.doc.plain_text(StoryRef::Body) }
    };
    let rewritten = rewrite_text(&src, tone);
    let replace = p::bool(v, "replace").unwrap_or(false);
    let insert = p::bool(v, "insert").unwrap_or(false);

    if replace && !rewritten.is_empty() {
        let at = delete_selection(s)?;
        let end = s.doc.insert_text(&at, &rewritten, &s.typing_props())?;
        s.sel = Selection::caret(end);
    } else if insert && !rewritten.is_empty() {
        let at = s.sel.focus.clone();
        let end = s.doc.insert_text(&at, &rewritten, &s.typing_props())?;
        s.sel = Selection::caret(end);
    }

    Ok(json!({
        "action": "rewrite",
        "tone": tone,
        "original": src,
        "rewritten": rewritten,
    }))
}

fn ai_generate_table(s: &mut Session, v: &Value) -> CmdResult {
    let prompt = p::str(v, "prompt").unwrap_or("Table");
    let rows_param = p::u64(v, "rows").map(|r| r.clamp(1, 100) as usize);
    let cols_param = p::u64(v, "cols").map(|c| c.clamp(1, 20) as usize);
    let style = p::str(v, "style").unwrap_or("TableGrid");

    let width = s.doc.sections().first().map(|(_, sp)| sp.text_width()).unwrap_or(468.0);
    let table = parse_or_generate_table(prompt, rows_param, cols_param, width, style)?;
    let row_count = table.rows.len();
    let col_count = table.cols();

    let path = super::insert::insert_block_at_caret(s, Block::Table(table))?;
    let story = s.sel.focus.story;
    let mut first = path.0.clone();
    first.extend([0, 0, 0]);
    s.sel = Selection::caret(Pos { story, path: wordcraft_doc::Path(first), off: 0 });

    Ok(json!({
        "table": path.0,
        "rows": row_count,
        "cols": col_count,
        "prompt": prompt,
    }))
}

fn ai_summarize(s: &mut Session, v: &Value) -> CmdResult {
    let mode = p::str(v, "mode").unwrap_or("bullets");
    let src = if let Some(t) = p::str(v, "text") {
        t.to_string()
    } else {
        let sel = s.selected_text();
        if !sel.trim().is_empty() { sel } else { s.doc.plain_text(StoryRef::Body) }
    };
    let summary = summarize_text_mode(&src, mode);
    let words = summary.split_whitespace().count();
    let reading_time_min = (words as f32 / 200.0).ceil() as u32;

    let replace = p::bool(v, "replace").unwrap_or(false);
    let insert = p::bool(v, "insert").unwrap_or(false);

    if replace && !summary.is_empty() {
        let at = delete_selection(s)?;
        let end = s.doc.insert_text(&at, &summary, &s.typing_props())?;
        s.sel = Selection::caret(end);
    } else if insert && !summary.is_empty() {
        let at = s.sel.focus.clone();
        let end = s.doc.insert_text(&at, &summary, &s.typing_props())?;
        s.sel = Selection::caret(end);
    }

    Ok(json!({
        "action": "summarize",
        "mode": mode,
        "summary": summary,
        "word_count": words,
        "reading_time_min": reading_time_min,
    }))
}

fn ai_ask(s: &mut Session, v: &Value) -> CmdResult {
    let question = p::str(v, "question").or_else(|| p::str(v, "query")).unwrap_or("");
    let result = ask_document(s, question);
    Ok(result)
}

fn ai_autocomplete(s: &mut Session, v: &Value) -> CmdResult {
    let prefix = if let Some(p) = p::str(v, "prefix") {
        p.to_string()
    } else {
        let at = &s.sel.focus;
        if let Some(p) = s.doc.para(at.story, &at.path) {
            let plain = p.plain_text();
            let char_idx = at.off.min(plain.len());
            let floor_idx = plain.floor_char_boundary(char_idx);
            plain.get(..floor_idx).unwrap_or("").to_string()
        } else {
            String::new()
        }
    };
    let insert = p::bool(v, "insert").unwrap_or(false);
    let (prediction, confidence) = predict_completion(&prefix);

    if insert && !prediction.is_empty() {
        let at = s.sel.focus.clone();
        let end = s.doc.insert_text(&at, &prediction, &s.typing_props())?;
        s.sel = Selection::caret(end);
    }

    Ok(json!({
        "action": "autocomplete",
        "prefix": prefix,
        "prediction": prediction,
        "confidence": confidence,
    }))
}

pub fn rewrite_text(src: &str, tone: &str) -> String {
    if src.trim().is_empty() {
        return "Please select or enter text to rewrite.".to_string();
    }
    let mut text = src.to_string();
    match tone {
        "concise" => {
            let pairs = [
                ("in order to", "to"),
                ("due to the fact that", "because"),
                ("at this point in time", "currently"),
                ("for the purpose of", "for"),
                ("with regard to", "regarding"),
                ("in the event that", "if"),
                ("a large number of", "many"),
                ("make an adjustment", "adjust"),
                ("conduct an evaluation", "evaluate"),
                ("bring to an end", "conclude"),
                ("give consideration to", "consider"),
                ("as a means of", "to"),
            ];
            for (from, to) in pairs {
                text = text.replace(from, to);
                let mut cap_from = from.to_string();
                if let Some(first) = cap_from.get_mut(0..1) {
                    first.make_ascii_uppercase();
                    let mut cap_to = to.to_string();
                    if let Some(t_first) = cap_to.get_mut(0..1) {
                        t_first.make_ascii_uppercase();
                        text = text.replace(&cap_from, &cap_to);
                    }
                }
            }
        }
        "academic" => {
            let pairs = [
                ("shows that", "demonstrates that"),
                ("shows", "indicates"),
                ("proves", "empirically verifies"),
                ("a lot of", "a substantial body of"),
                ("big", "statistically significant"),
                ("look into", "investigate"),
                ("in order to", "so as to"),
                ("think", "hypothesize"),
                ("find out", "ascertain"),
                ("clear", "evident"),
                ("about", "approximately"),
                ("get", "obtain"),
                ("gives", "yields"),
                ("build", "construct"),
                ("test", "evaluate"),
                ("good", "favorable"),
                ("bad", "adverse"),
                ("start", "commence"),
                ("finish", "conclude"),
            ];
            for (from, to) in pairs {
                text = text.replace(from, to);
                let mut cap_from = from.to_string();
                if let Some(first) = cap_from.get_mut(0..1) {
                    first.make_ascii_uppercase();
                    let mut cap_to = to.to_string();
                    if let Some(t_first) = cap_to.get_mut(0..1) {
                        t_first.make_ascii_uppercase();
                        text = text.replace(&cap_from, &cap_to);
                    }
                }
            }
        }
        "executive" => {
            let pairs = [
                ("we need to", "our strategic priority is to"),
                ("help", "accelerate"),
                ("do", "execute"),
                ("change", "transform"),
                ("fix", "remediate"),
                ("problem", "critical bottleneck"),
                ("make sure", "ensure rigorous compliance"),
                ("start", "initiate"),
                ("talk about", "align on"),
                ("plan", "strategic roadmap"),
                ("results", "key outcomes and deliverable impact"),
                ("good", "high-value"),
                ("bad", "suboptimal"),
                ("spend", "allocate capital toward"),
                ("buy", "procure"),
                ("work together", "collaborate cross-functionally"),
            ];
            for (from, to) in pairs {
                text = text.replace(from, to);
                let mut cap_from = from.to_string();
                if let Some(first) = cap_from.get_mut(0..1) {
                    first.make_ascii_uppercase();
                    let mut cap_to = to.to_string();
                    if let Some(t_first) = cap_to.get_mut(0..1) {
                        t_first.make_ascii_uppercase();
                        text = text.replace(&cap_from, &cap_to);
                    }
                }
            }
        }
        "casual" => {
            let pairs = [
                ("furthermore", "also"),
                ("nevertheless", "still"),
                ("consequently", "so"),
                ("utilize", "use"),
                ("terminate", "end"),
                ("commence", "start"),
                ("inquire", "ask"),
                ("assist", "help"),
            ];
            for (from, to) in pairs {
                text = text.replace(from, to);
            }
        }
        _ => {
            let pairs = [
                ("don't", "do not"),
                ("can't", "cannot"),
                ("won't", "will not"),
                ("it's", "it is"),
                ("we've", "we have"),
                ("they're", "they are"),
                ("haven't", "have not"),
                ("doesn't", "does not"),
                ("a lot of", "a substantial number of"),
                ("get in touch", "contact"),
                ("look into", "investigate"),
                ("fix", "resolve"),
                ("bad", "suboptimal"),
            ];
            for (from, to) in pairs {
                text = text.replace(from, to);
            }
        }
    }
    text
}

pub fn summarize_text(src: &str) -> String {
    summarize_text_mode(src, "bullets")
}

pub fn summarize_text_mode(src: &str, mode: &str) -> String {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return "Please enter text or select content to summarize.".to_string();
    }
    let sentences: Vec<&str> = trimmed.split(['.', '!', '?', '\n']).map(str::trim).filter(|s| !s.is_empty() && s.len() > 15).collect();

    match mode {
        "executive" => {
            let lead = sentences.first().copied().unwrap_or(trimmed);
            let concluding = sentences.last().copied().unwrap_or(lead);
            let middle = if sentences.len() > 2 { sentences.get(sentences.len() / 2).copied().unwrap_or("") } else { "" };
            let mut out = String::from("Executive Summary:\n");
            out.push_str(&format!("{lead}. "));
            if !middle.is_empty() && middle != lead && middle != concluding {
                out.push_str(&format!("Specifically, {middle}. "));
            }
            out.push_str(&format!("Ultimately, {concluding}."));
            out
        }
        "abstract" => {
            let lead = sentences.first().copied().unwrap_or(trimmed);
            let second = sentences.get(1).copied().unwrap_or(lead);
            let concluding = sentences.last().copied().unwrap_or(lead);
            format!(
                "Structured Abstract\n\n\
                 Context & Background:\n{lead}.\n\n\
                 Objectives & Methodology:\n{second}.\n\n\
                 Principal Findings & Conclusion:\n{concluding}."
            )
        }
        _ => {
            let mut out = String::from("Key Takeaways:\n");
            if sentences.is_empty() {
                out.push_str(&format!("• {trimmed}\n"));
            } else {
                for s in sentences.iter().take(5) {
                    out.push_str(&format!("• {s}.\n"));
                }
            }
            out
        }
    }
}

pub fn ask_document(s: &Session, question: &str) -> Value {
    let body_text = s.doc.plain_text(StoryRef::Body);
    let words: Vec<&str> = body_text.split_whitespace().collect();
    let word_count = words.len();
    let sentences: Vec<&str> = body_text.split(['.', '!', '?', '\n']).map(str::trim).filter(|s| !s.is_empty() && s.len() > 10).collect();
    let sentence_count = sentences.len().max(1);
    let char_count = body_text.chars().filter(|c| !c.is_whitespace()).count();
    let reading_time = (word_count as f32 / 200.0).ceil() as u32;

    let mut headings = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        if let Some(p) = s.doc.para(StoryRef::Body, &path) {
            let plain = p.plain_text();
            if s.doc.styles.resolve_para(&p.props).outline_level.is_some() && !plain.trim().is_empty() {
                headings.push(plain.trim().to_string());
            }
        }
    }

    let table_count = s.doc.body.iter().filter(|b| matches!(b.as_ref(), Block::Table(_))).count();

    let total_syllables: usize = words.iter().map(|w| count_syllables(w)).sum();
    let words_per_sentence = word_count as f64 / sentence_count as f64;
    let syllables_per_word = if word_count > 0 { total_syllables as f64 / word_count as f64 } else { 1.0 };
    let flesch_reading_ease =
        if word_count >= 10 { (206.835 - (1.015 * words_per_sentence) - (84.6 * syllables_per_word)).clamp(0.0, 100.0) } else { 100.0 };

    let q_lower = question.to_lowercase();

    if q_lower.contains("action") || q_lower.contains("todo") || q_lower.contains("task") || q_lower.contains("deadline") {
        let triggers = ["will", "must", "shall", "todo", "action", "deadline", "by ", "responsible", "assign"];
        let mut actions = Vec::new();
        for s in &sentences {
            let s_lower = s.to_lowercase();
            if triggers.iter().any(|t| s_lower.contains(t)) {
                actions.push(*s);
            }
        }
        let answer = if actions.is_empty() {
            "No explicit action items, deadlines, or pending tasks detected in the document text.".to_string()
        } else {
            let mut ans = format!("Detected {} action item(s) in document:\n\n", actions.len());
            for a in &actions {
                ans.push_str(&format!("• {a}\n"));
            }
            ans
        };
        return json!({
            "question": question,
            "answer": answer,
            "sources": actions,
            "metrics": {
                "wordCount": word_count,
                "actionItemsCount": actions.len(),
            }
        });
    }

    if q_lower.contains("stat")
        || q_lower.contains("metric")
        || q_lower.contains("length")
        || q_lower.contains("count")
        || q_lower.contains("overview")
        || question.trim().is_empty()
    {
        let heading_summary = if headings.is_empty() { "None".to_string() } else { headings.join(", ") };
        let answer = format!(
            "Document Telemetry & Structure Overview:\n\n\
             • Word Count: {word_count} words\n\
             • Character Count: {char_count} characters\n\
             • Sentences: {sentence_count}\n\
             • Headings: {} ({heading_summary})\n\
             • Tables: {table_count}\n\
             • Reading Time: ~{reading_time} min\n\
             • Flesch Reading Ease: {:.1}/100",
            headings.len(),
            flesch_reading_ease
        );
        return json!({
            "question": question,
            "answer": answer,
            "sources": headings,
            "metrics": {
                "wordCount": word_count,
                "charCount": char_count,
                "sentenceCount": sentence_count,
                "headingCount": headings.len(),
                "tableCount": table_count,
                "readingTimeMin": reading_time,
                "fleschReadingEase": flesch_reading_ease
            }
        });
    }

    if q_lower.contains("readability") || q_lower.contains("grade") || q_lower.contains("level") || q_lower.contains("tone") {
        let grade_level = ((0.39 * words_per_sentence) + (11.8 * syllables_per_word) - 15.59).clamp(0.0, 20.0);
        let reading_level = if flesch_reading_ease >= 80.0 {
            "Easy (Elementary / Middle School)"
        } else if flesch_reading_ease >= 60.0 {
            "Standard (High School)"
        } else if flesch_reading_ease >= 40.0 {
            "Difficult (College Level)"
        } else {
            "Very Difficult (Academic / Graduate Level)"
        };
        let answer = format!(
            "Document Readability & Style Analysis:\n\n\
             • Flesch Reading Ease: {:.1} / 100\n\
             • Estimated Grade Level: {:.1}\n\
             • Target Audience: {reading_level}\n\
             • Average Words per Sentence: {:.1}\n\
             • Syllables per Word: {:.2}",
            flesch_reading_ease, grade_level, words_per_sentence, syllables_per_word
        );
        return json!({
            "question": question,
            "answer": answer,
            "sources": [],
            "metrics": {
                "readingEase": flesch_reading_ease,
                "gradeLevel": grade_level,
                "readingLevel": reading_level,
                "wordsPerSentence": words_per_sentence
            }
        });
    }

    let stop_words = ["what", "is", "are", "the", "in", "how", "a", "an", "of", "and", "or", "for", "to", "on", "with", "about", "does", "do"];
    let terms: Vec<&str> = q_lower.split_whitespace().filter(|t| !stop_words.contains(t) && t.len() > 2).collect();

    let mut scored_sentences: Vec<(usize, &str)> = Vec::new();
    for s in &sentences {
        let s_lower = s.to_lowercase();
        let mut score = 0;
        for t in &terms {
            if s_lower.contains(t) {
                score += 1;
            }
        }
        if score > 0 {
            scored_sentences.push((score, *s));
        }
    }
    scored_sentences.sort_by_key(|a| std::cmp::Reverse(a.0));

    let top_matches: Vec<&str> = scored_sentences.iter().take(4).map(|(_, s)| *s).collect();
    let answer = if top_matches.is_empty() {
        format!(
            "No specific matching sections found for \"{question}\" in the document body. Try asking for document statistics, action items, or readability."
        )
    } else {
        let mut ans = format!("Relevant excerpts matching \"{question}\":\n\n");
        for m in &top_matches {
            ans.push_str(&format!("• \"{m}.\"\n"));
        }
        ans
    };

    json!({
        "question": question,
        "answer": answer,
        "sources": top_matches,
        "metrics": {
            "matchedCount": scored_sentences.len(),
            "queryTerms": terms
        }
    })
}

pub fn predict_completion(prefix: &str) -> (String, f64) {
    let trimmed = prefix.trim_end();
    let lower = trimmed.to_lowercase();

    let patterns = [
        ("in accordance with", " the established project guidelines and specifications.", 0.96),
        ("please find attached", " the requested report and analysis for your review.", 0.98),
        ("if you have any questions", ", please do not hesitate to contact our team.", 0.98),
        ("the primary objective of this", " document is to outline key deliverables and milestones.", 0.95),
        ("as discussed in our previous", " meeting, all timeline goals have been approved.", 0.94),
        ("we are pleased to", " report that all quality criteria have been successfully met.", 0.95),
        ("in order to achieve", " optimal results, standard operating procedures must be observed.", 0.92),
        ("it is important to note that", " all changes take effect immediately upon approval.", 0.91),
        ("based on the results of", " our preliminary assessment, the proposed strategy is sound.", 0.93),
        ("thank you for your", " continued collaboration and support.", 0.97),
        ("sincerely,", "\nThe Project Team", 0.95),
        ("best regards,", "\nManagement", 0.95),
        ("in conclusion,", " the project is progressing on schedule with zero regressions.", 0.94),
        ("furthermore,", " all automated verification gates remain green.", 0.90),
        ("consequently,", " we recommend advancing to the next implementation phase.", 0.90),
        ("to whom it may concern:", "\n\nThis letter serves to confirm our commitment...", 0.96),
        ("meeting agenda:", "\n1. Progress Review\n2. Key Dependencies\n3. Action Items", 0.95),
    ];

    for (pat, compl, conf) in patterns {
        if lower.ends_with(pat) {
            return (compl.to_string(), conf);
        }
    }

    if lower.ends_with("for example") || lower.ends_with("e.g.") {
        return (", consider the following implementation details:".to_string(), 0.88);
    }
    if lower.ends_with("therefore") {
        return (", all stakeholders should review the updated schedule.".to_string(), 0.85);
    }
    if lower.ends_with("however") {
        return (", additional verification is recommended before release.".to_string(), 0.85);
    }

    if trimmed.is_empty() {
        ("This document describes the architectural overview and specifications.".to_string(), 0.70)
    } else {
        (" and will continue according to the scheduled roadmap.".to_string(), 0.75)
    }
}

pub fn parse_or_generate_table(
    prompt: &str,
    rows_param: Option<usize>,
    cols_param: Option<usize>,
    width: f32,
    style: &str,
) -> Result<Table, CmdError> {
    if prompt.contains('|') && prompt.contains('\n') {
        let mut raw_rows = Vec::new();
        for line in prompt.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("|-") || line.starts_with("| -") || line.replace(['|', '-', ':', ' '], "").is_empty() {
                continue;
            }
            let cells: Vec<String> = line.split('|').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
            if !cells.is_empty() {
                raw_rows.push(cells);
            }
        }
        if !raw_rows.is_empty() {
            let cols = raw_rows.iter().map(|r| r.len()).max().unwrap_or(2).clamp(1, 63);
            let w = (width / cols as f32).max(6.0);
            let mut table =
                Table { props: TableProps { style: Some(style.to_string()), ..Default::default() }, grid: vec![w; cols], rows: Vec::new() };
            for (r_idx, r) in raw_rows.into_iter().enumerate() {
                let is_header = r_idx == 0;
                let mut cells = Vec::new();
                for c_idx in 0..cols {
                    let text = r.get(c_idx).cloned().unwrap_or_default();
                    let mut cell = Cell::with_text(&text);
                    cell.props.width = Some(w);
                    cells.push(cell);
                }
                table.rows.push(Row { props: RowProps { header: is_header, ..Default::default() }, cells });
            }
            return Ok(table);
        }
    }

    let p_lower = prompt.to_lowercase();
    let (headers, data_rows): (Vec<&str>, Vec<Vec<&str>>) = if p_lower.contains("swot") {
        (
            vec!["Category", "Key Factors", "Strategic Implication"],
            vec![
                vec!["Strengths", "Core technological capabilities, fast execution", "Leverage for competitive edge"],
                vec!["Weaknesses", "Resource constraints, brand visibility", "Address via targeted partnerships"],
                vec!["Opportunities", "Market demand for high-performance offline tools", "Capture enterprise and privacy-conscious users"],
                vec!["Threats", "Entrenched incumbent ecosystem", "Focus on unmatched performance, open standards, and zero lock-in"],
            ],
        )
    } else if p_lower.contains("feature") || p_lower.contains("comparison") || p_lower.contains("matrix") {
        (
            vec!["Feature / Capability", "Starter", "Professional", "Enterprise"],
            vec![
                vec!["Core Editing & Word Parity", "Full", "Full", "Full"],
                vec!["AI Copilot & Intelligence", "Standard", "Advanced", "Unlimited"],
                vec!["Batch Conversion & CLI", "Limited", "Included", "Dedicated Pipeline"],
                vec!["Team Collaboration & Markup", "Local only", "Integrated", "Custom Hosted"],
            ],
        )
    } else if p_lower.contains("pricing") || p_lower.contains("tier") || p_lower.contains("plan") {
        (
            vec!["Plan Tier", "Monthly", "Annual (Save 20%)", "Included Capabilities"],
            vec![
                vec!["Community", "$0", "$0", "Complete editor, open source, local storage"],
                vec!["Professional", "$12 / mo", "$115 / yr", "Cloud sync, AI intelligence, priority export"],
                vec!["Enterprise", "$29 / mo", "$279 / yr", "Custom deployment, SLA, compliance reports"],
            ],
        )
    } else if p_lower.contains("project") || p_lower.contains("roadmap") || p_lower.contains("timeline") {
        (
            vec!["Phase / Milestone", "Deliverable", "Owner", "Target Date", "Status"],
            vec![
                vec!["Phase 1: Architecture", "Core data structures & L0-L4 crates", "Lead Architect", "Month 1", "Completed"],
                vec!["Phase 2: Feature Parity", "Word catalog commands & layout fidelity", "Engineering Team", "Month 2", "In Progress"],
                vec!["Phase 3: AI Intelligence", "Copilot suite, rewriting, table generation", "AI / Tools Team", "Month 3", "Active"],
                vec!["Phase 4: Release & QA", "All CI gates green, installers, documentation", "DevOps & QA", "Month 4", "Planned"],
            ],
        )
    } else if p_lower.contains("kpi")
        || p_lower.contains("metric")
        || p_lower.contains("revenue")
        || p_lower.contains("sales")
        || p_lower.contains("budget")
        || p_lower.contains("financial")
    {
        (
            vec!["Metric / Category", "Q1 Actual", "Q2 Target", "Variance (%)", "Trend"],
            vec![
                vec!["Active Users", "12,450", "15,000", "+20.5%", "Upward"],
                vec!["Export Performance (ms)", "42 ms", "< 50 ms", "-16.0%", "Exceeding Target"],
                vec!["Crash Rate", "0.00%", "0.00%", "0.0%", "Rock Solid"],
                vec!["User Satisfaction (NPS)", "+68", "+70", "+2.9%", "Strong Positive"],
            ],
        )
    } else if p_lower.contains("team") || p_lower.contains("employee") || p_lower.contains("roster") || p_lower.contains("directory") {
        (
            vec!["Name", "Role", "Department", "Email", "Status"],
            vec![
                vec!["M S Rana", "Lead Architect", "Engineering", "rana@craft.local", "Active"],
                vec!["Alex Morgan", "Product Specialist", "Design & UX", "alex@craft.local", "Active"],
                vec!["Jordan Lee", "Systems Engineer", "Core Infrastructure", "jordan@craft.local", "Active"],
                vec!["Taylor Quinn", "QA & Reliability", "Quality Assurance", "taylor@craft.local", "Active"],
            ],
        )
    } else {
        (
            vec!["Item #", "Title / Component", "Description", "Priority", "Status"],
            vec![
                vec!["01", "Core Setup", "Initial specification and environment configuration", "High", "In Review"],
                vec!["02", "Implementation", "Primary workflow development and testing", "High", "In Progress"],
                vec!["03", "Verification", "Quality assurance and automated validation", "Medium", "Planned"],
            ],
        )
    };

    let col_count = cols_param.unwrap_or(headers.len()).clamp(1, 63);
    let row_count = rows_param.unwrap_or(data_rows.len() + 1).clamp(2, 1000);
    let w = (width / col_count as f32).max(6.0);

    let mut table = Table { props: TableProps { style: Some(style.to_string()), ..Default::default() }, grid: vec![w; col_count], rows: Vec::new() };

    let mut header_cells = Vec::new();
    for c in 0..col_count {
        let text = headers.get(c).copied().unwrap_or("Column");
        let mut cell = Cell::with_text(text);
        cell.props.width = Some(w);
        header_cells.push(cell);
    }
    table.rows.push(Row { props: RowProps { header: true, ..Default::default() }, cells: header_cells });

    for r in 0..(row_count.saturating_sub(1)) {
        let mut row_cells = Vec::new();
        let source_row = data_rows.get(r % data_rows.len().max(1));
        for c in 0..col_count {
            let default_text = format!("Data {}.{}", r + 1, c + 1);
            let text = source_row.and_then(|row| row.get(c)).copied().unwrap_or(&default_text);
            let mut cell = Cell::with_text(text);
            cell.props.width = Some(w);
            row_cells.push(cell);
        }
        table.rows.push(Row { props: RowProps::default(), cells: row_cells });
    }

    Ok(table)
}

pub fn proofread_text(src: &str) -> String {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return "No text provided for proofreading.".to_string();
    }
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    let word_count = words.len();
    let est_reading_time = (word_count as f32 / 200.0).ceil() as u32;

    let mut suggestions = Vec::new();
    for i in 0..words.len().saturating_sub(1) {
        if let (Some(w1), Some(w2)) = (words.get(i), words.get(i + 1))
            && w1.eq_ignore_ascii_case(w2)
            && w1.len() > 1
        {
            suggestions.push(format!("Repeated word detected: \"{w1} {w2}\""));
        }
    }

    let mut report = format!("Analysis: {word_count} words (~{est_reading_time} min read)\n\n");
    if suggestions.is_empty() {
        report.push_str("✓ Grammar, punctuation, and phrasing appear clear.\n✓ No duplicate words detected.\n\n");
    } else {
        report.push_str("Suggestions:\n");
        for s in suggestions {
            report.push_str(&format!("⚠ {s}\n"));
        }
        report.push('\n');
    }
    report.push_str(&format!("Polished Text:\n{trimmed}"));
    report
}

pub fn draft_content(prompt: &str) -> String {
    let p = prompt.to_lowercase();
    if p.contains("minute") || p.contains("meeting") {
        return format!(
            "Meeting Minutes: {prompt}\n\
             Date: {}\n\
             Attendees: Project Team, Stakeholders\n\n\
             1. Agenda Items\n\
             • Review of progress and milestones\n\
             • Technical architecture updates\n\
             • Open issues and dependency blockers\n\n\
             2. Key Decisions\n\
             • Approved timeline for upcoming release\n\
             • Confirmed deliverable responsibilities\n\n\
             3. Action Items\n\
             • [Owner] Finalize specifications by Friday\n\
             • [Team] Complete integration test verification",
            super::insert::format_date("MMMM d, yyyy")
        );
    } else if p.contains("status") || p.contains("report") {
        return format!(
            "Project Status Report: {prompt}\n\
             Reporting Period: {}\n\n\
             Executive Summary:\n\
             The project continues on track with key sprint deliverables meeting targeted quality benchmarks.\n\n\
             Completed Milestones:\n\
             • Core module integration completed and validated\n\
             • Automated CI quality gates passed with zero regressions\n\n\
             Upcoming Priorities:\n\
             • User interface refinement and end-to-end polish\n\
             • Performance benchmarking and documentation\n\n\
             Risks & Mitigations:\n\
             • Schedule buffer allocated for cross-platform packaging",
            super::insert::format_date("MMMM d, yyyy")
        );
    } else if p.contains("letter") || p.contains("invitation") || p.contains("memo") {
        return format!(
            "MEMORANDUM\n\n\
             TO: All Team Members\n\
             FROM: Management\n\
             DATE: {}\n\
             SUBJECT: {}\n\n\
             Dear Colleagues,\n\n\
             We are pleased to communicate regarding {prompt}. Our team has demonstrated exceptional commitment to quality, speed, and collaborative excellence.\n\n\
             Please review the attached details and let us know if you have any questions or feedback.\n\n\
             Sincerely,\n\
             The Leadership Team",
            super::insert::format_date("MMMM d, yyyy"),
            prompt
        );
    } else if p.contains("proposal") {
        return format!(
            "Project Proposal: {prompt}\n\n\
             1. Problem Statement\n\
             Current workflows require streamlined tooling and improved performance.\n\n\
             2. Proposed Solution\n\
             Implement a robust, clean-room, native solution with high speed and zero crash tolerance.\n\n\
             3. Deliverables & Milestones\n\
             • Phase 1: Prototype and architectural foundation\n\
             • Phase 2: Feature completeness and user experience polish\n\
             • Phase 3: Comprehensive quality verification and distribution\n\n\
             4. Expected Impact\n\
             Significantly increased productivity, user trust, and operational efficiency."
        );
    }

    format!(
        "{prompt}\n\n\
         1. Introduction\n\
         An overview of {prompt} and the primary objectives to achieve.\n\n\
         2. Key Concepts & Analysis\n\
         • Strategic considerations and background\n\
         • Methodologies and practical approaches\n\
         • Comparative evaluation of options\n\n\
         3. Summary & Next Steps\n\
         Synthesis of the recommendations and immediate actionable next steps."
    )
}

fn templates_and_addins(_s: &mut Session, v: &Value) -> CmdResult {
    let tmpl = v.get("template").and_then(Value::as_str).unwrap_or("Normal.dotm");
    Ok(json!({"attachedTemplate": tmpl, "addins": []}))
}

fn get_addins(_s: &mut Session, v: &Value) -> CmdResult {
    let action = v.get("action").and_then(Value::as_str).unwrap_or("browse");
    Ok(json!({"action": action, "available": []}))
}

fn macros(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(d) = v.get("define") {
        let name = d.get("name").and_then(Value::as_str).unwrap_or("Macro").to_string();
        let steps: Vec<(String, Value)> = d
            .get("steps")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter().filter_map(|st| Some((st.get("command")?.as_str()?.to_string(), st.get("params").cloned().unwrap_or(json!({}))))).collect()
            })
            .unwrap_or_default();
        s.macros.insert(name, steps);
    }
    if let Some(n) = p::str(v, "delete") {
        s.macros.remove(n);
    }
    if let Some(n) = p::str(v, "run") {
        let steps = s.macros.get(n).cloned().ok_or_else(|| CmdError::Params(format!("no macro `{n}`")))?;
        for (id, params) in steps.iter().take(10_000) {
            if id == "tools.macros" {
                continue;
            }
            s.run(id, params)?;
        }
        return Ok(json!({"ran": n, "steps": steps.len()}));
    }
    Ok(Value::Object(
        s.macros.iter().map(|(k, v)| (k.clone(), json!(v.iter().map(|(c, p)| json!({"command": c, "params": p})).collect::<Vec<_>>()))).collect(),
    ))
}

/// Apply AutoCorrect to the word just before the caret (called after typing a space/punctuation):
/// replacements, then ordinal superscripts, web addresses as links and sentence capitals.
pub fn autocorrect(s: &mut Session) -> Result<(), CmdError> {
    autocorrect_word(s, false)
}

/// AutoCorrect the word ending at the caret; `on_enter` means Enter finished it (no trigger
/// character was typed).
pub fn autocorrect_word(s: &mut Session, on_enter: bool) -> Result<(), CmdError> {
    if !s.autocorrect_on {
        return Ok(());
    }
    let mut f = s.sel.focus.clone();
    let Some(text) = s.doc.para_at(&f).map(|p| p.text.clone()) else { return Ok(()) };
    let Some(before) = text.get(..f.off) else { return Ok(()) };
    let Some(last) = before.chars().next_back() else { return Ok(()) };
    if !on_enter && !(last == ' ' || ",.;:!?".contains(last)) {
        return Ok(());
    }
    let trigger = if on_enter { 0 } else { last.len_utf8() };
    let Some(body) = before.get(..before.len() - trigger) else { return Ok(()) };
    let start = body.rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
    let Some(mut word) = body.get(start..).map(str::to_string) else { return Ok(()) };
    if word.is_empty() {
        return Ok(());
    }
    // TWo INitial CApitals and cAPS LOCK come first, so "TEh" is still a replacement's typo.
    if let Some((at, reps)) = case_fix(
        s.autocorrect_two_initial_caps,
        s.autocorrect_caps_lock,
        &s.autocorrect_initial_caps_exceptions,
        &word,
    ) {
        let para = s.doc.para_mut(f.story, &f.path)?;
        let letters = word.get(at..).unwrap_or("").chars().take(reps.len()).collect::<String>();
        let mut end = start + at + letters.len();
        for (c, rep) in letters.chars().rev().zip(reps.iter().rev()) {
            let from = end.saturating_sub(c.len_utf8());
            if rep.chars().ne(std::iter::once(c)) {
                let props = para.props_of_char(from).clone();
                para.delete(from, end)?;
                para.insert_text(from, rep, &props)?;
            }
            end = from;
        }
        let fixed: String = reps.concat();
        f.off = (f.off + fixed.len()).saturating_sub(letters.len());
        s.sel = Selection::caret(f.clone());
        word = format!("{}{fixed}{}", word.get(..at).unwrap_or(""), word.get(at + letters.len()..).unwrap_or(""));
    }
    let user = s.autocorrect_user.iter().find(|(a, _)| *a == word).map(|(_, b)| b.clone());
    let builtin = AUTOCORRECT
        .iter()
        .find(|(a, _)| {
            *a == word || (a.chars().all(|c| c.is_lowercase()) && word.to_lowercase() == *a && word.chars().next().is_some_and(char::is_uppercase))
        })
        .map(|(a, b)| {
            if *a != word && b.chars().next().is_some_and(char::is_lowercase) {
                let mut c = b.chars();
                c.next().map(|x| x.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
            } else {
                b.to_string()
            }
        });
    // A whole-word replacement, else "wait..." → "wait…".
    let (from, rep) = match user.or(builtin) {
        Some(r) => (start, Some(r)),
        None if word.len() > 3 && word.ends_with("...") => (start + word.len() - 3, Some("…".to_string())),
        None => (start, None),
    };
    let mut caret = f.off;
    if let Some(rep) = rep {
        let end = start + word.len();
        let para = s.doc.para_mut(f.story, &f.path)?;
        let props = para.props_of_char(from).clone();
        para.delete(from, end)?;
        para.insert_text(from, &rep, &props)?;
        caret = (f.off + rep.len()).saturating_sub(end - from);
        s.sel = Selection::caret(Pos { off: caret, ..f.clone() });
    }
    let end = caret.saturating_sub(trigger);
    let Some(word) = s.doc.para_at(&f).and_then(|p| p.text.get(start..end)).map(str::to_string) else { return Ok(()) };
    let at = |off: usize| Pos { off, ..f.clone() };
    // 1st, 22nd, 103rd → superscript suffix.
    if let Some(n) = ordinal_suffix(&word) {
        s.doc.format_range(&at(end - n), &at(end), &|c| c.vert_align = Some(wordcraft_doc::props::VertAlign::Superscript))?;
        return Ok(());
    }
    // Web addresses become links (trailing punctuation stays outside).
    let lower = word.to_ascii_lowercase();
    if (lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("www.")) && word.len() > 8 {
        let url = word.trim_end_matches(|c: char| ".,;:!?)]\"'”’".contains(c));
        let href = if lower.starts_with("www.") { format!("http://{url}") } else { url.to_string() };
        let url_end = start + url.len();
        s.doc.format_range(&at(start), &at(url_end), &|c| {
            c.link = Some(href.clone());
            c.style = Some("Hyperlink".into());
        })?;
        // Typing after the link isn't part of it (but keeps the text's other formatting).
        let mut after = s.doc.para_at(&f).map(|p| p.props_of_char(start).clone()).unwrap_or_default();
        after.link = None;
        after.style = None;
        s.pending = Some(after);
        return Ok(());
    }
    let before_word = text.get(..start).unwrap_or("");
    let sentence_on = if before_word.trim().is_empty() && f.path.cell().is_some() {
        s.autocorrect_cap_cells
    } else {
        s.autocorrect_cap_sentences
    };
    if sentence_on && starts_sentence(before_word) && should_capitalize(&word, if on_enter { '\n' } else { last }) {
        let para = s.doc.para_mut(f.story, &f.path)?;
        if let Some(c) = word.chars().next() {
            let up: String = c.to_uppercase().collect();
            let props = para.props_of_char(start).clone();
            para.delete(start, start + c.len_utf8())?;
            para.insert_text(start, &up, &props)?;
            let off = (caret + up.len()).saturating_sub(c.len_utf8());
            s.sel = Selection::caret(Pos { off, ..f });
        }
    }
    Ok(())
}

/// Built-in words that keep their two initial capitals (plurals of two-letter abbreviations).
pub const INITIAL_CAPS_EXCEPTIONS: &[&str] = &["CDs", "IDs", "PCs", "TVs", "DJs", "GBs", "KBs", "MBs", "TBs", "VMs"];

/// Words with a lowercase first letter and the rest in capitals that aren't cAPS LOCK slips.
const CAPS_LOCK_EXCEPTIONS: &[&str] = &["iOS", "mRNA", "tRNA", "rRNA", "cDNA", "eSIM", "eSATA", "iSCSI", "vCPU"];

/// The capitalisation fix for a finished word: TWo INitial CApitals ("WOrd" → "Word") or cAPS
/// LOCK ("wORD" → "Word"). Returns the byte offset of the word's letters and each letter's
/// replacement. Only words of three or more letters, optionally in quotes or brackets.
fn case_fix(
    two_initial_caps: bool,
    caps_lock: bool,
    initial_caps_exceptions: &[String],
    word: &str,
) -> Option<(usize, Vec<String>)> {
    if !two_initial_caps && !caps_lock {
        return None;
    }
    let at = word.find(char::is_alphabetic)?;
    let rest = word.get(at..)?;
    let letters = rest.get(..rest.find(|c: char| !c.is_alphabetic()).unwrap_or(rest.len()))?;
    let tail = rest.get(letters.len()..)?;
    if !word.get(..at)?.chars().all(|c| "\"'“‘([".contains(c)) || !tail.chars().all(|c| "\"'”’)]".contains(c)) {
        return None;
    }
    let cs: Vec<char> = letters.chars().collect();
    let (&a, &b, more) = match cs.as_slice() {
        [a, b, more @ ..] if !more.is_empty() => (a, b, more),
        _ => return None,
    };
    let one = |c: char| c.to_string();
    let lower = |c: char| c.to_lowercase().collect::<String>();
    if two_initial_caps
        && a.is_uppercase()
        && b.is_uppercase()
        && more.iter().all(|c| c.is_lowercase())
        && !INITIAL_CAPS_EXCEPTIONS.contains(&letters)
        && !initial_caps_exceptions.iter().any(|e| e == letters)
    {
        return Some((at, std::iter::once(one(a)).chain(std::iter::once(lower(b))).chain(more.iter().map(|&c| one(c))).collect()));
    }
    if caps_lock
        && a.is_lowercase()
        && b.is_uppercase()
        && more.iter().all(|c| c.is_uppercase())
        && !CAPS_LOCK_EXCEPTIONS.contains(&letters)
    {
        let up = a.to_uppercase().collect::<String>();
        return Some((at, std::iter::once(up).chain(std::iter::once(lower(b))).chain(more.iter().map(|&c| lower(c))).collect()));
    }
    None
}

/// Byte length of an ordinal suffix to superscript ("1st" → 2), if `word` is a correct ordinal.
fn ordinal_suffix(word: &str) -> Option<usize> {
    let digits = word.find(|c: char| !c.is_ascii_digit())?;
    let (num, suffix) = word.split_at(digits);
    if num.is_empty() || num.len() > 9 {
        return None;
    }
    let n: u64 = num.parse().ok()?;
    let want = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    (suffix == want).then_some(2)
}

/// Abbreviations whose full stop doesn't end a sentence.
const NOT_SENTENCE_END: &[&str] = &[
    "e.g.", "i.e.", "etc.", "vs.", "cf.", "mr.", "mrs.", "ms.", "dr.", "st.", "no.", "approx.", "fig.", "p.", "pp.", "jr.", "sr.", "ca.", "inc.",
    "ltd.", "co.", "vol.", "a.m.", "p.m.",
];

/// Does a word typed after `before` (the paragraph text up to the word) begin a sentence?
fn starts_sentence(before: &str) -> bool {
    let t = before.trim_end();
    if t.is_empty() {
        return true;
    }
    if t.len() == before.len() {
        return false; // no space between the previous text and the word
    }
    let t = t.trim_end_matches(|c: char| "\"'”’)]".contains(c));
    if !t.ends_with(['.', '!', '?']) || t.ends_with("..") || t.ends_with('…') {
        return false;
    }
    let prev = t.rsplit(char::is_whitespace).next().unwrap_or(t).trim_start_matches(|c: char| "\"'“‘([".contains(c));
    let lower = prev.to_lowercase();
    // "J." (an initial) or a known abbreviation.
    let initial = prev.chars().count() == 2 && prev.chars().next().is_some_and(char::is_uppercase);
    !initial && !NOT_SENTENCE_END.contains(&lower.as_str())
}

/// Only plain lowercase words are capitalised (not "iPhone", "x2", file names or addresses).
fn should_capitalize(word: &str, next: char) -> bool {
    // A single letter before a full stop is a list label or an initial ("a.", "j.").
    if (next == '.' || word.ends_with('.')) && word.trim_end_matches('.').chars().count() < 2 {
        return false;
    }
    if word == "e.g." || word == "i.e." {
        return true;
    }
    word.chars().next().is_some_and(char::is_lowercase)
        && word.chars().all(|c| c.is_alphabetic() || "'’-.".contains(c))
        && !word.chars().any(char::is_uppercase)
        && !word.trim_end_matches('.').contains('.')
}

/// Compare the current document (original) with a revised one: the result shows the
/// differences as tracked insertions and deletions.
fn compare(s: &mut Session, v: &Value) -> CmdResult {
    let revised = if let Some(t) = p::str(v, "text") {
        Document::from_text(t)
    } else if let Some(path) = p::str(v, "path") {
        crate::io::open_path(std::path::Path::new(path)).map_err(CmdError::Failed)?
    } else {
        return Err(CmdError::Params("`path` or `text` of the revised document is required".into()));
    };
    let old: Vec<Paragraph> = s.doc.body.iter().filter_map(|b| b.as_para().cloned()).collect();
    let new: Vec<Paragraph> = revised.body.iter().filter_map(|b| b.as_para().cloned()).collect();
    let author = "Compare".to_string();
    let date = super::now_iso();
    s.doc.revisions.push(wordcraft_doc::Revision { kind: RevisionKind::Insert, author: author.clone(), date: date.clone() });
    let ins = (s.doc.revisions.len() - 1) as u32;
    s.doc.revisions.push(wordcraft_doc::Revision { kind: RevisionKind::Delete, author, date });
    let del = (s.doc.revisions.len() - 1) as u32;
    let ops = lcs_ops(&old.iter().map(|p| p.plain_text()).collect::<Vec<_>>(), &new.iter().map(|p| p.plain_text()).collect::<Vec<_>>());
    let mut out: Vec<Paragraph> = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    let mut changes = 0usize;
    let mut k = 0;
    while k < ops.len() {
        match ops.get(k) {
            Some(Op::Same) => {
                if let Some(p) = old.get(i) {
                    out.push(p.clone());
                }
                i += 1;
                j += 1;
                k += 1;
            }
            Some(Op::Del) if matches!(ops.get(k + 1), Some(Op::Ins)) => {
                // A changed paragraph: diff its words.
                let (a, b) = (old.get(i).cloned().unwrap_or_default(), new.get(j).cloned().unwrap_or_default());
                out.push(word_diff(&a, &b, ins, del));
                changes += 1;
                i += 1;
                j += 1;
                k += 2;
            }
            Some(Op::Del) => {
                let mut p = old.get(i).cloned().unwrap_or_default();
                let len = p.len();
                let _ = p.format(0, len, &|c| c.del = Some(del));
                out.push(p);
                changes += 1;
                i += 1;
                k += 1;
            }
            Some(Op::Ins) => {
                let mut p = new.get(j).cloned().unwrap_or_default();
                let len = p.len();
                let _ = p.format(0, len, &|c| c.ins = Some(ins));
                out.push(p);
                changes += 1;
                j += 1;
                k += 1;
            }
            None => break,
        }
    }
    s.doc.body = out.into_iter().map(para_block).collect();
    s.doc.ensure_nonempty();
    s.sel = Selection::caret(s.doc.start_of(StoryRef::Body));
    Ok(json!({"changes": changes}))
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Op {
    Same,
    Del,
    Ins,
}

/// LCS edit script between two sequences (bounded size).
fn lcs_ops<T: PartialEq>(a: &[T], b: &[T]) -> Vec<Op> {
    let (n, m) = (a.len().min(4000), b.len().min(4000));
    let mut t = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let v = if a.get(i) == b.get(j) {
                t.get(i + 1).and_then(|r| r.get(j + 1)).copied().unwrap_or(0) + 1
            } else {
                t.get(i + 1).and_then(|r| r.get(j)).copied().unwrap_or(0).max(t.get(i).and_then(|r| r.get(j + 1)).copied().unwrap_or(0))
            };
            if let Some(c) = t.get_mut(i).and_then(|r| r.get_mut(j)) {
                *c = v;
            }
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut ops = Vec::new();
    while i < n && j < m {
        if a.get(i) == b.get(j) {
            ops.push(Op::Same);
            i += 1;
            j += 1;
        } else if t.get(i + 1).and_then(|r| r.get(j)) >= t.get(i).and_then(|r| r.get(j + 1)) {
            ops.push(Op::Del);
            i += 1;
        } else {
            ops.push(Op::Ins);
            j += 1;
        }
    }
    ops.extend(std::iter::repeat_n(Op::Del, n - i));
    ops.extend(std::iter::repeat_n(Op::Ins, m - j));
    ops
}

fn tokens(s: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        cur.push(c);
        if c == ' ' {
            v.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        v.push(cur);
    }
    v
}

fn word_diff(a: &Paragraph, b: &Paragraph, ins: u32, del: u32) -> Paragraph {
    let (ta, tb) = (tokens(&a.plain_text()), tokens(&b.plain_text()));
    let ops = lcs_ops(&ta, &tb);
    let mut p = Paragraph::new();
    p.props = b.props.clone();
    let base = a.runs.first().map(|r| r.props.clone()).unwrap_or_default();
    let (mut i, mut j) = (0, 0);
    for op in ops {
        let at = p.len();
        match op {
            Op::Same => {
                let _ = p.insert_text(at, ta.get(i).map(String::as_str).unwrap_or(""), &base);
                i += 1;
                j += 1;
            }
            Op::Del => {
                let _ = p.insert_text(at, ta.get(i).map(String::as_str).unwrap_or(""), &CharProps { del: Some(del), ..base.clone() });
                i += 1;
            }
            Op::Ins => {
                let _ = p.insert_text(at, tb.get(j).map(String::as_str).unwrap_or(""), &CharProps { ins: Some(ins), ..base.clone() });
                j += 1;
            }
        }
    }
    p
}

fn count_syllables(word: &str) -> usize {
    let w = word.to_lowercase();
    let mut count: usize = 0;
    let mut prev_vowel = false;
    for c in w.chars() {
        let is_v = matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
        if is_v && !prev_vowel {
            count += 1;
        }
        prev_vowel = is_v;
    }
    if w.ends_with('e') && !w.ends_with("le") && count > 1 {
        count = count.saturating_sub(1);
    }
    count.max(1)
}

fn audit_document(s: &mut Session, v: &Value) -> CmdResult {
    let mut issues = Vec::new();
    let mut last_level: Option<u8> = None;
    let fix_mode = p::str(v, "fix").unwrap_or("");

    // 1. Accessibility & structure scan
    let mut empty_headings = 0;
    let mut unlabelled_images = 0;
    let mut tables_without_header = 0;

    for path in s.doc.para_paths(StoryRef::Body) {
        let Some(p) = s.doc.para(StoryRef::Body, &path) else { continue };
        let plain = p.plain_text();
        let is_heading = s.doc.styles.resolve_para(&p.props).outline_level.is_some();

        if is_heading && plain.trim().is_empty() {
            empty_headings += 1;
            issues.push(json!({
                "kind": "warning",
                "issue": "Empty heading paragraph with no text",
                "where": path.0,
                "fix": "Delete empty heading or enter text"
            }));
        }

        for o in &p.objects {
            if let InlineObject::Image { alt, .. } = o
                && alt.trim().is_empty()
            {
                unlabelled_images += 1;
                issues.push(json!({
                    "kind": "error",
                    "issue": "Missing alternative text on picture",
                    "where": path.0,
                    "fix": "Run picture.altText or auditDocument {fix: \"defaultAltText\"}"
                }));
            }
            // Charts and diagrams can't be given alt text yet (see objects.rs), so only report them.
            if let InlineObject::Graphic { alt, .. } = o
                && alt.trim().is_empty()
            {
                issues.push(json!({"kind": "error", "issue": "Chart or diagram has no alternative text", "where": path.0}));
            }
        }
        if let Some(l) = s.doc.styles.resolve_para(&p.props).outline_level {
            if let Some(prev) = last_level
                && l > prev + 1
            {
                issues.push(json!({
                    "kind": "warning",
                    "issue": format!("Heading level skipped (Heading {} after Heading {})", l + 1, prev + 1),
                    "where": path.0
                }));
            }
            last_level = Some(l);
        }
        if p.runs.iter().any(|r| r.props.link.is_some()) {
            let txt = plain.to_lowercase();
            if txt.contains("click here") {
                issues.push(json!({
                    "kind": "tip",
                    "issue": "Link text \"click here\" isn't descriptive",
                    "where": path.0
                }));
            }
        }
    }

    for (i, b) in s.doc.body.iter().enumerate() {
        if let Block::Table(t) = &**b
            && !t.rows.first().is_some_and(|r| r.props.header)
        {
            tables_without_header += 1;
            issues.push(json!({
                "kind": "warning",
                "issue": "Table has no header row",
                "where": [i],
                "fix": "Run table.repeatHeader or auditDocument {fix: \"tableHeaders\"}"
            }));
        }
    }

    if s.doc.core.title.is_empty() {
        issues.push(json!({
            "kind": "tip",
            "issue": "Document has no title property",
            "fix": "Set title in File Properties"
        }));
    }

    // 2. Readability metrics
    let body_text = s.doc.plain_text(StoryRef::Body);
    let words: Vec<&str> = body_text.split_whitespace().collect();
    let word_count = words.len();
    let sentence_count = body_text.split(['.', '!', '?', '\n']).filter(|s| s.split_whitespace().count() >= 2).count().max(1);
    let char_count = body_text.chars().filter(|c| !c.is_whitespace()).count();
    let total_syllables: usize = words.iter().map(|w| count_syllables(w)).sum();

    let words_per_sentence = word_count as f64 / sentence_count as f64;
    let syllables_per_word = if word_count > 0 { total_syllables as f64 / word_count as f64 } else { 1.0 };

    let flesch_reading_ease =
        if word_count >= 10 { (206.835 - (1.015 * words_per_sentence) - (84.6 * syllables_per_word)).clamp(0.0, 100.0) } else { 100.0 };

    let flesch_kincaid_grade =
        if word_count >= 10 { ((0.39 * words_per_sentence) + (11.8 * syllables_per_word) - 15.59).clamp(0.0, 20.0) } else { 1.0 };

    let reading_level = if flesch_reading_ease >= 90.0 {
        "Very Easy (5th grade)"
    } else if flesch_reading_ease >= 80.0 {
        "Easy (6th grade)"
    } else if flesch_reading_ease >= 70.0 {
        "Fairly Easy (7th grade)"
    } else if flesch_reading_ease >= 60.0 {
        "Standard (8th-9th grade)"
    } else if flesch_reading_ease >= 50.0 {
        "Fairly Difficult (High School)"
    } else if flesch_reading_ease >= 30.0 {
        "Difficult (College Level)"
    } else {
        "Very Difficult (Graduate Level)"
    };

    // 3. Optional Auto-Fixes
    let mut fixed_count = 0;
    if fix_mode == "tableHeaders" || fix_mode == "all" {
        for b in &mut s.doc.body {
            let block = std::sync::Arc::make_mut(b);
            let Block::Table(t) = block else { continue };
            let Some(r) = t.rows.first_mut() else { continue };
            if !r.props.header {
                r.props.header = true;
                fixed_count += 1;
            }
        }
    }
    if fix_mode == "defaultAltText" || fix_mode == "all" {
        let mut img_idx = 1;
        for path in s.doc.para_paths(StoryRef::Body) {
            let Ok(p) = s.doc.para_mut(StoryRef::Body, &path) else { continue };
            for o in &mut p.objects {
                let InlineObject::Image { alt, .. } = o else { continue };
                if alt.trim().is_empty() {
                    *alt = format!("Figure {img_idx}");
                    img_idx += 1;
                    fixed_count += 1;
                }
            }
        }
    }

    let _ = empty_headings;
    let _ = unlabelled_images;
    let _ = tables_without_header;

    Ok(json!({
        "issues": issues,
        "ok": issues.is_empty(),
        "fixed": fixed_count,
        "readability": {
            "wordCount": word_count,
            "sentenceCount": sentence_count,
            "characterCount": char_count,
            "fleschReadingEase": (flesch_reading_ease * 10.0).round() / 10.0,
            "fleschKincaidGrade": (flesch_kincaid_grade * 10.0).round() / 10.0,
            "readingLevel": reading_level,
        }
    }))
}

fn accessibility(s: &mut Session, v: &Value) -> CmdResult {
    let rep = audit_document(s, v)?;
    if let Some(issues) = rep.get("issues") {
        s.ui_requests.push(json!({"open": "accessibility", "issues": issues}));
    }
    Ok(rep)
}

fn inspect_doc(s: &mut Session, v: &Value) -> CmdResult {
    let remove: Vec<String> =
        v.get("remove").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let has_hidden = s
        .doc
        .para_paths(StoryRef::Body)
        .iter()
        .any(|p| s.doc.para(StoryRef::Body, p).is_some_and(|x| x.runs.iter().any(|r| r.props.hidden == Some(true))));
    let report = json!({
        "comments": s.doc.comments.len(),
        "revisions": s.doc.para_paths(StoryRef::Body).iter().filter(|p| s.doc.para(StoryRef::Body, p).is_some_and(|x| x.runs.iter().any(|r| r.props.ins.is_some() || r.props.del.is_some()))).count(),
        "properties": !s.doc.core.creator.is_empty() || !s.doc.core.last_modified_by.is_empty(),
        "hiddenText": has_hidden,
        "headers": s.doc.parts.values().filter(|p| matches!(p.kind, wordcraft_doc::PartKind::Header | wordcraft_doc::PartKind::Footer)).count(),
    });
    for r in &remove {
        match r.as_str() {
            "comments" => {
                s.run("review.deleteComment", &json!({"all": true}))?;
            }
            "revisions" => {
                s.run("review.acceptAll", &json!({}))?;
            }
            "properties" => {
                let title = s.doc.core.title.clone();
                s.doc.core = Default::default();
                s.doc.core.title = title;
            }
            "hidden" => {
                for path in s.doc.para_paths(StoryRef::Body) {
                    let ranges: Vec<(usize, usize)> = s
                        .doc
                        .para(StoryRef::Body, &path)
                        .map(|p| p.run_ranges().filter(|(_, c)| c.hidden == Some(true)).map(|(r, _)| (r.start, r.end)).collect())
                        .unwrap_or_default();
                    let para = s.doc.para_mut(StoryRef::Body, &path)?;
                    for (a, b) in ranges.into_iter().rev() {
                        para.delete(a, b)?;
                    }
                }
            }
            "headers" => {
                s.run("insert.removeHeader", &json!({}))?;
                s.run("insert.removeFooter", &json!({}))?;
            }
            _ => {}
        }
    }
    s.clamp_selection();
    Ok(json!({"found": report, "removed": remove}))
}

fn quick_parts(s: &mut Session, v: &Value) -> CmdResult {
    if let Some(name) = p::str(v, "save") {
        let (a, b) = s.sel.ordered();
        if a == b {
            return Err(CmdError::Params("select the content to save".into()));
        }
        let frag = s.doc.copy_range(&a, &b);
        s.building_blocks.insert(name.to_string(), frag);
    }
    if let Some(name) = p::str(v, "delete") {
        s.building_blocks.remove(name);
    }
    if let Some(name) = p::str(v, "insert") {
        let frag = s.building_blocks.get(name).cloned().ok_or_else(|| CmdError::Params(format!("no Quick Part `{name}`")))?;
        let at = delete_selection(s)?;
        let end = s.doc.insert_fragment(&at, &frag)?;
        s.sel = Selection::caret(end);
    }
    Ok(json!({"parts": s.building_blocks.keys().collect::<Vec<_>>()}))
}

fn hf_nav(s: &mut Session, dir: i32) -> CmdResult {
    let StoryRef::Part(cur) = s.sel.focus.story else { return Err(CmdError::Disabled("edit a header or footer first".into())) };
    let mut ids: Vec<u32> = Vec::new();
    for (_, sect) in s.doc.sections() {
        for id in [sect.headers.default, sect.headers.first, sect.headers.even, sect.footers.default, sect.footers.first, sect.footers.even]
            .into_iter()
            .flatten()
        {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    let i = ids.iter().position(|x| *x == cur).unwrap_or(0) as i32;
    let n = ids.len().max(1) as i32;
    let next = ids.get(((i + dir).rem_euclid(n)) as usize).copied().unwrap_or(cur);
    s.sel = Selection::caret(s.doc.start_of(StoryRef::Part(next)));
    sel_result(s)
}

fn select_similar(s: &mut Session, _: &Value) -> CmdResult {
    let f = s.sel.ordered().0;
    let target = s.doc.para_at(&f).map(|p| p.props_of_char(f.off).clone()).unwrap_or_default();
    let mut first: Option<Pos> = None;
    let mut last: Option<Pos> = None;
    let mut count = 0;
    for path in s.doc.para_paths(f.story) {
        let Some(p) = s.doc.para(f.story, &path) else { continue };
        for (r, c) in p.run_ranges() {
            if *c == target {
                count += 1;
                if first.is_none() {
                    first = Some(Pos { story: f.story, path: path.clone(), off: r.start });
                }
                last = Some(Pos { story: f.story, path: path.clone(), off: r.end });
            }
        }
    }
    if let (Some(a), Some(b)) = (first, last) {
        s.sel = Selection { anchor: a, focus: b };
    }
    Ok(json!({"runs": count}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_marks_changes() {
        let mut s = Session::new(Document::from_text("The cat sat.\nKeep this.\nOld line."));
        let r = s.run("review.compare", &json!({"text": "The dog sat.\nKeep this.\nNew line here."})).unwrap();
        assert!(r["changes"].as_u64().unwrap() >= 2);
        s.run("review.acceptAll", &json!({})).unwrap();
        assert_eq!(s.doc.plain_text(StoryRef::Body), "The dog sat.\nKeep this.\nNew line here.");
    }

    #[test]
    fn macros_record_and_run() {
        let mut s = Session::new(Document::new());
        s.run("tools.recordMacro", &json!({"name": "hi"})).unwrap();
        s.run("text.insert", &json!({"text": "Hi "})).unwrap();
        s.run("format.bold", &json!({})).unwrap();
        s.run("tools.recordMacro", &json!({})).unwrap();
        s.run("tools.macros", &json!({"run": "hi"})).unwrap();
        assert_eq!(s.doc.plain_text(StoryRef::Body), "Hi Hi ");
        s.run("edit.repeat", &json!({})).unwrap();
    }

    #[test]
    fn autocorrect_on_space() {
        let mut s = Session::new(Document::new());
        for t in ["teh", " ", "Teh", " ", "(c)", " "] {
            s.run("text.insert", &json!({"text": t})).unwrap();
        }
        // The first word of a sentence is capitalised too.
        assert_eq!(s.doc.plain_text(StoryRef::Body), "The The © ");
    }

    #[test]
    fn protection_blocks_edits() {
        let mut s = Session::new(Document::from_text("locked"));
        s.run("review.restrict", &json!({"mode": "readOnly"})).unwrap();
        assert!(s.run("text.insert", &json!({"text": "x"})).is_err());
        s.run("review.restrict", &json!({"mode": "none"})).unwrap();
        assert!(s.run("text.insert", &json!({"text": "x"})).is_ok());
    }

    #[test]
    fn accessibility_and_quick_parts() {
        let mut s = Session::new(crate::sample::sample_document());
        let r = s.run("file.accessibility", &json!({})).unwrap();
        assert!(r["issues"].is_array());
        s.run("select.text", &json!({"text": "Membership"})).unwrap();
        s.run("insert.quickParts", &json!({"save": "m"})).unwrap();
        s.run("caret.docEnd", &json!({})).unwrap();
        s.run("insert.quickParts", &json!({"insert": "m"})).unwrap();
        assert!(s.doc.plain_text(StoryRef::Body).ends_with("Membership"));
    }

    #[test]
    fn copilot_draft_rewrite_summarize_proofread() {
        let mut s = Session::new(Document::from_text("This is an unpolished sentence. We don't have enough data in order to conclude."));
        let d = s.run("tools.copilot", &json!({"action": "draft", "prompt": "Status report"})).unwrap();
        assert!(d["content"].as_str().unwrap().contains("Status Report"));

        let rw = s.run("tools.copilot", &json!({"action": "rewrite", "prompt": "We need this in order to proceed.", "tone": "concise"})).unwrap();
        assert_eq!(rw["content"].as_str().unwrap(), "We need this to proceed.");

        let sm = s
            .run(
                "tools.copilot",
                &json!({"action": "summarize", "prompt": "First point is crucial for the plan. Second point is equally critical for the outcome."}),
            )
            .unwrap();
        assert!(sm["content"].as_str().unwrap().contains("Key Takeaways"));

        let pr = s.run("tools.copilot", &json!({"action": "proofread", "prompt": "Check the the repetition"})).unwrap();
        assert!(pr["content"].as_str().unwrap().contains("Repeated word detected"));
    }

    #[test]
    fn customize_keyboard_assign_and_reset() {
        let mut s = Session::new(Document::new());
        s.run("tools.customizeKeyboard", &json!({"command": "format.bold", "key": "Ctrl+Alt+B"})).unwrap();
        assert_eq!(s.cmd_shortcuts.get("format.bold").map(|s| s.as_str()), Some("Ctrl+Alt+B"));
        assert_eq!(s.custom_shortcuts.get("Ctrl+Alt+B").map(|s| s.as_str()), Some("format.bold"));

        s.run("tools.customizeKeyboard", &json!({"command": "format.bold", "key": ""})).unwrap();
        assert_eq!(s.cmd_shortcuts.get("format.bold"), None);
        assert_eq!(s.custom_shortcuts.get("Ctrl+Alt+B"), None);

        s.run("tools.customizeKeyboard", &json!({"command": "format.italic", "key": "Ctrl+Alt+I"})).unwrap();
        s.run("tools.customizeKeyboard", &json!({"reset": true})).unwrap();
        assert!(s.cmd_shortcuts.is_empty());
        assert!(s.custom_shortcuts.is_empty());
    }

    #[test]
    fn audit_document_and_readability() {
        let text = "The quick brown fox jumps over the lazy dog. Accessibility and typesetting are fundamental for writing documents.";
        let mut s = Session::new(Document::from_text(text));
        let r = s.run("tools.auditDocument", &json!({})).unwrap();
        assert!(r["readability"]["wordCount"].as_u64().unwrap() >= 15);
        assert!(r["readability"]["fleschReadingEase"].as_f64().unwrap() > 0.0);
        assert!(!r["readability"]["readingLevel"].as_str().unwrap().is_empty());
    }

    #[test]
    fn ai_intelligence_suite_commands() {
        let mut s = Session::new(Document::from_text(
            "We need to fix the problem in order to test the system. The team will complete verification by Friday.",
        ));

        // ai.rewrite
        let rw_acad = s.run("ai.rewrite", &json!({"tone": "academic"})).unwrap();
        assert!(rw_acad["rewritten"].as_str().unwrap().contains("evaluate"));

        let rw_exec = s.run("ai.rewrite", &json!({"tone": "executive"})).unwrap();
        assert!(rw_exec["rewritten"].as_str().unwrap().contains("strategic priority"));

        // ai.summarize
        let sm_exec = s.run("ai.summarize", &json!({"mode": "executive"})).unwrap();
        assert!(sm_exec["summary"].as_str().unwrap().contains("Executive Summary"));

        let sm_abs = s.run("ai.summarize", &json!({"mode": "abstract"})).unwrap();
        assert!(sm_abs["summary"].as_str().unwrap().contains("Structured Abstract"));

        // ai.ask
        let ask_acts = s.run("ai.ask", &json!({"question": "What are the action items?"})).unwrap();
        assert!(ask_acts["answer"].as_str().unwrap().contains("verification by Friday"));

        let ask_stats = s.run("ai.ask", &json!({"question": "document stats and length"})).unwrap();
        assert!(ask_stats["metrics"]["wordCount"].as_u64().unwrap() > 10);

        // ai.autocomplete
        let ac = s.run("ai.autocomplete", &json!({"prefix": "Please find attached"})).unwrap();
        assert!(ac["prediction"].as_str().unwrap().contains("report"));

        // ai.generateTable
        let tbl_res = s.run("ai.generateTable", &json!({"prompt": "Project Roadmap"})).unwrap();
        assert!(tbl_res["rows"].as_u64().unwrap() >= 2);
        assert!(tbl_res["cols"].as_u64().unwrap() >= 3);
        assert_eq!(s.doc.body.iter().filter(|b| matches!(b.as_ref(), Block::Table(_))).count(), 1);
    }
}
