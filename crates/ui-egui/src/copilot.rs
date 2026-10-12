//! The Copilot pane: a chat with the user's AI provider about the document.
//!
//! The header carries New chat, Settings (the gear: provider, model, key, how the model answers,
//! language, standing instructions, how much of the document it sees) and Close. Below it the
//! conversation, and at the bottom the composer: a message, or a `/command` (`/check-doc`,
//! `/proofread`, `/summarize`, `/rewrite`, `/draft`, `/table`, `/ask`).
//!
//! Provider calls run on a background thread, so the document keeps responding (typing,
//! scrolling, menus) while the model thinks; the chat shows the elapsed time and Cancel, and
//! [`poll`] collects the answer each frame. An answer only lands in the chat: the document changes
//! when the user inserts it (`text.insert`), so a late or cancelled answer never edits the
//! document. Calls that answer offline (no API key, `/table`) run `tools.copilot` directly.

use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Align, Layout, RichText, Stroke, Ui, vec2};
use serde_json::{Value, json};

use crate::theme::{APP_COLOR, Tokens, regular, semibold};
use crate::{WordApp, now_ms};

/// One message in the conversation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatMsg {
    /// Sent by the user (else Copilot's answer).
    pub user: bool,
    pub text: String,
    /// The answer's kind (`Summary`, `Rewrite`…); empty for the user's messages.
    pub title: String,
    /// A note (an error, Cancelled, a table inserted), not text to insert.
    pub error: bool,
}

/// The slash commands: command, what it does.
pub const COMMANDS: &[(&str, &str)] = &[
    ("/check-doc", "Find problems in the document"),
    ("/proofread", "Correct spelling and grammar"),
    ("/summarize", "Summarize the selection or document"),
    ("/rewrite", "Rewrite the selection"),
    ("/draft", "Write new text"),
    ("/table", "Insert a table"),
    ("/ask", "Ask about the document"),
];

/// Model presets per provider (any model ID can be typed in Settings).
pub fn model_presets(provider: &str) -> &'static [&'static str] {
    match provider {
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
        _ => &[],
    }
}

/// A provider call in flight.
struct Job {
    rx: Receiver<Result<String, String>>,
    provider: String,
    title: String,
    started: f64,
}

/// The pane's background call, if any.
#[derive(Default)]
pub struct CopilotLink {
    job: Option<Job>,
}

impl CopilotLink {
    /// A provider call is in flight.
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
    /// Seconds since the call in flight started.
    pub fn elapsed_s(&self) -> Option<f64> {
        self.job.as_ref().map(|j| ((now_ms() - j.started) / 1000.0).max(0.0))
    }
}

/// A message's `tools.copilot` action and its text: `/summarize key points` →
/// (`summarize`, `key points`). Plain text (or an unknown command) is a question about the document.
pub fn parse(message: &str) -> (&'static str, String) {
    let m = message.trim();
    let (head, rest) = m.split_once(char::is_whitespace).unwrap_or((m, ""));
    let rest = rest.trim().to_string();
    match head.to_lowercase().as_str() {
        "/check-doc" => {
            let extra = if rest.is_empty() { String::new() } else { format!(" Also: {rest}") };
            ("ask", format!("{}{extra}", wordcraft_engine::cmd::tools::CHECK_DOC_PROMPT))
        }
        "/proofread" => ("proofread", rest),
        "/summarize" | "/summarise" => ("summarize", rest),
        "/rewrite" => ("rewrite", rest),
        "/draft" => ("draft", rest),
        "/table" => ("table", rest),
        "/ask" => ("ask", rest),
        _ => ("ask", m.to_string()),
    }
}

/// `tools.copilot` params for `action` with `text`, from the pane's settings. For Rewrite,
/// Summarize and Proofread the text says how (instructions) and the text worked on is the
/// selection (or the document); for the others it is the request itself. `history` is the chat
/// before this message.
pub fn payload(app: &WordApp, action: &str, text: &str, history: &[ChatMsg]) -> Value {
    let u = &app.ui;
    let mut v = json!({
        "action": action,
        "tone": u.ai_tone,
        "provider": u.ai_provider,
        "model": u.ai_model,
        "temperature": u.ai_temperature,
        "max_tokens": u.ai_max_tokens,
        "timeout_s": u.ai_timeout_s,
        "language": u.ai_language,
        "custom_instructions": u.ai_custom_instructions,
        "scope": if u.ai_whole_document { "auto" } else { "selection" },
        "max_chars": u.ai_max_chars,
    });
    let key = if matches!(action, "rewrite" | "summarize" | "proofread") { "instructions" } else { "prompt" };
    v[key] = json!(text);
    if !u.ai_api_key.trim().is_empty() {
        v["api_key"] = json!(u.ai_api_key);
    }
    if !u.ai_base_url.trim().is_empty() {
        v["base_url"] = json!(u.ai_base_url);
    }
    if u.ai_remember_chat {
        let h: Vec<Value> = history
            .iter()
            .filter(|m| !m.error && !m.text.trim().is_empty())
            .map(|m| json!({"role": if m.user { "user" } else { "assistant" }, "text": m.text}))
            .collect();
        if !h.is_empty() {
            v["history"] = Value::Array(h);
        }
    }
    v
}

/// Send `message` from the composer: it joins the chat and Copilot answers. Nothing happens while
/// an answer is still coming.
pub fn send(app: &mut WordApp, message: &str) {
    let message = message.trim();
    if message.is_empty() || app.copilot.busy() {
        return;
    }
    let (action, text) = parse(message);
    let history = app.ui.ai_chat.clone();
    app.ui.ai_chat.push(ChatMsg { user: true, text: message.to_string(), ..Default::default() });
    let v = payload(app, action, &text, &history);
    run(app, &v, &text);
}

fn answer(app: &mut WordApp, title: &str, text: String) {
    app.ui.ai_chat.push(ChatMsg { user: false, text, title: title.to_string(), error: false });
}

fn note(app: &mut WordApp, title: &str, text: String) {
    app.ui.ai_chat.push(ChatMsg { user: false, text, title: title.to_string(), error: true });
}

/// Run a `tools.copilot` request (`payload`). `label` names a generated table in the answer. A
/// provider call goes to a background thread; anything else answers now.
pub fn run(app: &mut WordApp, payload: &Value, label: &str) {
    if app.copilot.busy() {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(req) = wordcraft_engine::cmd::tools::copilot_request(&app.session, payload) {
        let (tx, rx) = std::sync::mpsc::channel();
        let wake = app.ctx.clone();
        let provider = req.provider.clone();
        let title = req.title.clone();
        let spawned = std::thread::Builder::new().name("copilot".into()).spawn(move || {
            // A cancelled job has dropped its receiver: the answer is just discarded.
            let _ = tx.send(req.send());
            if let Some(c) = &wake {
                c.request_repaint();
            }
        });
        match spawned {
            Ok(_) => app.copilot.job = Some(Job { rx, provider, title, started: now_ms() }),
            Err(e) => note(app, "", format!("Could not start Copilot: {e}")),
        }
        return;
    }
    match app.session.run("tools.copilot", payload) {
        Ok(res) => {
            let title = res.get("title").and_then(Value::as_str).unwrap_or("").to_string();
            if let Some(c) = res.get("content").and_then(Value::as_str) {
                answer(app, &title, c.to_string());
            } else if res.get("rows").is_some() {
                let rows = res.get("rows").and_then(Value::as_u64).unwrap_or(0);
                let cols = res.get("cols").and_then(Value::as_u64).unwrap_or(0);
                note(app, "Table", format!("Inserted a {rows}×{cols} table for \"{label}\" at the caret."));
            }
        }
        Err(e) => note(app, "", e.to_string()),
    }
}

/// Stop waiting for the call in flight (the thread ends on its own; its answer is dropped).
pub fn cancel(app: &mut WordApp) {
    if app.copilot.job.take().is_some() {
        note(app, "", "Cancelled".into());
    }
}

/// Collect the answer of the call in flight (each frame).
pub fn poll(app: &mut WordApp, ctx: &egui::Context) {
    let Some(job) = &app.copilot.job else { return };
    match job.rx.try_recv() {
        Ok(Ok(text)) => {
            let title = job.title.clone();
            app.copilot.job = None;
            answer(app, &title, text);
        }
        Ok(Err(e)) => {
            let msg = format!("AI Provider ({}) error: {e}", job.provider);
            app.copilot.job = None;
            note(app, "", msg);
        }
        Err(TryRecvError::Disconnected) => {
            app.copilot.job = None;
            note(app, "", "Copilot stopped without an answer".into());
        }
        // Keep the elapsed time ticking.
        Err(TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(250)),
    }
}

/// Start a new conversation (an answer still coming is dropped).
pub fn new_chat(app: &mut WordApp) {
    app.copilot.job = None;
    app.ui.ai_chat.clear();
    app.ui.ai_prompt.clear();
}

/// Put every setting back to its default (the provider, key and model stay).
pub fn reset_settings(app: &mut WordApp) {
    let d = crate::UiState::default();
    let u = &mut app.ui;
    u.ai_temperature = d.ai_temperature;
    u.ai_max_tokens = d.ai_max_tokens;
    u.ai_timeout_s = d.ai_timeout_s;
    u.ai_language = d.ai_language;
    u.ai_tone = d.ai_tone;
    u.ai_custom_instructions = d.ai_custom_instructions;
    u.ai_whole_document = d.ai_whole_document;
    u.ai_max_chars = d.ai_max_chars;
    u.ai_remember_chat = d.ai_remember_chat;
}

// ── The pane ────────────────────────────────────────────────────────────────────────────────

const COMPOSER_ID: &str = "copilot_composer";

/// The Copilot pane.
pub fn pane(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    // Header: title, then New chat, Settings, Close at the right.
    let mut close = false;
    ui.horizontal(|ui| {
        let title = if app.ui.ai_settings_open { tl!("Copilot settings") } else { tl!("Copilot") };
        ui.label(RichText::new(title).font(semibold(15.0)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if crate::widgets::icon_button(ui, "close", tl!("Close")).clicked() {
                close = true;
            }
            let gear = crate::widgets::icon_button(ui, "settings", tl!("Settings"));
            if app.ui.ai_settings_open {
                ui.painter().rect_stroke(gear.rect, 4.0, Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
            }
            if gear.clicked() {
                app.ui.ai_settings_open = !app.ui.ai_settings_open;
            }
            if !app.ui.ai_settings_open && crate::widgets::icon_button(ui, "newChat", tl!("New chat")).clicked() {
                new_chat(app);
            }
        });
    });
    if close {
        app.ui.ai_pane = false;
        return;
    }
    // Provider status.
    let online = !app.ui.ai_api_key.trim().is_empty();
    let (dot, status) = if online {
        let model = if app.ui.ai_model.is_empty() { tl!("default model").to_string() } else { app.ui.ai_model.clone() };
        (t.green, format!("{} · {model}", app.ui.ai_provider))
    } else {
        (t.text_dim, tl!("Offline: add an API key in Settings for full answers").to_string())
    };
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter().circle_filled(r.center(), 3.5, dot);
        ui.add(egui::Label::new(RichText::new(status).font(regular(11.0)).color(t.text_dim)).truncate());
    });
    ui.add_space(4.0);
    ui.separator();

    if app.ui.ai_settings_open {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| settings(app, ui));
        return;
    }
    // Composer docked at the bottom, the conversation in the space above it.
    egui::Panel::bottom("copilot_composer_bar")
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.inner_margin(egui::Margin { top: 6, ..Default::default() }))
        .show(ui, |ui| composer(app, ui));
    egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| conversation(app, ui));
}

/// The empty chat's suggestions, or the messages and the thinking indicator.
fn conversation(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    if app.ui.ai_chat.is_empty() && !app.copilot.busy() {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(tl!("How can I help with this document?")).font(semibold(13.0)));
            ui.add_space(10.0);
            for (cmd, what) in COMMANDS {
                let b = egui::Button::new(RichText::new(*cmd).font(regular(12.5)).color(t.text_dim))
                    .corner_radius(14.0)
                    .min_size(vec2(130.0, 28.0))
                    .fill(t.panel)
                    .stroke(Stroke::new(1.0, t.border));
                if ui.add(b).on_hover_text(tl!(what)).clicked() {
                    app.ui.ai_prompt = format!("{cmd} ");
                    ui.memory_mut(|m| m.request_focus(egui::Id::new(COMPOSER_ID)));
                }
                ui.add_space(4.0);
            }
        });
        return;
    }
    let width = ui.available_width();
    let has_sel = !app.session.sel.is_collapsed();
    let mut action: Option<(usize, &'static str)> = None;
    for (i, m) in app.ui.ai_chat.iter().enumerate() {
        if m.user {
            ui.with_layout(Layout::top_down(Align::Max), |ui| {
                egui::Frame::NONE.fill(t.checked).corner_radius(10.0).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                    ui.set_max_width(width * 0.85);
                    ui.add(egui::Label::new(RichText::new(&m.text).font(regular(12.5))).wrap());
                });
            });
        } else {
            if !m.title.is_empty() {
                ui.label(RichText::new(tl!(&m.title)).font(semibold(11.0)).color(t.text_dim));
            }
            let color = if m.error { t.text_dim } else { t.text };
            ui.add(egui::Label::new(RichText::new(&m.text).font(regular(12.5)).color(color)).wrap().selectable(true));
            if !m.error {
                ui.horizontal(|ui| {
                    if ui.small_button(tl!("Insert")).on_hover_text(tl!("Insert at the caret")).clicked() {
                        action = Some((i, "insert"));
                    }
                    if has_sel && ui.small_button(tl!("Replace")).on_hover_text(tl!("Replace the selection")).clicked() {
                        action = Some((i, "replace"));
                    }
                    if ui.small_button(tl!("Copy")).clicked() {
                        action = Some((i, "copy"));
                    }
                });
            }
        }
        ui.add_space(8.0);
    }
    if let Some(secs) = app.copilot.elapsed_s() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new(format!("{} {secs:.0}s", tl!("Thinking…"))).font(regular(12.0)).color(t.text_dim));
            if ui.small_button(tl!("Cancel")).clicked() {
                cancel(app);
            }
        });
    }
    if let Some((i, what)) = action {
        let text = app.ui.ai_chat.get(i).map(|m| m.text.clone()).unwrap_or_default();
        match what {
            // Typing over a selection replaces it.
            "replace" => {
                let _ = app.run("text.insert", json!({"text": text}));
            }
            // Insert leaves the selection alone: the text goes after it.
            "insert" => {
                let end = app.session.sel.ordered().1;
                app.session.sel = wordcraft_engine::Selection::caret(end);
                let _ = app.run("text.insert", json!({"text": text}));
            }
            _ => ui.ctx().copy_text(text),
        }
    }
}

/// The message box, the command suggestions while typing `/`, and the send row.
fn composer(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let id = egui::Id::new(COMPOSER_ID);
    // Enter sends; Shift+Enter is a new line.
    let enter = ui.memory(|m| m.has_focus(id)) && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let mut send_now = enter;
    egui::Frame::NONE.fill(t.input).stroke(Stroke::new(1.0, t.input_border)).corner_radius(10.0).inner_margin(8).show(ui, |ui| {
        ui.set_width(ui.available_width());
        {
            // Command suggestions while the message is a bare "/…".
            let typed = app.ui.ai_prompt.trim_start().to_lowercase();
            if typed.starts_with('/') && !typed.contains(char::is_whitespace) {
                for (cmd, what) in COMMANDS.iter().filter(|(c, _)| c.starts_with(typed.as_str())) {
                    let r = ui.add(egui::Button::new(RichText::new(format!("{cmd}  ·  {}", tl!(what))).font(regular(11.5))).frame(false));
                    if r.clicked() {
                        app.ui.ai_prompt = format!("{cmd} ");
                        ui.memory_mut(|m| m.request_focus(id));
                    }
                }
            }
            ui.add(
                egui::TextEdit::multiline(&mut app.ui.ai_prompt)
                    .id(id)
                    .hint_text(tl!("Ask Copilot for help, or / for commands"))
                    .frame(egui::Frame::NONE)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                let words = app.session.selected_text().split_whitespace().count();
                let target = if words > 0 {
                    format!("{} {words}", tl!("Selected words:"))
                } else if app.ui.ai_whole_document {
                    tl!("Whole document").to_string()
                } else {
                    tl!("No selection").to_string()
                };
                ui.label(RichText::new(target).font(regular(10.5)).color(t.text_dim));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let can_send = !app.copilot.busy() && !app.ui.ai_prompt.trim().is_empty();
                    let r = ui.add_enabled_ui(can_send, |ui| crate::widgets::icon_button(ui, "send", tl!("Send (Enter)"))).inner;
                    if r.clicked() {
                        send_now = true;
                    }
                    let presets = model_presets(&app.ui.ai_provider);
                    if !presets.is_empty() {
                        let shown = app.ui.ai_model.rsplit('/').next().unwrap_or("").to_string();
                        let shown = if shown.is_empty() { tl!("Model").to_string() } else { shown };
                        egui::ComboBox::from_id_salt("copilot_model").selected_text(RichText::new(shown).font(regular(10.5))).width(120.0).show_ui(
                            ui,
                            |ui| {
                                for m in presets {
                                    ui.selectable_value(&mut app.ui.ai_model, m.to_string(), *m);
                                }
                            },
                        );
                    }
                });
            });
        }
    });
    if send_now && !app.copilot.busy() && !app.ui.ai_prompt.trim().is_empty() {
        let message = std::mem::take(&mut app.ui.ai_prompt);
        send(app, &message);
    }
}

/// A settings section heading.
fn section(ui: &mut Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(tl!(title)).font(semibold(12.5)));
    ui.add_space(2.0);
}

/// A checkbox whose label wraps to the pane's width (a long label must not widen the pane).
fn check(ui: &mut Ui, value: &mut bool, label: &str) {
    ui.horizontal(|ui| {
        ui.checkbox(value, "");
        if ui.add(egui::Label::new(RichText::new(tl!(label)).font(regular(11.5))).wrap().sense(egui::Sense::click())).clicked() {
            *value = !*value;
        }
    });
}

/// One labelled setting row.
fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(vec2(100.0, 20.0), egui::Label::new(RichText::new(tl!(label)).font(regular(11.5))).truncate());
        add(ui);
    });
}

/// The settings page (the gear).
fn settings(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    // Keep every row inside the pane: sliders are shorter than egui's default.
    ui.spacing_mut().slider_width = 90.0;
    ui.set_max_width(ui.available_width());
    let mut reset = false;
    {
        let u = &mut app.ui;
        section(ui, "Provider");
        ui.horizontal_wrapped(|ui| {
            for p in ["OpenRouter", "DeepSeek", "Gemini", "Claude", "Custom"] {
                if ui.selectable_label(u.ai_provider == p, p).clicked() && u.ai_provider != p {
                    u.ai_provider = p.into();
                    u.ai_model = model_presets(p).first().map(|m| m.to_string()).unwrap_or_default();
                }
            }
        });
        let show_id = egui::Id::new("copilot_show_key");
        let mut show = ui.data(|d| d.get_temp::<bool>(show_id)).unwrap_or(false);
        row(ui, "API key", |ui| {
            ui.add(egui::TextEdit::singleline(&mut u.ai_api_key).password(!show).hint_text("sk-…").desired_width(115.0));
            ui.checkbox(&mut show, tl!("Show"));
        });
        ui.data_mut(|d| d.insert_temp(show_id, show));
        let presets = model_presets(&u.ai_provider);
        if !presets.is_empty() {
            row(ui, "Model", |ui| {
                let shown = if u.ai_model.is_empty() { tl!("Default").to_string() } else { u.ai_model.clone() };
                egui::ComboBox::from_id_salt("copilot_settings_model").selected_text(shown).width(170.0).show_ui(ui, |ui| {
                    for m in presets {
                        ui.selectable_value(&mut u.ai_model, m.to_string(), *m);
                    }
                });
            });
        }
        row(ui, "Model ID", |ui| {
            ui.add(egui::TextEdit::singleline(&mut u.ai_model).hint_text(tl!("Any model ID")).desired_width(f32::INFINITY));
        });
        row(ui, "Base URL", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut u.ai_base_url).hint_text(tl!("Optional: an OpenAI-compatible endpoint")).desired_width(f32::INFINITY),
            );
        });

        section(ui, "Response");
        row(ui, "Creativity", |ui| {
            ui.add(egui::Slider::new(&mut u.ai_temperature, 0.0..=2.0).step_by(0.1).fixed_decimals(1));
        });
        ui.label(RichText::new(tl!("Lower is more precise and repeatable; higher is more inventive.")).font(regular(10.5)).color(t.text_dim));
        row(ui, "Longest answer", |ui| {
            ui.add(egui::DragValue::new(&mut u.ai_max_tokens).range(64..=32_768).speed(32.0).suffix(" tokens"));
        });
        row(ui, "Wait up to", |ui| {
            ui.add(egui::Slider::new(&mut u.ai_timeout_s, 10..=300).suffix(" s"));
        });

        section(ui, "Writing");
        row(ui, "Answer language", |ui| {
            egui::ComboBox::from_id_salt("copilot_language").selected_text(tl!(&u.ai_language)).width(150.0).show_ui(ui, |ui| {
                for l in ["Auto", "English", "Bangla", "Hindi", "Arabic", "Spanish", "French", "German", "Chinese", "Japanese"] {
                    ui.selectable_value(&mut u.ai_language, l.to_string(), tl!(l));
                }
            });
        });
        row(ui, "Tone", |ui| {
            egui::ComboBox::from_id_salt("copilot_tone").selected_text(tl!(&u.ai_tone)).width(150.0).show_ui(ui, |ui| {
                for tone in ["professional", "academic", "executive", "concise", "casual", "formal", "friendly"] {
                    ui.selectable_value(&mut u.ai_tone, tone.to_string(), tl!(tone));
                }
            });
        });
        ui.label(RichText::new(tl!("Standing instructions (sent with every request)")).font(regular(11.5)));
        ui.add(
            egui::TextEdit::multiline(&mut u.ai_custom_instructions)
                .hint_text(tl!("e.g. Use formal Bangla. Keep amounts in Bangla numerals."))
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );

        section(ui, "Document context");
        check(ui, &mut u.ai_whole_document, "Use the whole document when nothing is selected");
        row(ui, "Most text sent", |ui| {
            ui.add(egui::Slider::new(&mut u.ai_max_chars, 5_000..=120_000).step_by(5_000.0).suffix(" ch"));
        });
        check(ui, &mut u.ai_remember_chat, "Remember earlier messages in the conversation");

        ui.add_space(10.0);
        ui.label(
            RichText::new(tl!(
                "Settings and the API key are saved in your user profile on this computer. Document text goes to the provider only when you send a message."
            ))
            .font(regular(10.5))
            .color(t.text_dim),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(tl!("Reset to defaults")).clicked() {
                reset = true;
            }
            if ui.add(egui::Button::new(RichText::new(tl!("Done")).color(egui::Color32::WHITE)).fill(APP_COLOR)).clicked() {
                u.ai_settings_open = false;
            }
        });
    }
    if reset {
        reset_settings(app);
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    use serde_json::json;
    use wordcraft_doc::Document;
    use wordcraft_engine::Session;

    use super::ChatMsg;
    use crate::{Services, WordApp};

    /// A fake OpenAI-compatible provider that answers `answer` after `delay`; the request body it
    /// received arrives on the returned channel.
    fn fake_provider(delay: Duration, answer: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut buf = vec![0u8; 1 << 20];
            let mut got = 0;
            // Read until the JSON body is complete (headers, then Content-Length bytes).
            loop {
                let n = s.read(&mut buf[got..]).unwrap_or(0);
                got += n;
                let text = String::from_utf8_lossy(&buf[..got]).to_string();
                if let Some(h) = text.find("\r\n\r\n") {
                    let len = text[..h]
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                        .unwrap_or(0);
                    if got >= h + 4 + len {
                        let _ = tx.send(text[h + 4..].to_string());
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            std::thread::sleep(delay);
            let body = json!({"choices": [{"message": {"content": answer}}]}).to_string();
            let _ =
                write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        });
        (format!("http://{addr}/v1/chat/completions"), rx)
    }

    fn app_with(url: &str) -> WordApp {
        let mut app = WordApp::new(Session::new(Document::from_text("The loan was approved.")), Services::default());
        app.ui.ai_provider = "Custom".into();
        app.ui.ai_model = "m".into();
        app.ui.ai_api_key = "k".into();
        app.ui.ai_base_url = url.into();
        app
    }

    fn wait(app: &mut WordApp) {
        let ctx = egui::Context::default();
        for _ in 0..500 {
            super::poll(app, &ctx);
            if !app.copilot.busy() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("Copilot never answered");
    }

    fn last(app: &WordApp) -> ChatMsg {
        app.ui.ai_chat.last().cloned().unwrap_or_default()
    }

    #[test]
    fn a_slow_provider_does_not_block_the_ui_and_its_answer_arrives() {
        let (url, _) = fake_provider(Duration::from_millis(800), "On approval day.");
        let mut app = app_with(&url);
        let t = Instant::now();
        super::send(&mut app, "When?");
        assert!(t.elapsed() < Duration::from_millis(300), "sending returns at once: {:?}", t.elapsed());
        assert!(app.copilot.busy());
        // A second message while one is in flight is not sent.
        super::send(&mut app, "And?");
        assert_eq!(app.ui.ai_chat.len(), 1);
        wait(&mut app);
        assert_eq!(last(&app), ChatMsg { user: false, text: "On approval day.".into(), title: "Answer".into(), error: false });
    }

    #[test]
    fn the_settings_and_the_conversation_reach_the_provider() {
        let (url, body) = fake_provider(Duration::ZERO, "Second answer");
        let mut app = app_with(&url);
        app.ui.ai_language = "Bangla".into();
        app.ui.ai_temperature = 0.3;
        app.ui.ai_max_tokens = 777;
        app.ui.ai_custom_instructions = "Use formal register.".into();
        app.ui.ai_chat = vec![
            ChatMsg { user: true, text: "First question".into(), ..Default::default() },
            ChatMsg { user: false, text: "First answer".into(), title: "Answer".into(), error: false },
        ];
        super::send(&mut app, "/summarize key points");
        wait(&mut app);
        let sent: serde_json::Value = serde_json::from_str(&body.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
        assert_eq!(sent["max_tokens"], 777);
        assert!((sent["temperature"].as_f64().unwrap() - 0.3).abs() < 1e-6);
        let system = sent["messages"][0]["content"].as_str().unwrap();
        let user = sent["messages"][1]["content"].as_str().unwrap();
        assert!(system.contains("Summarize") && system.contains("key points") && system.contains("Bangla") && system.contains("formal register"));
        assert!(user.contains("User: First question\nAssistant: First answer"), "{user}");
        assert!(user.contains("The loan was approved."), "the document is what gets summarized");
        assert_eq!(last(&app).title, "Summary");
    }

    #[test]
    fn a_cancelled_call_drops_its_late_answer() {
        let (url, _) = fake_provider(Duration::from_millis(200), "Late answer");
        let mut app = app_with(&url);
        let ctx = egui::Context::default();
        super::send(&mut app, "When?");
        super::cancel(&mut app);
        assert!(!app.copilot.busy());
        std::thread::sleep(Duration::from_millis(400));
        super::poll(&mut app, &ctx);
        assert!(app.ui.ai_chat.iter().all(|m| m.text != "Late answer"), "a cancelled answer never shows");
        assert!(last(&app).error);
    }

    #[test]
    fn a_provider_error_is_shown_not_panicked() {
        let mut app = app_with("http://127.0.0.1:9/unreachable");
        super::send(&mut app, "When?");
        wait(&mut app);
        let m = last(&app);
        assert!(m.error && m.text.starts_with("AI Provider (Custom)"), "{m:?}");
    }

    #[test]
    fn slash_commands_choose_the_action() {
        assert_eq!(super::parse("/summarize key points"), ("summarize", "key points".to_string()));
        assert_eq!(super::parse("/REWRITE  more formal "), ("rewrite", "more formal".to_string()));
        assert_eq!(super::parse("What is the loan amount?"), ("ask", "What is the loan amount?".to_string()));
        assert_eq!(super::parse("/unknown thing").0, "ask");
        let (a, text) = super::parse("/check-doc");
        assert_eq!(a, "ask");
        assert!(text.contains("Check this document"));
    }

    #[test]
    fn offline_answers_and_tables_need_no_provider() {
        let mut app = WordApp::new(Session::new(Document::from_text("Revenue rose.")), Services::default());
        super::send(&mut app, "/table Team Roster");
        assert!(!app.copilot.busy());
        assert_eq!(last(&app).title, "Table");
        assert!(app.session.doc.body.iter().any(|b| b.as_table().is_some()));
        super::send(&mut app, "How long is it?");
        assert!(!last(&app).text.is_empty());
    }

    #[test]
    fn reset_restores_the_defaults_but_keeps_the_provider() {
        let mut app = app_with("http://127.0.0.1:9/");
        app.ui.ai_temperature = 1.9;
        app.ui.ai_language = "German".into();
        super::reset_settings(&mut app);
        let d = crate::UiState::default();
        assert_eq!((app.ui.ai_temperature, app.ui.ai_language.as_str()), (d.ai_temperature, d.ai_language.as_str()));
        assert_eq!(app.ui.ai_api_key, "k");
    }

    #[test]
    fn the_pane_draws_chat_and_settings_without_panicking() {
        let mut app = WordApp::new(Session::new(Document::from_text("Text")), Services::default());
        app.ui.ai_chat = vec![
            ChatMsg { user: true, text: "Hi".into(), ..Default::default() },
            ChatMsg { user: false, text: "Hello".into(), title: "Answer".into(), error: false },
        ];
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        for settings_open in [false, true] {
            app.ui.ai_settings_open = settings_open;
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
            let output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
                egui::Panel::right("p").default_size(320.0).show(ui, |ui| super::pane(&mut app, ui));
            });
            output.drop_without_applying_deltas();
        }
    }
}
