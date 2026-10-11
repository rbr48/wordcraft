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

fn normalize_blocks_bijoy(blocks: &mut Vec<std::sync::Arc<wordcraft_doc::Block>>, doc_has_bijoy: bool) {
    for b in blocks {
        let block = std::sync::Arc::make_mut(b);
        match block {
            wordcraft_doc::Block::Para(p) => {
                let mut para_has_bijoy = doc_has_bijoy;
                for r in &p.runs {
                    if let Some(f) = &r.props.font
                        && is_bijoy_font(f)
                    {
                        para_has_bijoy = true;
                        break;
                    }
                }
                if (para_has_bijoy || p.runs.is_empty()) && !p.text.is_empty() {
                    let mut offset = 0;
                    let mut new_text = String::new();
                    let mut new_runs = Vec::new();

                    for r in &p.runs {
                        let end = (offset + r.len).min(p.text.len());
                        let run_slice = if offset < p.text.len() && offset <= end { &p.text[offset..end] } else { "" };
                        offset = end;

                        let is_bijoy = r.props.font.as_deref().is_some_and(is_bijoy_font) || (r.props.font.is_none() && para_has_bijoy);

                        if is_bijoy {
                            let converted = crate::cmd::bangla::ansi_to_unicode(run_slice);
                            let mut props = r.props.clone();
                            props.font = Some("Kalpurush".into());
                            new_runs.push(wordcraft_doc::Run { len: converted.len(), props });
                            new_text.push_str(&converted);
                        } else {
                            new_runs.push(r.clone());
                            new_text.push_str(run_slice);
                        }
                    }

                    if new_runs.is_empty() {
                        let converted = crate::cmd::bangla::ansi_to_unicode(&p.text);
                        new_runs.push(wordcraft_doc::Run {
                            len: converted.len(),
                            props: wordcraft_doc::CharProps { font: Some("Kalpurush".into()), ..Default::default() },
                        });
                        new_text = converted;
                    }

                    p.text = new_text;
                    p.runs = new_runs;
                }
            }
            wordcraft_doc::Block::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        normalize_blocks_bijoy(&mut cell.blocks, doc_has_bijoy);
                    }
                }
            }
        }
    }
}

/// Automatically detects legacy Bijoy/ANSI fonts in the document, transparently
/// converts raw ASCII glyph keystrokes to standard Bengali Unicode, and assigns
/// a clean OpenType Unicode font ("Kalpurush") while strictly preserving character
/// sizing, bold styling, and document colors.
pub fn normalize_bijoy_fonts(doc: &mut Document) {
    let mut has_bijoy = false;

    // Check style sheet default font
    if let Some(f) = &doc.styles.default_chr.font
        && is_bijoy_font(f)
    {
        has_bijoy = true;
    }

    // Check paragraphs in body
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

    // Check parts (headers, footers, text boxes)
    if !has_bijoy {
        for part in doc.parts.values() {
            for b in &part.blocks {
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
            if has_bijoy {
                break;
            }
        }
    }

    if !has_bijoy {
        return;
    }

    // Decode all body blocks recursively
    normalize_blocks_bijoy(&mut doc.body, has_bijoy);

    // Decode all secondary parts (headers, footers, text boxes)
    for part in doc.parts.values_mut() {
        normalize_blocks_bijoy(&mut part.blocks, has_bijoy);
    }

    // Update style sheet default font if it was Bijoy
    if let Some(f) = &doc.styles.default_chr.font
        && is_bijoy_font(f)
    {
        doc.styles.default_chr.font = Some("Kalpurush".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::{Block, CharProps, Document, Paragraph, Rgb, Run, TextColor};

    #[test]
    fn normalize_bijoy_fonts_preserves_formatting_properties() {
        let text = "evsjv‡`k K…wl e¨vsK".to_string();
        let green = TextColor::Rgb(Rgb(0, 102, 0));
        let props = CharProps { font: Some("SutonnyMJ".into()), size: Some(26.0), bold: Some(true), color: Some(green), ..Default::default() };
        let p = Paragraph { text: text.clone(), runs: vec![Run { len: text.len(), props }], ..Default::default() };
        let mut doc = Document { body: vec![std::sync::Arc::new(Block::Para(p))], ..Default::default() };

        normalize_bijoy_fonts(&mut doc);

        let p_out = doc.body[0].as_para().unwrap();
        assert_eq!(p_out.text, "বাংলাদেশ কৃষি ব্যাংক");
        assert_eq!(p_out.runs.len(), 1);
        assert_eq!(p_out.runs[0].props.font.as_deref(), Some("Kalpurush"));
        assert_eq!(p_out.runs[0].props.size, Some(26.0));
        assert_eq!(p_out.runs[0].props.bold, Some(true));
        assert_eq!(p_out.runs[0].props.color, Some(green));
    }
}
