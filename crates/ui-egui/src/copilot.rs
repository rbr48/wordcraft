//! Copilot in the app: calls to the user's AI provider run on a background thread, so the
//! document keeps responding (typing, scrolling, menus) while the model thinks. The pane shows
//! the elapsed time and a Cancel button; [`poll`] collects the answer each frame.
//!
//! The model's answer only fills the pane's Result box. The document changes when the user
//! inserts it (`text.insert`), so a late or cancelled answer never edits the document. Calls that
//! answer offline (no API key, Table) still run `tools.copilot` directly: they are instant.

use std::sync::mpsc::{Receiver, TryRecvError};

use serde_json::Value;

use crate::{WordApp, now_ms};

/// A provider call in flight.
struct Job {
    rx: Receiver<Result<String, String>>,
    provider: String,
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

/// Run a Copilot request from the pane (`tools.copilot` params). `label` names a generated table
/// in the result. A provider call goes to a background thread; anything else answers now.
pub fn run(app: &mut WordApp, payload: &Value, label: &str) {
    if app.copilot.busy() {
        app.ui.ai_status = "Copilot is still working on the last request".into();
        return;
    }
    app.ui.ai_status.clear();
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(req) = wordcraft_engine::cmd::tools::copilot_request(&app.session, payload) {
        let (tx, rx) = std::sync::mpsc::channel();
        let wake = app.ctx.clone();
        let provider = req.provider.clone();
        let spawned = std::thread::Builder::new().name("copilot".into()).spawn(move || {
            // A cancelled job has dropped its receiver: the answer is just discarded.
            let _ = tx.send(req.send());
            if let Some(c) = &wake {
                c.request_repaint();
            }
        });
        match spawned {
            Ok(_) => {
                app.ui.ai_response.clear();
                app.copilot.job = Some(Job { rx, provider, started: now_ms() });
            }
            Err(e) => app.ui.ai_status = format!("Error: could not start Copilot: {e}"),
        }
        return;
    }
    match app.session.run("tools.copilot", payload) {
        Ok(res) => {
            if let Some(c) = res.get("content").and_then(Value::as_str) {
                app.ui.ai_response = c.to_string();
            } else if res.get("rows").is_some() {
                let rows = res.get("rows").and_then(Value::as_u64).unwrap_or(0);
                let cols = res.get("cols").and_then(Value::as_u64).unwrap_or(0);
                app.ui.ai_response = format!("✓ Inserted native {rows}×{cols} table for \"{label}\" into document at caret.");
            }
        }
        Err(e) => app.ui.ai_status = format!("Error: {e}"),
    }
}

/// Stop waiting for the call in flight (the thread ends on its own; its answer is dropped).
pub fn cancel(app: &mut WordApp) {
    if app.copilot.job.take().is_some() {
        app.ui.ai_status = "Cancelled".into();
    }
}

/// Collect the answer of the call in flight (each frame).
pub fn poll(app: &mut WordApp, ctx: &egui::Context) {
    let Some(job) = &app.copilot.job else { return };
    match job.rx.try_recv() {
        Ok(Ok(text)) => {
            app.ui.ai_response = text;
            app.copilot.job = None;
        }
        Ok(Err(e)) => {
            app.ui.ai_status = format!("Error: AI Provider ({}) error: {e}", job.provider);
            app.copilot.job = None;
        }
        Err(TryRecvError::Disconnected) => {
            app.ui.ai_status = "Error: Copilot stopped without an answer".into();
            app.copilot.job = None;
        }
        // Keep the elapsed time ticking.
        Err(TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(250)),
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

    use crate::{Services, WordApp};

    /// A fake OpenAI-compatible provider that answers `answer` after `delay`.
    fn fake_provider(delay: Duration, answer: &'static str) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut buf = [0u8; 65536];
            let _ = s.read(&mut buf);
            std::thread::sleep(delay);
            let body = json!({"choices": [{"message": {"content": answer}}]}).to_string();
            let _ =
                write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        });
        format!("http://{addr}/v1/chat/completions")
    }

    fn app_with(url: &str) -> (WordApp, serde_json::Value) {
        let app = WordApp::new(Session::new(Document::from_text("The loan was approved.")), Services::default());
        let payload = json!({"action": "ask", "prompt": "When?", "provider": "Custom", "model": "m", "api_key": "k", "base_url": url});
        (app, payload)
    }

    #[test]
    fn a_slow_provider_does_not_block_the_ui_and_its_answer_arrives() {
        let url = fake_provider(Duration::from_millis(800), "On approval day.");
        let (mut app, payload) = app_with(&url);
        let ctx = egui::Context::default();
        let t = Instant::now();
        super::run(&mut app, &payload, "");
        assert!(t.elapsed() < Duration::from_millis(300), "starting a call returns at once: {:?}", t.elapsed());
        assert!(app.copilot.busy());
        // A second request while one is in flight is refused, not queued.
        super::run(&mut app, &payload, "");
        assert!(app.ui.ai_status.contains("still working"));
        for _ in 0..500 {
            super::poll(&mut app, &ctx);
            if !app.copilot.busy() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!app.copilot.busy());
        assert_eq!(app.ui.ai_response, "On approval day.");
    }

    #[test]
    fn a_cancelled_call_drops_its_late_answer() {
        let url = fake_provider(Duration::from_millis(200), "Late answer");
        let (mut app, payload) = app_with(&url);
        let ctx = egui::Context::default();
        super::run(&mut app, &payload, "");
        super::cancel(&mut app);
        assert!(!app.copilot.busy());
        std::thread::sleep(Duration::from_millis(400));
        super::poll(&mut app, &ctx);
        assert!(app.ui.ai_response.is_empty(), "a cancelled answer never shows");
        assert_eq!(app.ui.ai_status, "Cancelled");
    }

    #[test]
    fn a_provider_error_is_shown_not_panicked() {
        let (mut app, payload) = app_with("http://127.0.0.1:9/unreachable");
        let ctx = egui::Context::default();
        super::run(&mut app, &payload, "");
        for _ in 0..500 {
            super::poll(&mut app, &ctx);
            if !app.copilot.busy() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(app.ui.ai_status.starts_with("Error: AI Provider (Custom)"), "{}", app.ui.ai_status);
    }
}
