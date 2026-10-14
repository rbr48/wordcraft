//! File tab (Backstage): Home/New, Open, Info, Save As, Print, Export, Options, About.

use egui::{Align2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

use crate::theme::{APP_COLOR, Tokens, medium, regular, semibold};
use crate::{WordApp, icons};

const PAGES: [(&str, &str); 9] = [
    ("home", "Home"),
    ("new", "New"),
    ("open", "Open"),
    ("info", "Info"),
    ("save", "Save"),
    ("saveAs", "Save As"),
    ("print", "Print"),
    ("export", "Export"),
    ("options", "Options"),
];

pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let mac = app.integrated_titlebar;
    egui::Panel::left("backstage_nav")
        .exact_size(200.0)
        .frame(egui::Frame::NONE.fill(APP_COLOR).inner_margin(egui::Margin { left: 0, right: 0, top: if mac { 0 } else { 12 }, bottom: 12 }))
        .show(ui, |ui| {
            let back = if mac {
                // macOS (#222): the Backstage covers the title bar, so its top band is the title
                // bar: the back button sits beside the traffic lights, centred with them and as far
                // from them as the title bar's own controls, and the rest of the band drags the window.
                let (band, drag) = ui.allocate_exact_size(vec2(200.0, crate::chrome::MAC_TITLE_BAR), Sense::click_and_drag());
                if drag.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                // The chevron's stroke starts at the content edge.
                let c = pos2(band.min.x + crate::chrome::MAC_CONTENT_LEFT + 3.0, band.center().y);
                let resp = ui.interact(Rect::from_center_size(c, vec2(22.0, band.height())), ui.id().with("backstage_back"), Sense::click());
                icons::paint(ui.painter(), Rect::from_center_size(c, vec2(18.0, 18.0)), "chevronLeft", egui::Color32::WHITE, egui::Color32::WHITE);
                resp
            } else {
                let (r, resp) = ui.allocate_exact_size(vec2(200.0, 40.0), Sense::click());
                icons::paint(
                    ui.painter(),
                    Rect::from_center_size(pos2(r.min.x + 26.0, r.center().y), vec2(18.0, 18.0)),
                    "chevronLeft",
                    egui::Color32::WHITE,
                    egui::Color32::WHITE,
                );
                resp
            };
            if back.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                app.ui.backstage = false;
                app.canvas.want_focus = true;
            }
            ui.add_space(if mac { 12.0 } else { 6.0 });
            for (id, label) in PAGES {
                let (r, resp) = ui.allocate_exact_size(vec2(200.0, 38.0), Sense::click());
                let active = app.ui.backstage_page == id;
                if active {
                    ui.painter().rect_filled(r, 0.0, egui::Color32::from_white_alpha(46));
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, egui::Color32::from_white_alpha(26));
                }
                ui.painter().text(
                    pos2(r.min.x + 22.0, r.center().y),
                    Align2::LEFT_CENTER,
                    tl!(label),
                    if active { semibold(14.0) } else { medium(14.0) },
                    egui::Color32::WHITE,
                );
                if resp.clicked() {
                    match id {
                        "save" => {
                            let _ = app.run("file.save", json!({}));
                        }
                        "saveAs" => {
                            app.save_as_dialog();
                        }
                        "open" => {
                            app.ui.backstage_page = id.into();
                        }
                        _ => app.ui.backstage_page = id.into(),
                    }
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(200.0, 32.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, egui::Color32::from_white_alpha(26));
                }
                ui.painter().text(pos2(r.min.x + 22.0, r.center().y), Align2::LEFT_CENTER, tl!("About"), regular(13.0), egui::Color32::WHITE);
                if resp.clicked() {
                    app.dialog = crate::dialogs::Dialog::open("about", app);
                }
            });
        });
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.ribbon).inner_margin(egui::Margin::symmetric(40, 30))).show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| match app.ui.backstage_page.as_str() {
            "home" => home_page(app, ui),
            "new" => new_page(app, ui),
            "open" => open_page(app, ui),
            "info" => info_page(app, ui),
            "print" => print_page(app, ui),
            "export" => export_page(app, ui),
            "options" => options_page(app, ui),
            _ => home_page(app, ui),
        });
    });
}

fn heading(ui: &mut Ui, s: &str) {
    ui.label(egui::RichText::new(tl!(s)).font(semibold(26.0)));
    ui.add_space(16.0);
}

fn template_tile(ui: &mut Ui, app: &mut WordApp, label: &str, template: &str) {
    let t = Tokens::get(ui.ctx());
    ui.vertical(|ui| {
        // The label sits right under the thumbnail whatever row gap the gallery uses.
        ui.spacing_mut().item_spacing.y = 0.0;
        let (r, resp) = ui.allocate_exact_size(vec2(150.0, 194.0), Sense::click());
        ui.painter().rect(
            r,
            2.0,
            egui::Color32::WHITE,
            Stroke::new(if resp.hovered() { 2.0 } else { 1.0 }, if resp.hovered() { t.accent } else { t.border_strong }),
            egui::StrokeKind::Inside,
        );
        let ppp = ui.ctx().pixels_per_point();
        let key = format!("tpl:{template}:{ppp}");
        let tex = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(egui::Id::new(&key))).or_else(|| {
            let mut s = wordcraft_engine::Session::new(wordcraft_doc::Document::new());
            let _ = s.run("file.new", &json!({"template": template}));
            let l = s.export_layout();
            let page = l.pages.first()?;
            let img = wordcraft_render::render_page(&s.doc, page, 150.0 / page.w * ppp, &crate::canvas::screen_render_options());
            let px = img.to_straight();
            let h = ui.ctx().load_texture(
                &key,
                egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &px),
                egui::TextureOptions::LINEAR,
            );
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(&key), h.clone()));
            Some(h)
        });
        if let Some(h) = tex {
            ui.painter().image(h.id(), r.shrink(1.0), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        ui.label(egui::RichText::new(tl!(label)).font(regular(12.5)));
        if resp.clicked() {
            let _ = app.run("file.new", json!({"template": template}));
            app.ui.backstage = false;
            app.canvas.want_focus = true;
        }
    });
}

/// The built-in templates in gallery order: display label and the `file.new` template id.
const TEMPLATES: [(&str, &str); 5] =
    [("Blank document", "blank"), ("Studio handbook (sample)", "sample"), ("Letter", "letter"), ("Résumé", "resume"), ("Report", "report")];

/// How many templates Home shows before "More templates" (the rest are on New).
const HOME_TEMPLATES: usize = 4;

/// The templates whose English or translated label, or id, contains `query` (case-insensitive;
/// the id lets "resume" find "Résumé"); all of them for an empty query.
fn matching_templates(query: &str) -> Vec<(&'static str, &'static str)> {
    let q = query.trim().to_lowercase();
    TEMPLATES
        .into_iter()
        .filter(|(label, id)| q.is_empty() || id.contains(&q) || label.to_lowercase().contains(&q) || tl!(label).to_lowercase().contains(&q))
        .collect()
}

/// Home: a greeting, Blank plus a short row of templates with a link to New, then Recent.
fn home_page(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let hour = wordcraft_engine::cmd::now_iso().get(11..13).and_then(|h| h.parse::<u32>().ok()).unwrap_or(9);
    heading(
        ui,
        if hour < 12 {
            "Good morning"
        } else if hour < 18 {
            "Good afternoon"
        } else {
            "Good evening"
        },
    );
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(18.0, 0.0);
        for (label, template) in TEMPLATES.into_iter().take(HOME_TEMPLATES) {
            template_tile(ui, app, label, template);
        }
    });
    ui.add_space(8.0);
    let more = ui.add(egui::Label::new(egui::RichText::new(tl!("More templates →")).font(medium(13.5)).color(t.accent)).sense(Sense::click()));
    if more.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        app.ui.backstage_page = "new".into();
    }
    ui.add_space(24.0);
    open_list(app, ui);
}

/// New: the whole template gallery with a search box; no Recent list (that is on Home and Open).
fn new_page(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    heading(ui, "New");
    let id = egui::Id::new("backstage_template_search");
    let mut query = ui.ctx().data(|d| d.get_temp::<String>(id)).unwrap_or_default();
    let search = ui.add(egui::TextEdit::singleline(&mut query).hint_text(tl!("Search for templates")).desired_width(360.0));
    if search.changed() {
        ui.ctx().data_mut(|d| d.insert_temp(id, query.clone()));
    }
    ui.add_space(18.0);
    let found = matching_templates(&query);
    if found.is_empty() {
        ui.label(egui::RichText::new(tl!("No templates match your search.")).color(t.text_dim));
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(18.0, 18.0);
        for (label, template) in found {
            template_tile(ui, app, label, template);
        }
    });
}

fn open_list(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(tl!("Recent")).font(semibold(16.0)));
    ui.add_space(6.0);
    if app.ui.recent.is_empty() {
        ui.label(egui::RichText::new(tl!("Documents you open will show up here.")).color(t.text_dim));
    }
    for p in app.ui.recent.clone() {
        let name = std::path::Path::new(&p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(p.clone());
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(700.0), 40.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 4.0, t.hover);
        }
        icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 18.0, r.center().y), vec2(22.0, 22.0)), "printLayout", t.icon, t.accent);
        ui.painter().text(pos2(r.min.x + 40.0, r.min.y + 13.0), Align2::LEFT_CENTER, &name, medium(13.0), t.text);
        ui.painter().text(pos2(r.min.x + 40.0, r.min.y + 29.0), Align2::LEFT_CENTER, &p, regular(11.0), t.text_dim);
        if resp.clicked() {
            let _ = app.run("file.open", json!({"path": p}));
            app.ui.backstage = false;
            app.canvas.want_focus = true;
        }
    }
}

fn open_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, "Open");
    if ui.button(egui::RichText::new(tl!("Browse…")).font(medium(14.0))).clicked() {
        let _ = app.run("ui.openFileDialog", json!({}));
    }
    ui.add_space(18.0);
    open_list(app, ui);
}

fn info_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, "Info");
    let info = app.session.run("file.info", &json!({})).unwrap_or_default();
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(egui::RichText::new(tl!("Properties")).font(semibold(15.0)));
        // The fields edit a copy of the document's properties that is read afresh every frame,
        // so each change goes into the document as it is typed (#260): waiting for the field to
        // lose focus dropped the keystrokes, and leaving File with Escape never loses it at all.
        // The keystrokes of one visit to a field are a single undo step.
        let mut props = app.session.doc.core.clone();
        let mut edited = None;
        let mut focused = None;
        egui::Grid::new("props").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
            for (l, v) in [("Title", &mut props.title), ("Subject", &mut props.subject), ("Author", &mut props.creator), ("Keywords", &mut props.keywords), ("Category", &mut props.category)] {
                let label = ui.label(tl!(l));
                let r = ui.add(egui::TextEdit::singleline(v).id_salt(("info-prop", l))).labelled_by(label.id);
                if r.changed() {
                    edited = Some(l);
                }
                if r.has_focus() {
                    focused = Some(l);
                }
                ui.end_row();
            }
        });
        if let Some(field) = edited {
            if app.info_editing == Some((field, app.session.rev())) {
                app.session.join_next_undo();
            }
            let ok = app.run("file.properties", json!({"title": props.title, "subject": props.subject, "author": props.creator, "keywords": props.keywords, "category": props.category})).is_ok();
            app.info_editing = ok.then(|| (field, app.session.rev()));
        }
        if focused != app.info_editing.map(|(f, _)| f) {
            app.info_editing = None;
        }
        // Protect Document › Encrypt with Password (#55).
        ui.add_space(18.0);
        ui.label(egui::RichText::new(tl!("Protect Document")).font(semibold(15.0)));
        let encrypted = info.get("encrypted").and_then(|v| v.as_bool()).unwrap_or(false);
        ui.label(if encrypted {
            tl!("A password is required to open this document.")
        } else {
            tl!("Anyone can open this document. Encrypt it with a password to keep it private.")
        });
        ui.horizontal(|ui| {
            let label = if encrypted { tl!("Change Password…") } else { tl!("Encrypt with Password…") };
            if ui.button(egui::RichText::new(label).font(medium(13.5))).clicked() {
                let _ = app.run("file.encrypt", json!({}));
            }
            if encrypted && ui.button(egui::RichText::new(tl!("Remove Password")).font(medium(13.5))).clicked() {
                let _ = app.run("file.encrypt", json!({"password": null}));
            }
        });
        let ui = &mut cols[1];
        ui.label(egui::RichText::new(tl!("Statistics")).font(semibold(15.0)));
        for (l, k) in [("Pages", "pages"), ("Words", "words"), ("Paragraphs", "paragraphs"), ("Sections", "sections"), ("Comments", "comments")] {
            ui.label(format!("{}: {}", tl!(l), info.get(k).map(|v| v.to_string()).unwrap_or_default()));
        }
        let path = info.get("path").and_then(|v| v.as_str()).unwrap_or(tl!("Not saved yet"));
        ui.label(crate::i18n::fmt(tl!("Location: {path}"), &[("path", path)]));
    });
}

fn print_page(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    heading(ui, "Print");

    if app.services.print.is_some() {
        ui.label(tl!("Send the document straight to the system print dialog, or save a copy in another format below."));
        ui.add_space(12.0);
        if ui.add(egui::Button::new(egui::RichText::new(tl!("Print")).font(medium(13.5))).min_size(vec2(320.0, 34.0))).clicked() {
            let _ = app.run("ui.print", json!({}));
        }
        ui.add_space(18.0);
        ui.separator();
        ui.add_space(14.0);
        render_export_buttons(app, ui);
        return;
    }

    // Fetch system printers via engine command
    let printers_resp = app.run("file.printers", json!({})).unwrap_or(json!({"printers": []}));
    let printers = printers_resp.get("printers").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    // Default printer auto-selection if none is active
    if app.ui.print.printer.is_empty() {
        if let Some(def) = printers.iter().find(|p| p.get("default").and_then(|v| v.as_bool()) == Some(true)) {
            if let Some(name) = def.get("name").and_then(|v| v.as_str()) {
                app.ui.print.printer = name.to_string();
            }
        } else if let Some(first) = printers.first().and_then(|p| p.get("name").and_then(|v| v.as_str())) {
            app.ui.print.printer = first.to_string();
        }
    }

    // Top action row: [ Print ] button and Copies counter
    ui.horizontal(|ui| {
        let print_btn = ui.add(
            egui::Button::new(egui::RichText::new(format!("🖶  {}", tl!("Print"))).font(medium(15.0)).color(egui::Color32::WHITE))
                .fill(crate::theme::APP_COLOR)
                .min_size(vec2(140.0, 42.0)),
        );

        if print_btn.clicked() {
            let mut params = json!({
                "copies": app.ui.print.copies,
                "collate": app.ui.print.collate,
                "scaling": app.ui.print.scaling,
                "grayscale": app.ui.print.grayscale,
            });
            if !app.ui.print.printer.is_empty() {
                params["printer"] = json!(app.ui.print.printer);
            }
            if app.ui.print.duplex != "default" {
                params["duplex"] = json!(app.ui.print.duplex);
            }
            match app.ui.print.pages_mode.as_str() {
                "current" => {
                    params["currentPage"] = json!(true);
                }
                "custom" => {
                    params["range"] = json!(app.ui.print.custom_range);
                }
                _ => {}
            }
            match app.run("file.printNative", params) {
                Ok(_) => app.status(tl!("Document sent to printer")),
                Err(e) => app.status(format!("{}: {}", tl!("Print error"), e)),
            }
        }

        ui.add_space(24.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(tl!("Copies:")).font(medium(12.5)));
            ui.horizontal(|ui| {
                if ui.button(" - ").clicked() && app.ui.print.copies > 1 {
                    app.ui.print.copies -= 1;
                }
                ui.add(egui::DragValue::new(&mut app.ui.print.copies).range(1..=100));
                if ui.button(" + ").clicked() && app.ui.print.copies < 100 {
                    app.ui.print.copies += 1;
                }
            });
        });
    });

    ui.add_space(18.0);
    ui.separator();
    ui.add_space(14.0);

    // Section 1: Printer Selection
    ui.label(egui::RichText::new(tl!("Printer")).font(semibold(16.0)));
    ui.add_space(6.0);

    let current_printer = if app.ui.print.printer.is_empty() { tl!("Default Printer").to_string() } else { app.ui.print.printer.clone() };

    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("backstage_printer_select")
            .selected_text(egui::RichText::new(&current_printer).font(medium(13.5)))
            .width(360.0)
            .show_ui(ui, |ui| {
                if printers.is_empty() {
                    ui.selectable_value(&mut app.ui.print.printer, String::new(), tl!("(No printers discovered)"));
                } else {
                    for p in &printers {
                        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                        let is_def = p.get("default").and_then(|v| v.as_bool()).unwrap_or(false);
                        let label = if is_def { format!("{name}  [{}]", tl!("Default")) } else { name.to_string() };
                        ui.selectable_value(&mut app.ui.print.printer, name.to_string(), label);
                    }
                }
            });
    });

    if !printers.is_empty() {
        let is_selected_def = printers.iter().any(|p| {
            p.get("name").and_then(|v| v.as_str()) == Some(app.ui.print.printer.as_str()) && p.get("default").and_then(|v| v.as_bool()) == Some(true)
        });
        let status_desc = if is_selected_def { tl!("Status: Ready · System Default") } else { tl!("Status: Ready") };
        ui.add_space(2.0);
        ui.label(egui::RichText::new(status_desc).font(regular(11.5)).color(t.text_dim));
    }

    ui.add_space(18.0);
    ui.separator();
    ui.add_space(14.0);

    // Section 2: Print Settings
    ui.label(egui::RichText::new(tl!("Settings")).font(semibold(16.0)));
    ui.add_space(8.0);

    // Helper for aligned setting rows
    let setting_label = |ui: &mut Ui, label: &str| {
        ui.add_sized(vec2(100.0, 20.0), egui::Label::new(egui::RichText::new(format!("{label}:")).font(medium(13.0))));
    };

    // Pages Mode
    ui.horizontal(|ui| {
        setting_label(ui, tl!("Pages"));
        let mode_text = match app.ui.print.pages_mode.as_str() {
            "current" => tl!("Print Current Page"),
            "custom" => tl!("Custom Print"),
            _ => tl!("Print All Pages"),
        };
        egui::ComboBox::from_id_salt("backstage_pages_mode").selected_text(egui::RichText::new(mode_text).font(medium(13.0))).width(260.0).show_ui(
            ui,
            |ui| {
                ui.selectable_value(&mut app.ui.print.pages_mode, "all".into(), tl!("Print All Pages (The entire document)"));
                ui.selectable_value(&mut app.ui.print.pages_mode, "current".into(), tl!("Print Current Page (Just this page)"));
                ui.selectable_value(&mut app.ui.print.pages_mode, "custom".into(), tl!("Custom Print (Enter specific pages)"));
            },
        );
    });

    if app.ui.print.pages_mode == "custom" {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            setting_label(ui, tl!("Range"));
            ui.add(egui::TextEdit::singleline(&mut app.ui.print.custom_range).hint_text("e.g. 1-3, 5, 8-11").desired_width(260.0));
        });
    }

    ui.add_space(8.0);

    // Print Sides (Duplex)
    ui.horizontal(|ui| {
        setting_label(ui, tl!("Print Sides"));
        let duplex_text = match app.ui.print.duplex.as_str() {
            "off" => tl!("Print One Sided"),
            "long-edge" => tl!("Print on Both Sides (Flip on Long Edge)"),
            "short-edge" => tl!("Print on Both Sides (Flip on Short Edge)"),
            _ => tl!("Printer Default"),
        };
        egui::ComboBox::from_id_salt("backstage_duplex_select")
            .selected_text(egui::RichText::new(duplex_text).font(medium(13.0)))
            .width(260.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.ui.print.duplex, "default".into(), tl!("Printer Default"));
                ui.selectable_value(&mut app.ui.print.duplex, "off".into(), tl!("Print One Sided"));
                ui.selectable_value(&mut app.ui.print.duplex, "long-edge".into(), tl!("Print on Both Sides (Flip on Long Edge)"));
                ui.selectable_value(&mut app.ui.print.duplex, "short-edge".into(), tl!("Print on Both Sides (Flip on Short Edge)"));
            });
    });

    ui.add_space(8.0);

    // Collate
    ui.horizontal(|ui| {
        setting_label(ui, tl!("Collation"));
        let collate_text = if app.ui.print.collate { tl!("Collated (1,2,3  1,2,3)") } else { tl!("Uncollated (1,1,1  2,2,2)") };
        egui::ComboBox::from_id_salt("backstage_collate_select")
            .selected_text(egui::RichText::new(collate_text).font(medium(13.0)))
            .width(260.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.ui.print.collate, true, tl!("Collated (1,2,3  1,2,3)"));
                ui.selectable_value(&mut app.ui.print.collate, false, tl!("Uncollated (1,1,1  2,2,2)"));
            });
    });

    ui.add_space(8.0);

    // Color / Grayscale
    ui.horizontal(|ui| {
        setting_label(ui, tl!("Color Mode"));
        let color_text = if app.ui.print.grayscale { tl!("Grayscale (Black & White)") } else { tl!("Color") };
        egui::ComboBox::from_id_salt("backstage_color_select")
            .selected_text(egui::RichText::new(color_text).font(medium(13.0)))
            .width(260.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.ui.print.grayscale, false, tl!("Color"));
                ui.selectable_value(&mut app.ui.print.grayscale, true, tl!("Grayscale (Black & White)"));
            });
    });

    ui.add_space(8.0);

    // Paper Scaling
    ui.horizontal(|ui| {
        setting_label(ui, tl!("Scaling"));
        let scaling_text = match app.ui.print.scaling.as_str() {
            "shrink-to-fit" => tl!("Shrink to Printable Area"),
            _ => tl!("100% (No Scaling / Exact Size)"),
        };
        egui::ComboBox::from_id_salt("backstage_scaling_select")
            .selected_text(egui::RichText::new(scaling_text).font(medium(13.0)))
            .width(260.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.ui.print.scaling, "exact".into(), tl!("100% (No Scaling / Exact Size)"));
                ui.selectable_value(&mut app.ui.print.scaling, "shrink-to-fit".into(), tl!("Shrink to Printable Area"));
            });
    });

    ui.add_space(24.0);
    ui.separator();
    ui.add_space(14.0);
    render_export_buttons(app, ui);
}

fn render_export_buttons(app: &mut WordApp, ui: &mut Ui) {
    ui.label(egui::RichText::new(tl!("Or export to other formats:")).font(medium(13.5)));
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        for (label, ext) in [
            ("PDF document (*.pdf)", "pdf"),
            ("Word document (*.docx)", "docx"),
            ("EPUB 3 eBook (*.epub)", "epub"),
            ("OpenDocument Text (*.odt)", "odt"),
        ] {
            if ui.add(egui::Button::new(egui::RichText::new(tl!(label)).font(regular(12.5))).min_size(vec2(180.0, 30.0))).clicked() {
                app.export_dialog(ext);
            }
        }
    });
}

fn export_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, "Export");
    ui.label(tl!("Save a copy in another format. Printing goes through a PDF you can print from any viewer."));
    ui.add_space(12.0);
    for (label, ext) in [
        ("PDF document (*.pdf)", "pdf"),
        ("Word document (*.docx)", "docx"),
        ("EPUB 3 eBook (*.epub)", "epub"),
        ("OpenDocument Text (*.odt)", "odt"),
        ("Rich Text Format (*.rtf)", "rtf"),
        ("Web page (*.html)", "html"),
        ("Markdown (*.md)", "md"),
        ("LaTeX (*.tex)", "tex"),
        ("Plain text (*.txt)", "txt"),
        ("Page image (*.png)", "png"),
    ] {
        if ui.add(egui::Button::new(egui::RichText::new(tl!(label)).font(medium(13.5))).min_size(vec2(320.0, 34.0))).clicked() {
            app.export_dialog(ext);
        }
        ui.add_space(4.0);
    }
}

fn options_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, "Options");
    ui.label(egui::RichText::new(tl!("General")).font(semibold(15.0)));
    language_picker(app, ui);
    ui.horizontal(|ui| {
        ui.label(tl!("User name:"));
        let mut n = app.session.author.clone();
        if ui.text_edit_singleline(&mut n).changed() {
            app.session.author = n;
        }
    });
    theme_picker(app, ui);
    interface_size_picker(app, ui);
    // The browser can't write to the user's files, so AutoSave can't be turned on there (#176).
    let browser = app.autosave_block() == Some(crate::AutoSaveBlock::Browser);
    let mut autosave = app.autosave && !browser;
    let r = ui.add_enabled(!browser, egui::Checkbox::new(&mut autosave, tl!("AutoSave documents you have saved in WordCraft")));
    if browser {
        r.on_disabled_hover_text(crate::AutoSaveBlock::Browser.reason());
    } else if r.changed() {
        app.autosave = autosave;
        app.session.autosave = autosave;
    }
    let mut dark_page = app.session.view.dark_mode;
    if ui
        .checkbox(&mut dark_page, tl!("Dark page (white text on black)"))
        .on_hover_text(tl!("Show documents with their colours inverted, like View › Switch Modes. Saving, printing and PDFs are unchanged."))
        .changed()
    {
        let _ = app.run("view.darkMode", json!({"value": dark_page}));
    }
    ui.add_space(10.0);
    ui.label(egui::RichText::new(tl!("Display")).font(semibold(15.0)));
    let mut marks = app.session.view.marks;
    if ui.checkbox(&mut marks, tl!("Show all formatting marks")).changed() {
        let _ = app.run("view.marks", json!({"value": marks}));
    }
    let mut ruler = app.session.view.ruler;
    if ui.checkbox(&mut ruler, tl!("Show rulers")).changed() {
        let _ = app.run("view.ruler", json!({"value": ruler}));
    }
    ui.add_space(10.0);
    ui.label(egui::RichText::new(tl!("Opening files")).font(semibold(15.0)));
    ui.checkbox(&mut app.session.prefs.convert_legacy, tl!("Convert Word 97–2003 files (.doc) to .docx when opening"));
    ui.label(egui::RichText::new(tl!("A .docx is saved beside the original, which is left unchanged.")).font(crate::theme::regular(11.0)).weak());
    ui.add_space(10.0);
    ui.label(egui::RichText::new(tl!("Proofing")).font(semibold(15.0)));
    if ui.button(tl!("AutoCorrect Options…")).clicked() {
        app.dialog = crate::dialogs::Dialog::open("autocorrect", app);
        app.ui.backstage = false;
    }
    ui.add_space(10.0);
    ui.label(egui::RichText::new(tl!("Keyboard Shortcuts")).font(semibold(15.0)));
    if ui.button(tl!("Customize Keyboard Shortcuts…")).clicked() {
        app.dialog = Some(crate::dialogs::Dialog::open("customizeKeyboard", app).unwrap_or(crate::dialogs::Dialog::CustomizeKeyboard {
            category: "All Commands".into(),
            query: String::new(),
            selected_cmd: String::new(),
            new_key: String::new(),
        }));
        app.ui.backstage = false;
    }
    ui.add_space(10.0);
    ui.label(egui::RichText::new(tl!("Agents")).font(semibold(15.0)));
    ui.label(tl!("Every command is available to scripts and AI agents: run `wordcraft-cli mcp` for an MCP server, or start the app with `--control <port>` for the JSON control channel."));
}

/// File ▸ Options ▸ Interface theme: Light, Dark, or follow the system's appearance (#115).
fn theme_picker(app: &mut WordApp, ui: &mut Ui) {
    use crate::theme::Appearance;
    ui.horizontal(|ui| {
        ui.label(tl!("Interface theme:"));
        egui::ComboBox::from_id_salt("interface_theme").selected_text(tl!(app.ui.theme.label())).width(220.0).show_ui(ui, |ui| {
            for a in Appearance::ALL {
                if ui.selectable_label(app.ui.theme == a, tl!(a.label())).clicked() {
                    let _ = app.run("ui.theme", json!({"value": a.code()}));
                }
            }
        });
    });
}

/// File ▸ Options ▸ Interface size: 80–200 %, on top of the system's display scale (#475).
fn interface_size_picker(app: &mut WordApp, ui: &mut Ui) {
    use crate::theme::InterfaceSize;
    let current = app.ui.interface_size.percent();
    ui.horizontal(|ui| {
        ui.label(tl!("Interface size:"));
        egui::ComboBox::from_id_salt("interface_size").selected_text(format!("{current}%")).width(220.0).show_ui(ui, |ui| {
            for pct in InterfaceSize::PRESETS {
                if ui.selectable_label(current == pct, format!("{pct}%")).clicked() {
                    let _ = app.run("ui.interfaceSize", json!({"percent": pct}));
                }
            }
        });
    });
    ui.label(
        egui::RichText::new(tl!(
            "Makes the ribbon, menus and dialogs larger or smaller, on top of your system's display scaling. Document zoom is separate."
        ))
        .small()
        .weak(),
    );
}

/// File ▸ Options ▸ Interface language: follow the system (the default) or pick one (#8).
fn language_picker(app: &mut WordApp, ui: &mut Ui) {
    use crate::i18n::{AUTO, Lang};
    // The language names are in their own scripts: fonts for them load from the next frame.
    app.want_system_cjk = true;
    let system = crate::i18n::system_lang();
    let auto_label = crate::i18n::fmt(tl!("Automatic ({language})"), &[("language", system.name())]);
    let current = if app.ui.language == AUTO { auto_label.clone() } else { Lang::from_pref(&app.ui.language).name().to_string() };
    ui.horizontal(|ui| {
        ui.label(tl!("Interface language:"));
        egui::ComboBox::from_id_salt("interface_language").selected_text(current).width(220.0).show_ui(ui, |ui| {
            if ui.selectable_label(app.ui.language == AUTO, auto_label.as_str()).clicked() {
                let _ = app.run("ui.language", json!({"value": AUTO}));
            }
            for lang in Lang::all() {
                if ui.selectable_label(app.ui.language == lang.code(), lang.name()).clicked() {
                    let _ = app.run("ui.language", json!({"value": lang.code()}));
                }
            }
        });
    });
    ui.label(egui::RichText::new(tl!("Automatic follows your system's language. Menus and commands change; your documents don't.")).small().weak());
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable;
    use serde_json::json;

    use super::matching_templates;
    use crate::{Services, WordApp};

    #[test]
    fn template_search_matches_labels_case_insensitively() {
        let ids = |q: &str| matching_templates(q).into_iter().map(|(_, id)| id).collect::<Vec<_>>();
        assert_eq!(ids(""), ["blank", "sample", "letter", "resume", "report"]);
        assert_eq!(ids("  LETTER "), ["letter"]);
        assert_eq!(ids("re"), ["resume", "report"]);
        assert_eq!(ids("résumé"), ["resume"]);
        assert!(ids("no such template").is_empty());
    }

    /// Home and New are different pages (#141): Home has the short template row, the link to New
    /// and Recent; New has every template and no Recent list.
    #[test]
    fn home_and_new_show_different_sections() {
        let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Services::default());
        a.run("ui.backstage", json!({"value": true, "page": "home"})).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        h.run_steps(4);
        assert!(h.query_by_label("Recent").is_some(), "Home lists recent documents");
        assert!(h.query_by_label("Letter").is_some(), "Home shows the first templates");
        assert!(h.query_by_label("Report").is_none(), "Home shows only a short row of templates");
        h.get_by_label("More templates →").click();
        h.run_steps(4);
        assert_eq!(h.state().ui.backstage_page, "new", "the link opens New");
        assert!(h.query_by_label("Report").is_some(), "New shows every template");
        assert!(h.query_by_label("Recent").is_none(), "New has no Recent list");
    }

    /// #260: File › Info's Title and Author fields dropped every keystroke (they edited a copy
    /// re-read each frame and only wrote it back when the field lost focus), so a save had no
    /// title or author. Typing now lands in the document as it happens, one undo step per field,
    /// and the browser's Save downloads it with the save stamps advanced (#262).
    #[test]
    fn info_fields_keep_typed_properties_through_a_web_save() {
        use egui::accesskit::Role;
        let got: std::rc::Rc<std::cell::RefCell<Vec<(String, Vec<u8>)>>> = Default::default();
        let sink = got.clone();
        let services = Services {
            download: Some(Box::new(move |n: &str, b: &[u8]| {
                sink.borrow_mut().push((n.to_string(), b.to_vec()));
                Ok(())
            })),
            ..Default::default()
        };
        let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("XYZ")), services);
        a.session.author = "GAMMA".into();
        a.run("ui.backstage", json!({"value": true, "page": "info"})).unwrap();
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            a,
        );
        h.run_steps(4);
        for (field, text) in [("Title", "ALPHA"), ("Author", "BETA")] {
            h.get_by_role_and_label(Role::TextInput, field).click();
            h.run_steps(2);
            for c in text.chars() {
                h.get_by_role_and_label(Role::TextInput, field).type_text(&c.to_string());
                h.run_steps(2);
            }
            assert_eq!(h.get_by_role_and_label(Role::TextInput, field).value().as_deref(), Some(text), "{field} shows what was typed");
        }
        let core = h.state().session.doc.core.clone();
        assert_eq!((core.title.as_str(), core.creator.as_str()), ("ALPHA", "BETA"));
        assert!(h.state().session.dirty, "the property edits are unsaved changes");

        // Save (web: a download) writes them into docProps/core.xml, with fresh save stamps.
        let a = h.state_mut();
        let created = a.session.doc.core.created.clone();
        a.run("file.save", json!({})).unwrap();
        a.run("file.save", json!({})).unwrap();
        let (name, bytes) = got.borrow_mut().pop().unwrap();
        let back = wordcraft_engine::io::open_bytes(&name, &bytes).unwrap();
        assert_eq!((back.core.title.as_str(), back.core.creator.as_str()), ("ALPHA", "BETA"));
        assert_eq!(back.core.last_modified_by, "GAMMA");
        assert_eq!(back.core.revision, core.revision + 2, "each save advances the revision");
        assert!(created.is_empty() && !back.core.created.is_empty(), "the first save sets the creation time");
        assert!(!back.core.modified.is_empty());
        assert!(!a.session.dirty);

        // Each field's typing is one undo step.
        a.session.run("edit.undo", &json!({})).unwrap();
        assert_eq!((a.session.doc.core.title.as_str(), a.session.doc.core.creator.as_str()), ("ALPHA", ""));
        a.session.run("edit.undo", &json!({})).unwrap();
        assert_eq!(a.session.doc.core.title, "");
    }
}
