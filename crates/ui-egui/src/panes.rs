//! Side panes: Navigation (headings, pages, search results), Clipboard, Styles, Style Inspector, Comments.

use egui::{Stroke, Ui, vec2};
use serde_json::{Value, json};
use wordcraft_doc::{Pos, StoryRef};

use crate::WordApp;
use crate::theme::{Tokens, regular, semibold};

pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    if app.session.view.nav_pane {
        egui::Panel::left("nav_pane")
            .default_size(260.0)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(t.panel).inner_margin(10).stroke(Stroke::new(1.0, t.border)))
            .show(ui, |ui| nav(app, ui));
    }
    if app.session.view.clipboard_pane {
        egui::Panel::left("clipboard_pane")
            .default_size(250.0)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(t.panel).inner_margin(10).stroke(Stroke::new(1.0, t.border)))
            .show(ui, |ui| clipboard(app, ui));
    }
    if app.session.view.styles_pane {
        egui::Panel::right("styles_pane")
            .default_size(250.0)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(t.panel).inner_margin(10).stroke(Stroke::new(1.0, t.border)))
            .show(ui, |ui| styles(app, ui));
    }
    if app.session.view.style_inspector {
        egui::Panel::right("style_inspector")
            .default_size(240.0)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(t.panel).inner_margin(10).stroke(Stroke::new(1.0, t.border)))
            .show(ui, |ui| inspector(app, ui));
    }
    if app.session.view.comments_pane {
        egui::Panel::right("comments_pane")
            .default_size(290.0)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(t.panel).inner_margin(10).stroke(Stroke::new(1.0, t.border)))
            .show(ui, |ui| comments(app, ui));
    }
    if app.ui.ai_pane {
        egui::Panel::right("ai_pane")
            .default_size(320.0)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(t.panel).inner_margin(10).stroke(Stroke::new(1.0, t.border)))
            .show(ui, |ui| ai_copilot(app, ui));
    }
}

fn header(ui: &mut Ui, title: &str) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tl!(title)).font(semibold(15.0)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Drawn, not the "✕" character: the interface fonts have no glyph for it.
            if crate::widgets::icon_button(ui, "close", tl!("Close")).clicked() {
                close = true;
            }
        });
    });
    ui.add_space(4.0);
    close
}

fn nav(app: &mut WordApp, ui: &mut Ui) {
    if header(ui, "Navigation") {
        let _ = app.run("view.navigationPane", json!({"value": false}));
        return;
    }
    let qid = egui::Id::new("nav_query");
    let mut q = ui.data(|d| d.get_temp::<String>(qid)).unwrap_or_default();
    let r = ui.add(egui::TextEdit::singleline(&mut q).hint_text(tl!("Search document")).desired_width(f32::INFINITY));
    if r.changed() {
        ui.data_mut(|d| d.insert_temp(qid, q.clone()));
        if !q.is_empty() {
            let _ = app.session.run("edit.find", &json!({"text": q}));
        } else {
            app.session.find.results.clear();
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        for (k, l) in [("headings", "Headings"), ("pages", "Pages"), ("results", "Results")] {
            if ui.selectable_label(app.ui.nav_tab == k, tl!(l)).clicked() {
                app.ui.nav_tab = k.into();
            }
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| match app.ui.nav_tab.as_str() {
        "pages" => {
            let l = app.session.layout();
            for (i, p) in l.pages.iter().enumerate() {
                let label = crate::i18n::fmt(tl!("Page {page}"), &[("page", &p.number.to_string())]);
                if ui.selectable_label(app.session.page_hint == i, label).clicked() {
                    let _ = app.run("edit.goto", json!({"page": i + 1}));
                }
            }
        }
        "results" => {
            let results = app.session.find.results.clone();
            ui.label(egui::RichText::new(format!("{} results", results.len())).small().weak());
            for (a, b) in results.iter().take(500) {
                let Some(p) = app.session.doc.para_at(a) else { continue };
                let from = p.clamp(a.off.saturating_sub(30));
                let to = p.clamp((b.off + 40).min(p.len()));
                let pre = p.text.get(from..a.off).unwrap_or("");
                let hit = p.text.get(a.off..b.off).unwrap_or("");
                let post = p.text.get(b.off..to).unwrap_or("");
                let mut job = egui::text::LayoutJob::default();
                let fmt = |c| egui::TextFormat { font_id: regular(12.0), color: c, ..Default::default() };
                job.append(pre, 0.0, fmt(ui.visuals().text_color()));
                job.append(hit, 0.0, egui::TextFormat { font_id: semibold(12.0), background: egui::Color32::from_rgb(0xFF, 0xF1, 0x76), color: egui::Color32::BLACK, ..Default::default() });
                job.append(post, 0.0, fmt(ui.visuals().text_color()));
                job.wrap.max_width = ui.available_width();
                if ui.add(egui::Button::new(job).frame(false)).clicked() {
                    let _ = app.run("select.range", json!({"anchor": a, "focus": b}));
                }
                ui.separator();
            }
        }
        _ => {
            let caret = app.session.sel.focus.clone();
            let mut current: Option<usize> = None;
            let mut items = Vec::new();
            for path in app.session.doc.para_paths(StoryRef::Body) {
                let Some(p) = app.session.doc.para(StoryRef::Body, &path) else { continue };
                let rp = app.session.doc.styles.resolve_para(&p.props);
                if let Some(lv) = rp.outline_level {
                    let txt = p.plain_text().trim().to_string();
                    if txt.is_empty() || rp.style.starts_with("TOC") {
                        continue;
                    }
                    if path <= caret.path {
                        current = Some(items.len());
                    }
                    items.push((path, lv, txt));
                }
            }
            if items.is_empty() {
                ui.label(egui::RichText::new(tl!("Create an interactive outline of your document.\n\nIt's a great way to keep track of where you are or quickly move your content around.\n\nTo get started, go to the Home tab and apply Heading styles to the headings in your document.")).weak());
            }
            for (i, (path, lv, txt)) in items.into_iter().enumerate() {
                if nav_heading(ui, lv, &txt, current == Some(i)).clicked() {
                    let _ = app.run("caret.set", json!({"pos": Pos { story: StoryRef::Body, path, off: 0 }}));
                    app.canvas.scroll_to_caret = true;
                    app.canvas.want_focus = true;
                }
            }
        }
    });
}

/// Constrain each heading to the available pane width so long document text cannot
/// enlarge the panel. The full heading stays accessible on hover.
fn nav_heading(ui: &mut Ui, level: u8, text: &str, selected: bool) -> egui::Response {
    ui.horizontal(|ui| {
        ui.add_space(level as f32 * 14.0);
        ui.add_sized(
            vec2(ui.available_width(), ui.spacing().interact_size.y),
            egui::Button::selectable(selected, egui::RichText::new(text).font(if level == 0 { semibold(12.5) } else { regular(12.5) })).truncate(),
        )
        .on_hover_text(text)
    })
    .inner
}

/// Items collected by Copy and Cut, newest first; clicking one pastes it at the caret.
fn clipboard(app: &mut WordApp, ui: &mut Ui) {
    if header(ui, "Clipboard") {
        let _ = app.run("edit.clipboardPane", json!({"value": false}));
        return;
    }
    let items: Vec<String> = app.session.clip_history.items().iter().map(|i| i.preview()).collect();
    ui.horizontal(|ui| {
        if ui.add_enabled(!items.is_empty(), egui::Button::new(tl!("Paste All"))).clicked() {
            let _ = app.run("edit.pasteAllClipboard", json!({}));
            app.canvas.want_focus = true;
        }
        if ui.add_enabled(!items.is_empty(), egui::Button::new(tl!("Clear All"))).clicked() {
            let _ = app.run("edit.clearClipboard", json!({}));
        }
    });
    ui.add_space(4.0);
    let hint = if items.is_empty() { "Nothing collected yet. Items you copy or cut appear here." } else { "Click an item to paste it." };
    ui.label(egui::RichText::new(tl!(hint)).small().weak());
    ui.separator();
    let t = Tokens::get(ui.ctx());
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, preview) in items.iter().enumerate() {
            egui::Frame::NONE.fill(t.input).stroke(Stroke::new(1.0, t.border)).corner_radius(6).inner_margin(8).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let text = if preview.is_empty() { tl!("(picture or object)").to_string() } else { preview.clone() };
                let mut job =
                    egui::text::LayoutJob::single_section(text, egui::TextFormat { font_id: regular(12.5), color: t.text, ..Default::default() });
                job.wrap.max_width = ui.available_width();
                job.wrap.max_rows = 3;
                if ui.add(egui::Button::new(job).frame(false)).on_hover_text(tl!("Paste")).clicked() {
                    let _ = app.run("edit.pasteClipboardItem", json!({"index": i}));
                    app.canvas.want_focus = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if ui.small_button(tl!("Delete")).clicked() {
                        let _ = app.run("edit.deleteClipboardItem", json!({"index": i}));
                    }
                });
            });
            ui.add_space(6.0);
        }
    });
}

fn styles(app: &mut WordApp, ui: &mut Ui) {
    if header(ui, "Styles") {
        let _ = app.run("view.stylesPane", json!({"value": false}));
        return;
    }
    let st = app.session.run("format.state", &json!({})).unwrap_or_default();
    let cur = st.get("style").and_then(Value::as_str).unwrap_or("Normal").to_string();
    ui.label(
        egui::RichText::new(crate::i18n::fmt(tl!("Current style: {style}"), &[("style", st.get("styleName").and_then(Value::as_str).unwrap_or(""))]))
            .small(),
    );
    ui.horizontal(|ui| {
        if ui.button(tl!("New Style…")).clicked() {
            app.dialog = crate::dialogs::Dialog::open("newStyle", app);
        }
        if ui.button(tl!("Update to Match")).clicked() {
            let _ = app.run("styles.updateToMatch", json!({}));
        }
        if ui.button(tl!("Clear")).clicked() {
            let _ = app.run("format.clear", json!({}));
        }
    });
    if ui.button(tl!("Manage Styles…")).clicked() {
        let _ = app.run("styles.manage", json!({}));
    }
    if ui.selectable_label(app.session.view.style_inspector, tl!("Style Inspector")).clicked() {
        let _ = app.run("styles.inspector", json!({}));
    }
    ui.separator();
    let mut list: Vec<(String, String, bool)> = app
        .session
        .doc
        .styles
        .styles
        .iter()
        .filter(|s| !s.hidden && s.kind != wordcraft_doc::StyleKind::Table)
        .map(|s| (s.id.clone(), s.name.clone(), s.kind == wordcraft_doc::StyleKind::Character))
        .collect();
    list.sort_by_key(|s| s.1.to_lowercase());
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (id, name, is_char) in list {
            ui.horizontal(|ui| {
                let r = ui.selectable_label(id == cur, &name);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(if is_char { "a" } else { "¶" }).weak());
                });
                if r.clicked() {
                    let _ = app.run("para.style", json!({"style": id}));
                    app.canvas.want_focus = true;
                }
                r.context_menu(|ui| {
                    if ui.button(tl!("Modify…")).clicked() {
                        app.dialog = crate::dialogs::Dialog::modify_style(app, &id);
                        ui.close();
                    }
                    if ui.button(tl!("Update to Match Selection")).clicked() {
                        let _ = app.run("styles.updateToMatch", json!({"style": id}));
                        ui.close();
                    }
                    if ui.button(tl!("Delete")).clicked() {
                        let _ = app.run("styles.delete", json!({"style": id}));
                        ui.close();
                    }
                });
            });
        }
    });
}

fn comments(app: &mut WordApp, ui: &mut Ui) {
    if header(ui, "Comments") {
        let _ = app.run("view.commentsPane", json!({"value": false}));
        return;
    }
    if crate::widgets::icon_text_button(ui, "newComment", tl!("New Comment"), regular(12.5)).clicked() {
        let _ = app.run("review.newComment", json!({"text": ""}));
    }
    ui.separator();
    let list = app.session.run("review.comments", &json!({})).unwrap_or_default();
    let t = Tokens::get(ui.ctx());
    egui::ScrollArea::vertical().show(ui, |ui| {
        for c in list.as_array().cloned().unwrap_or_default() {
            let id = c.get("id").and_then(Value::as_u64).unwrap_or(0);
            let reply = c.get("parent").is_some_and(|p| !p.is_null());
            egui::Frame::NONE.fill(t.input).stroke(Stroke::new(1.0, t.border)).corner_radius(6).inner_margin(8).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if reply {
                    ui.add_space(0.0);
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(c.get("author").and_then(Value::as_str).unwrap_or("")).font(semibold(12.0)));
                    ui.label(egui::RichText::new(c.get("date").and_then(Value::as_str).unwrap_or("").get(..10).unwrap_or("")).small().weak());
                    if c.get("resolved").and_then(Value::as_bool).unwrap_or(false) {
                        ui.label(egui::RichText::new(tl!("Resolved")).small().color(t.green));
                    }
                });
                // Editable comment text (one undo step per edit, like the balloons).
                comment_editor(app, ui, id as u32, None, false);
                let open_key = egui::Id::new(("comment_reply_open", id));
                let mut replying = ui.data(|d| d.get_temp::<bool>(open_key)).unwrap_or(false);
                ui.horizontal(|ui| {
                    if ui.small_button(tl!("Go to")).clicked()
                        && let Some(a) = c.get("anchor").filter(|a| !a.is_null())
                    {
                        let _ = app.run("caret.set", json!({"pos": a}));
                    }
                    if ui.small_button(tl!("Reply")).clicked() {
                        replying = true;
                        ui.data_mut(|d| d.insert_temp(open_key, true));
                        ui.data_mut(|d| d.insert_temp(egui::Id::new(("comment_reply_focus", id)), true));
                    }
                    let resolved = c.get("resolved").and_then(Value::as_bool).unwrap_or(false);
                    if ui.small_button(if resolved { tl!("Reopen") } else { tl!("Resolve") }).clicked() {
                        let _ = app.run("review.resolveComment", json!({"id": id}));
                    }
                    if ui.small_button(tl!("Delete")).clicked() {
                        let _ = app.run("review.deleteComment", json!({"id": id}));
                    }
                });
                if replying {
                    let focus = ui.data_mut(|d| d.remove_temp::<bool>(egui::Id::new(("comment_reply_focus", id)))).unwrap_or(false);
                    if reply_editor(app, ui, id as u32, None, focus) {
                        ui.data_mut(|d| d.remove::<bool>(open_key));
                    }
                }
            });
            ui.add_space(6.0);
        }
    });
}

fn edit_key(id: u32) -> egui::Id {
    egui::Id::new(("comment_edit", id))
}

fn reply_key(id: u32) -> egui::Id {
    egui::Id::new(("comment_reply", id))
}

/// A comment's text, editable in place (the Comments pane and the balloons). Typing stays in a
/// draft until the field loses focus; then [`commit_comment`] writes it back with one
/// `review.editComment`, so each editing session is one undo step.
pub(crate) fn comment_editor(app: &mut WordApp, ui: &mut Ui, id: u32, font: Option<egui::FontId>, focus: bool) {
    let Some(part) = app.session.doc.comments.get(&id).map(|c| c.part) else { return };
    let key = edit_key(id);
    let mut text = ui.data(|d| d.get_temp::<String>(key)).unwrap_or_else(|| app.session.doc.plain_text(StoryRef::Part(part)));
    let mut edit = egui::TextEdit::multiline(&mut text).desired_rows(1).desired_width(f32::INFINITY).hint_text(tl!("Add a comment"));
    if let Some(f) = font {
        edit = edit.font(f);
    }
    let r = ui.add(edit);
    if focus {
        r.request_focus();
    }
    if r.changed() {
        ui.data_mut(|d| d.insert_temp(key, text));
    }
    if r.lost_focus() {
        commit_comment(app, ui.ctx(), id);
    }
}

/// Write a comment's draft text back, when it has one that differs from the comment.
pub(crate) fn commit_comment(app: &mut WordApp, ctx: &egui::Context, id: u32) {
    let Some(text) = ctx.data_mut(|d| d.remove_temp::<String>(edit_key(id))) else { return };
    let Some(part) = app.session.doc.comments.get(&id).map(|c| c.part) else { return };
    if text != app.session.doc.plain_text(StoryRef::Part(part)) {
        let _ = app.run("review.editComment", json!({"id": id, "text": text}));
    }
}

/// The comment a reply to `id` answers: replies to a reply join its thread.
fn thread_root(app: &WordApp, id: u32) -> u32 {
    match app.session.doc.comments.get(&id).and_then(|c| c.parent) {
        Some(p) if app.session.doc.comments.contains_key(&p) => p,
        _ => id,
    }
}

/// A field for replying to a comment. When it loses focus a non-empty reply is posted with
/// `review.reply`; returns true when the field is done (posted or left empty).
pub(crate) fn reply_editor(app: &mut WordApp, ui: &mut Ui, id: u32, font: Option<egui::FontId>, focus: bool) -> bool {
    let key = reply_key(id);
    let mut text = ui.data(|d| d.get_temp::<String>(key)).unwrap_or_default();
    let mut edit = egui::TextEdit::multiline(&mut text).desired_rows(1).desired_width(f32::INFINITY).hint_text(tl!("Write a reply"));
    if let Some(f) = font {
        edit = edit.font(f);
    }
    let r = ui.add(edit);
    if focus {
        r.request_focus();
    }
    if r.changed() {
        ui.data_mut(|d| d.insert_temp(key, text));
    }
    if r.lost_focus() {
        commit_reply(app, ui.ctx(), id);
        return true;
    }
    false
}

/// Post a pending reply draft to comment `id` (nothing when there is none or it is blank).
pub(crate) fn commit_reply(app: &mut WordApp, ctx: &egui::Context, id: u32) {
    let Some(text) = ctx.data_mut(|d| d.remove_temp::<String>(reply_key(id))) else { return };
    if !text.trim().is_empty() {
        let root = thread_root(app, id);
        let _ = app.run("review.reply", json!({"id": root, "text": text.trim_end()}));
    }
}

/// Style Inspector: the paragraph and character levels at the caret, each with its style and the
/// direct formatting on top, and a button to clear each level.
fn inspector(app: &mut WordApp, ui: &mut Ui) {
    if header(ui, "Style Inspector") {
        let _ = app.run("styles.inspector", json!({"value": false}));
        return;
    }
    // Read-only, so call it directly: running the command every frame would pay for the command
    // machinery and consume per-command state (a pending `join_next_undo`) just to draw the pane.
    let r = wordcraft_engine::cmd::inspector::inspect(&app.session);
    let t = Tokens::get(ui.ctx());
    let none = tl!("None").to_string();
    let para = &r["paragraph"];
    let chr = &r["character"];
    let pstyle = para["styleName"].as_str().unwrap_or("Normal").to_string();
    let cstyle = chr["styleName"].as_str().map(str::to_string);
    let pdirect = direct_list(&para["direct"]);
    let cdirect = direct_list(&chr["direct"]);
    let mut clear: Option<&str> = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        section(ui, &t, "Paragraph", |ui| {
            if level_row(ui, &t, "Style", &pstyle, false, "Reset to Normal", pstyle == "Normal" && para["style"] == "Normal") {
                clear = Some("paragraphStyle");
            }
            let (txt, empty) = if pdirect.is_empty() { (none.clone(), true) } else { (pdirect.join(", "), false) };
            if level_row(ui, &t, "Direct formatting", &txt, empty, "Clear Paragraph Formatting", empty) {
                clear = Some("paragraphFormatting");
            }
        });
        ui.add_space(8.0);
        section(ui, &t, "Characters", |ui| {
            let (txt, empty) = match &cstyle {
                Some(n) => (n.clone(), false),
                None => (none.clone(), true),
            };
            if level_row(ui, &t, "Style", &txt, empty, "Clear Character Style", empty) {
                clear = Some("characterStyle");
            }
            let (txt, empty) = if cdirect.is_empty() { (none.clone(), true) } else { (cdirect.join(", "), false) };
            if level_row(ui, &t, "Direct formatting", &txt, empty, "Clear Character Formatting", empty) {
                clear = Some("characterFormatting");
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button(tl!("Clear All")).on_hover_text(tl!("Clear All Formatting")).clicked() {
                let _ = app.run("format.clear", json!({}));
                app.canvas.want_focus = true;
            }
            if ui.button(tl!("Styles Pane")).clicked() {
                let _ = app.run("view.stylesPane", json!({"value": true}));
            }
        });
    });
    if let Some(level) = clear {
        let _ = app.run("styles.inspectorClear", json!({"level": level}));
        app.canvas.want_focus = true;
    }
}

/// A titled, bordered group in the inspector.
fn section(ui: &mut Ui, t: &Tokens, title: &str, body: impl FnOnce(&mut Ui)) {
    ui.label(egui::RichText::new(tl!(title)).font(semibold(12.5)).color(t.text));
    ui.add_space(2.0);
    egui::Frame::NONE.fill(t.input).stroke(Stroke::new(1.0, t.border)).corner_radius(4).inner_margin(8).show(ui, |ui| {
        ui.set_width(ui.available_width());
        body(ui);
    });
}

/// One level: a caption, its value, and a clear button (disabled when there's nothing to clear).
/// Returns true when the button was clicked.
fn level_row(ui: &mut Ui, t: &Tokens, caption: &str, value: &str, dim: bool, tip: &str, disabled: bool) -> bool {
    ui.label(egui::RichText::new(tl!(caption)).font(regular(11.0)).color(t.text_dim));
    let mut clicked = false;
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 26.0).max(40.0);
        ui.allocate_ui(vec2(w, 18.0), |ui| {
            ui.set_width(w);
            let text = egui::RichText::new(value).font(regular(12.5)).color(if dim { t.text_disabled } else { t.text });
            ui.add(egui::Label::new(text).wrap());
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            let (r, resp) = ui.allocate_exact_size(vec2(22.0, 20.0), egui::Sense::click());
            let enabled = !disabled;
            if enabled && resp.hovered() {
                ui.painter().rect_filled(r, 3.0, t.hover);
            }
            let c = if enabled { t.icon } else { t.text_disabled };
            crate::icons::paint(
                ui.painter(),
                egui::Rect::from_center_size(r.center(), vec2(16.0, 16.0)),
                "clear",
                c,
                if enabled { t.accent } else { c },
            );
            let resp = resp.on_hover_text(tl!(tip));
            clicked = enabled && resp.clicked();
        });
    });
    ui.add_space(4.0);
    clicked
}

/// Readable descriptions of the inspector's `direct` entries (`[{"prop", "value"}]`).
fn direct_list(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().take(64).map(|d| describe(d["prop"].as_str().unwrap_or(""), &d["value"])).collect()).unwrap_or_default()
}

/// Each `{"prop", "value"}` of a list (the Style Inspector's, or a formatting change's) as text.
pub(crate) fn describe_props(list: &[Value]) -> Vec<String> {
    list.iter().take(64).map(|d| describe(d["prop"].as_str().unwrap_or(""), &d["value"])).collect()
}

/// "Bold", "Not Italic", "Font: Arial", "Size: 14 pt"…
fn describe(prop: &str, v: &Value) -> String {
    let label = match prop {
        "font" => "Font",
        "size" => "Size",
        "bold" => "Bold",
        "italic" => "Italic",
        "underline" => "Underline",
        "underlineColor" => "Underline color",
        "strike" => "Strikethrough",
        "doubleStrike" => "Double strikethrough",
        "color" => "Font color",
        "highlight" => "Highlight",
        "shading" => "Shading",
        "vertAlign" => "Position",
        "caps" => "All caps",
        "smallCaps" => "Small caps",
        "hidden" => "Hidden",
        "spacing" => "Character spacing",
        "scale" => "Scale",
        "position" => "Raised/lowered",
        "outline" => "Outline",
        "shadow" => "Shadow",
        "emboss" => "Emboss",
        "engrave" => "Engrave",
        "align" => "Alignment",
        "indentLeft" => "Left indent",
        "indentRight" => "Right indent",
        "indentFirst" => "First line",
        "spaceBefore" => "Space before",
        "spaceAfter" => "Space after",
        "lineSpacing" => "Line spacing",
        "contextualSpacing" => "Don't add space between paragraphs of the same style",
        "keepNext" => "Keep with next",
        "keepLines" => "Keep lines together",
        "pageBreakBefore" => "Page break before",
        "widowControl" => "Widow/Orphan control",
        "outlineLevel" => "Outline level",
        "tabs" => "Tabs",
        "borders" => "Borders",
        "suppressHyphens" => "Don't hyphenate",
        "suppressLineNumbers" => "Suppress line numbers",
        "bidi" => "Right-to-left",
        "dropCap" => "Drop cap",
        "style" => "Style",
        "numbering" => "Numbering",
        "lang" => "Language",
        other => other,
    };
    let label = tl!(label);
    let pt = |x: f64| crate::i18n::fmt(tl!("{n} pt"), &[("n", &trim_num(x))]);
    let hex = |c: &Value| -> Option<String> {
        let a = c.as_array()?;
        let ch = |i: usize| a.get(i).and_then(Value::as_u64).unwrap_or(0).min(255);
        Some(format!("#{:02X}{:02X}{:02X}", ch(0), ch(1), ch(2)))
    };
    let value = match (prop, v) {
        (_, Value::Bool(true)) => return label.to_string(),
        (_, Value::Bool(false)) => return crate::i18n::fmt(tl!("Not {name}"), &[("name", label)]),
        ("scale", Value::Number(n)) => format!("{}%", trim_num(n.as_f64().unwrap_or(100.0))),
        ("outlineLevel", Value::Number(n)) => match n.as_u64().unwrap_or(9) {
            l @ 0..=8 => crate::i18n::fmt(tl!("Level {n}"), &[("n", &(l + 1).to_string())]),
            _ => tl!("Body Text").to_string(),
        },
        ("dropCap", Value::Number(n)) => crate::i18n::fmt(tl!("{n} lines"), &[("n", &n.to_string())]),
        (_, Value::Number(n)) => pt(n.as_f64().unwrap_or(0.0)),
        ("align", Value::String(s)) => tl!(match s.as_str() {
            "center" => "Centered",
            "right" => "Right",
            "justify" => "Justified",
            "distribute" => "Distributed",
            _ => "Left",
        })
        .to_string(),
        ("color", Value::String(_)) => tl!("Automatic").to_string(),
        ("color", Value::Object(o)) => o.get("Rgb").and_then(hex).unwrap_or_default(),
        (_, Value::Array(_)) if prop != "tabs" => hex(v).unwrap_or_default(),
        ("lineSpacing", Value::Object(o)) => {
            let x = o.get("value").and_then(Value::as_f64).unwrap_or(1.0);
            match o.get("rule").and_then(Value::as_str) {
                Some("atLeast") => crate::i18n::fmt(tl!("At least {n}"), &[("n", &pt(x))]),
                Some("exactly") => crate::i18n::fmt(tl!("Exactly {n}"), &[("n", &pt(x))]),
                _ => crate::i18n::fmt(tl!("{n} lines"), &[("n", &trim_num(x))]),
            }
        }
        (_, Value::String(s)) => tl!(&camel_words(s)).to_string(),
        _ => return label.to_string(),
    };
    format!("{label}: {value}")
}

/// `1.50` → `1.5`, `12.0` → `12`.
fn trim_num(x: f64) -> String {
    let s = format!("{:.2}", if x.is_finite() { x } else { 0.0 });
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `dotDash` → `Dot dash` (enum values from the document model).
fn camel_words(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().take(64).enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else if c.is_uppercase() {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn ai_copilot(app: &mut WordApp, ui: &mut Ui) {
    if header(ui, "Copilot") {
        app.ui.ai_pane = false;
        return;
    }

    // AI Provider & Model Bar + Settings Button
    ui.horizontal(|ui| {
        let is_custom = !app.ui.ai_api_key.trim().is_empty();
        let status_color = if is_custom { crate::theme::APP_COLOR } else { egui::Color32::from_rgb(120, 120, 120) };
        let status_badge = if is_custom { format!("● {}", app.ui.ai_provider) } else { "Offline (Built-in)".to_string() };
        ui.label(egui::RichText::new(status_badge).font(semibold(11.0)).color(status_color));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(egui::RichText::new("⚙ API Settings").font(regular(11.0))).clicked() {
                app.ui.ai_settings_open = !app.ui.ai_settings_open;
            }
        });
    });

    // Model selection dropdown & manual input
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Model:").font(regular(11.5)));
        let models: &[&str] = match app.ui.ai_provider.as_str() {
            "OpenRouter" => &[
                "nvidia/nemotron-3-ultra-550b-a55b:free",
                "deepseek/deepseek-chat",
                "deepseek/deepseek-r1",
                "google/gemini-2.0-flash-exp:free",
                "anthropic/claude-3.5-sonnet",
                "openai/gpt-4o",
                "openai/gpt-4o-mini",
                "meta-llama/llama-3.3-70b-instruct",
            ],
            "DeepSeek" => &["deepseek-chat", "deepseek-reasoner"],
            "Gemini" => &["gemini-2.0-flash", "gemini-2.0-pro-exp-02-05", "gemini-1.5-pro", "gemini-1.5-flash"],
            "Claude" => &["claude-3-5-sonnet-20241022", "claude-3-5-haiku-20241022", "claude-3-opus-20240229"],
            _ => &["default-model", "custom-model"],
        };
        egui::ComboBox::from_id_salt("ai_model_select")
            .selected_text(if app.ui.ai_model.is_empty() { "Select Preset" } else { &app.ui.ai_model })
            .width(170.0)
            .show_ui(ui, |ui| {
                for m in models {
                    ui.selectable_value(&mut app.ui.ai_model, m.to_string(), *m);
                }
            });
    });

    // Editable text field to manually type or modify any custom model ID
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Custom Model:").font(regular(10.5)).weak());
        ui.add(egui::TextEdit::singleline(&mut app.ui.ai_model).hint_text(tl!("Type any provider model ID…")).desired_width(f32::INFINITY));
    });

    // AI Settings Modal / Collapsible
    if app.ui.ai_settings_open {
        let t = Tokens::get(ui.ctx());
        egui::Frame::NONE.fill(t.menu).inner_margin(8).stroke(Stroke::new(1.0, t.border)).show(ui, |ui| {
            ui.label(egui::RichText::new("AI Provider & API Key Configuration").font(semibold(12.0)));
            ui.add_space(2.0);

            ui.horizontal(|ui| {
                ui.label("Provider:");
                for p in ["OpenRouter", "DeepSeek", "Gemini", "Claude", "Custom"] {
                    if ui.selectable_label(app.ui.ai_provider == p, p).clicked() {
                        app.ui.ai_provider = p.into();
                        // Set sensible default model when switching provider
                        match p {
                            "OpenRouter" => app.ui.ai_model = "nvidia/nemotron-3-ultra-550b-a55b:free".into(),
                            "DeepSeek" => app.ui.ai_model = "deepseek-chat".into(),
                            "Gemini" => app.ui.ai_model = "gemini-2.0-flash".into(),
                            "Claude" => app.ui.ai_model = "claude-3-5-sonnet-20241022".into(),
                            _ => {}
                        }
                    }
                }
            });

            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label("API Key:");
                let edit = egui::TextEdit::singleline(&mut app.ui.ai_api_key).password(true).desired_width(180.0).hint_text("sk-...");
                ui.add(edit);
            });

            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label("Model ID:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.ui.ai_model)
                        .desired_width(180.0)
                        .hint_text("Enter model ID directly (e.g. nvidia/nemotron-3-ultra-550b-a55b:free)"),
                );
            });

            if app.ui.ai_provider == "Custom" {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label("Base URL:");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.ui.ai_base_url)
                            .desired_width(180.0)
                            .hint_text("https://api.example.com/v1/chat/completions"),
                    );
                });
            }

            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Keys and custom models are saved locally in your user profile.").font(regular(10.5)).weak());
                if ui.button(egui::RichText::new("Done").font(regular(11.0))).clicked() {
                    app.ui.ai_settings_open = false;
                }
            });
        });
        ui.add_space(3.0);
    }

    if !app.ui.ai_status.is_empty() {
        ui.label(egui::RichText::new(&app.ui.ai_status).font(regular(11.0)).color(egui::Color32::from_rgb(220, 50, 50)));
        ui.add_space(2.0);
    }

    ui.add_space(2.0);
    ui.horizontal(|ui| {
        for m in ["Draft", "Rewrite", "Summarize", "Proofread", "Table", "Ask"] {
            if ui.selectable_label(app.ui.ai_mode == m, tl!(m)).clicked() {
                app.ui.ai_mode = m.into();
            }
        }
    });
    ui.separator();

    let sel = app.session.selected_text();
    let has_sel = !sel.trim().is_empty();

    ui.horizontal(|ui| {
        if has_sel {
            let wc = sel.split_whitespace().count();
            ui.label(egui::RichText::new(format!("Selected: {wc} words")).small().color(crate::theme::APP_COLOR));
        } else {
            ui.label(egui::RichText::new("Target: Whole Document").small().weak());
        }
    });
    ui.add_space(4.0);

    // Quick prompt chips
    ui.horizontal_wrapped(|ui| {
        let chips: &[&str] = match app.ui.ai_mode.as_str() {
            "Rewrite" => &["Professional", "Academic", "Executive", "Concise", "Casual"],
            "Summarize" => &["Key Takeaways", "Executive Summary", "Structured Abstract"],
            "Proofread" => &["Check Clarity", "Find Repeated Words"],
            "Table" => &["Project Roadmap", "Feature Comparison", "Pricing Tiers", "Team Roster", "KPI Metrics", "SWOT Analysis"],
            "Ask" => &["Action Items & Deadlines", "Document Statistics & Length", "Readability Analysis", "Outline & Headings"],
            _ => &["Meeting Minutes", "Status Report", "Project Proposal", "Executive Memo"],
        };
        for chip in chips {
            if ui.add(egui::Button::new(egui::RichText::new(*chip).small())).clicked() {
                app.ui.ai_prompt = chip.to_string();
                let mode_lower = app.ui.ai_mode.to_lowercase();
                let tone = chip.to_lowercase();
                let mut payload = json!({
                    "action": mode_lower,
                    "prompt": chip,
                    "tone": tone,
                    "provider": app.ui.ai_provider,
                    "model": app.ui.ai_model,
                });
                if !app.ui.ai_api_key.trim().is_empty() {
                    payload["api_key"] = json!(app.ui.ai_api_key);
                }
                if !app.ui.ai_base_url.trim().is_empty() {
                    payload["base_url"] = json!(app.ui.ai_base_url);
                }
                app.ui.ai_status.clear();
                let r = app.session.run("tools.copilot", &payload);
                match r {
                    Ok(res) => {
                        if let Some(c) = res.get("content").and_then(Value::as_str) {
                            app.ui.ai_response = c.to_string();
                        } else if mode_lower == "table" {
                            let rows = res.get("rows").and_then(Value::as_u64).unwrap_or(0);
                            let cols = res.get("cols").and_then(Value::as_u64).unwrap_or(0);
                            app.ui.ai_response = format!("✓ Inserted native {rows}×{cols} table for \"{chip}\" into document at caret.");
                        }
                    }
                    Err(e) => {
                        app.ui.ai_status = format!("Error: {e}");
                    }
                }
            }
        }
    });
    ui.add_space(4.0);

    let hint = match app.ui.ai_mode.as_str() {
        "Rewrite" => tl!("Prompt or tone for rewriting selected text…"),
        "Summarize" => tl!("Summarization instructions…"),
        "Proofread" => tl!("Proofreading instructions or focus…"),
        "Table" => tl!("Describe the table or paste markdown table rows…"),
        "Ask" => tl!("Ask a question about document content, stats, or tasks…"),
        _ => tl!("Describe what you want to draft…"),
    };

    ui.add(egui::TextEdit::multiline(&mut app.ui.ai_prompt).hint_text(hint).desired_rows(3).desired_width(f32::INFINITY));
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        let btn_label = match app.ui.ai_mode.as_str() {
            "Rewrite" => "Rewrite Selection",
            "Summarize" => "Summarize",
            "Proofread" => "Proofread",
            "Table" => "Generate Table",
            "Ask" => "Ask Document",
            _ => "Generate Draft",
        };
        let run_btn = ui.add(
            egui::Button::new(egui::RichText::new(tl!(btn_label)).font(semibold(12.5)).color(egui::Color32::WHITE))
                .fill(crate::theme::APP_COLOR)
                .min_size(vec2(140.0, 26.0)),
        );
        if run_btn.clicked() {
            let mode_lower = app.ui.ai_mode.to_lowercase();
            let tone = if mode_lower == "rewrite" {
                let p = app.ui.ai_prompt.to_lowercase();
                if p.contains("academic") {
                    "academic"
                } else if p.contains("executive") {
                    "executive"
                } else if p.contains("concise") {
                    "concise"
                } else if p.contains("casual") {
                    "casual"
                } else {
                    "professional"
                }
            } else if mode_lower == "summarize" {
                let p = app.ui.ai_prompt.to_lowercase();
                if p.contains("abstract") {
                    "abstract"
                } else if p.contains("executive") {
                    "executive"
                } else {
                    "bullets"
                }
            } else {
                "professional"
            };
            let mut payload = json!({
                "action": mode_lower,
                "prompt": app.ui.ai_prompt,
                "tone": tone,
                "provider": app.ui.ai_provider,
                "model": app.ui.ai_model,
            });
            if !app.ui.ai_api_key.trim().is_empty() {
                payload["api_key"] = json!(app.ui.ai_api_key);
            }
            if !app.ui.ai_base_url.trim().is_empty() {
                payload["base_url"] = json!(app.ui.ai_base_url);
            }
            app.ui.ai_status.clear();
            let r = app.session.run("tools.copilot", &payload);
            match r {
                Ok(res) => {
                    if let Some(c) = res.get("content").and_then(Value::as_str) {
                        app.ui.ai_response = c.to_string();
                    } else if mode_lower == "table" {
                        let rows = res.get("rows").and_then(Value::as_u64).unwrap_or(0);
                        let cols = res.get("cols").and_then(Value::as_u64).unwrap_or(0);
                        app.ui.ai_response = format!("✓ Inserted native {rows}×{cols} table for \"{}\" into document at caret.", app.ui.ai_prompt);
                    }
                }
                Err(e) => {
                    app.ui.ai_status = format!("Error: {e}");
                }
            }
        }
        if !app.ui.ai_response.is_empty() && ui.small_button(tl!("Clear")).clicked() {
            app.ui.ai_response.clear();
        }
    });

    if !app.ui.ai_response.is_empty() {
        ui.add_space(6.0);
        ui.separator();
        ui.label(egui::RichText::new(tl!("Result")).font(semibold(13.0)));

        let t = Tokens::get(ui.ctx());
        egui::Frame::NONE.fill(t.menu).inner_margin(8).stroke(Stroke::new(1.0, t.border)).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                ui.label(egui::RichText::new(&app.ui.ai_response).font(regular(12.0)));
            });
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button(tl!("Insert at Caret")).clicked() {
                let _ = app.run("text.insert", json!({"text": app.ui.ai_response}));
            }
            if has_sel && ui.button(tl!("Replace Selection")).clicked() {
                let _ = app.run("text.insert", json!({"text": app.ui.ai_response}));
            }
            if ui.button(tl!("Copy")).clicked() {
                ui.ctx().copy_text(app.ui.ai_response.clone());
                app.status("Copied to clipboard");
            }
        });
    }
}

#[cfg(test)]
mod navigation_tests {
    use super::*;

    #[test]
    fn long_headings_truncate_without_stretching_navigation() {
        let text = "An extremely long heading with words that must stay inside the navigation pane ".repeat(20);
        for (width, level) in [(260.0, 0), (180.0, 5)] {
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(900.0, 600.0));
            let mut panel_rect = egui::Rect::NOTHING;
            let mut heading_rect = egui::Rect::NOTHING;
            let output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
                let result = egui::Panel::left("navigation_test")
                    .default_size(width)
                    .resizable(true)
                    .show(ui, |ui| egui::ScrollArea::vertical().show(ui, |ui| nav_heading(ui, level, &text, true)).inner);
                panel_rect = result.response.rect;
                heading_rect = result.inner.rect;
            });

            assert!(panel_rect.width() < width + 20.0, "long heading expanded the {width}px pane to {panel_rect:?}");
            assert!(heading_rect.max.x <= panel_rect.max.x, "heading escaped pane: {heading_rect:?} / {panel_rect:?}");
            assert!(
                output.shapes.iter().any(|s| match &s.shape {
                    egui::Shape::Text(t) => {
                        t.galley.elided && t.galley.rows.last().is_some_and(|r| r.row.glyphs.last().is_some_and(|g| g.chr == '…'))
                    }
                    _ => false,
                }),
                "long heading must render a trailing ellipsis at {width}px"
            );
            output.drop_without_applying_deltas();
        }
    }
}
