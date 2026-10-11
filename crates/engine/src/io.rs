//! File format dispatch by extension. Format crates plug in here.

use wordcraft_doc::Document;

/// Formats WordCraft opens.
pub const OPEN_EXTS: &[&str] = &[
    "docx",
    "docm",
    "dotx",
    "dotm",
    "doc",
    "dot",
    "txt",
    "md",
    "markdown",
    "html",
    "htm",
    "rtf",
    "odt",
    "tex",
    "latex",
    "ltx",
    "wcraft.json",
    "json",
];
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

/// A document password: wiped from memory when dropped, never printed (`Debug` shows `***`).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Password(zeroize::Zeroizing<String>);

impl Password {
    pub fn new(s: &str) -> Password {
        Password(zeroize::Zeroizing::new(s.to_string()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// The text, for a password box to edit in place.
    pub fn as_mut_string(&mut self) -> &mut String {
        &mut self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(***)")
    }
}

/// Does this open error mean the file is password-protected and no password was given?
pub fn needs_password(err: &str) -> bool {
    err.contains(wordcraft_docx::PASSWORD_REQUIRED)
}

/// Does this open error mean the password given was wrong?
pub fn wrong_password(err: &str) -> bool {
    err.contains(wordcraft_docx::WRONG_PASSWORD)
}

/// Can files with this extension be saved with a password (Word's packages)?
pub fn can_encrypt(name: &str) -> bool {
    is_word_package(&ext_of(name))
}

/// Parse a document from bytes; `name` gives the format by extension.
pub fn open_bytes(name: &str, bytes: &[u8]) -> Result<Document, String> {
    open_bytes_with(name, bytes, None).map(|(doc, _)| doc)
}

/// [`open_bytes`] for a file that may be password-protected: a Word document encrypted with a
/// password ([MS-OFFCRYPTO] agile encryption) is decrypted with `password` first. Also says
/// whether the file was encrypted. Without the password needed the error satisfies
/// [`needs_password`]; with a wrong one, [`wrong_password`].
pub fn open_bytes_with(name: &str, bytes: &[u8], password: Option<&str>) -> Result<(Document, bool), String> {
    let ext = ext_of(name);
    if matches!(ext.as_str(), "docx" | "docm" | "dotx" | "dotm" | "doc" | "dot") && wordcraft_docx::is_encrypted(bytes) {
        let mut doc = wordcraft_docx::read_with_password(bytes, password).map_err(|e| e.to_string())?;
        normalize_bijoy_fonts(&mut doc);
        doc.ensure_nonempty();
        return Ok((doc, true));
    }
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
    Ok((doc, false))
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

/// [`save_bytes`], encrypted with `password` when there is one and the format is a Word package
/// (other formats are copies and are written as they are).
pub fn save_bytes_with(name: &str, doc: &Document, password: Option<&str>) -> Result<Vec<u8>, String> {
    let bytes = save_bytes(name, doc)?;
    match password {
        Some(pw) if can_encrypt(name) => wordcraft_docx::encrypt(&bytes, pw).map_err(|e| e.to_string()),
        _ => Ok(bytes),
    }
}

pub fn open_path(path: &std::path::Path) -> Result<Document, String> {
    open_path_with(path, None).map(|(doc, _)| doc)
}

/// [`open_path`] for a file that may be password-protected (see [`open_bytes_with`]).
#[cfg(not(target_arch = "wasm32"))]
pub fn open_path_with(path: &std::path::Path, password: Option<&str>) -> Result<(Document, bool), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > 2 << 30 {
        return Err(format!("{}: file is larger than 2 GB", path.display()));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.to_string_lossy();
    if matches!(ext_of(&name).as_str(), "html" | "htm" | "xhtml") {
        // Pictures referenced by a relative path load from beside the HTML file.
        let images = LocalImages::new(path.parent().unwrap_or(std::path::Path::new("")));
        let mut doc = wordcraft_formats::html::import_with(&wordcraft_formats::html::decode(&bytes), &|src| images.load(src));
        doc.ensure_nonempty();
        return Ok((doc, false));
    }
    open_bytes_with(&name, &bytes, password)
}

/// The pictures an HTML file names by a path relative to its folder.
///
/// Only regular files inside the folder (after resolving `..` and symlinks) are read. URLs
/// (`http:`, `file:`…) and absolute paths are not followed. Each file is read at most once and
/// at most `max_file` bytes of it; every reference, cached or not, counts against a budget for
/// the whole document, so one large picture can't be multiplied by referencing it many times.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct LocalImages {
    /// The folder, canonical; `None` when it can't be resolved (nothing loads then).
    dir: Option<std::path::PathBuf>,
    max_file: u64,
    /// Bytes the document may still load.
    left: std::cell::Cell<u64>,
    cache: std::cell::RefCell<Vec<(std::path::PathBuf, std::sync::Arc<Vec<u8>>)>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LocalImages {
    /// Largest picture file read.
    const MAX_FILE: u64 = 256 << 20;
    /// Picture bytes one document may load, counting every reference.
    const BUDGET: u64 = 1 << 30;
    /// Files whose bytes are kept for further references.
    const CACHE: usize = 64;

    pub(crate) fn new(dir: &std::path::Path) -> LocalImages {
        LocalImages::with_limits(dir, LocalImages::MAX_FILE, LocalImages::BUDGET)
    }

    pub(crate) fn with_limits(dir: &std::path::Path, max_file: u64, budget: u64) -> LocalImages {
        let dir = if dir.as_os_str().is_empty() { std::path::Path::new(".") } else { dir };
        LocalImages { dir: std::fs::canonicalize(dir).ok(), max_file, left: std::cell::Cell::new(budget), cache: Default::default() }
    }

    /// Bytes of the file `src` names, or `None` when it isn't a regular file inside the folder,
    /// is too large, or the budget is spent.
    pub(crate) fn load(&self, src: &str) -> Option<std::sync::Arc<Vec<u8>>> {
        let path = self.resolve(src)?;
        let cached = self.cache.borrow().iter().find(|(p, _)| *p == path).map(|(_, d)| d.clone());
        if let Some(data) = cached {
            self.charge(data.len())?;
            return Some(data);
        }
        let limit = self.max_file.min(self.left.get());
        let bytes = read_regular_file(&path, limit)?;
        // What was read is spent even when it's over the limit.
        self.charge(bytes.len())?;
        if bytes.len() as u64 > limit {
            return None;
        }
        let data = std::sync::Arc::new(bytes);
        let mut cache = self.cache.borrow_mut();
        if cache.len() < LocalImages::CACHE {
            cache.push((path, data.clone()));
        }
        Some(data)
    }

    /// Take `n` bytes from the budget; `None` (and the budget emptied) when there aren't enough.
    fn charge(&self, n: usize) -> Option<()> {
        let left = self.left.get().checked_sub(n as u64);
        self.left.set(left.unwrap_or(0));
        left.map(|_| ())
    }

    /// The canonical path `src` names, if it's inside the folder.
    fn resolve(&self, src: &str) -> Option<std::path::PathBuf> {
        let src = src.split(['#', '?']).next().unwrap_or("");
        if src.is_empty() || src.starts_with("//") || src.split_once(':').is_some_and(|(scheme, _)| !scheme.contains('/')) {
            return None;
        }
        let rel = String::from_utf8(wordcraft_formats::model::percent_decode(src)).ok()?;
        let rel = std::path::Path::new(&rel);
        if rel.components().any(|c| matches!(c, std::path::Component::Prefix(_) | std::path::Component::RootDir)) {
            return None;
        }
        let dir = self.dir.as_ref()?;
        // Canonical paths have `..` and symlinks resolved, so this keeps both inside the folder.
        let path = std::fs::canonicalize(dir.join(rel)).ok()?;
        path.starts_with(dir).then_some(path)
    }
}

/// Up to `limit + 1` bytes of the regular file at `path` (more than `limit` means too large), or
/// `None` for anything else. A named pipe blocks when opened and a device reports a length of 0
/// and may never end, so the path is checked before opening, the opened handle is checked again,
/// and the read is bounded whatever length the file reports.
#[cfg(not(target_arch = "wasm32"))]
fn read_regular_file(path: &std::path::Path, limit: u64) -> Option<Vec<u8>> {
    use std::io::Read;
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut out = Vec::with_capacity(usize::try_from(meta.len().min(limit)).unwrap_or(0));
    file.take(limit.saturating_add(1)).read_to_end(&mut out).ok()?;
    Some(out)
}

#[cfg(target_arch = "wasm32")]
pub fn open_path_with(path: &std::path::Path, _password: Option<&str>) -> Result<(Document, bool), String> {
    Err(format!("{}: files are opened through the browser on the web", path.display()))
}

pub fn save_path(path: &std::path::Path, doc: &Document) -> Result<(), String> {
    save_path_with(path, doc, None)
}

/// [`save_path`], encrypted with `password` as [`save_bytes_with`] does.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_path_with(path: &std::path::Path, doc: &Document, password: Option<&str>) -> Result<(), String> {
    let bytes = save_bytes_with(&path.to_string_lossy(), doc, password)?;
    // Write atomically: temp file next to the target, then rename.
    let tmp = path.with_extension(format!("{}.tmp", ext_of(&path.to_string_lossy())));
    std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(target_arch = "wasm32")]
pub fn save_path_with(path: &std::path::Path, _doc: &Document, _password: Option<&str>) -> Result<(), String> {
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
    let lower = font_name.trim().to_ascii_lowercase();
    lower.contains("sutonnymj")
        || lower.contains("sutonny")
        || lower.contains("siyam rupali ansi")
        || lower.contains("bijoy")
        || lower == "classic bangla"
        || lower.ends_with("mj")
}

fn char_props_has_bijoy(c: &wordcraft_doc::CharProps) -> bool {
    c.font.as_deref().is_some_and(is_bijoy_font) || c.font_cs.as_deref().is_some_and(is_bijoy_font)
}

fn normalize_bijoy_char_props(props: &mut wordcraft_doc::CharProps) {
    props.font = Some("Kalpurush".into());
    props.font_cs = Some("Kalpurush".into());
    if props.size.is_some() {
        props.size_cs = props.size;
    }
    if props.bold.is_some() {
        props.bold_cs = props.bold;
    }
    if props.italic.is_some() {
        props.italic_cs = props.italic;
    }
}

fn is_sutonny_glyph(c: char) -> bool {
    matches!(
        c,
        '†' | '‡' | 'ˆ' | '‰' | 'Š' | '‹' | 'Œ' | '‚' | '„' | '•' | '›' | 'œ' | 'š' | 'Ÿ' | 'ƒ'
    ) || ('\u{00A1}'..='\u{00FF}').contains(&c)
}

/// Detects paragraphs that were partially converted in an earlier session (containing Bengali Unicode
/// mixed with leftover SutonnyMJ glyphs or unconverted `w`/`z` adjacent to Bengali characters).
fn has_leftover_bijoy(text: &str) -> bool {
    let has_bn = text.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c));
    if !has_bn {
        return false;
    }
    if text.chars().any(is_sutonny_glyph) || text.contains("šত্ন") || text.contains("¯ত্ন") || text.contains("চঁাদ") {
        return true;
    }
    let chars: Vec<char> = text.chars().collect();
    for (i, ch) in chars.iter().enumerate() {
        if matches!(ch, 'w' | 'z') {
            let prev_bn = i > 0 && chars.get(i - 1).is_some_and(|c| ('\u{0980}'..='\u{09FF}').contains(c));
            let next_bn = chars.get(i + 1).is_some_and(|c| ('\u{0980}'..='\u{09FF}').contains(c));
            if prev_bn || next_bn {
                return true;
            }
        }
    }
    false
}

/// Repairs a partially converted Bengali string without altering pure English words or numbers.
fn repair_partial_bijoy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut token = String::new();
    let mut in_space = false;

    for ch in text.chars() {
        let is_ws = ch.is_whitespace();
        if is_ws != in_space && !token.is_empty() {
            if !in_space && (token.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c)) || token.chars().any(is_sutonny_glyph)) {
                out.push_str(&crate::cmd::bangla::ansi_to_unicode(&token));
            } else {
                out.push_str(&token);
            }
            token.clear();
        }
        in_space = is_ws;
        token.push(ch);
    }
    if !token.is_empty() {
        if !in_space && (token.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c)) || token.chars().any(is_sutonny_glyph)) {
            out.push_str(&crate::cmd::bangla::ansi_to_unicode(&token));
        } else {
            out.push_str(&token);
        }
    }
    out
}

fn blocks_have_bijoy(blocks: &[std::sync::Arc<wordcraft_doc::Block>]) -> bool {
    for b in blocks {
        match &**b {
            wordcraft_doc::Block::Para(p) => {
                if char_props_has_bijoy(&p.mark)
                    || p.runs.iter().any(|r| char_props_has_bijoy(&r.props))
                    || has_leftover_bijoy(&p.text)
                {
                    return true;
                }
            }
            wordcraft_doc::Block::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        if blocks_have_bijoy(&cell.blocks) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn slice_safe(s: &str, start: usize, end: usize) -> &str {
    let end = end.min(s.len());
    let start = start.min(end);
    s.get(start..end).unwrap_or("")
}

fn normalize_blocks_bijoy(
    blocks: &mut Vec<std::sync::Arc<wordcraft_doc::Block>>,
    default_is_bijoy: bool,
    bijoy_style_ids: &std::collections::HashSet<String>,
) {
    for b in blocks {
        let block = std::sync::Arc::make_mut(b);
        match block {
            wordcraft_doc::Block::Para(p) => {
                // A paragraph is Bijoy if: doc default is Bijoy, or the paragraph mark says so,
                // or any run has a Bijoy font, or the named paragraph style resolves to Bijoy.
                let style_is_bijoy = p.props.style.as_deref()
                    .is_some_and(|s| bijoy_style_ids.contains(s));

                let mut para_has_bijoy = default_is_bijoy
                    || char_props_has_bijoy(&p.mark)
                    || style_is_bijoy;
                if !para_has_bijoy {
                    for r in &p.runs {
                        if char_props_has_bijoy(&r.props) {
                            para_has_bijoy = true;
                            break;
                        }
                    }
                }
                let para_is_partial = !para_has_bijoy && has_leftover_bijoy(&p.text);

                if para_has_bijoy || para_is_partial {
                    normalize_bijoy_char_props(&mut p.mark);
                    // Also normalize the paragraph mark's tracked-change old props
                    if let Some(fc) = &mut p.mark.fmt_change {
                        if char_props_has_bijoy(&fc.old) {
                            normalize_bijoy_char_props(&mut fc.old);
                        }
                    }
                }

                if (para_has_bijoy || para_is_partial || (p.runs.is_empty() && default_is_bijoy)) && !p.text.is_empty() {
                    // 1. Extract slices and classify each run
                    struct PendingRun {
                        slice: String,
                        props: wordcraft_doc::CharProps,
                        is_bijoy: bool,
                        is_partial: bool,
                    }

                    let mut offset = 0;
                    let mut staged: Vec<PendingRun> = Vec::with_capacity(p.runs.len().max(1));

                    for r in &p.runs {
                        let end = (offset + r.len).min(p.text.len());
                        let run_slice = slice_safe(&p.text, offset, end);
                        offset = end;

                        // A run is Bijoy if it explicitly says so, or if it has no font of its
                        // own and the paragraph (or its style) is known to be Bijoy.
                        let is_bijoy = char_props_has_bijoy(&r.props)
                            || (r.props.font.is_none()
                                && r.props.font_cs.is_none()
                                && (para_has_bijoy || style_is_bijoy));
                        let is_partial = !is_bijoy && has_leftover_bijoy(run_slice);

                        let mut props = r.props.clone();
                        if is_bijoy || is_partial {
                            normalize_bijoy_char_props(&mut props);
                            // Normalize the run's tracked-change old formatting too
                            if let Some(fc) = &mut props.fmt_change {
                                if char_props_has_bijoy(&fc.old) {
                                    normalize_bijoy_char_props(&mut fc.old);
                                }
                            }
                        }

                        // Coalesce consecutive Bijoy runs that share identical formatting OR split a single word
                        // across an XML `<w:r>` boundary so pre-base vowels (`w`, `†`, `‰`), conjuncts (`¯Í`),
                        // and reph (`©`) reorder cleanly across the entire word.
                        // Do NOT merge across OBJ placeholder characters (inline images / objects).
                        const OBJ: char = wordcraft_doc::para::OBJ;
                        if let Some(last) = staged.last_mut()
                            && last.is_bijoy
                            && is_bijoy
                        {
                            let same_props = last.props == props;
                            let splits_word = !last.slice.ends_with(char::is_whitespace)
                                && !run_slice.starts_with(char::is_whitespace)
                                && !last.slice.ends_with(OBJ)
                                && !run_slice.starts_with(OBJ);
                            if same_props || splits_word {
                                last.slice.push_str(run_slice);
                                continue;
                            }
                        }

                        staged.push(PendingRun {
                            slice: run_slice.to_string(),
                            props,
                            is_bijoy,
                            is_partial,
                        });
                    }

                    let mut new_text = String::new();
                    let mut new_runs = Vec::with_capacity(staged.len().max(1));

                    for item in staged {
                        if item.is_bijoy {
                            let converted = crate::cmd::bangla::ansi_to_unicode(&item.slice);
                            new_runs.push(wordcraft_doc::Run { len: converted.len(), props: item.props });
                            new_text.push_str(&converted);
                        } else if item.is_partial {
                            let converted = repair_partial_bijoy(&item.slice);
                            new_runs.push(wordcraft_doc::Run { len: converted.len(), props: item.props });
                            new_text.push_str(&converted);
                        } else {
                            new_runs.push(wordcraft_doc::Run { len: item.slice.len(), props: item.props });
                            new_text.push_str(&item.slice);
                        }
                    }

                    if new_runs.is_empty() {
                        let converted = if para_is_partial {
                            repair_partial_bijoy(&p.text)
                        } else {
                            crate::cmd::bangla::ansi_to_unicode(&p.text)
                        };
                        let mut props = wordcraft_doc::CharProps::default();
                        normalize_bijoy_char_props(&mut props);
                        new_runs.push(wordcraft_doc::Run { len: converted.len(), props });
                        new_text = converted;
                    }

                    p.text = new_text;
                    p.runs = new_runs;
                }
            }
            wordcraft_doc::Block::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        normalize_blocks_bijoy(&mut cell.blocks, default_is_bijoy, bijoy_style_ids);
                    }
                }
            }
        }
    }
}

/// Build the set of style IDs whose resolved character properties include a Bijoy font.
/// Walks each style's own `chr` and up to 32 `based_on` hops through the style sheet.
fn bijoy_style_set(styles: &wordcraft_doc::StyleSheet) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    for st in &styles.styles {
        if char_props_has_bijoy(&st.chr) {
            set.insert(st.id.clone());
            continue;
        }
        // Walk the based_on chain
        let mut base = st.based_on.as_deref();
        let mut hops = 0usize;
        while let Some(b) = base {
            hops += 1;
            if hops > 32 {
                break;
            }
            let Some(parent) = styles.styles.iter().find(|s| s.id == b) else { break };
            if char_props_has_bijoy(&parent.chr) {
                set.insert(st.id.clone());
                break;
            }
            base = parent.based_on.as_deref();
        }
    }
    set
}

/// Automatically detects legacy Bijoy/ANSI fonts in the document, transparently
/// converts raw ASCII glyph keystrokes to standard Bengali Unicode, and assigns
/// a clean OpenType Unicode font ("Kalpurush") while strictly preserving character
/// sizing, bold styling, and document colors.
pub fn normalize_bijoy_fonts(doc: &mut Document) {
    let default_is_bijoy = char_props_has_bijoy(&doc.styles.default_chr)
        || doc.styles.styles.iter().any(|s| char_props_has_bijoy(&s.chr));

    let has_bijoy = default_is_bijoy
        || blocks_have_bijoy(&doc.body)
        || doc.parts.values().any(|part| blocks_have_bijoy(&part.blocks));

    if !has_bijoy {
        return;
    }

    // Build the set of style IDs whose font chain resolves to a Bijoy font.
    // This lets us catch paragraphs that use a named style (e.g. "BanglaText")
    // inheriting from SutonnyMJ even if no individual run has an explicit font.
    let bijoy_style_ids = bijoy_style_set(&doc.styles);

    // Decode all body blocks recursively (including tables)
    normalize_blocks_bijoy(&mut doc.body, default_is_bijoy, &bijoy_style_ids);

    // Decode all secondary parts (headers, footers, footnotes, endnotes, comments, text boxes)
    for part in doc.parts.values_mut() {
        normalize_blocks_bijoy(&mut part.blocks, default_is_bijoy, &bijoy_style_ids);
    }

    // Update style sheet default font and any named styles that referenced Bijoy
    if char_props_has_bijoy(&doc.styles.default_chr) {
        normalize_bijoy_char_props(&mut doc.styles.default_chr);
    }
    for st in &mut doc.styles.styles {
        if char_props_has_bijoy(&st.chr) {
            normalize_bijoy_char_props(&mut st.chr);
        }
    }

    // Convert watermark text if it uses a Bijoy font
    if let Some(w) = &mut doc.settings.watermark {
        if is_bijoy_font(&w.font) {
            w.text = crate::cmd::bangla::ansi_to_unicode(&w.text);
            w.font = "Kalpurush".into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::{Block, Cell, CharProps, Document, Paragraph, Rgb, Row, Run, Table, TextColor};

    #[test]
    fn normalize_bijoy_fonts_preserves_formatting_properties() {
        let text = "evsjv‡`k K…wl e¨vsK".to_string();
        let green = TextColor::Rgb(Rgb(0, 102, 0));
        let props = CharProps {
            font: Some("SutonnyMJ".into()),
            font_cs: Some("Calibri".into()),
            size: Some(26.0),
            bold: Some(true),
            color: Some(green),
            ..Default::default()
        };
        let p = Paragraph {
            text: text.clone(),
            runs: vec![Run { len: text.len(), props }],
            mark: CharProps { font: Some("SutonnyMJ".into()), ..Default::default() },
            ..Default::default()
        };
        let mut doc = Document { body: vec![std::sync::Arc::new(Block::Para(p))], ..Default::default() };

        normalize_bijoy_fonts(&mut doc);

        let p_out = doc.body[0].as_para().unwrap();
        assert_eq!(p_out.text, "বাংলাদেশ কৃষি ব্যাংক");
        assert_eq!(p_out.runs.len(), 1);
        assert_eq!(p_out.runs[0].props.font.as_deref(), Some("Kalpurush"));
        assert_eq!(p_out.runs[0].props.font_cs.as_deref(), Some("Kalpurush"));
        assert_eq!(p_out.runs[0].props.size, Some(26.0));
        assert_eq!(p_out.runs[0].props.size_cs, Some(26.0));
        assert_eq!(p_out.runs[0].props.bold, Some(true));
        assert_eq!(p_out.runs[0].props.bold_cs, Some(true));
        assert_eq!(p_out.runs[0].props.color, Some(green));
        assert_eq!(p_out.mark.font.as_deref(), Some("Kalpurush"));
        assert_eq!(p_out.mark.font_cs.as_deref(), Some("Kalpurush"));
    }

    #[test]
    fn normalize_bijoy_coalesces_split_runs_and_tables() {
        // Word splits "BZ¨vw`" across two runs ("BZ¨vw" and "`") inside a table cell
        let r1 = Run { len: "BZ¨vw".len(), props: CharProps { font: Some("SutonnyMJ".into()), ..Default::default() } };
        let r2 = Run { len: "`".len(), props: CharProps { font: Some("SutonnyMJ".into()), ..Default::default() } };
        let cell_para = Paragraph { text: "BZ¨vw`".into(), runs: vec![r1, r2], ..Default::default() };
        let table = Table {
            rows: vec![Row {
                cells: vec![Cell { blocks: vec![std::sync::Arc::new(Block::Para(cell_para))], ..Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut doc = Document { body: vec![std::sync::Arc::new(Block::Table(table))], ..Default::default() };

        normalize_bijoy_fonts(&mut doc);

        let Block::Table(t) = &*doc.body[0] else { panic!("expected table") };
        let p_out = t.rows[0].cells[0].blocks[0].as_para().unwrap();
        assert_eq!(p_out.text, "ইত্যাদি");
    }

    #[test]
    fn normalize_bijoy_style_chain_and_watermark() {
        use wordcraft_doc::{Style, StyleKind, Watermark};

        // Base style with SutonnyMJ
        let mut st1 = Style {
            id: "BaseBangla".into(),
            name: "Base Bangla".into(),
            kind: StyleKind::Paragraph,
            ..Default::default()
        };
        st1.chr.font = Some("SutonnyMJ".into());

        // Child style inheriting from BaseBangla
        let st2 = Style {
            id: "ChildBangla".into(),
            name: "Child Bangla".into(),
            kind: StyleKind::Paragraph,
            based_on: Some("BaseBangla".into()),
            ..Default::default()
        };

        let mut doc = Document::new();
        doc.styles.styles.push(st1);
        doc.styles.styles.push(st2);

        // Paragraph using ChildBangla with no explicit font on runs
        let text = "evsjv‡`k".to_string();
        let mut p = Paragraph::new();
        p.props.style = Some("ChildBangla".into());
        p.text = text.clone();
        p.runs = vec![Run { len: text.len(), props: CharProps::default() }];
        doc.body = vec![std::sync::Arc::new(Block::Para(p))];

        // Watermark in SutonnyMJ
        doc.settings.watermark = Some(Watermark {
            text: "evsjv‡`k".into(),
            font: "SutonnyMJ".into(),
            ..Default::default()
        });

        normalize_bijoy_fonts(&mut doc);

        let p_out = doc.body[0].as_para().unwrap();
        assert_eq!(p_out.text, "বাংলাদেশ");
        assert_eq!(p_out.runs[0].props.font.as_deref(), Some("Kalpurush"));

        let wm = doc.settings.watermark.unwrap();
        assert_eq!(wm.text, "বাংলাদেশ");
        assert_eq!(wm.font, "Kalpurush");
    }
}

