//! Keyboard: typing, editing keys and command shortcuts (from the registry's `shortcut` fields).

use egui::{Key, Modifiers};
use serde_json::{Value, json};

use crate::WordApp;

/// The registry's name for a key.
pub fn key_name(k: Key) -> Option<&'static str> {
    Some(match k {
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::ArrowUp => "Up",
        Key::ArrowDown => "Down",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::Enter => "Enter",
        Key::Tab => "Tab",
        Key::Backspace => "Backspace",
        Key::Delete => "Delete",
        Key::Escape => "Escape",
        Key::Space => "Space",
        Key::Equals | Key::Plus => "=",
        Key::Minus => "-",
        Key::Period => ".",
        Key::Comma => ",",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Num0 => "0",
        Key::Num1 => "1",
        Key::Num2 => "2",
        Key::Num3 => "3",
        Key::Num5 => "5",
        Key::Num8 => "8",
        Key::Num9 => "9",
        Key::F3 => "F3",
        Key::F5 => "F5",
        Key::F7 => "F7",
        Key::F9 => "F9",
        Key::F12 => "F12",
        Key::A => "A",
        Key::B => "B",
        Key::C => "C",
        Key::D => "D",
        Key::E => "E",
        Key::F => "F",
        Key::G => "G",
        Key::H => "H",
        Key::I => "I",
        Key::J => "J",
        Key::K => "K",
        Key::L => "L",
        Key::M => "M",
        Key::N => "N",
        Key::O => "O",
        Key::P => "P",
        Key::R => "R",
        Key::S => "S",
        Key::T => "T",
        Key::U => "U",
        Key::V => "V",
        Key::W => "W",
        Key::X => "X",
        Key::Y => "Y",
        Key::Z => "Z",
        _ => return None,
    })
}

fn combo(m: Modifiers, key: &str, with_shift: bool) -> String {
    let mut s = String::new();
    if m.command {
        s.push_str("Mod+");
    }
    if m.ctrl && !m.command {
        s.push_str("Ctrl+");
    }
    if m.mac_cmd && m.ctrl {
        s.push_str("Ctrl+");
    }
    if m.alt {
        s.push_str("Alt+");
    }
    if m.shift && with_shift {
        s.push_str("Shift+");
    }
    s.push_str(key);
    s
}

/// Find and run the command bound to this key. Returns true if handled.
fn dispatch(app: &mut WordApp, key: Key, m: Modifiers) -> bool {
    let Some(name) = key_name(key) else { return false };
    let cmb = combo(m, name, true);
    let custom_cmd = app.session.custom_shortcuts.get(&cmb).cloned();
    if let Some(cmd_id) = custom_cmd {
        let _ = app.run(&cmd_id, json!({}));
        return true;
    }
    let reg = app.session.registry.clone();
    if let Some(spec) = reg.by_shortcut(&cmb) {
        let _ = app.run(spec.id, json!({}));
        return true;
    }
    // Shift extends caret movement.
    if m.shift
        && let Some(spec) = reg.by_shortcut(&combo(m, name, false))
        && spec.id.starts_with("caret.")
    {
        let _ = app.run(spec.id, json!({"extend": true}));
        return true;
    }
    false
}

/// Events for the focused canvas: text, editing keys, clipboard, IME.
pub fn canvas_events(app: &mut WordApp, ctx: &egui::Context) {
    let events = ctx.input(|i| i.events.clone());
    for e in events {
        match e {
            egui::Event::Text(t) => {
                let m = ctx.input(|i| i.modifiers);
                if m.command || (m.ctrl && !cfg!(target_os = "macos")) {
                    continue;
                }
                if t.chars().all(|c| !c.is_control()) && !t.is_empty() {
                    match app.session.bangla_mode {
                        wordcraft_engine::cmd::bangla::BanglaMode::Off => {
                            let _ = app.run("text.insert", json!({"text": t}));
                        }
                        wordcraft_engine::cmd::bangla::BanglaMode::Jatiya => {
                            let mut bn = String::new();
                            for c in t.chars() {
                                if let Some(s) = wordcraft_engine::cmd::bangla::jatiya_key(c) {
                                    bn.push_str(s);
                                } else {
                                    bn.push(c);
                                }
                            }
                            let _ = app.run("text.insert", json!({"text": bn}));
                        }
                        wordcraft_engine::cmd::bangla::BanglaMode::Ansi => {
                            let mut bn = String::new();
                            for c in t.chars() {
                                if let Some(s) = wordcraft_engine::cmd::bangla::jatiya_key(c) {
                                    bn.push_str(s);
                                } else {
                                    bn.push(c);
                                }
                            }
                            let ansi = wordcraft_engine::cmd::bangla::unicode_to_ansi(&bn);
                            let _ = app.run("text.insert", json!({"text": ansi}));
                        }
                        wordcraft_engine::cmd::bangla::BanglaMode::Phonetic => {
                            let is_delim = t.chars().all(|c| c.is_whitespace() || ",.!?;:()[]{}\"'/\\-+=*&^%$#@~`<>|".contains(c));
                            if is_delim {
                                app.ui.phonetic_buffer.clear();
                                app.ui.phonetic_rendered_len = 0;
                                let _ = app.run("text.insert", json!({"text": t}));
                            } else {
                                app.ui.phonetic_buffer.push_str(&t);
                                let bn = wordcraft_engine::cmd::bangla::phonetic_transliterate(&app.ui.phonetic_buffer);
                                for _ in 0..app.ui.phonetic_rendered_len {
                                    let _ = app.run("text.backspace", json!({}));
                                }
                                let _ = app.run("text.insert", json!({"text": bn}));
                                app.ui.phonetic_rendered_len = bn.chars().count();
                            }
                        }
                    }
                }
            }
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => app.canvas.ime_preedit = text,
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                app.canvas.ime_preedit.clear();
                if !text.is_empty() {
                    let _ = app.run("text.insert", json!({"text": text}));
                }
            }
            egui::Event::Paste(t) => {
                let id = if ctx.input(|i| i.modifiers.shift && i.modifiers.alt) { "edit.pasteText" } else { "edit.paste" };
                let _ = app.run(id, json!({"text": t}));
            }
            egui::Event::Copy | egui::Event::Cut => {
                let id = if matches!(e, egui::Event::Cut) { "edit.cut" } else { "edit.copy" };
                if let Ok(r) = app.run(id, json!({}))
                    && let Some(t) = r.get("text").and_then(Value::as_str)
                    && !t.is_empty()
                {
                    ctx.copy_text(t.to_string());
                }
            }
            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                if key == Key::F12 && !modifiers.shift && !modifiers.alt && !modifiers.ctrl && !modifiers.command {
                    let _ = app.run("tools.banglaMode", json!({"toggle": true}));
                    app.ui.phonetic_buffer.clear();
                    app.ui.phonetic_rendered_len = 0;
                    continue;
                }
                if key == Key::Backspace
                    && app.session.bangla_mode == wordcraft_engine::cmd::bangla::BanglaMode::Phonetic
                    && !app.ui.phonetic_buffer.is_empty()
                {
                    app.ui.phonetic_buffer.pop();
                    for _ in 0..app.ui.phonetic_rendered_len {
                        let _ = app.run("text.backspace", json!({}));
                    }
                    if !app.ui.phonetic_buffer.is_empty() {
                        let bn = wordcraft_engine::cmd::bangla::phonetic_transliterate(&app.ui.phonetic_buffer);
                        let _ = app.run("text.insert", json!({"text": bn}));
                        app.ui.phonetic_rendered_len = bn.chars().count();
                    } else {
                        app.ui.phonetic_rendered_len = 0;
                    }
                    continue;
                }
                if matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::Enter) {
                    app.ui.phonetic_buffer.clear();
                    app.ui.phonetic_rendered_len = 0;
                }
                if key == Key::Escape && app.session.painter.is_some() {
                    app.session.painter = None;
                    continue;
                }
                if key == Key::Escape && app.canvas.draw_tool.is_some() {
                    app.canvas.draw_tool = None;
                    continue;
                }
                if key == Key::Escape && matches!(app.session.sel.focus.story, wordcraft_doc::StoryRef::Part(_)) && app.session.sel.is_collapsed() {
                    let _ = app.run("insert.closeHeader", json!({}));
                    continue;
                }
                dispatch(app, key, modifiers);
            }
            _ => {}
        }
    }
}

/// Command shortcuts when the canvas isn't focused but no text field is either.
pub fn global_shortcuts(app: &mut WordApp, ctx: &egui::Context) {
    if app.canvas.focused || ctx.egui_wants_keyboard_input() {
        return;
    }
    let events = ctx.input(|i| i.events.clone());
    for e in events {
        if let egui::Event::Key { key, pressed: true, modifiers, .. } = e
            && (modifiers.command || matches!(key, Key::F3 | Key::F5 | Key::F7 | Key::F9 | Key::F12))
        {
            dispatch(app, key, modifiers);
        }
    }
}
