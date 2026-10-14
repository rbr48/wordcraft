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
    "pdf",
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

/// The narrowest text width of the document's sections, points: what a picture must fit.
pub(crate) fn text_width(doc: &Document) -> f32 {
    doc.sections().iter().map(|(_, sp)| sp.text_width()).fold(f32::INFINITY, f32::min).clamp(36.0, 4000.0)
}

/// Shrink inline pictures wider than `max_w` to it, keeping their proportions. Layout keeps a
/// picture's size (as Word does), so formats whose pictures come at their pixel size (HTML,
/// Markdown, ODT, RTF) are fitted when they are read, as Insert › Pictures does.
pub(crate) fn fit_block_pictures(b: &mut wordcraft_doc::Block, max_w: f32) {
    crate::cmd::paste::each_para(b, 0, &mut |p| {
        let mut changed = false;
        for o in &mut p.objects {
            if let wordcraft_doc::InlineObject::Image { w, h, float, .. } = o
                && float.wrap == wordcraft_doc::para::Wrap::Inline
                && w.is_finite()
                && *w > max_w
            {
                let k = max_w / *w;
                *w = max_w;
                *h = (*h * k).max(1.0);
                changed = true;
            }
        }
        if changed {
            p.touch();
        }
    });
}

/// [`fit_block_pictures`] for a whole document read from a file, against its own text width.
pub(crate) fn fit_pictures(doc: &mut Document) {
    let max_w = text_width(doc);
    for b in &mut doc.body {
        fit_block_pictures(std::sync::Arc::make_mut(b), max_w);
    }
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
        fit_pictures(&mut doc);
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
    // Kalpurush renders slightly larger than SutonnyMJ at the same nominal point size.
    // Reduce by 2 pt (clamped to a minimum of 6 pt) so converted documents look the
    // same as they did in Word / Bijoy.
    if props.size.is_none() {
        props.size = props.size_cs;
    }
    if let Some(sz) = props.size.as_mut() {
        *sz = (*sz - 2.0).max(6.0);
    }
    // Keep size_cs in sync with the adjusted size.
    props.size_cs = props.size;
    if props.bold.is_some() {
        props.bold_cs = props.bold;
    }
    if props.italic.is_some() {
        props.italic_cs = props.italic;
    }
}

fn resolve_style_size(styles: &wordcraft_doc::StyleSheet, style_id: Option<&str>) -> Option<f32> {
    let mut cur = style_id;
    let mut hops = 0usize;
    while let Some(id) = cur {
        hops += 1;
        if hops > 32 {
            break;
        }
        let Some(st) = styles.styles.iter().find(|s| s.id == id) else { break };
        if let Some(sz) = st.chr.size.or(st.chr.size_cs) {
            return Some(sz);
        }
        cur = st.based_on.as_deref();
    }
    styles.default_chr.size.or(styles.default_chr.size_cs)
}

fn is_sutonny_glyph(c: char) -> bool {
    matches!(c, '†' | '‡' | 'ˆ' | '‰' | 'Š' | '‹' | 'Œ' | '‚' | '„' | '•' | '›' | 'œ' | 'š' | 'Ÿ' | 'ƒ') || ('\u{00A1}'..='\u{00FF}').contains(&c)
}

/// A Bengali letter or sign; the Bengali digits (U+09E6–U+09EF) don't count, so a number with a
/// symbol (`২৫°C`, `১০০£`) is never taken for a half-converted word.
fn is_bn_letter(c: char) -> bool {
    ('\u{0980}'..='\u{09E5}').contains(&c) || ('\u{09F0}'..='\u{09FF}').contains(&c)
}

/// One word of Unicode Bangla that still holds SutonnyMJ from an earlier, partial conversion: a
/// Bengali letter together with a SutonnyMJ glyph in the same word, an unconverted pre-base `w`/`z`
/// against a Bengali letter, or a known broken sequence. Words are split at whitespace, so a
/// symbol standing apart (`© ২০২৪`, `½ কেজি`) or a Latin word (`café`) is ordinary text.
fn is_leftover_word(word: &str) -> bool {
    if !word.chars().any(is_bn_letter) {
        return false;
    }
    if word.chars().any(is_sutonny_glyph) || word.contains("šত্ন") || word.contains("¯ত্ন") || word.contains("চঁাদ") {
        return true;
    }
    let chars: Vec<char> = word.chars().collect();
    chars.iter().enumerate().any(|(i, ch)| {
        let near = |j: Option<usize>| j.and_then(|j| chars.get(j)).is_some_and(|c| is_bn_letter(*c));
        let malformed_cluster = matches!(ch, 'া' | 'ি' | 'ী' | 'ু' | 'ূ' | 'ৃ' | 'ে' | 'ৈ' | 'ো' | 'ৌ')
            && chars.get(i + 1) == Some(&'্')
            && chars.get(i + 2).copied().is_some_and(crate::cmd::bangla::is_bn_consonant);
        malformed_cluster || (matches!(ch, 'w' | 'z') && (near(i.checked_sub(1)) || near(i.checked_add(1))))
    })
}

/// Detects text that was partially converted in an earlier session: any of its words is a
/// leftover (see [`is_leftover_word`]).
fn has_leftover_bijoy(text: &str) -> bool {
    text.split_whitespace().any(is_leftover_word)
}

/// Repairs a partially converted Bengali string word by word: only the leftover words are decoded;
/// every other word (Unicode Bangla, English, numbers, symbols) and the whitespace stay as they are.
fn repair_partial_bijoy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut token = String::new();
    let mut in_space = false;
    let flush = |token: &str, in_space: bool, out: &mut String| {
        if !in_space && is_leftover_word(token) {
            out.push_str(&crate::cmd::bangla::ansi_to_unicode(token));
        } else {
            out.push_str(token);
        }
    };
    for ch in text.chars() {
        let is_ws = ch.is_whitespace();
        if is_ws != in_space && !token.is_empty() {
            flush(&token, in_space, &mut out);
            token.clear();
        }
        in_space = is_ws;
        token.push(ch);
    }
    if !token.is_empty() {
        flush(&token, in_space, &mut out);
    }
    out
}

fn blocks_have_bijoy(blocks: &[std::sync::Arc<wordcraft_doc::Block>]) -> bool {
    for b in blocks {
        match &**b {
            wordcraft_doc::Block::Para(p) => {
                if char_props_has_bijoy(&p.mark) || p.runs.iter().any(|r| char_props_has_bijoy(&r.props)) || has_leftover_bijoy(&p.text) {
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

pub(crate) fn slice_safe(s: &str, start: usize, end: usize) -> &str {
    let end = end.min(s.len());
    let start = start.min(end);
    s.get(start..end).unwrap_or("")
}

/// Legacy Bijoy documents often align right-hand header fields (such as `তারিখ`) using spaces
/// plus multiple tabs plus long runs of spaces (`   \t\t\t                `). Because Unicode
/// `Kalpurush` glyphs and spaces are slightly wider than `SutonnyMJ`, trailing spaces before `\t`
/// can push a tab past a 36pt stop and long space runs after `\t` can wrap the date onto line 2.
fn compact_bijoy_tab_spaces(s: &str) -> String {
    if !s.contains('\t') {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    let mut after_tab = false;
    while i < chars.len() {
        let Some(&ch) = chars.get(i) else { break };
        if ch == ' ' {
            let mut j = i;
            while chars.get(j) == Some(&' ') {
                j += 1;
            }
            let space_count = j - i;
            if chars.get(j) == Some(&'\t') {
                // Strip spaces immediately preceding a tab stop.
                i = j;
                continue;
            }
            if after_tab && space_count >= 4 {
                let keep = (space_count / 4).max(1);
                for _ in 0..keep {
                    out.push(' ');
                }
                after_tab = false;
                i = j;
                continue;
            }
            for _ in 0..space_count {
                out.push(' ');
            }
            after_tab = false;
            i = j;
        } else if ch == '\t' {
            out.push('\t');
            after_tab = true;
            i += 1;
        } else {
            out.push(ch);
            after_tab = false;
            i += 1;
        }
    }
    out
}

fn normalize_blocks_bijoy(
    blocks: &mut [std::sync::Arc<wordcraft_doc::Block>],
    default_is_bijoy: bool,
    bijoy_style_ids: &std::collections::HashSet<String>,
    styles: &wordcraft_doc::StyleSheet,
) {
    for b in blocks.iter_mut() {
        let block = std::sync::Arc::make_mut(b);
        match block {
            wordcraft_doc::Block::Para(p) => {
                let style_is_bijoy = match p.props.style.as_deref() {
                    Some(s) => bijoy_style_ids.contains(s),
                    None => default_is_bijoy,
                };
                let style_size = resolve_style_size(styles, p.props.style.as_deref());

                let mut para_has_bijoy = style_is_bijoy || char_props_has_bijoy(&p.mark);
                if !para_has_bijoy {
                    for r in &p.runs {
                        if char_props_has_bijoy(&r.props) {
                            para_has_bijoy = true;
                            break;
                        }
                    }
                }
                let para_is_partial = !para_has_bijoy && has_leftover_bijoy(&p.text);

                if para_has_bijoy {
                    if p.mark.size.is_none() && p.mark.size_cs.is_none() {
                        p.mark.size = style_size;
                    }
                    normalize_bijoy_char_props(&mut p.mark);
                    // Also normalize the paragraph mark's tracked-change old props
                    if let Some(fc) = &mut p.mark.fmt_change
                        && char_props_has_bijoy(&fc.old)
                    {
                        normalize_bijoy_char_props(&mut fc.old);
                    }
                    // Force single line spacing (1.0×) on converted Bijoy paragraphs (including empty ones).
                    p.props.line_spacing = Some(wordcraft_doc::props::LineSpacing::Multiple(1.0));
                } else if para_is_partial {
                    p.mark.font = Some("Kalpurush".into());
                    p.mark.font_cs = Some("Kalpurush".into());
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
                        let is_bijoy = char_props_has_bijoy(&r.props) || (r.props.font.is_none() && r.props.font_cs.is_none() && style_is_bijoy);
                        let is_partial = !is_bijoy && has_leftover_bijoy(run_slice);

                        let mut props = r.props.clone();
                        if is_bijoy {
                            if props.size.is_none() && props.size_cs.is_none() {
                                props.size = style_size;
                            }
                            normalize_bijoy_char_props(&mut props);
                            // Normalize the run's tracked-change old formatting too
                            if let Some(fc) = &mut props.fmt_change
                                && char_props_has_bijoy(&fc.old)
                            {
                                normalize_bijoy_char_props(&mut fc.old);
                            }
                        } else if is_partial {
                            props.font = Some("Kalpurush".into());
                            props.font_cs = Some("Kalpurush".into());
                        } else if para_has_bijoy {
                            // Inline Latin/English runs inside a Bijoy paragraph also reduce
                            // their font size by 2pt so they stay proportional to the surrounding
                            // Bengali sentence.
                            if props.size.is_none() && props.size_cs.is_none() {
                                props.size = style_size;
                            }
                            if props.size.is_none() {
                                props.size = props.size_cs;
                            }
                            if let Some(sz) = props.size.as_mut() {
                                *sz = (*sz - 2.0).max(6.0);
                            }
                            props.size_cs = props.size;
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

                        staged.push(PendingRun { slice: run_slice.to_string(), props, is_bijoy, is_partial });
                    }

                    let mut new_text = String::new();
                    let mut new_runs = Vec::with_capacity(staged.len().max(1));

                    for item in staged {
                        if item.is_bijoy {
                            let converted = compact_bijoy_tab_spaces(&crate::cmd::bangla::ansi_to_unicode(&item.slice));
                            new_runs.push(wordcraft_doc::Run { len: converted.len(), props: item.props });
                            new_text.push_str(&converted);
                        } else if item.is_partial {
                            let converted = compact_bijoy_tab_spaces(&repair_partial_bijoy(&item.slice));
                            new_runs.push(wordcraft_doc::Run { len: converted.len(), props: item.props });
                            new_text.push_str(&converted);
                        } else {
                            new_runs.push(wordcraft_doc::Run { len: item.slice.len(), props: item.props });
                            new_text.push_str(&item.slice);
                        }
                    }

                    if new_runs.is_empty() {
                        let raw = if para_is_partial { repair_partial_bijoy(&p.text) } else { crate::cmd::bangla::ansi_to_unicode(&p.text) };
                        let converted = compact_bijoy_tab_spaces(&raw);
                        let mut props = wordcraft_doc::CharProps { size: style_size, ..Default::default() };
                        normalize_bijoy_char_props(&mut props);
                        new_runs.push(wordcraft_doc::Run { len: converted.len(), props });
                        new_text = converted;
                    }

                    p.text = new_text;
                    p.runs = new_runs;
                    p.props.line_spacing = Some(wordcraft_doc::props::LineSpacing::Multiple(1.0));
                }
            }
            wordcraft_doc::Block::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        normalize_blocks_bijoy(&mut cell.blocks, default_is_bijoy, bijoy_style_ids, styles);
                    }
                }
            }
        }
    }

    // Keep centered bold underlined Bijoy section headings (and any single empty spacing
    // paragraph right below them) together with the following body paragraph so reducing
    // font sizes by 2pt never orphans a section title at the bottom of a page.
    let mut keep_empty = false;
    for b in blocks.iter_mut() {
        if let wordcraft_doc::Block::Para(p) = std::sync::Arc::make_mut(b) {
            if p.text.trim().is_empty() {
                if keep_empty {
                    p.props.keep_next = Some(true);
                    keep_empty = false;
                }
            } else {
                keep_empty = false;
                let is_centered = p.props.align == Some(wordcraft_doc::props::Align::Center);
                let is_bold_ul = p.mark.bold == Some(true) && p.mark.underline.is_some();
                if is_centered && is_bold_ul && p.mark.font.as_deref() == Some("Kalpurush") {
                    p.props.keep_next = Some(true);
                    keep_empty = true;
                }
            }
        } else {
            keep_empty = false;
        }
    }
}

/// Build the set of style IDs whose resolved character properties include a Bijoy font.
/// Resolves font classification with nearest-style precedence: if a style or an intervening
/// ancestor explicitly specifies a font, that nearest declaration determines whether the
/// style is Bijoy (an explicit Latin font overrides an ancestor Bijoy font).
fn bijoy_style_set(styles: &wordcraft_doc::StyleSheet) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let default_is_bijoy = char_props_has_bijoy(&styles.default_chr);
    for st in &styles.styles {
        // 1. If the style itself explicitly defines a font, that nearest declaration is decisive.
        if st.chr.font.is_some() || st.chr.font_cs.is_some() {
            if char_props_has_bijoy(&st.chr) {
                set.insert(st.id.clone());
            }
            continue;
        }

        // 2. Otherwise walk the based_on chain to find the nearest style with an explicit font.
        let mut base = st.based_on.as_deref();
        let mut hops = 0usize;
        let mut resolved_bijoy = None;
        while let Some(b) = base {
            hops += 1;
            if hops > 32 {
                break;
            }
            let Some(parent) = styles.styles.iter().find(|s| s.id == b) else { break };
            if parent.chr.font.is_some() || parent.chr.font_cs.is_some() {
                resolved_bijoy = Some(char_props_has_bijoy(&parent.chr));
                break;
            }
            base = parent.based_on.as_deref();
        }

        // 3. If found an ancestor with an explicit font, use that; otherwise fall back to doc default.
        let is_bijoy = resolved_bijoy.unwrap_or(default_is_bijoy);
        if is_bijoy {
            set.insert(st.id.clone());
        }
    }
    set
}

/// Automatically detects legacy Bijoy/ANSI fonts in the document, transparently
/// converts raw ASCII glyph keystrokes to standard Bengali Unicode, and assigns
/// a clean OpenType Unicode font ("Kalpurush") while strictly preserving character
/// sizing, bold styling, and document colors.
pub fn normalize_bijoy_fonts(doc: &mut Document) {
    let default_is_bijoy = char_props_has_bijoy(&doc.styles.default_chr);
    let bijoy_style_ids = bijoy_style_set(&doc.styles);

    let has_bijoy = default_is_bijoy
        || !bijoy_style_ids.is_empty()
        || blocks_have_bijoy(&doc.body)
        || doc.parts.values().any(|part| blocks_have_bijoy(&part.blocks));

    if !has_bijoy {
        return;
    }

    // Decode all body blocks recursively (including tables)
    normalize_blocks_bijoy(&mut doc.body, default_is_bijoy, &bijoy_style_ids, &doc.styles);

    // Decode all secondary parts (headers, footers, footnotes, endnotes, comments, text boxes)
    for part in doc.parts.values_mut() {
        normalize_blocks_bijoy(&mut part.blocks, default_is_bijoy, &bijoy_style_ids, &doc.styles);
    }

    // Update style sheet default font and any named styles that referenced Bijoy
    if char_props_has_bijoy(&doc.styles.default_chr) {
        normalize_bijoy_char_props(&mut doc.styles.default_chr);
    }
    for st in &mut doc.styles.styles {
        if char_props_has_bijoy(&st.chr) {
            normalize_bijoy_char_props(&mut st.chr);
            st.para.line_spacing = Some(wordcraft_doc::props::LineSpacing::Multiple(1.0));
        }
    }

    // Convert watermark text if it uses a Bijoy font
    if let Some(w) = &mut doc.settings.watermark
        && is_bijoy_font(&w.font)
    {
        w.text = crate::cmd::bangla::ansi_to_unicode(&w.text);
        w.font = "Kalpurush".into();
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
        // Font size reduced by 2 pt (26 pt Bijoy → 24 pt Kalpurush).
        assert_eq!(p_out.runs[0].props.size, Some(24.0));
        assert_eq!(p_out.runs[0].props.size_cs, Some(24.0));
        assert_eq!(p_out.runs[0].props.bold, Some(true));
        assert_eq!(p_out.runs[0].props.bold_cs, Some(true));
        assert_eq!(p_out.runs[0].props.color, Some(green));
        assert_eq!(p_out.mark.font.as_deref(), Some("Kalpurush"));
        assert_eq!(p_out.mark.font_cs.as_deref(), Some("Kalpurush"));
        // Line spacing is forced to single after conversion.
        assert_eq!(p_out.props.line_spacing, Some(wordcraft_doc::props::LineSpacing::Multiple(1.0)));
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

        // Base style with SutonnyMJ 14pt
        let mut st1 = Style { id: "BaseBangla".into(), name: "Base Bangla".into(), kind: StyleKind::Paragraph, ..Default::default() };
        st1.chr.font = Some("SutonnyMJ".into());
        st1.chr.size = Some(14.0);

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

        // Paragraph using ChildBangla with no explicit font or size on runs
        let text = "evsjv‡`k".to_string();
        let mut p = Paragraph::new();
        p.props.style = Some("ChildBangla".into());
        p.text = text.clone();
        p.runs = vec![Run { len: text.len(), props: CharProps::default() }];
        doc.body = vec![std::sync::Arc::new(Block::Para(p))];

        // Watermark in SutonnyMJ
        doc.settings.watermark = Some(Watermark { text: "evsjv‡`k".into(), font: "SutonnyMJ".into(), ..Default::default() });

        normalize_bijoy_fonts(&mut doc);

        let p_out = doc.body[0].as_para().unwrap();
        assert_eq!(p_out.text, "বাংলাদেশ");
        assert_eq!(p_out.runs[0].props.font.as_deref(), Some("Kalpurush"));
        assert_eq!(p_out.runs[0].props.size, Some(12.0));
        assert_eq!(p_out.props.line_spacing, Some(wordcraft_doc::props::LineSpacing::Multiple(1.0)));

        let wm = doc.settings.watermark.unwrap();
        assert_eq!(wm.text, "বাংলাদেশ");
        assert_eq!(wm.font, "Kalpurush");
    }

    /// A one-paragraph document whose text is in an ordinary (Unicode) Bengali font.
    fn unicode_doc(text: &str) -> Document {
        let props = CharProps { font: Some("Kalpurush".into()), ..Default::default() };
        let p = Paragraph { text: text.into(), runs: vec![Run { len: text.len(), props }], ..Default::default() };
        Document { body: vec![std::sync::Arc::new(Block::Para(p))], ..Default::default() }
    }

    fn body_text(doc: &Document) -> String {
        doc.body.first().and_then(|b| b.as_para()).map(|p| p.text.clone()).unwrap_or_default()
    }

    /// Opening a document never rewrites ordinary Unicode Bangla, even beside characters SutonnyMJ
    /// also uses as glyphs (©, °, £, ½, é): set apart by spaces or next to digits they are text.
    #[test]
    fn normalize_bijoy_leaves_unicode_bangla_beside_symbols_alone() {
        for text in ["© ২০২৪ বাংলাদেশ", "তাপমাত্রা ২৫°C আজ", "দাম ১০০£ মাত্র", "½ কেজি চাল", "ক্যাফে café বন্ধ"]
        {
            let mut doc = unicode_doc(text);
            normalize_bijoy_fonts(&mut doc);
            assert_eq!(body_text(&doc), text, "changed on open");
        }
    }

    /// In Unicode text left half-converted, only the word that still holds a SutonnyMJ glyph is
    /// repaired; the Bangla words around it stay as they are.
    #[test]
    fn normalize_bijoy_repairs_only_the_leftover_word() {
        let mut doc = unicode_doc("আমি šত্ন করি");
        normalize_bijoy_fonts(&mut doc);
        let out = body_text(&doc);
        let words: Vec<&str> = out.split(' ').collect();
        assert_eq!(words.len(), 3, "{out:?}");
        assert_eq!((words[0], words[2]), ("আমি", "করি"), "{out:?}");
        assert_ne!(words[1], "šত্ন", "the leftover word is repaired: {out:?}");
        assert!(!words[1].contains('š'), "{out:?}");
    }
    #[test]
    fn unused_bijoy_style_must_not_convert_plain_text() {
        let mut doc = Document::from_text("Hello world");
        doc.styles.default_chr = CharProps { font: Some("Calibri".into()), ..Default::default() };
        doc.styles.styles.push(wordcraft_doc::Style {
            id: "UnusedBanglaStyle".into(),
            name: "Unused Bangla Style".into(),
            chr: CharProps { font: Some("SutonnyMJ".into()), ..Default::default() },
            ..Default::default()
        });
        normalize_bijoy_fonts(&mut doc);
        assert_eq!(body_text(&doc), "Hello world", "plain text should not be decoded as Bijoy");
    }

    #[test]
    fn mixed_english_and_bijoy_import_preserves_english_and_converts_bijoy() {
        let mut doc = Document::default();
        doc.styles.default_chr = CharProps { font: Some("Calibri".into()), ..Default::default() };
        doc.styles.styles.push(wordcraft_doc::Style {
            id: "BanglaStyle".into(),
            name: "Bangla Style".into(),
            chr: CharProps { font: Some("SutonnyMJ".into()), ..Default::default() },
            ..Default::default()
        });
        let p_en = Paragraph {
            text: "Hello world".into(),
            props: wordcraft_doc::props::ParaProps { style: Some("Normal".into()), ..Default::default() },
            ..Default::default()
        };
        let p_bn = Paragraph {
            text: "Avgvi †mvbvi evsjv".into(),
            props: wordcraft_doc::props::ParaProps { style: Some("BanglaStyle".into()), ..Default::default() },
            ..Default::default()
        };
        doc.body = vec![std::sync::Arc::new(Block::Para(p_en)), std::sync::Arc::new(Block::Para(p_bn))];

        normalize_bijoy_fonts(&mut doc);

        let out_en = doc.body[0].as_para().map(|p| p.text.as_str()).unwrap_or("");
        let out_bn = doc.body[1].as_para().map(|p| p.text.as_str()).unwrap_or("");
        assert_eq!(out_en, "Hello world");
        assert_eq!(out_bn, "আমার সোনার বাংলা");
        assert_eq!(doc.body[1].as_para().and_then(|p| p.mark.font.as_deref()), Some("Kalpurush"));
    }

    #[test]
    fn latin_child_style_over_bijoy_ancestor_must_not_convert_plain_text() {
        let mut doc = Document::default();
        doc.styles.default_chr = CharProps { font: Some("Calibri".into()), ..Default::default() };
        // Grandparent/parent style has SutonnyMJ
        doc.styles.styles.push(wordcraft_doc::Style {
            id: "BanglaParent".into(),
            name: "Bangla Parent".into(),
            chr: CharProps { font: Some("SutonnyMJ".into()), ..Default::default() },
            ..Default::default()
        });
        // Child style explicitly overrides font with Latin Calibri
        doc.styles.styles.push(wordcraft_doc::Style {
            id: "LatinChild".into(),
            name: "Latin Child".into(),
            based_on: Some("BanglaParent".into()),
            chr: CharProps { font: Some("Calibri".into()), ..Default::default() },
            ..Default::default()
        });
        let p = Paragraph {
            text: "Hello world".into(),
            props: wordcraft_doc::props::ParaProps { style: Some("LatinChild".into()), ..Default::default() },
            ..Default::default()
        };
        doc.body = vec![std::sync::Arc::new(Block::Para(p))];

        normalize_bijoy_fonts(&mut doc);

        let out = doc.body[0].as_para().map(|p| p.text.as_str()).unwrap_or("");
        assert_eq!(out, "Hello world", "explicit Latin child style must override Bijoy ancestor");
        // Style font must remain Calibri, NOT converted to Kalpurush
        let child_style = doc.styles.styles.iter().find(|s| s.id == "LatinChild").unwrap();
        assert_eq!(child_style.chr.font.as_deref(), Some("Calibri"));
    }

    #[test]
    fn intervening_latin_parent_breaks_bijoy_inheritance() {
        let mut doc = Document::default();
        doc.styles.default_chr = CharProps { font: Some("Calibri".into()), ..Default::default() };
        // Grandparent style has SutonnyMJ
        doc.styles.styles.push(wordcraft_doc::Style {
            id: "BanglaGrandparent".into(),
            name: "Bangla Grandparent".into(),
            chr: CharProps { font: Some("SutonnyMJ".into()), ..Default::default() },
            ..Default::default()
        });
        // Intervening parent style overrides with Latin Calibri
        doc.styles.styles.push(wordcraft_doc::Style {
            id: "LatinParent".into(),
            name: "Latin Parent".into(),
            based_on: Some("BanglaGrandparent".into()),
            chr: CharProps { font: Some("Calibri".into()), ..Default::default() },
            ..Default::default()
        });
        // Child style specifies no explicit font, inherits from LatinParent
        doc.styles.styles.push(wordcraft_doc::Style {
            id: "ChildStyle".into(),
            name: "Child Style".into(),
            based_on: Some("LatinParent".into()),
            ..Default::default()
        });
        let p = Paragraph {
            text: "Important announcement".into(),
            props: wordcraft_doc::props::ParaProps { style: Some("ChildStyle".into()), ..Default::default() },
            ..Default::default()
        };
        doc.body = vec![std::sync::Arc::new(Block::Para(p))];

        normalize_bijoy_fonts(&mut doc);

        let out = doc.body[0].as_para().map(|p| p.text.as_str()).unwrap_or("");
        assert_eq!(out, "Important announcement", "intervening Latin parent must break Bijoy inheritance");
    }
}
