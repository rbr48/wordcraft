//! WordCraft Android Entrypoint (eframe + android-activity).
//!
//! Exposes `android_main` as a native cdylib for the Android NativeActivity runtime.
//! When launched, initializes Android logging and starts WordApp inside eframe.

// Android NativeActivity requires an exported `android_main` symbol via `#[unsafe(no_mangle)]`.
#![allow(unsafe_code)]

#[cfg(target_os = "android")]
use wordcraft_engine::Session;
#[cfg(target_os = "android")]
use wordcraft_ui_egui::Services;
use wordcraft_ui_egui::WordApp;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("WordCraft"));

    log::info!("WordCraft Android starting...");

    let mut options = eframe::NativeOptions::default();
    options.android_app = Some(app);

    if let Err(e) = eframe::run_native(
        "WordCraft",
        options,
        Box::new(|_cc| {
            let doc = wordcraft_doc::Document::new();
            let mut app = WordApp::new(Session::new(doc), Services::default());
            app.autosave = true;
            Ok(Box::new(AndroidShell { app }))
        }),
    ) {
        log::error!("Failed to start WordCraft on Android: {e:?}");
    }
}

/// Android application container wrapping WordApp for eframe::App lifecycle.
#[allow(dead_code)]
struct AndroidShell {
    app: WordApp,
}

impl eframe::App for AndroidShell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.app.logic(ctx);
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.app.raw_input_hook(raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }
}
