//! File format dispatch by extension. Format crates plug in here.

use wordcraft_doc::Document;

/// Formats WordCraft opens.
pub const OPEN_EXTS: &[&str] =
    &["docx", "docm", "dotx", "dotm", "txt", "md", "markdown", "html", "htm", "rtf", "odt", "tex", "latex", "ltx", "wcraft.json", "json"];
/// Formats WordCraft saves (Save As).
pub const SAVE_EXTS: &[&str] = &["docx", "docm", "dotx", "dotm", "pdf", "txt", "md", "html", "rtf", "odt", "tex", "png", "json", "epub"];

fn ext_of(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    lower.rsplit('.').next().unwrap_or("").to_string()
}

/// Is `ext` one of Word's package formats (.docx/.docm/.dotx/.dotm), which save without loss?
pub fn is_word_package(ext: &str) -> bool {
    wordcraft_docx::Flavor::from_ext(ext).is_some()
}

/// Parse a document from bytes; `name` gives the format by extension.
pub fn open_bytes(name: &str, bytes: &[u8]) -> Result<Document, String> {
    let ext = ext_of(name);
    let mut doc = match ext.as_str() {
        "txt" | "text" | "" => Document::from_text(&decode_text(bytes)),
        "json" => serde_json::from_slice::<Document>(bytes).map_err(|e| format!("{name}: {e}"))?,
        other => match crate::io_ext::open(other, bytes) {
            Some(r) => r?,
            None => return Err(format!("{name}: unsupported format `.{other}`")),
        },
    };
    normalize_bijoy_fonts(&mut doc);
    doc.ensure_nonempty();
    Ok(doc)
}

/// Serialise a document; `name` gives the format by extension.
pub fn save_bytes(name: &str, doc: &Document) -> Result<Vec<u8>, String> {
    let ext = ext_of(name);
    match ext.as_str() {
        "txt" | "text" => Ok(doc.plain_text(wordcraft_doc::StoryRef::Body).replace('\n', "\r\n").into_bytes()),
        "json" => serde_json::to_vec_pretty(doc).map_err(|e| e.to_string()),
        other => match crate::io_ext::save(other, doc) {
            Some(r) => r,
            None => Err(format!("{name}: can't save as `.{other}`")),
        },
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn open_path(path: &std::path::Path) -> Result<Document, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > 2 << 30 {
        return Err(format!("{}: file is larger than 2 GB", path.display()));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    open_bytes(&path.to_string_lossy(), &bytes)
}

#[cfg(target_arch = "wasm32")]
pub fn open_path(path: &std::path::Path) -> Result<Document, String> {
    Err(format!("{}: files are opened through the browser on the web", path.display()))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_path(path: &std::path::Path, doc: &Document) -> Result<(), String> {
    let bytes = save_bytes(&path.to_string_lossy(), doc)?;
    // Write atomically: temp file next to the target, then rename.
    let tmp = path.with_extension(format!("{}.tmp", ext_of(&path.to_string_lossy())));
    std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(target_arch = "wasm32")]
pub fn save_path(path: &std::path::Path, _doc: &Document) -> Result<(), String> {
    Err(format!("{}: files are saved through the browser on the web", path.display()))
}

/// UTF-8 (with or without BOM), UTF-16 with BOM, else Latin-1.
pub fn decode_text(b: &[u8]) -> String {
    if let Some(rest) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if b.len() >= 2 && (b[0] == 0xFF && b[1] == 0xFE || b[0] == 0xFE && b[1] == 0xFF) {
        let le = b[0] == 0xFF;
        let units: Vec<u16> =
            b[2..].as_chunks::<2>().0.iter().map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        return String::from_utf16_lossy(&units);
    }
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b.iter().map(|c| *c as char).collect(),
    }
}

/// Known legacy Bijoy / ANSI Bengali font names that store glyph-encoded ASCII.
fn is_bijoy_font(font_name: &str) -> bool {
    let lower = font_name.to_ascii_lowercase();
    lower.contains("sutonnymj")
        || lower.contains("sutonny")
        || lower.contains("siyam rupali ansi")
        || lower.contains("bijoy")
        || lower == "classic bangla"
}

fn normalize_blocks_bijoy(blocks: &mut Vec<std::sync::Arc<wordcraft_doc::Block>>) {
    for b in blocks {
        let block = std::sync::Arc::make_mut(b);
        match block {
            wordcraft_doc::Block::Para(p) => {
                let mut para_has_bijoy = false;
                for r in &mut p.runs {
                    if let Some(f) = &r.props.font
                        && is_bijoy_font(f)
                    {
                        r.props.font = Some("Kalpurush".into());
                        para_has_bijoy = true;
                    }
                }
                if (para_has_bijoy || p.runs.is_empty()) && !p.text.is_empty() {
                    let converted = crate::cmd::bangla::ansi_to_unicode(&p.text);
                    p.text = converted.clone();
                    p.runs = vec![wordcraft_doc::Run {
                        len: converted.len(),
                        props: wordcraft_doc::CharProps { font: Some("Kalpurush".into()), ..Default::default() },
                    }];
                }
            }
            wordcraft_doc::Block::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        normalize_blocks_bijoy(&mut cell.blocks);
                    }
                }
            }
        }
    }
}

/// Automatically detects legacy Bijoy/ANSI fonts in the document, transparently
/// converts raw ASCII glyph keystrokes to standard Bengali Unicode, and assigns
/// a clean OpenType Unicode font ("Kalpurush").
pub fn normalize_bijoy_fonts(doc: &mut Document) {
    let mut has_bijoy = false;

    // Check style sheet default font
    if let Some(f) = &doc.styles.default_chr.font
        && is_bijoy_font(f)
    {
        has_bijoy = true;
    }

    // Check paragraphs
    if !has_bijoy {
        for b in &doc.body {
            if let wordcraft_doc::Block::Para(p) = &**b {
                for r in &p.runs {
                    if let Some(f) = &r.props.font
                        && is_bijoy_font(f)
                    {
                        has_bijoy = true;
                        break;
                    }
                }
            }
            if has_bijoy {
                break;
            }
        }
    }

    if !has_bijoy {
        return;
    }

    // Decode all blocks recursively
    normalize_blocks_bijoy(&mut doc.body);

    // Update style sheet default font if it was Bijoy
    if let Some(f) = &doc.styles.default_chr.font
        && is_bijoy_font(f)
    {
        doc.styles.default_chr.font = Some("Kalpurush".into());
    }
}
