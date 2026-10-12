//! WordCraft proofing.
//!
//! - **Spelling** against the public-domain Moby word list (~160k words, the same list that
//!   drives hyphenation), with inflections (`-s`, `-es`, `-ed`, `-ing`, `-ly`, `'s`…), a user
//!   dictionary and ignore lists; suggestions by edit distance.
//! - **Grammar** checks that are cheap and reliable: repeated words, `a`/`an`, capital letter
//!   after a sentence end, spaces before punctuation, doubled spaces.
//! - **Hyphenation** ([`hyphen`]): dictionary, Liang patterns, heuristic.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod dict;
pub mod hyphen;
pub mod patterns;

use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};

/// A problem found in text: byte range, kind, message and suggestions.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub start: usize,
    pub end: usize,
    pub kind: IssueKind,
    pub message: String,
    pub suggestions: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueKind {
    Spelling,
    Grammar,
}

use std::sync::atomic::{AtomicU64, Ordering};

/// Incremented every time a word is added or removed from the custom dictionary.
pub static DICTIONARY_GENERATION: AtomicU64 = AtomicU64::new(0);

fn user_words() -> &'static RwLock<HashSet<String>> {
    static U: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
    U.get_or_init(|| RwLock::new(HashSet::new()))
}

fn ignored_words() -> &'static RwLock<HashSet<String>> {
    static I: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
    I.get_or_init(|| RwLock::new(HashSet::new()))
}

/// Longest custom-dictionary word kept, in characters (mirrors `file.options` validation).
pub const MAX_CUSTOM_WORD_CHARS: usize = 64;
/// Most custom-dictionary words read from one file (caps a hostile file).
const MAX_CUSTOM_WORDS: usize = 100_000;
/// Largest custom-dictionary file read, in bytes (caps a hostile file).
const MAX_CUSTOM_FILE_BYTES: u64 = 1_000_000;

/// `w` as a custom-dictionary word: trimmed and lower-cased, or `None` when it isn't a single
/// word of at most [`MAX_CUSTOM_WORD_CHARS`] characters (empty, spaces, control characters, or
/// bytes that weren't UTF-8).
///
/// Adding and loading both use it, so a word that can be added is never dropped on the next load.
pub fn custom_word(w: &str) -> Option<String> {
    let w = w.trim();
    if w.is_empty()
        || w.chars().count() > MAX_CUSTOM_WORD_CHARS
        || w.chars().any(|c| c.is_whitespace() || c.is_control() || c == char::REPLACEMENT_CHARACTER)
    {
        return None;
    }
    Some(w.to_lowercase())
}

/// Add a word to the user dictionary ("Add to Dictionary").
pub fn add_word(w: &str) {
    if let Some(w) = custom_word(w)
        && user_words().write().unwrap_or_else(|e| e.into_inner()).insert(w)
    {
        DICTIONARY_GENERATION.fetch_add(1, Ordering::Relaxed);
    }
}

/// Remove a word from the user dictionary.
pub fn remove_word(w: &str) -> bool {
    let removed = user_words().write().unwrap_or_else(|e| e.into_inner()).remove(&w.to_lowercase());
    if removed {
        DICTIONARY_GENERATION.fetch_add(1, Ordering::Relaxed);
    }
    removed
}

/// Words in the user dictionary.
pub fn user_dictionary() -> Vec<String> {
    let mut v: Vec<String> = user_words().read().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect();
    v.sort();
    v
}

/// Add a word to the session-only ignore list ("Ignore All").
pub fn ignore_word(w: &str) {
    ignored_words().write().unwrap_or_else(|e| e.into_inner()).insert(w.to_lowercase());
}

/// The valid words of the custom-dictionary file at `path`, reading at most
/// [`MAX_CUSTOM_FILE_BYTES`] of it and keeping at most [`MAX_CUSTOM_WORDS`]. A missing or
/// unreadable file reads as no words.
fn read_dictionary_file(path: &std::path::Path) -> Vec<String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    let Ok(file) = std::fs::File::open(path) else { return Vec::new() };
    // `take` bounds the read itself, so a huge file is never read whole.
    if file.take(MAX_CUSTOM_FILE_BYTES).read_to_end(&mut bytes).is_err() {
        return Vec::new();
    }
    // Lossy: a cut multi-byte tail becomes U+FFFD and is skipped by validation below,
    // instead of discarding the whole file.
    String::from_utf8_lossy(&bytes).lines().filter_map(custom_word).take(MAX_CUSTOM_WORDS).collect()
}

/// Read the user dictionary from `path` into the in-memory dictionary.
pub fn load_user_dictionary(path: &std::path::Path) -> Vec<String> {
    let words = read_dictionary_file(path);
    if words.is_empty() {
        return Vec::new();
    }
    let mut set = user_words().write().unwrap_or_else(|e| e.into_inner());
    let mut added = Vec::new();
    for w in words {
        if set.len() >= MAX_CUSTOM_WORDS {
            break;
        }
        if set.insert(w.clone()) {
            added.push(w);
        }
    }
    if !added.is_empty() {
        DICTIONARY_GENERATION.fetch_add(1, Ordering::Relaxed);
    }
    added
}

/// Save the user dictionary to `path`, merging with words already on disk and leaving `removed`
/// out. The file is replaced atomically.
pub fn save_user_dictionary(path: &std::path::Path, removed: &[String]) -> std::io::Result<()> {
    let removed: HashSet<String> = removed.iter().map(|w| w.to_lowercase()).collect();
    let on_disk = read_dictionary_file(path);
    {
        let mut set = user_words().write().unwrap_or_else(|e| e.into_inner());
        let mut changed = false;
        for w in on_disk {
            if set.len() >= MAX_CUSTOM_WORDS {
                break;
            }
            if !removed.contains(&w) && set.insert(w) {
                changed = true;
            }
        }
        for w in &removed {
            changed |= set.remove(w);
        }
        if changed {
            DICTIONARY_GENERATION.fetch_add(1, Ordering::Relaxed);
        }
    }
    let mut out = String::new();
    for w in user_dictionary() {
        out.push_str(&w);
        out.push('\n');
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "custom-dictionary.txt".into());
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let written = std::fs::write(&tmp, out).and_then(|()| std::fs::rename(&tmp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

/// Common words the source list lacks (modern vocabulary, contractions).
const COMMON: &str = "a an the and or but if of to in on at by for with from as is am are was were be been being it its we us our you your he him his she her they them their i me my mine this that these those there here what which who whom whose when where why how all any both each few more most other some such no nor not only own same so than too very can will just should now do does did done has have had having may might must shall would could up down out over under again further then once off into onto upon about above below between through during before after until while because per via vs etc ok yes yeah oh ah hi hey bye one two three four five six seven eight nine ten first last next new old get got go goes went gone see saw seen say said make made take took come came give gave know knew think thought look want need use used find found tell told ask asked feel felt try left keep let put mean set run show turn move live play pay hear hold bring begin seem help talk start might";

const EXTRA: &[&str] = &[
    "don't",
    "doesn't",
    "didn't",
    "can't",
    "won't",
    "isn't",
    "aren't",
    "wasn't",
    "weren't",
    "shouldn't",
    "wouldn't",
    "couldn't",
    "i'm",
    "i've",
    "i'd",
    "i'll",
    "you're",
    "you've",
    "you'll",
    "you'd",
    "we're",
    "we've",
    "we'll",
    "they're",
    "they've",
    "they'll",
    "it's",
    "that's",
    "there's",
    "here's",
    "what's",
    "let's",
    "email",
    "emails",
    "online",
    "website",
    "websites",
    "internet",
    "software",
    "app",
    "apps",
    "smartphone",
    "blog",
    "login",
    "logout",
    "username",
    "wifi",
    "toolbar",
    "workflow",
    "workflows",
    "dataset",
    "datasets",
    "metadata",
    "startup",
    "startups",
    "covid",
    "selfie",
    "podcast",
    "podcasts",
    "hashtag",
    "wordcraft",
    "artcraft",
    "discord",
    "github",
    "rust",
    "ok",
    "okay",
    "unlabelled",
    "labelled",
    "colour",
    "colours",
    "favourite",
    "centre",
    "organise",
];

fn known_core(w: &str) -> bool {
    let d = dict::Dictionary::en_us();
    if d.get(w).is_some() || EXTRA.contains(&w) || COMMON.split(' ').any(|c| c == w) {
        return true;
    }
    if user_words().read().unwrap_or_else(|e| e.into_inner()).contains(w) || ignored_words().read().unwrap_or_else(|e| e.into_inner()).contains(w) {
        return true;
    }
    // Inflections.
    let stems: [(&str, &[&str]); 12] = [
        ("'s", &[""]),
        ("s", &[""]),
        ("es", &["", "e"]),
        ("ies", &["y"]),
        ("ed", &["", "e"]),
        ("ied", &["y"]),
        ("ing", &["", "e"]),
        ("ly", &["", "le"]),
        ("ily", &["y"]),
        ("er", &["", "e"]),
        ("est", &["", "e"]),
        ("ness", &[""]),
    ];
    for (suf, adds) in stems {
        if let Some(stem) = w.strip_suffix(suf)
            && stem.len() >= 2
        {
            for a in adds {
                let s = format!("{stem}{a}");
                if d.get(&s).is_some() || EXTRA.contains(&s.as_str()) {
                    return true;
                }
            }
            // Doubled consonant: running → run, stopped → stop.
            let b = stem.as_bytes();
            if (suf == "ing" || suf == "ed" || suf == "er" || suf == "est")
                && b.len() >= 3
                && b.last() == b.get(b.len() - 2)
                && let Some(s) = stem.get(..stem.len() - 1)
                && d.get(s).is_some()
            {
                return true;
            }
        }
    }
    // Prefixes.
    for pre in ["un", "re", "non", "pre", "over", "under", "co", "mis"] {
        if let Some(rest) = w.strip_prefix(pre)
            && rest.len() >= 3
            && d.get(rest).is_some()
        {
            return true;
        }
    }
    false
}

/// Is `word` spelled correctly? Numbers, single letters, ALL-CAPS acronyms, URLs and words with
/// digits are accepted.
pub fn is_correct(word: &str) -> bool {
    let w = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'');
    let w = w.trim_matches('\'');
    if w.chars().count() <= 1 || w.chars().any(|c| c.is_ascii_digit()) || !w.chars().any(char::is_alphabetic) {
        return true;
    }
    if w.chars().all(|c| !c.is_lowercase()) && w.chars().count() <= 6 {
        return true;
    }
    if !w.is_ascii() && !w.chars().any(|c| c.is_ascii_alphabetic()) {
        return true; // other scripts: no dictionary
    }
    // Bengali Unicode script or SutonnyMJ legacy glyphs: no English dictionary
    if w.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c))
        || w.chars().any(|c| {
            matches!(
                c,
                '†' | '‰'
                    | '©'
                    | 'Ö'
                    | '¨'
                    | '„'
                    | '›'
                    | 'š'
                    | '¤'
                    | '¯'
                    | '®'
                    | '”'
                    | '“'
                    | '™'
                    | '¶'
                    | 'Á'
                    | 'Î'
                    | '°'
                    | '±'
                    | '³'
                    | '»'
                    | '¼'
                    | '½'
                    | '¾'
                    | 'Â'
                    | 'Ã'
                    | 'Ä'
                    | 'Å'
                    | 'Æ'
                    | 'Ç'
                    | 'È'
                    | 'É'
                    | 'Ê'
                    | 'Ë'
                    | 'Ì'
                    | 'Ï'
                    | '×'
                    | 'Ø'
                    | 'Ù'
                    | 'Û'
                    | 'Ü'
                    | 'Ý'
                    | 'Þ'
                    | 'ß'
                    | 'à'
                    | 'ã'
                    | 'ä'
                    | 'å'
                    | 'é'
                    | 'ë'
                    | 'ì'
                    | 'í'
                    | 'ð'
                    | 'ó'
                    | 'ô'
                    | 'õ'
                    | 'ö'
                    | '÷'
                    | 'ø'
                    | 'ù'
                    | 'ú'
                    | 'ý'
                    | 'ü'
            )
        })
    {
        return true;
    }
    let lower = w.to_lowercase().replace('’', "'");
    if known_core(&lower) {
        return true;
    }
    // Hyphenated compounds: each part.
    if lower.contains('-') {
        return lower.split('-').all(|p| p.is_empty() || known_core(p));
    }
    false
}

/// Spelling suggestions for a misspelled word (best first, at most `max`).
pub fn suggest(word: &str, max: usize) -> Vec<String> {
    let lower = word.to_lowercase();
    let n = lower.chars().count();
    if n == 0 || n > 40 {
        return Vec::new();
    }
    let d = dict::Dictionary::en_us();
    let mut scored: Vec<(usize, i64, String)> = Vec::new();
    let target: Vec<char> = lower.chars().collect();
    for (w, _) in d.iter() {
        let wn = w.chars().count();
        if wn + 2 < n || wn > n + 2 {
            continue;
        }
        let dist = damerau(&target, w);
        if dist <= 2 {
            let same_first = w.chars().next() == target.first().copied();
            scored.push((dist, if same_first { 0 } else { 1 }, w.to_string()));
        }
    }
    scored.sort_by(|a, b| (a.0, a.1, a.2.len().abs_diff(n)).cmp(&(b.0, b.1, b.2.len().abs_diff(n))).then(a.2.cmp(&b.2)));
    let cap = word.chars().next().is_some_and(char::is_uppercase);
    let upper = word.chars().all(|c| !c.is_lowercase());
    scored
        .into_iter()
        .take(max)
        .map(|(_, _, w)| {
            if upper && n > 1 {
                w.to_uppercase()
            } else if cap {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or(w)
            } else {
                w
            }
        })
        .collect()
}

/// Optimal string alignment distance, early-exit at 3.
fn damerau(a: &[char], b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    let mut prev2 = vec![0usize; m + 1];
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        if let Some(c) = cur.get_mut(0) {
            *c = i;
        }
        let mut row_min = i;
        for j in 1..=m {
            let (ai, bj) = (a.get(i - 1), b.get(j - 1));
            let cost = usize::from(ai != bj);
            let mut v = (prev.get(j).copied().unwrap_or(9) + 1)
                .min(cur.get(j - 1).copied().unwrap_or(9) + 1)
                .min(prev.get(j - 1).copied().unwrap_or(9) + cost);
            if i > 1 && j > 1 && a.get(i - 1) == b.get(j - 2) && a.get(i - 2) == b.get(j - 1) {
                v = v.min(prev2.get(j - 2).copied().unwrap_or(9) + 1);
            }
            if let Some(c) = cur.get_mut(j) {
                *c = v;
            }
            row_min = row_min.min(v);
        }
        if row_min > 2 {
            return 3;
        }
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }
    prev.get(m).copied().unwrap_or(9)
}

/// Words in `text`: (byte start, byte end).
pub fn words(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let is_w = |c: char| c.is_alphanumeric() || c == '\'' || c == '’' || c == '-';
    for (i, c) in text.char_indices() {
        match (start, is_w(c)) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                out.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, text.len()));
    }
    // Trim surrounding apostrophes/hyphens.
    out.into_iter()
        .filter_map(|(a, b)| {
            let w = text.get(a..b)?;
            let lead = w.len() - w.trim_start_matches(['\'', '’', '-']).len();
            let trail = w.len() - w.trim_end_matches(['\'', '’', '-']).len();
            (a + lead < b - trail).then_some((a + lead, b - trail))
        })
        .collect()
}

/// Spelling issues in a paragraph of text (no suggestions; ask [`suggest`] on demand).
pub fn check_spelling(text: &str) -> Vec<Issue> {
    let mut v = Vec::new();
    for (a, b) in words(text) {
        let Some(w) = text.get(a..b) else { continue };
        // Skip URLs/emails.
        let ts = text.get(..a).and_then(|t| t.rfind(char::is_whitespace)).map(|i| i + 1).unwrap_or(0);
        let te = text.get(b..).and_then(|t| t.find(char::is_whitespace)).map(|i| b + i).unwrap_or(text.len());
        let token = text.get(ts..te).unwrap_or("");
        if token.contains("://") || token.contains('@') || token.starts_with("www.") {
            continue;
        }
        if !is_correct(w) {
            v.push(Issue { start: a, end: b, kind: IssueKind::Spelling, message: "Possible spelling mistake".into(), suggestions: Vec::new() });
        }
    }
    v
}

/// Grammar issues in a paragraph of text.
pub fn check_grammar(text: &str) -> Vec<Issue> {
    let mut v = Vec::new();
    let ws = words(text);
    for pair in ws.windows(2) {
        let [(a0, b0), (a1, b1)] = pair else { continue };
        let (Some(w0), Some(w1), Some(gap)) = (text.get(*a0..*b0), text.get(*a1..*b1), text.get(*b0..*a1)) else { continue };
        if gap.trim().is_empty()
            && w0.eq_ignore_ascii_case(w1)
            && w0.chars().all(char::is_alphabetic)
            && !matches!(w0.to_lowercase().as_str(), "had" | "that" | "bye" | "so")
        {
            v.push(Issue {
                start: *a0,
                end: *b1,
                kind: IssueKind::Grammar,
                message: format!("Repeated word: \"{w1}\""),
                suggestions: vec![w0.to_string()],
            });
        }
        if gap == " " && (w0 == "a" || w0 == "A") && w1.chars().next().is_some_and(|c| "aeiouAEIOU".contains(c)) && !starts_consonant_sound(w1) {
            v.push(Issue {
                start: *a0,
                end: *b0,
                kind: IssueKind::Grammar,
                message: "Use \"an\" before a vowel sound".into(),
                suggestions: vec![if w0 == "A" { "An" } else { "an" }.into()],
            });
        }
        if gap == " "
            && (w0 == "an" || w0 == "An")
            && w1.chars().next().is_some_and(|c| c.is_alphabetic() && !"aeiouAEIOUhH".contains(c))
            && !w1.chars().all(|c| c.is_uppercase())
        {
            v.push(Issue {
                start: *a0,
                end: *b0,
                kind: IssueKind::Grammar,
                message: "Use \"a\" before a consonant sound".into(),
                suggestions: vec![if w0 == "An" { "A" } else { "a" }.into()],
            });
        }
    }
    // Space before punctuation; doubled spaces.
    for (i, _) in text.match_indices(" ,").chain(text.match_indices(" .")).chain(text.match_indices(" ;")) {
        if text.get(i + 2..).is_some_and(|r| r.starts_with('.') || r.starts_with(',')) {
            continue; // "..." or numbers like " .5"
        }
        v.push(Issue {
            start: i,
            end: i + 2,
            kind: IssueKind::Grammar,
            message: "Remove the space before the punctuation".into(),
            suggestions: vec![text.get(i + 1..i + 2).unwrap_or("").to_string()],
        });
    }
    for (i, _) in text.match_indices("  ") {
        if i > 0 && text.get(..i).is_some_and(|t| t.ends_with(' ')) {
            continue;
        }
        v.push(Issue { start: i, end: i + 2, kind: IssueKind::Grammar, message: "Extra space".into(), suggestions: vec![" ".into()] });
    }
    // Sentence start capital.
    let mut after_end = true;
    for (a, b) in &ws {
        let Some(w) = text.get(*a..*b) else { continue };
        if after_end
            && w.chars().next().is_some_and(char::is_lowercase)
            && w.chars().all(char::is_alphabetic)
            && !w.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c))
            && !w.chars().any(|c| {
                matches!(
                    c,
                    '†' | '‰'
                        | '©'
                        | 'Ö'
                        | '¨'
                        | '„'
                        | '›'
                        | 'š'
                        | '¤'
                        | '¯'
                        | '®'
                        | '”'
                        | '“'
                        | '™'
                        | '¶'
                        | 'Á'
                        | 'Î'
                        | '°'
                        | '±'
                        | '³'
                        | '»'
                        | '¼'
                        | '½'
                        | '¾'
                        | 'Â'
                        | 'Ã'
                        | 'Ä'
                        | 'Å'
                        | 'Æ'
                        | 'Ç'
                        | 'È'
                        | 'É'
                        | 'Ê'
                        | 'Ë'
                        | 'Ì'
                        | 'Ï'
                        | '×'
                        | 'Ø'
                        | 'Ù'
                        | 'Û'
                        | 'Ü'
                        | 'Ý'
                        | 'Þ'
                        | 'ß'
                        | 'à'
                        | 'ã'
                        | 'ä'
                        | 'å'
                        | 'é'
                        | 'ë'
                        | 'ì'
                        | 'í'
                        | 'ð'
                        | 'ó'
                        | 'ô'
                        | 'õ'
                        | 'ö'
                        | '÷'
                        | 'ø'
                        | 'ù'
                        | 'ú'
                        | 'ý'
                        | 'ü'
                )
            })
        {
            let fixed: String = w.chars().next().map(|f| f.to_uppercase().chain(w.chars().skip(1)).collect()).unwrap_or_default();
            v.push(Issue {
                start: *a,
                end: *b,
                kind: IssueKind::Grammar,
                message: "Capitalize the first word of a sentence".into(),
                suggestions: vec![fixed],
            });
        }
        let tail = text.get(*b..).unwrap_or("");
        let next_non_space = tail.trim_start_matches(['"', '”', ')', '\'']).chars().next();
        after_end = matches!(next_non_space, Some('.' | '!' | '?')) && tail.trim_start_matches(['.', '!', '?', '"', '”', ')']).starts_with(' ');
    }
    v.sort_by_key(|i| i.start);
    v
}

fn starts_consonant_sound(w: &str) -> bool {
    let l = w.to_lowercase();
    ["uni", "use", "usu", "eu", "one", "once", "ubiq", "uti", "ure", "uro"].iter().any(|p| l.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_loads() {
        assert!(dict::Dictionary::en_us().len() > 100_000);
    }

    #[test]
    fn spelling() {
        for w in [
            "if",
            "we",
            "hello",
            "world",
            "running",
            "studios",
            "don't",
            "Thursday",
            "NASA",
            "2026",
            "co-operate",
            "unhappy",
            "colour",
            "it's",
            "stopped",
        ] {
            assert!(is_correct(w), "{w}");
        }
        for w in ["helo", "wrold", "sentense", "recieve"] {
            assert!(!is_correct(w), "{w}");
        }
        let s = suggest("recieve", 5);
        assert!(s.contains(&"receive".to_string()), "{s:?}");
        assert!(suggest("Wrold", 3).contains(&"World".to_string()));
    }

    #[test]
    fn spelling_issues_in_text() {
        let t = "This sentense has a mispelled word, see https://example.com/xyzq.";
        let v = check_spelling(t);
        let bad: Vec<&str> = v.iter().map(|i| &t[i.start..i.end]).collect();
        assert_eq!(bad, vec!["sentense", "mispelled"]);
    }

    #[test]
    fn grammar_issues() {
        let t = "this is is a apple , and an car.  Done";
        let v = check_grammar(t);
        let msgs: Vec<&str> = v.iter().map(|i| i.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("Repeated")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("\"an\"")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("\"a\" before")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("space before")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("Capitalize")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("Extra space")), "{msgs:?}");
        assert!(check_grammar("A university is an hour away.").is_empty(), "{:?}", check_grammar("A university is an hour away."));
    }

    #[test]
    fn user_dictionary_works() {
        assert!(!is_correct("zxqwordcrafty"));
        add_word("zxqwordcrafty");
        assert!(is_correct("zxqwordcrafty"));
    }

    /// #40: the user dictionary survives a save and a fresh load.
    #[test]
    fn user_dictionary_persists_through_a_file() {
        let dir = std::env::temp_dir().join(format!("wc-dict-{}-{}", std::process::id(), "persist"));
        let _ = std::fs::remove_dir_all(&dir);
        // The folder is created on the first save.
        let path = dir.join("sub/custom-dictionary.txt");
        add_word("zxqpersistone");
        add_word("zxqpersisttwo");
        save_user_dictionary(&path, &[]).expect("save works");
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.lines().any(|w| w == "zxqpersistone") && saved.lines().any(|w| w == "zxqpersisttwo"), "{saved}");
        // Written atomically: no temporary file is left beside it.
        let names: Vec<_> = std::fs::read_dir(dir.join("sub")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, ["custom-dictionary.txt"]);
        remove_word("zxqpersistone");
        assert!(!is_correct("zxqpersistone"));
        load_user_dictionary(&path);
        assert!(is_correct("zxqpersistone"), "loading brings the word back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// #40: a hostile file is capped: invalid lines are skipped, and only the first megabyte is read.
    #[test]
    fn hostile_dictionary_files_are_capped() {
        let dir = std::env::temp_dir().join(format!("wc-dict-{}-{}", std::process::id(), "hostile"));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("custom-dictionary.txt");
        let mut hostile = vec![0xff, 0xfe, b'\n'];
        hostile.extend(b"zxqhostileok\n  zxqhostiletrim  \nzxq hostile space\nzxq\x07bell\n");
        hostile.extend("zxq".repeat(30).bytes()); // 90 characters: too long
        hostile.push(b'\n');
        // Pad past the 1 MB cap with blank lines; the word after it is never read.
        hostile.resize(1_000_100, b'\n');
        hostile.extend(b"zxqhostilepastcap\n");
        std::fs::write(&path, &hostile).unwrap();
        let added = load_user_dictionary(&path);
        assert!(added.contains(&"zxqhostileok".to_string()) && added.contains(&"zxqhostiletrim".to_string()), "{added:?}");
        assert!(!added.iter().any(|w| w.contains("space") || w.contains("bell") || w.chars().count() > MAX_CUSTOM_WORD_CHARS), "{added:?}");
        assert!(!is_correct("zxqhostilepastcap"), "the file is read only up to the cap");
        assert_eq!(read_dictionary_file(&path).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// #40: saving merges in words another process saved, and leaves out the ones just removed.
    #[test]
    fn saving_merges_with_the_file() {
        let dir = std::env::temp_dir().join(format!("wc-dict-{}-{}", std::process::id(), "merge"));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("custom-dictionary.txt");
        std::fs::write(&path, "zxqmergeother\nzxqmergegone\n").unwrap();
        add_word("zxqmergemine");
        save_user_dictionary(&path, &["zxqmergegone".to_string()]).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.lines().any(|w| w == "zxqmergeother"), "the other process's word is kept: {saved}");
        assert!(saved.lines().any(|w| w == "zxqmergemine"), "{saved}");
        assert!(!saved.lines().any(|w| w == "zxqmergegone"), "a removed word stays out: {saved}");
        assert!(is_correct("zxqmergeother") && !is_correct("zxqmergegone"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ignore All lasts for the session: the word is accepted but isn't in the dictionary.
    #[test]
    fn ignored_words_are_not_dictionary_words() {
        assert!(!is_correct("zxqignoredword"));
        ignore_word("ZxqIgnoredWord");
        assert!(is_correct("zxqignoredword"));
        assert!(!user_dictionary().contains(&"zxqignoredword".to_string()));
    }

    #[test]
    fn custom_words_are_single_short_words() {
        assert_eq!(custom_word("  Hello "), Some("hello".into()));
        for bad in ["", "   ", "two words", "tab\tbed", "bell\u{7}", "bad\u{FFFD}", &"x".repeat(MAX_CUSTOM_WORD_CHARS + 1)] {
            assert_eq!(custom_word(bad), None, "{bad:?}");
        }
        assert!(custom_word(&"é".repeat(MAX_CUSTOM_WORD_CHARS)).is_some(), "the cap counts characters, not bytes");
    }

    /// #40: a missing file loads as empty, never an error.
    #[test]
    fn missing_dictionary_file_loads_empty() {
        let path = std::env::temp_dir().join(format!("wc-dict-{}-missing.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert!(load_user_dictionary(&path).is_empty());
    }

    #[test]
    fn hyphenation() {
        let l = hyphen::Limits::default();
        assert!(hyphen::hyphenate_word("hyphenation", &l).matches('-').count() >= 2);
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "\\PC{0,80}") {
            let _ = check_spelling(&s);
            let _ = check_grammar(&s);
            let _ = suggest(&s, 3);
            let _ = hyphen::hyphen_points(&s, &hyphen::Limits::default());
        }
    }
}
