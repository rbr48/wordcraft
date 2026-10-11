//! Dialogs: Font, Paragraph, Find & Replace, Go To, Insert Table, Page Setup, Link, Bookmark,
//! Word Count, Zoom, Watermark, New/Modify Style, Command search, About. Every dialog ends by
//! running a command, so agents get the same result without the dialog.

use egui::{Sense, Stroke, Ui, vec2};
use serde::Serialize;
use serde_json::{Value, json};

use crate::WordApp;
use crate::theme::{Tokens, semibold};

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "dialog", rename_all = "camelCase")]
pub enum Dialog {
    Font {
        font: String,
        size: String,
        bold: bool,
        italic: bool,
        underline_style: String,
        strike: bool,
        double_strike: bool,
        sup: bool,
        sub: bool,
        small_caps: bool,
        caps: bool,
        hidden: bool,
        color: String,
        spacing: f32,
        position: f32,
    },
    Paragraph {
        align: String,
        left: f32,
        right: f32,
        first: f32,
        before: f32,
        after: f32,
        line: f32,
        keep_next: bool,
        keep_lines: bool,
        page_break: bool,
        widow: bool,
    },
    Find {
        query: String,
        replace: String,
        match_case: bool,
        whole_word: bool,
        regex: bool,
        replace_mode: bool,
        message: String,
    },
    Goto {
        page: String,
    },
    InsertTable {
        rows: u32,
        cols: u32,
    },
    PageSetup {
        top: f32,
        bottom: f32,
        left: f32,
        right: f32,
        landscape: bool,
    },
    Link {
        url: String,
        text: String,
    },
    Bookmark {
        name: String,
    },
    WordCount {
        stats: Value,
    },
    Zoom {
        percent: f32,
    },
    Watermark {
        text: String,
        diagonal: bool,
    },
    NewStyle {
        name: String,
        based_on: String,
    },
    ModifyStyle {
        id: String,
        name: String,
        font: String,
        size: f32,
        bold: bool,
        italic: bool,
        color: String,
        before: f32,
        after: f32,
    },
    Commands {
        query: String,
    },
    /// Tabs: 0 About, 1 Contributors, 2 Models.
    About {
        tab: u8,
    },
    Accessibility {
        issues: Vec<Value>,
    },
    CustomizeKeyboard {
        category: String,
        query: String,
        selected_cmd: String,
        new_key: String,
    },
    BanglaConverter {
        source_text: String,
        result_text: String,
    },
    CheckUpdates {
        checking: bool,
        latest_version: String,
        release_notes: String,
        download_url: String,
        status_msg: String,
    },
}

impl Dialog {
    pub fn name(&self) -> &'static str {
        match self {
            Dialog::Font { .. } => "font",
            Dialog::Paragraph { .. } => "paragraph",
            Dialog::Find { replace_mode: false, .. } => "find",
            Dialog::Find { .. } => "replace",
            Dialog::Goto { .. } => "goto",
            Dialog::InsertTable { .. } => "insertTable",
            Dialog::PageSetup { .. } => "pageSetup",
            Dialog::Link { .. } => "link",
            Dialog::Bookmark { .. } => "bookmark",
            Dialog::WordCount { .. } => "wordCount",
            Dialog::Zoom { .. } => "zoom",
            Dialog::Watermark { .. } => "watermark",
            Dialog::NewStyle { .. } => "newStyle",
            Dialog::ModifyStyle { .. } => "modifyStyle",
            Dialog::Commands { .. } => "commands",
            Dialog::About { .. } => "about",
            Dialog::Accessibility { .. } => "accessibility",
            Dialog::CustomizeKeyboard { .. } => "customizeKeyboard",
            Dialog::BanglaConverter { .. } => "banglaConverter",
            Dialog::CheckUpdates { .. } => "checkUpdates",
        }
    }

    pub fn open(name: &str, app: &mut WordApp) -> Option<Dialog> {
        let st = app.session.run("format.state", &json!({})).unwrap_or_default();
        let s = |k: &str| st.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let b = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(false);
        Some(match name {
            "font" => Dialog::Font {
                font: s("font"),
                size: st.get("size").and_then(Value::as_f64).map(|v| v.to_string()).unwrap_or_default(),
                bold: b("bold"),
                italic: b("italic"),
                underline_style: if b("underline") { "single".into() } else { "none".into() },
                strike: b("strike"),
                double_strike: b("doubleStrike"),
                sup: b("superscript"),
                sub: b("subscript"),
                small_caps: false,
                caps: false,
                hidden: false,
                color: s("color"),
                spacing: 0.0,
                position: 0.0,
            },
            "paragraph" => {
                let rp = app.session.doc.para_at(&app.session.sel.focus).map(|p| app.session.doc.styles.resolve_para(&p.props));
                let rp = rp?;
                let line = match rp.line_spacing {
                    wordcraft_doc::props::LineSpacing::Multiple(m) => m,
                    _ => 1.0,
                };
                Dialog::Paragraph {
                    align: format!("{:?}", rp.align).to_lowercase(),
                    left: rp.indent_left / 72.0,
                    right: rp.indent_right / 72.0,
                    first: rp.indent_first / 72.0,
                    before: rp.space_before,
                    after: rp.space_after,
                    line,
                    keep_next: rp.keep_next,
                    keep_lines: rp.keep_lines,
                    page_break: rp.page_break_before,
                    widow: rp.widow_control,
                }
            }
            "find" | "replace" => Dialog::Find {
                query: if app.session.sel.is_collapsed() { app.session.find.query.clone() } else { app.session.selected_text() },
                replace: app.session.find.replace.clone(),
                match_case: app.session.find.match_case,
                whole_word: app.session.find.whole_word,
                regex: app.session.find.regex,
                replace_mode: name == "replace",
                message: String::new(),
            },
            "goto" => Dialog::Goto { page: String::new() },
            "insertTable" => Dialog::InsertTable { rows: 2, cols: 5 },
            "pageSetup" => {
                let sp = wordcraft_engine::cmd::page::sect(&app.session);
                Dialog::PageSetup {
                    top: sp.margin_top / 72.0,
                    bottom: sp.margin_bottom / 72.0,
                    left: sp.margin_left / 72.0,
                    right: sp.margin_right / 72.0,
                    landscape: sp.landscape,
                }
            }
            "link" => Dialog::Link { url: "https://".into(), text: app.session.selected_text() },
            "bookmark" => Dialog::Bookmark { name: String::new() },
            "wordCount" => Dialog::WordCount { stats: app.session.run("review.wordCount", &json!({})).unwrap_or_default() },
            "zoom" => Dialog::Zoom { percent: (app.session.view.zoom * 100.0).round() },
            "watermark" => Dialog::Watermark { text: "CONFIDENTIAL".into(), diagonal: true },
            "newStyle" => Dialog::NewStyle { name: "Style1".into(), based_on: "Normal".into() },
            "commands" => Dialog::Commands { query: String::new() },
            "about" => Dialog::About { tab: 0 },
            "contributors" => Dialog::About { tab: 1 },
            "models" => Dialog::About { tab: 2 },
            "accessibility" => {
                let r = app.session.run("file.accessibility", &json!({})).unwrap_or_default();
                let issues = r.get("issues").and_then(Value::as_array).cloned().unwrap_or_default();
                Dialog::Accessibility { issues }
            }
            "customizeKeyboard" => Dialog::CustomizeKeyboard {
                category: "All Commands".into(),
                query: String::new(),
                selected_cmd: String::new(),
                new_key: String::new(),
            },
            "banglaConverter" => Dialog::BanglaConverter {
                source_text: if app.session.sel.is_collapsed() { String::new() } else { app.session.selected_text() },
                result_text: String::new(),
            },
            "checkUpdates" => Dialog::CheckUpdates {
                checking: false,
                latest_version: env!("CARGO_PKG_VERSION").to_string(),
                release_notes: tl!("You are currently running WordCraft v0.4.1 (Latest Stable Release).").to_string(),
                download_url: "https://github.com/rbr48/wordcraft/releases/latest".into(),
                status_msg: tl!("Checking for updates from GitHub...").to_string(),
            },
            _ => return None,
        })
    }

    pub fn modify_style(app: &WordApp, id: &str) -> Option<Dialog> {
        let st = app.session.doc.styles.get(id)?;
        let rc = app.session.doc.styles.resolve_char(
            if st.kind == wordcraft_doc::StyleKind::Paragraph { Some(id) } else { None },
            &wordcraft_doc::CharProps {
                style: if st.kind == wordcraft_doc::StyleKind::Character { Some(id.into()) } else { None },
                ..Default::default()
            },
        );
        let rp = app.session.doc.styles.resolve_para(&wordcraft_doc::ParaProps { style: Some(id.into()), ..Default::default() });
        Some(Dialog::ModifyStyle {
            id: id.into(),
            name: st.name.clone(),
            font: rc.font,
            size: rc.size,
            bold: rc.bold,
            italic: rc.italic,
            color: match rc.color {
                wordcraft_doc::TextColor::Rgb(c) => c.hex(),
                wordcraft_doc::TextColor::Auto => "auto".into(),
            },
            before: rp.space_before,
            after: rp.space_after,
        })
    }
}

/// Grid picker for Insert › Table (hover to size, click to insert).
pub fn table_grid_picker(ui: &mut Ui, app: &mut WordApp) {
    let t = Tokens::get(ui.ctx());
    let id = egui::Id::new("table_picker_hover");
    let hover: (usize, usize) = ui.data(|d| d.get_temp(id)).unwrap_or((0, 0));
    ui.label(if hover.0 > 0 { format!("{}x{} Table", hover.1, hover.0) } else { "Insert Table".to_string() });
    let mut new_hover = (0, 0);
    let mut clicked = None;
    egui::Grid::new("tgp").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
        for r in 1..=8 {
            for c in 1..=10 {
                let (rect, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                let on = r <= hover.0 && c <= hover.1;
                ui.painter().rect(
                    rect,
                    1.0,
                    if on { t.checked } else { t.input },
                    egui::Stroke::new(1.0, if on { t.accent } else { t.border_strong }),
                    egui::StrokeKind::Inside,
                );
                if resp.hovered() {
                    new_hover = (r, c);
                }
                if resp.clicked() {
                    clicked = Some((r, c));
                }
            }
            ui.end_row();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, new_hover));
    if let Some((r, c)) = clicked {
        let _ = app.run("insert.table", json!({"rows": r, "cols": c}));
        ui.close();
    }
}

pub fn show(app: &mut WordApp, ctx: &egui::Context) {
    let Some(mut d) = app.dialog.take() else { return };
    let mut open = true;
    let mut close = false;
    let title = match &d {
        Dialog::Font { .. } => "Font",
        Dialog::Paragraph { .. } => "Paragraph",
        Dialog::Find { replace_mode: false, .. } => "Find",
        Dialog::Find { .. } => "Find and Replace",
        Dialog::Goto { .. } => "Go To",
        Dialog::InsertTable { .. } => "Insert Table",
        Dialog::PageSetup { .. } => "Page Setup",
        Dialog::Link { .. } => "Insert Hyperlink",
        Dialog::Bookmark { .. } => "Bookmark",
        Dialog::WordCount { .. } => "Word Count",
        Dialog::Zoom { .. } => "Zoom",
        Dialog::Watermark { .. } => "Custom Watermark",
        Dialog::NewStyle { .. } => "Create New Style",
        Dialog::ModifyStyle { .. } => "Modify Style",
        Dialog::Commands { .. } => "Search Commands",
        Dialog::About { .. } => "About WordCraft",
        Dialog::Accessibility { .. } => "Accessibility Checker",
        Dialog::CustomizeKeyboard { .. } => "Customize Keyboard",
        Dialog::BanglaConverter { .. } => "Bangla Converter",
        Dialog::CheckUpdates { .. } => "Check for Updates",
    };
    egui::Window::new(tl!(title))
        .id(egui::Id::new(("dialog", title)))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, -40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            close = body(app, ui, &mut d);
        });
    if open && !close {
        app.dialog = Some(d);
    } else {
        app.canvas.want_focus = true;
    }
}

fn buttons(ui: &mut Ui, ok: &str) -> (bool, bool) {
    let mut r = (false, false);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(tl!("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                r.1 = true;
            }
            let okb = ui.add(egui::Button::new(egui::RichText::new(ok).color(egui::Color32::WHITE)).fill(crate::theme::APP_COLOR));
            if okb.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                r.0 = true;
            }
        });
    });
    r
}

/// Returns true to close.
fn body(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    match d {
        Dialog::Font {
            font,
            size,
            bold,
            italic,
            underline_style,
            strike,
            double_strike,
            sup,
            sub,
            small_caps,
            caps,
            hidden,
            color,
            spacing,
            position,
        } => {
            egui::Grid::new("fontdlg").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                ui.label(tl!("Font:"));
                ui.text_edit_singleline(font);
                ui.end_row();
                ui.label(tl!("Size:"));
                ui.text_edit_singleline(size);
                ui.end_row();
                ui.label(tl!("Font color:"));
                ui.text_edit_singleline(color);
                ui.end_row();
                ui.label(tl!("Underline style:"));
                egui::ComboBox::from_id_salt("u_style").selected_text(underline_style.as_str()).show_ui(ui, |ui| {
                    for u in ["none", "single", "double", "thick", "dotted", "dash", "wave"] {
                        ui.selectable_value(underline_style, u.to_string(), u);
                    }
                });
                ui.end_row();
                ui.label(tl!("Spacing (pt):"));
                ui.add(egui::DragValue::new(spacing).speed(0.1).range(-20.0..=20.0));
                ui.end_row();
                ui.label(tl!("Position (pt):"));
                ui.add(egui::DragValue::new(position).speed(0.1).range(-100.0..=100.0));
                ui.end_row();
            });
            ui.separator();
            ui.label(egui::RichText::new(tl!("Effects")).font(semibold(12.5)));
            egui::Grid::new("fx").num_columns(2).show(ui, |ui| {
                ui.checkbox(bold, tl!("Bold"));
                ui.checkbox(italic, tl!("Italic"));
                ui.end_row();
                ui.checkbox(strike, tl!("Strikethrough"));
                ui.checkbox(double_strike, tl!("Double strikethrough"));
                ui.end_row();
                ui.checkbox(sup, tl!("Superscript"));
                ui.checkbox(sub, tl!("Subscript"));
                ui.end_row();
                ui.checkbox(small_caps, tl!("Small caps"));
                ui.checkbox(caps, tl!("All caps"));
                ui.end_row();
                ui.checkbox(hidden, tl!("Hidden"));
                ui.end_row();
            });
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let mut props = json!({
                    "bold": *bold, "italic": *italic, "strike": *strike, "doubleStrike": *double_strike,
                    "underline": underline_style.as_str(),
                    "vertAlign": if *sup { "superscript" } else if *sub { "subscript" } else { "baseline" },
                    "smallCaps": *small_caps, "caps": *caps, "hidden": *hidden,
                    "spacing": *spacing, "position": *position,
                });
                if !font.trim().is_empty() {
                    props["font"] = json!(font.trim());
                }
                if let Ok(s) = size.trim().parse::<f64>() {
                    props["size"] = json!(s);
                }
                if let Some(c) = wordcraft_doc::Rgb::parse(color) {
                    props["color"] = json!({"Rgb": [c.0, c.1, c.2]});
                }
                let _ = app.run("format.set", json!({"props": props}));
            }
            ok || cancel
        }
        Dialog::Paragraph { align, left, right, first, before, after, line, keep_next, keep_lines, page_break, widow } => {
            ui.label(egui::RichText::new(tl!("General")).font(semibold(12.5)));
            egui::ComboBox::from_label(tl!("Alignment")).selected_text(align.clone()).show_ui(ui, |ui| {
                for a in ["left", "center", "right", "justify"] {
                    ui.selectable_value(align, a.to_string(), a);
                }
            });
            ui.label(egui::RichText::new(tl!("Indentation")).font(semibold(12.5)));
            egui::Grid::new("ind").num_columns(4).show(ui, |ui| {
                ui.label(tl!("Left:"));
                ui.add(egui::DragValue::new(left).speed(0.05).suffix("\"").max_decimals(2));
                ui.label(tl!("Right:"));
                ui.add(egui::DragValue::new(right).speed(0.05).suffix("\"").max_decimals(2));
                ui.end_row();
                ui.label(tl!("First line:"));
                ui.add(egui::DragValue::new(first).speed(0.05).suffix("\"").max_decimals(2));
                ui.end_row();
            });
            ui.label(egui::RichText::new(tl!("Spacing")).font(semibold(12.5)));
            egui::Grid::new("sp").num_columns(4).show(ui, |ui| {
                ui.label(tl!("Before:"));
                ui.add(egui::DragValue::new(before).speed(1.0).range(0.0..=1584.0).suffix(" pt"));
                ui.label(tl!("Line spacing:"));
                let preset = match *line {
                    1.0 => "Single",
                    1.5 => "1.5 lines",
                    2.0 => "Double",
                    _ => "Multiple",
                };
                egui::ComboBox::from_id_salt("line_sp_mode").selected_text(preset).show_ui(ui, |ui| {
                    if ui.selectable_label(preset == "Single", "Single").clicked() {
                        *line = 1.0;
                    }
                    if ui.selectable_label(preset == "1.5 lines", "1.5 lines").clicked() {
                        *line = 1.5;
                    }
                    if ui.selectable_label(preset == "Double", "Double").clicked() {
                        *line = 2.0;
                    }
                    if ui.selectable_label(preset == "Multiple", "Multiple").clicked() {}
                });
                ui.end_row();
                ui.label(tl!("After:"));
                ui.add(egui::DragValue::new(after).speed(1.0).range(0.0..=1584.0).suffix(" pt"));
                ui.label(tl!("At:"));
                ui.add(egui::DragValue::new(line).speed(0.05).range(0.5..=5.0));
                ui.end_row();
            });
            ui.label(egui::RichText::new(tl!("Line and Page Breaks")).font(semibold(12.5)));
            ui.checkbox(widow, tl!("Widow/Orphan control"));
            ui.checkbox(keep_next, tl!("Keep with next"));
            ui.checkbox(keep_lines, tl!("Keep lines together"));
            ui.checkbox(page_break, tl!("Page break before"));
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("para.align", json!({"value": align}));
                let _ = app.run(
                    "para.set",
                    json!({"props": {
                        "indentLeft": *left * 72.0, "indentRight": *right * 72.0, "indentFirst": *first * 72.0,
                        "spaceBefore": *before, "spaceAfter": *after, "lineSpacing": {"rule": "multiple", "value": *line},
                        "keepNext": *keep_next, "keepLines": *keep_lines, "pageBreakBefore": *page_break, "widowControl": *widow,
                    }}),
                );
            }
            ok || cancel
        }
        Dialog::Find { query, replace, match_case, whole_word, regex, replace_mode, message } => {
            ui.horizontal(|ui| {
                if ui.selectable_label(!*replace_mode, tl!("Find")).clicked() {
                    *replace_mode = false;
                }
                if ui.selectable_label(*replace_mode, tl!("Replace")).clicked() {
                    *replace_mode = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label(tl!("Find what:"));
                ui.add(egui::TextEdit::singleline(query).desired_width(240.0));
            });
            if *replace_mode {
                ui.horizontal(|ui| {
                    ui.label(tl!("Replace with:"));
                    ui.add(egui::TextEdit::singleline(replace).desired_width(228.0));
                });
            }
            ui.horizontal(|ui| {
                ui.checkbox(match_case, tl!("Match case"));
                ui.checkbox(whole_word, tl!("Whole words"));
                ui.checkbox(regex, tl!("Wildcards (regex)"));
            });
            if !message.is_empty() {
                ui.label(egui::RichText::new(message.as_str()).weak());
            }
            let opts = json!({"text": query, "with": replace, "matchCase": *match_case, "wholeWord": *whole_word, "regex": *regex});
            let mut close = false;
            ui.horizontal(|ui| {
                if *replace_mode {
                    if ui.button(tl!("Replace All")).clicked() {
                        match app.run("edit.replaceAll", opts.clone()) {
                            Ok(r) => *message = format!("All done. We made {} replacements.", r["replaced"]),
                            Err(e) => *message = e,
                        }
                    }
                    if ui.button(tl!("Replace")).clicked() {
                        let _ = app.run("edit.replace", opts.clone());
                    }
                }
                if ui.button(tl!("Find Next")).clicked() || (ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift)) {
                    let _ = app.session.run("edit.find", &opts);
                    match app.run("edit.findNext", json!({})) {
                        Ok(r) => *message = format!("Match {} of {}", r["index"].as_u64().unwrap_or(0) + 1, r["count"]),
                        Err(e) => *message = e,
                    }
                }
                if ui.button(tl!("Find Previous")).clicked() || (ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.shift)) {
                    let _ = app.session.run("edit.find", &opts);
                    match app.run("edit.findPrevious", json!({})) {
                        Ok(r) => *message = format!("Match {} of {}", r["index"].as_u64().unwrap_or(0) + 1, r["count"]),
                        Err(e) => *message = e,
                    }
                }
                if ui.button(tl!("Close")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close = true;
                }
            });
            close
        }
        Dialog::Goto { page } => {
            let mut submit = false;
            ui.horizontal(|ui| {
                ui.label(tl!("Enter page number or section (e.g. 3, s2):"));
                let resp = ui.text_edit_singleline(page);
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
            });
            let (ok, cancel) = buttons(ui, tl!("Go To"));
            if ok || submit {
                let trimmed = page.trim();
                if let Some(sec) = trimmed.strip_prefix('s').or_else(|| trimmed.strip_prefix('S')) {
                    if let Ok(n) = sec.parse::<u64>() {
                        let _ = app.run("edit.goto", json!({"section": n}));
                    }
                } else if let Ok(n) = trimmed.parse::<u64>() {
                    let _ = app.run("edit.goto", json!({"page": n}));
                } else if !trimmed.is_empty() {
                    let _ = app.run("edit.goto", json!({"bookmark": trimmed}));
                }
                return true;
            }
            cancel
        }
        Dialog::InsertTable { rows, cols } => {
            egui::Grid::new("it").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Number of columns:"));
                ui.add(egui::DragValue::new(cols).range(1..=63));
                ui.end_row();
                ui.label(tl!("Number of rows:"));
                ui.add(egui::DragValue::new(rows).range(1..=1000));
                ui.end_row();
            });
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("insert.table", json!({"rows": *rows, "cols": *cols}));
            }
            ok || cancel
        }
        Dialog::PageSetup { top, bottom, left, right, landscape } => {
            ui.label(egui::RichText::new(tl!("Margins")).font(semibold(12.5)));
            egui::Grid::new("ps").num_columns(4).show(ui, |ui| {
                ui.label(tl!("Top:"));
                ui.add(egui::DragValue::new(top).speed(0.05).suffix("\"").max_decimals(2));
                ui.label(tl!("Bottom:"));
                ui.add(egui::DragValue::new(bottom).speed(0.05).suffix("\"").max_decimals(2));
                ui.end_row();
                ui.label(tl!("Left:"));
                ui.add(egui::DragValue::new(left).speed(0.05).suffix("\"").max_decimals(2));
                ui.label(tl!("Right:"));
                ui.add(egui::DragValue::new(right).speed(0.05).suffix("\"").max_decimals(2));
                ui.end_row();
            });
            ui.label(egui::RichText::new(tl!("Orientation")).font(semibold(12.5)));
            ui.horizontal(|ui| {
                ui.radio_value(landscape, false, tl!("Portrait"));
                ui.radio_value(landscape, true, tl!("Landscape"));
            });
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("layout.orientation", json!({"value": if *landscape { "landscape" } else { "portrait" }}));
                let _ =
                    app.run("layout.margins", json!({"top": *top * 72.0, "bottom": *bottom * 72.0, "left": *left * 72.0, "right": *right * 72.0}));
            }
            ok || cancel
        }
        Dialog::Link { url, text } => {
            egui::Grid::new("lnk").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Text to display:"));
                ui.text_edit_singleline(text);
                ui.end_row();
                ui.label(tl!("Address:"));
                ui.text_edit_singleline(url);
                ui.end_row();
            });
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("insert.link", json!({"url": url, "text": text}));
            }
            ok || cancel
        }
        Dialog::Bookmark { name } => {
            ui.label(tl!("Bookmark name:"));
            ui.text_edit_singleline(name);
            let marks = app.session.doc.bookmarks();
            for (n, _) in &marks {
                if ui.selectable_label(false, n).clicked() {
                    let _ = app.run("edit.goto", json!({"bookmark": n}));
                }
            }
            let (ok, cancel) = buttons(ui, tl!("Add"));
            if ok {
                let _ = app.run("insert.bookmark", json!({"name": name}));
            }
            ok || cancel
        }
        Dialog::WordCount { stats } => {
            egui::Grid::new("wc").num_columns(2).spacing(vec2(30.0, 6.0)).show(ui, |ui| {
                for (l, k) in [
                    ("Pages", "pages"),
                    ("Words", "words"),
                    ("Characters (no spaces)", "characters"),
                    ("Characters (with spaces)", "charactersWithSpaces"),
                    ("Paragraphs", "paragraphs"),
                    ("Lines", "lines"),
                ] {
                    ui.label(tl!(l));
                    ui.label(stats.get(k).map(|v| v.to_string()).unwrap_or_default());
                    ui.end_row();
                }
            });
            let (ok, cancel) = buttons(ui, tl!("Close"));
            ok || cancel
        }
        Dialog::Zoom { percent } => {
            ui.horizontal(|ui| {
                for p in [200.0, 100.0, 75.0] {
                    ui.radio_value(percent, p, format!("{p}%"));
                }
            });
            ui.add(egui::Slider::new(percent, 10.0..=500.0).suffix("%"));
            let mut fit = None;
            ui.horizontal(|ui| {
                if ui.button(tl!("Page width")).clicked() {
                    fit = Some("pageWidth");
                }
                if ui.button(tl!("Whole page")).clicked() {
                    fit = Some("onePage");
                }
                if ui.button(tl!("Many pages")).clicked() {
                    fit = Some("multiplePages");
                }
            });
            if let Some(f) = fit {
                let _ = app.run("view.zoom", json!({"value": f}));
                return true;
            }
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("view.zoom", json!({"value": *percent}));
            }
            ok || cancel
        }
        Dialog::Watermark { text, diagonal } => {
            ui.horizontal(|ui| {
                ui.label(tl!("Text:"));
                ui.text_edit_singleline(text);
            });
            ui.horizontal(|ui| {
                ui.label(tl!("Presets:"));
                for p in ["CONFIDENTIAL", "DO NOT COPY", "DRAFT", "URGENT", "SAMPLE"] {
                    if ui.button(p).clicked() {
                        *text = p.to_string();
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.radio_value(diagonal, true, tl!("Diagonal"));
                ui.radio_value(diagonal, false, tl!("Horizontal"));
            });
            let mut remove = false;
            ui.horizontal(|ui| {
                if ui.button(tl!("Remove Watermark")).clicked() {
                    remove = true;
                }
            });
            if remove {
                let _ = app.run("design.watermark", json!({"remove": true}));
                return true;
            }
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("design.watermark", json!({"text": text, "diagonal": *diagonal}));
            }
            ok || cancel
        }
        Dialog::NewStyle { name, based_on } => {
            egui::Grid::new("ns").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Name:"));
                ui.text_edit_singleline(name);
                ui.end_row();
                ui.label(tl!("Style based on:"));
                ui.text_edit_singleline(based_on);
                ui.end_row();
            });
            ui.label(egui::RichText::new(tl!("The new style takes the formatting of the current paragraph.")).small().weak());
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let _ = app.run("styles.create", json!({"name": name, "basedOn": based_on}));
            }
            ok || cancel
        }
        Dialog::ModifyStyle { id, name, font, size, bold, italic, color, before, after } => {
            egui::Grid::new("ms").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Name:"));
                ui.text_edit_singleline(name);
                ui.end_row();
                ui.label(tl!("Font:"));
                ui.text_edit_singleline(font);
                ui.end_row();
                ui.label(tl!("Size:"));
                ui.add(egui::DragValue::new(size).range(1.0..=1638.0).speed(0.5));
                ui.end_row();
                ui.label(tl!("Color:"));
                ui.text_edit_singleline(color);
                ui.end_row();
                ui.label(tl!("Space before/after:"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(before).range(0.0..=1584.0));
                    ui.add(egui::DragValue::new(after).range(0.0..=1584.0));
                });
                ui.end_row();
            });
            ui.horizontal(|ui| {
                ui.checkbox(bold, tl!("Bold"));
                ui.checkbox(italic, tl!("Italic"));
            });
            let (ok, cancel) = buttons(ui, tl!("OK"));
            if ok {
                let mut chr = json!({"font": font, "size": *size, "bold": *bold, "italic": *italic});
                if let Some(c) = wordcraft_doc::Rgb::parse(color) {
                    chr["color"] = json!({"Rgb": [c.0, c.1, c.2]});
                }
                let _ =
                    app.run("styles.modify", json!({"style": id, "name": name, "chr": chr, "para": {"spaceBefore": *before, "spaceAfter": *after}}));
            }
            ok || cancel
        }
        Dialog::Commands { query } => {
            let r = ui.add(egui::TextEdit::singleline(query).hint_text(tl!("Type a command, e.g. \"insert table\"")).desired_width(380.0));
            r.request_focus();
            let q = query.to_lowercase();
            let reg = app.session.registry.clone();
            let mut hits: Vec<&wordcraft_engine::CommandSpec> = reg
                .all()
                .iter()
                .filter(|c| {
                    !q.is_empty()
                        && (c.label.to_lowercase().contains(&q)
                            || tl!(c.label).to_lowercase().contains(&q)
                            || c.id.to_lowercase().contains(&q)
                            || c.location.to_lowercase().contains(&q)
                            || crate::i18n::location(c.location).to_lowercase().contains(&q))
                })
                .collect();
            hits.truncate(14);
            let mut close = false;
            for c in hits {
                let sc = crate::widgets::shortcut_text(app, c.id);
                let label = format!("{}   —   {}", tl!(c.label), crate::i18n::location(c.location));
                if ui.add(egui::Button::new(label).shortcut_text(sc).min_size(vec2(380.0, 0.0))).clicked() {
                    let _ = app.run(c.id, json!({}));
                    close = true;
                }
            }
            close || ui.input(|i| i.key_pressed(egui::Key::Escape))
        }
        Dialog::About { tab } => {
            ui.set_width(660.0);
            ui.horizontal(|ui| {
                for (i, l) in ["About", "Contributors", "Models"].into_iter().enumerate() {
                    let i = i as u8;
                    if ui.selectable_label(*tab == i, tl!(l)).clicked() {
                        *tab = i;
                    }
                }
            });
            ui.separator();
            match *tab {
                1 => crate::credits::contributors_ui(ui),
                2 => crate::credits::models_ui(ui),
                _ => {
                    ui.label(egui::RichText::new(tl!("WordCraft")).font(semibold(22.0)));
                    let build = option_env!("WORDCRAFT_BUILD_SHA").map(|s| s.get(..8).unwrap_or(s)).unwrap_or(tl!("development build"));
                    ui.label(crate::i18n::fmt(tl!("Version {version} ({build})"), &[("version", env!("CARGO_PKG_VERSION")), ("build", build)]));
                    ui.label(tl!(
                        "A clean-room, open-source word processor written in native Rust.\nLead Developer & Top Contributor: M S Rana (@rbr48)."
                    ));
                    ui.add_space(6.0);
                    ui.hyperlink_to(tl!("Source code: github.com/rbr48/wordcraft"), "https://github.com/rbr48/wordcraft");
                    ui.add_space(8.0);
                    if ui.button(tl!("Check for Updates...")).clicked() {
                        let _ = app.run("ui.dialog", json!({"name": "checkUpdates"}));
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(tl!("MIT OR Apache-2.0. Copyright (c) 2026 M S Rana and WordCraft contributors.")).small().weak());
                }
            }
            let (ok, cancel) = buttons(ui, tl!("Close"));
            ok || cancel
        }
        Dialog::Accessibility { issues } => {
            ui.set_width(480.0);
            ui.label(egui::RichText::new(tl!("Accessibility Checker")).font(semibold(16.0)));
            ui.add_space(6.0);
            if issues.is_empty() {
                ui.label(tl!("No accessibility issues found. People with disabilities should have no difficulty reading this document."));
            } else {
                ui.label(crate::i18n::fmt(tl!("{count} issue(s) found:"), &[("count", &issues.len().to_string())]));
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                    for it in issues {
                        let kind = it.get("kind").and_then(Value::as_str).unwrap_or("info");
                        let issue = it.get("issue").and_then(Value::as_str).unwrap_or("");
                        let fix = it.get("fix").and_then(Value::as_str).unwrap_or("");
                        let (icon_color, tag) = match kind {
                            "error" => (egui::Color32::from_rgb(0xD1, 0x3B, 0x3B), "Error"),
                            "warning" => (egui::Color32::from_rgb(0xE0, 0x7B, 0x1F), "Warning"),
                            _ => (egui::Color32::from_rgb(0x2B, 0x47, 0xB5), "Tip"),
                        };
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(tag).color(icon_color).strong());
                                ui.label(issue);
                            });
                            if !fix.is_empty() {
                                ui.label(egui::RichText::new(format!("How to fix: {fix}")).small().weak());
                            }
                        });
                    }
                });
            }
            let (ok, cancel) = buttons(ui, tl!("Close"));
            ok || cancel
        }
        Dialog::CustomizeKeyboard { category, query, selected_cmd, new_key } => {
            ui.set_width(520.0);
            ui.horizontal(|ui| {
                ui.label(tl!("Categories:"));
                egui::ComboBox::from_id_salt("cat_picker").selected_text(category.as_str()).width(180.0).show_ui(ui, |ui| {
                    for cat in ["All Commands", "File", "Home", "Insert", "Layout", "Review", "View", "Tools"] {
                        if ui.selectable_label(category.as_str() == cat, cat).clicked() {
                            *category = cat.into();
                        }
                    }
                });
                ui.add_space(8.0);
                ui.add(egui::TextEdit::singleline(query).hint_text(tl!("Filter commands…")).desired_width(180.0));
            });
            ui.add_space(6.0);

            ui.label(tl!("Commands:"));
            let q = query.to_lowercase();
            let cat = category.clone();
            let reg = app.session.registry.clone();
            let all_specs = reg.all();

            let filtered: Vec<&wordcraft_engine::CommandSpec> = all_specs
                .iter()
                .filter(|c| {
                    let cat_match = if cat == "All Commands" { true } else { c.location.to_lowercase().contains(&cat.to_lowercase()) };
                    let query_match = q.is_empty()
                        || c.label.to_lowercase().contains(&q)
                        || c.id.to_lowercase().contains(&q)
                        || tl!(c.label).to_lowercase().contains(&q);
                    cat_match && query_match
                })
                .collect();

            let mut chosen = selected_cmd.clone();
            egui::Frame::NONE
                .fill(ui.visuals().extreme_bg_color)
                .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
                .inner_margin(4)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(170.0).show(ui, |ui| {
                        for c in &filtered {
                            let is_sel = *selected_cmd == c.id;
                            let sc = crate::widgets::shortcut_text(app, c.id);
                            let text = if sc.is_empty() {
                                format!("{} ({})", tl!(c.label), c.id)
                            } else {
                                format!("{} ({})   [{}]", tl!(c.label), c.id, sc)
                            };
                            if ui.selectable_label(is_sel, text).clicked() {
                                chosen = c.id.to_string();
                            }
                        }
                    });
                });
            *selected_cmd = chosen;

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            let cur_sc = if !selected_cmd.is_empty() {
                let sc = crate::widgets::shortcut_text(app, selected_cmd);
                if sc.is_empty() { "None".to_string() } else { sc }
            } else {
                "Select a command above".to_string()
            };

            ui.horizontal(|ui| {
                ui.label(tl!("Current shortcut:"));
                ui.label(egui::RichText::new(&cur_sc).strong().color(crate::theme::APP_COLOR));
            });
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(tl!("Press new shortcut key:"));
                ui.add(egui::TextEdit::singleline(new_key).desired_width(160.0).hint_text("e.g. Ctrl+Alt+K, Mod+Shift+T"));
                if ui.small_button("+Ctrl").clicked() && !new_key.contains("Ctrl+") && !new_key.contains("Mod+") {
                    *new_key = format!("Ctrl+{new_key}");
                }
                if ui.small_button("+Shift").clicked() && !new_key.contains("Shift+") {
                    *new_key = format!("Shift+{new_key}");
                }
                if ui.small_button("+Alt").clicked() && !new_key.contains("Alt+") {
                    *new_key = format!("Alt+{new_key}");
                }
            });

            ui.add_space(8.0);
            let mut close = false;
            ui.horizontal(|ui| {
                let can_assign = !selected_cmd.is_empty() && !new_key.trim().is_empty();
                if ui.add_enabled(can_assign, egui::Button::new(tl!("Assign"))).clicked() {
                    let standard_key = new_key.replace("Ctrl+", "Mod+").trim().to_string();
                    let _ = app.run("tools.customizeKeyboard", json!({"command": selected_cmd, "key": standard_key}));
                    app.status(format!("Assigned shortcut {standard_key} to {selected_cmd}"));
                    new_key.clear();
                }
                let can_remove = !selected_cmd.is_empty() && cur_sc != "None";
                if ui.add_enabled(can_remove, egui::Button::new(tl!("Remove"))).clicked() {
                    let _ = app.run("tools.customizeKeyboard", json!({"command": selected_cmd, "key": ""}));
                    app.status(format!("Removed shortcut for {selected_cmd}"));
                }
                if ui.button(tl!("Reset All")).clicked() {
                    let _ = app.run("tools.customizeKeyboard", json!({"reset": true}));
                    app.status("Reset all shortcuts to defaults");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tl!("Close")).clicked() {
                        close = true;
                    }
                });
            });
            close || ui.input(|i| i.key_pressed(egui::Key::Escape))
        }
        Dialog::BanglaConverter { source_text, result_text } => {
            ui.heading(tl!("Bangla Converter (Unicode ⇄ ANSI / SutonnyMJ)"));
            ui.label(tl!("Convert text between modern Unicode and legacy Bijoy/SutonnyMJ ANSI without external software."));
            ui.add_space(8.0);

            ui.label(tl!("Input Text:"));
            ui.add(egui::TextEdit::multiline(source_text).desired_rows(5).desired_width(f32::INFINITY));

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(tl!("Unicode ➔ ANSI (SutonnyMJ)")).clicked() {
                    *result_text = wordcraft_engine::cmd::bangla::unicode_to_ansi(source_text);
                }
                if ui.button(tl!("ANSI (SutonnyMJ) ➔ Unicode")).clicked() {
                    *result_text = wordcraft_engine::cmd::bangla::ansi_to_unicode(source_text);
                }
                if ui.button(tl!("Phonetic ➔ Unicode")).clicked() {
                    *result_text = wordcraft_engine::cmd::bangla::phonetic_transliterate(source_text);
                }
                if ui.button(tl!("Clear")).clicked() {
                    source_text.clear();
                    result_text.clear();
                }
            });

            ui.add_space(8.0);
            ui.label(tl!("Converted Result:"));
            ui.add(egui::TextEdit::multiline(result_text).desired_rows(5).desired_width(f32::INFINITY));

            ui.add_space(6.0);
            let mut close = false;
            ui.horizontal(|ui| {
                if ui.button(tl!("Copy Result")).clicked() && !result_text.is_empty() {
                    ui.ctx().copy_text(result_text.clone());
                    app.status(tl!("Copied converted text to clipboard"));
                }
                if ui.button(tl!("Insert at Caret")).clicked() && !result_text.is_empty() {
                    let _ = app.run("text.insert", json!({"text": result_text}));
                    close = true;
                }
                ui.separator();
                let has_sel = !app.session.sel.is_collapsed();
                if ui.add_enabled(has_sel, egui::Button::new(tl!("Convert Selection to ANSI"))).clicked() {
                    let _ = app.run("tools.banglaConvert", json!({"direction": "unicodeToAnsi", "target": "selection"}));
                    close = true;
                }
                if ui.add_enabled(has_sel, egui::Button::new(tl!("Convert Selection to Unicode"))).clicked() {
                    let _ = app.run("tools.banglaConvert", json!({"direction": "ansiToUnicode", "target": "selection"}));
                    close = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tl!("Close")).clicked() {
                        close = true;
                    }
                });
            });
            close || ui.input(|i| i.key_pressed(egui::Key::Escape))
        }
        Dialog::CheckUpdates { checking: _, latest_version, release_notes, download_url, status_msg } => {
            ui.set_width(460.0);
            ui.label(egui::RichText::new(tl!("WordCraft Update Center")).font(semibold(18.0)));
            ui.add_space(4.0);

            let current_v = env!("CARGO_PKG_VERSION");
            ui.horizontal(|ui| {
                ui.label(tl!("Current Version:"));
                ui.label(egui::RichText::new(format!("v{current_v}")).font(semibold(13.0)));
            });
            ui.horizontal(|ui| {
                ui.label(tl!("Latest Version:"));
                ui.label(egui::RichText::new(format!("v{latest_version}")).color(egui::Color32::from_rgb(0x10, 0x7C, 0x41)).font(semibold(13.0)));
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            ui.label(egui::RichText::new(status_msg.as_str()).italics());
            ui.add_space(6.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new(tl!("Release Information:")).font(semibold(12.0)));
                ui.label(release_notes.as_str());
            });

            ui.add_space(10.0);
            let mut close = false;
            ui.horizontal(|ui| {
                if ui.button(tl!("Visit Releases / Download")).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(download_url.as_str()));
                }
                if ui.button(tl!("Download Windows Setup")).clicked() {
                    let setup_url = "https://github.com/rbr48/wordcraft/releases/latest/download/wordcraft-0.4.1-windows-x64-setup.exe";
                    ui.ctx().open_url(egui::OpenUrl::new_tab(setup_url));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tl!("Close")).clicked() {
                        close = true;
                    }
                });
            });
            close || ui.input(|i| i.key_pressed(egui::Key::Escape))
        }
    }
}
