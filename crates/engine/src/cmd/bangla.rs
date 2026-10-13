//! Built-in Native Bangla Input Engine (IME) & Unicode ⇄ ANSI (SutonnyMJ) Converter.
//!
//! Enables native Bengali typing (Phonetic, Jatiya / National, ANSI) and two-way
//! document/selection conversions without requiring external software like Bijoy or Avro.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::{CmdResult, CommandSpec, Session, p};

/// Active Bangla typing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BanglaMode {
    #[default]
    Off,
    Phonetic,
    Jatiya,
    Ansi,
}

impl BanglaMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Phonetic => "phonetic",
            Self::Jatiya => "jatiya",
            Self::Ansi => "ansi",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "English",
            Self::Phonetic => "বাং ফনেটিক (Unicode)",
            Self::Jatiya => "বাং জাতীয় (Unicode)",
            Self::Ansi => "বাং ANSI (SutonnyMJ)",
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Self::Off => "EN",
            Self::Phonetic => "বাং Phonetic",
            Self::Jatiya => "বাং জাতীয়",
            Self::Ansi => "বাং ANSI",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Off => Self::Phonetic,
            Self::Phonetic => Self::Jatiya,
            Self::Jatiya => Self::Ansi,
            Self::Ansi => Self::Off,
        }
    }
}

// ============================================================================
// 1. Phonetic Transliteration Engine (Avro-style)
// ============================================================================

/// Transliterates English phonetic text to Bengali Unicode.
pub fn phonetic_transliterate(input: &str) -> String {
    let preprocessed = input
        .replace("du:kho", "দুঃখ")
        .replace("du:kh", "দুঃখ")
        .replace("du:khi", "দুঃখী")
        .replace("dukkhi", "দুঃখী")
        .replace("puno:", "পুনঃ")
        .replace("onto:", "অন্তঃ");

    let mut out = String::with_capacity(preprocessed.len().saturating_mul(2));
    let mut words = Vec::new();
    let mut current_word = String::new();
    let mut is_alpha = false;

    // Split text into tokens (alphabetic word tokens vs whitespace/punctuation)
    for ch in preprocessed.chars() {
        let alpha = ch.is_ascii_alphabetic() || ch == '\'' || ch == '^' || ch == '`';
        if alpha == is_alpha {
            current_word.push(ch);
        } else {
            if !current_word.is_empty() {
                words.push((is_alpha, current_word));
            }
            current_word = String::new();
            current_word.push(ch);
            is_alpha = alpha;
        }
    }
    if !current_word.is_empty() {
        words.push((is_alpha, current_word));
    }

    for (is_word, token) in words {
        if is_word {
            out.push_str(&phonetic_word(&token));
        } else {
            // Convert numbers and punctuation
            for ch in token.chars() {
                match ch {
                    '0'..='9' => {
                        let digit = (ch as u32).saturating_sub('0' as u32);
                        if let Some(bn_digit) = char::from_u32(0x09E6 + digit) {
                            out.push(bn_digit);
                        } else {
                            out.push(ch);
                        }
                    }
                    '$' => out.push('৳'),
                    '|' => out.push('।'),
                    ':' => out.push(':'), // Literal colon
                    '.' => {
                        let prev_is_digit = out.chars().next_back().is_some_and(|c| ('০'..='৯').contains(&c) || c.is_ascii_digit());
                        if prev_is_digit {
                            out.push('.');
                        } else {
                            out.push('।');
                        }
                    }
                    _ => out.push(ch),
                }
            }
        }
    }

    out
}

/// Transliterates a single alphabetic word phonetically.
fn phonetic_word(word: &str) -> String {
    // If word contains internal separators or explicit uppercase 'A' (independent আ), let the character engine handle it
    let has_explicit_independent = word.contains('A') || word.contains('\'') || word.contains('`');

    let lower = word.to_ascii_lowercase();

    // Specific conjunct and word overrides
    match lower.as_str() {
        "jng" => return "জ্ঞ".into(),
        "kkh" => return "ক্ষ".into(),
        "sonar" => return "সোনার".into(),
        "shonar" => return "সোনার".into(),
        "du:kh" | "du:kho" | "dukho" => return "দুঃখ".into(),
        "du:khi" | "dukkhi" => return "দুঃখী".into(),
        "ut`sob" | "utsob" => return "উৎসব".into(),
        "hot`hat" | "hothat" => return "হঠাৎ".into(),
        "ki" => return "কি".into(),
        "koi" => return "কই".into(),
        _ => {}
    }

    // 1. Exact match in full vocabulary database
    if !has_explicit_independent && let Some(&(_, val)) = BANGLA_VOCAB.iter().find(|&&(k, _)| k == lower) {
        return val.to_string();
    }

    let mut res = String::new();
    let chars: Vec<char> = word.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut last_was_consonant = false;
    let mut has_chandrabindu = false;

    while i < len {
        let ch0 = chars[i];
        if ch0 == ':' {
            res.push('ঃ');
            last_was_consonant = false;
            i += 1;
            continue;
        }
        if ch0 == '^' {
            has_chandrabindu = true;
            i += 1;
            continue;
        }
        if ch0 == '\'' || ch0 == '`' {
            // Avro syllable separator (resets consonant context, suppressed in output)
            last_was_consonant = false;
            i += 1;
            continue;
        }
        if ch0 == 'A' {
            res.push('আ');
            last_was_consonant = false;
            i += 1;
            continue;
        }

        // 1. Check for 3-letter consonant matches
        if i + 2 < len {
            let trio: String = chars[i..i + 3].iter().collect();
            let lower_trio = trio.to_ascii_lowercase();
            if lower_trio == "kkh" {
                res.push_str("ক্ষ");
                last_was_consonant = true;
                i += 3;
                continue;
            } else if lower_trio == "jng" {
                res.push_str("জ্ঞ");
                last_was_consonant = true;
                i += 3;
                continue;
            } else if lower_trio == "cch" {
                res.push('ছ');
                last_was_consonant = true;
                i += 3;
                continue;
            } else if lower_trio == "shh" {
                res.push('ষ');
                last_was_consonant = true;
                i += 3;
                continue;
            } else if lower_trio == "rri" {
                if last_was_consonant {
                    res.push('ৃ');
                } else {
                    res.push('ঋ');
                }
                last_was_consonant = false;
                i += 3;
                continue;
            }
        }

        // 2. Check for 2-letter vowel matches
        if i + 1 < len {
            let c2 = chars[i + 1];
            let vowel_match = if c2 == 'A' {
                None
            } else {
                let pair: String = chars[i..i + 2].iter().collect();
                let lower_pair = pair.to_ascii_lowercase();
                match lower_pair.as_str() {
                    "aa" => Some(('আ', 'া')),
                    "ee" => Some(('ঈ', 'ী')),
                    "oo" => Some(('ঊ', 'ূ')),
                    "oi" => Some(('ঐ', 'ৈ')),
                    "ou" => Some(('ঔ', 'ৌ')),
                    _ => None,
                }
            };
            if let Some((ind, dep)) = vowel_match {
                if last_was_consonant {
                    res.push(dep);
                } else {
                    res.push(ind);
                }
                if has_chandrabindu {
                    res.push('ঁ');
                    has_chandrabindu = false;
                }
                last_was_consonant = false;
                i += 2;
                continue;
            }

            // 2-letter consonant matches
            let con_match = match (chars[i], chars[i + 1]) {
                ('k', 'h') | ('K', 'h') => Some("খ"),
                ('g', 'h') | ('G', 'h') => Some("ঘ"),
                ('n', 'g') | ('N', 'g') => {
                    // ng at end or before consonant is anusvara (ং), else ঙ
                    if i + 2 == len || (i + 2 < len && !is_vowel(chars[i + 2])) { Some("ং") } else { Some("ঙ") }
                }
                ('c', 'h') | ('C', 'h') => Some("চ"),
                ('j', 'h') | ('J', 'h') => Some("ঝ"),
                ('T', 'h') => Some("ঠ"),
                ('D', 'h') => Some("ঢ"),
                ('t', 'h') => Some("থ"),
                ('d', 'h') => Some("ধ"),
                ('p', 'h') | ('P', 'h') => Some("ফ"),
                ('b', 'h') | ('B', 'h') => Some("ভ"),
                ('s', 'h') | ('S', 'h') => Some("শ"),
                ('s', 's') => Some("স"),
                ('R', 'h') => Some("ঢ়"),
                ('t', '`') | ('T', '`') => Some("ৎ"),
                _ => None,
            };
            if let Some(bn) = con_match {
                if last_was_consonant {
                    res.push('্');
                }
                res.push_str(bn);
                last_was_consonant = bn != "ৎ" && bn != "ং";
                i += 2;
                continue;
            }
        }

        // 3. Single-character checks
        let c = chars[i];
        let c_lower = c.to_ascii_lowercase();

        if is_vowel(c) {
            let is_upper = c.is_ascii_uppercase();
            let (ind, dep) = match c_lower {
                'a' => ('আ', 'া'),
                'i' => {
                    if is_upper {
                        ('ঈ', 'ী')
                    } else {
                        ('ই', 'ি')
                    }
                }
                'u' => {
                    if is_upper {
                        ('ঊ', 'ূ')
                    } else {
                        ('উ', 'ু')
                    }
                }
                'e' => ('এ', 'ে'),
                'o' => {
                    if is_upper {
                        ('ও', 'ো')
                    } else {
                        ('অ', 'ো')
                    }
                }
                _ => ('অ', ' '),
            };

            if last_was_consonant {
                if dep != ' ' {
                    res.push(dep);
                }
            } else {
                res.push(ind);
            }
            if has_chandrabindu {
                res.push('ঁ');
                has_chandrabindu = false;
            }
            last_was_consonant = false;
            i += 1;
            continue;
        }

        // Single consonant
        let bn_c = match c {
            'k' => "ক",
            'K' => "খ",
            'g' => "গ",
            'G' => "ঘ",
            'c' => "চ",
            'C' => "ছ",
            'j' => "জ",
            'J' => "ঝ",
            'T' => "ট",
            't' => "ত",
            'D' => "ড",
            'd' => "দ",
            'N' => "ণ",
            'n' => "ন",
            'p' => "প",
            'P' => "ফ",
            'f' | 'F' => "ফ",
            'b' => "ব",
            'B' => "ভ",
            'v' | 'V' => "ভ",
            'm' | 'M' => "ম",
            'z' | 'Z' => "জ",
            'r' => "র",
            'l' | 'L' => "ল",
            'S' => "ষ",
            's' => "স",
            'h' | 'H' => "হ",
            'R' => "ড়",
            'y' => {
                if last_was_consonant {
                    "্য"
                } else {
                    "য়"
                }
            }
            'Y' => "্য",
            'w' | 'W' => {
                if last_was_consonant {
                    "্ব"
                } else {
                    "ও"
                }
            }
            _ => "",
        };

        if !bn_c.is_empty() {
            if has_chandrabindu {
                res.push('ঁ');
                has_chandrabindu = false;
            }
            if last_was_consonant && bn_c != "্য" && bn_c != "্ব" {
                // Join consecutive consonants with virama (্)
                res.push('্');
            }
            res.push_str(bn_c);
            last_was_consonant = true;
        } else {
            if has_chandrabindu {
                res.push('ঁ');
                has_chandrabindu = false;
            }
            res.push(c);
            last_was_consonant = false;
        }

        i += 1;
    }

    if has_chandrabindu {
        res.push('ঁ');
    }

    res
}

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'A' | 'e' | 'E' | 'i' | 'I' | 'o' | 'O' | 'u' | 'U')
}

// ============================================================================
// 2. Jatiya / National Keyboard Layout Engine (Unicode)
// ============================================================================

/// Maps a single keypress to Bengali Unicode using the National / Jatiya layout.
pub fn jatiya_key(ch: char) -> Option<&'static str> {
    match ch {
        // Lowercase
        'q' => Some("ঙ"),
        'w' => Some("য"),
        'e' => Some("ড"),
        'r' => Some("প"),
        't' => Some("ট"),
        'y' => Some("চ"),
        'u' => Some("জ"),
        'i' => Some("হ"),
        'o' => Some("গ"),
        'p' => Some("ড়"),
        'a' => Some("ৃ"),
        's' => Some("ু"),
        'd' => Some("ি"), // i-kar
        'f' => Some("া"),  // aa-kar
        'g' => Some("্"),  // Linker / Hasanta
        'h' => Some("ব"),
        'j' => Some("ক"),
        'k' => Some("ত"),
        'l' => Some("দ"),
        'z' => Some("্র"), // Rafala
        'x' => Some("ও"),
        'c' => Some("ে"), // e-kar
        'v' => Some("র"),
        'b' => Some("ন"),
        'n' => Some("স"),
        'm' => Some("ম"),

        // Uppercase
        'Q' => Some("ং"),
        'W' => Some("য়"),
        'E' => Some("ঢ"),
        'R' => Some("ফ"),
        'T' => Some("ঠ"),
        'Y' => Some("ছ"),
        'U' => Some("ঝ"),
        'I' => Some("ঞ"),
        'O' => Some("ঘ"),
        'P' => Some("ঢ়"),
        'A' => Some("র্"), // Reph
        'S' => Some("ূ"),  // uu-kar
        'D' => Some("ী"), // ee-kar
        'F' => Some("অ"), // Dependent vowel A
        'G' => Some("।"), // Dari (Shift+g)
        'H' => Some("ভ"),
        'J' => Some("খ"),
        'K' => Some("থ"),
        'L' => Some("ধ"),
        'Z' => Some("্য"), // Yafala
        'X' => Some("ৌ"),
        'C' => Some("ৈ"), // oi-kar
        'V' => Some("ল"),
        'B' => Some("ণ"),
        'N' => Some("ষ"),
        'M' => Some("শ"),

        // Numbers
        '0' => Some("০"),
        '1' => Some("১"),
        '2' => Some("২"),
        '3' => Some("৩"),
        '4' => Some("৪"),
        '5' => Some("৫"),
        '6' => Some("৬"),
        '7' => Some("৭"),
        '8' => Some("৮"),
        '9' => Some("৯"),

        // Symbols
        ':' => Some(":"),
        ';' => Some(";"),
        '.' => Some("।"),
        '$' => Some("৳"),
        '&' | '^' => Some("ঁ"),
        '|' => Some("ঃ"), // Bisarga (Shift of \ / ৎ)
        '\\' => Some("ৎ"),
        '~' => Some("ঁ"),

        _ => None,
    }
}

/// Maps a physical keypress directly to SutonnyMJ ANSI ASCII character for Bijoy Classic typing.
pub fn bijoy_ansi_key(ch: char) -> Option<&'static str> {
    match ch {
        // Lowercase
        'q' => Some("O"), // ঙ
        'w' => Some("h"), // য
        'e' => Some("W"), // ড
        'r' => Some("c"), // প
        't' => Some("U"), // ট
        'y' => Some("P"), // চ
        'u' => Some("R"), // জ
        'i' => Some("n"), // হ
        'o' => Some("M"), // গ
        'p' => Some("o"), // ড়
        'a' => Some("„"), // ৃ
        's' => Some("y"), // ু
        'd' => Some("w"), // ি (pre-kar)
        'f' => Some("v"), // া
        'g' => Some("&"), // ্ (hasanta)
        'h' => Some("e"), // ব
        'j' => Some("K"), // ক
        'k' => Some("Z"), // ত
        'l' => Some("`"), // দ
        'z' => Some("Ö"), // ্র
        'x' => Some("I"), // ও
        'c' => Some("†"), // ে (pre-kar)
        'v' => Some("i"), // র
        'b' => Some("b"), // ন
        'n' => Some("m"), // স
        'm' => Some("g"), // ম

        // Uppercase
        'Q' => Some("s"),  // ং
        'W' => Some("q"),  // য়
        'E' => Some("X"),  // ঢ
        'R' => Some("d"),  // ফ
        'T' => Some("V"),  // ঠ
        'Y' => Some("Q"),  // ছ
        'U' => Some("S"),  // ঝ
        'I' => Some("T"),  // ঞ
        'O' => Some("N"),  // ঘ
        'P' => Some("p"),  // ঢ়
        'A' => Some("©"),  // Reph (Shift+a in ANSI)
        'S' => Some("~"),  // ূ
        'D' => Some("x"),  // ী
        'F' => Some("A"),  // অ
        'G' => Some("।"),  // । (Dari in Bijoy/SutonnyMJ, Shift+g)
        'H' => Some("f"),  // ভ
        'J' => Some("L"),  // খ
        'K' => Some("_"),  // থ
        'L' => Some("a"),  // ধ
        'Z' => Some("¨"),  // ্য
        'X' => Some("†Š"), // ৌ
        'C' => Some("‰"),  // ৈ (pre-kar)
        'V' => Some("j"),  // ল
        'B' => Some("Y"),  // ণ
        'N' => Some("l"),  // ষ
        'M' => Some("k"),  // শ

        // Numbers
        '0' => Some("0"),
        '1' => Some("1"),
        '2' => Some("2"),
        '3' => Some("3"),
        '4' => Some("4"),
        '5' => Some("5"),
        '6' => Some("6"),
        '7' => Some("7"),
        '8' => Some("8"),
        '9' => Some("9"),

        // Symbols
        ':' => Some(":"), // Literal colon
        ';' => Some(";"),
        '.' => Some("."), // । (SutonnyMJ '.' renders as Bengali Dari)
        '$' => Some("৳"),
        '&' | '^' => Some("u"), // ঁ
        '|' => Some("t"),       // ঃ (Bisarga in SutonnyMJ, Shift of \ / r / ৎ)
        '\\' => Some("r"),      // ৎ
        '~' => Some("u"),       // ঁ

        _ => None,
    }
}

/// Returns the independent Unicode vowel corresponding to 'g' (linker) + vowel key.
pub fn jatiya_linker_vowel(ch: char) -> Option<&'static str> {
    match ch {
        'f' => Some("আ"), // g + f = আ
        'd' => Some("ই"), // g + d = ই
        'D' => Some("ঈ"), // g + D = ঈ
        's' => Some("উ"), // g + s = উ
        'S' => Some("ঊ"), // g + S = ঊ
        'a' => Some("ঋ"), // g + a = ঋ
        'c' => Some("এ"), // g + c = এ
        'C' => Some("ঐ"), // g + C = ঐ
        'X' => Some("ঔ"), // g + X = ঔ
        'x' => Some("ও"), // g + x = ও
        'F' => Some("অ"), // g + F = অ
        _ => None,
    }
}

/// Returns the SutonnyMJ glyph for an independent vowel typed via 'g' (linker) + vowel key.
pub fn bijoy_linker_vowel(ch: char) -> Option<&'static str> {
    match ch {
        'f' => Some("Av"), // g + f = আ
        'd' => Some("B"),  // g + d = ই
        'D' => Some("C"),  // g + D = ঈ
        's' => Some("D"),  // g + s = উ
        'S' => Some("E"),  // g + S = ঊ
        'a' => Some("F"),  // g + a = ঋ
        'c' => Some("G"),  // g + c = এ
        'C' => Some("H"),  // g + C = ঐ
        'X' => Some("J"),  // g + X = ঔ
        'x' => Some("I"),  // g + x = ও
        'F' => Some("A"),  // g + F = অ
        _ => None,
    }
}

/// Looks up the SutonnyMJ conjunct glyph for two Bengali consonants joined by 'g' (hasanta).
pub fn find_bijoy_conjunct(c1: char, c2: char) -> Option<&'static str> {
    match (c1, c2) {
        ('ক', 'ক') => Some("°"),   // ক্ক
        ('ক', 'ট') => Some("±"),   // ক্ট
        ('ক', 'ত') => Some("³"),   // ক্ত
        ('ক', 'ব') => Some("K^"),  // ক্ব
        ('ক', 'ম') => Some("K¥"),  // ক্ম
        ('ক', 'ল') => Some("Kø"),  // ক্ল
        ('ক', 'ষ') => Some("¶"),   // ক্ষ
        ('ক', 'স') => Some("·"),   // ক্স
        ('খ', 'ব') => Some("L^"),  // খ্ব
        ('গ', 'ধ') => Some("»"),   // গ্ধ
        ('গ', 'ন') => Some("Mœ"),  // গ্ন
        ('গ', 'ব') => Some("M^"),  // গ্ব
        ('গ', 'ম') => Some("M¥"),  // গ্ম
        ('গ', 'ল') => Some("Mø"),  // গ্ল
        ('ঘ', 'ন') => Some("Nœ"),  // ঘ্ন
        ('ঙ', 'ক') => Some("¼"),   // ঙ্ক
        ('ঙ', 'খ') => Some("•"),   // ঙ্খ
        ('ঙ', 'গ') => Some("½"),   // ঙ্গ
        ('ঙ', 'ঘ') => Some("O&o"), // ঙ্ঘ
        ('ঙ', 'ম') => Some("O¥"),  // ঙ্ম
        ('চ', 'চ') => Some("”P"),  // চ্চ
        ('চ', 'ছ') => Some("”Q"),  // চ্ছ
        ('চ', 'ঞ') => Some("P&T"), // চ্ঞ
        ('চ', 'ব') => Some("P^"),  // চ্ব
        ('জ', 'জ') => Some("¾"),   // জ্জ
        ('জ', 'ঝ') => Some("S&R"), // জ্ঝ
        ('জ', 'ঞ') => Some("Á"),   // জ্ঞ
        ('জ', 'ব') => Some("R^"),  // জ্ব
        ('ঞ', 'চ') => Some("Â"),   // ঞ্চ
        ('ঞ', 'ছ') => Some("Ã"),   // ঞ্ছ
        ('ঞ', 'জ') => Some("Ä"),   // ঞ্জ
        ('ঞ', 'ঝ') => Some("Å"),   // ঞ্ঝ
        ('ট', 'ট') => Some("Æ"),   // ট্ট
        ('ট', 'ব') => Some("U^"),  // ট্ব
        ('ঠ', 'ব') => Some("V^"),  // ঠ্ব
        ('ড', 'ড') => Some("Ç"),   // ড্ড
        ('ড', 'ব') => Some("W^"),  // ড্ব
        ('ণ', 'ট') => Some("È"),   // ণ্ট
        ('ণ', 'ঠ') => Some("É"),   // ণ্ঠ
        ('ণ', 'ড') => Some("Ê"),   // ণ্ড
        ('ণ', 'ণ') => Some("Yœ"),  // ণ্ণ
        ('ণ', 'ব') => Some("Y^"),  // ণ্ব
        ('ণ', 'ম') => Some("Y¥"),  // ণ্ম
        ('ত', 'ত') => Some("Ë"),   // ত্ত
        ('ত', 'থ') => Some("Ì"),   // ত্থ
        ('ত', 'ন') => Some("Zœ"),  // ত্ন
        ('ত', 'ব') => Some("Z^"),  // ত্ব
        ('ত', 'ম') => Some("Z¥"),  // ত্ম
        ('থ', 'ব') => Some("_^"),  // থ্ব
        ('দ', 'গ') => Some("`M"),  // দ্গ
        ('দ', 'ঘ') => Some("`N"),  // দ্ঘ
        ('দ', 'দ') => Some("Ï"),   // দ্দ
        ('দ', 'ধ') => Some("×"),   // দ্ধ
        ('দ', 'ব') => Some("Ø"),   // দ্ব
        ('দ', 'ভ') => Some("™¢"),  // দ্ভ
        ('দ', 'ম') => Some("Ù"),   // দ্ম
        ('ধ', 'ন') => Some("aœ"),  // ধ্ন
        ('ধ', 'ব') => Some("a^"),  // ধ্ব
        ('ন', 'ট') => Some("›U"),  // ন্ট
        ('ন', 'ঠ') => Some("›V"),  // ন্ঠ
        ('ন', 'ড') => Some("Û"),   // ন্ড
        ('ন', 'ত') => Some("šÍ"),  // ন্ত
        ('ন', 'থ') => Some("š’"),  // ন্থ
        ('ন', 'দ') => Some("›`"),  // ন্দ
        ('ন', 'ধ') => Some("Ü"),   // ন্ধ
        ('ন', 'ন') => Some("bœ"),  // ন্ন
        ('ন', 'ব') => Some("b^"),  // ন্ব
        ('ন', 'ম') => Some("b¥"),  // ন্ম
        ('ন', 'স') => Some("Ý"),   // ন্স
        ('প', 'ট') => Some("Þ"),   // প্ট
        ('প', 'ত') => Some("ß"),   // প্ত
        ('প', 'ন') => Some("cœ"),  // প্ন
        ('প', 'প') => Some("à"),   // প্প
        ('প', 'ল') => Some("cø"),  // প্ল
        ('প', 'স') => Some("cm"),  // প্স
        ('ফ', 'ট') => Some("d&U"), // ফ্ট
        ('ফ', 'ল') => Some("dø"),  // ফ্ল
        ('ব', 'জ') => Some("eR"),  // ব্জ
        ('ব', 'দ') => Some("ã"),   // ব্দ
        ('ব', 'ধ') => Some("ä"),   // ব্ধ
        ('ব', 'ব') => Some("eŸ"),  // ব্ব
        ('ব', 'ল') => Some("ej"),  // ব্ল
        ('ভ', 'ল') => Some("fj"),  // ভ্ল
        ('ম', 'ন') => Some("¤œ"),  // ম্ন
        ('ম', 'প') => Some("¤ú"),  // ম্প
        ('ম', 'ফ') => Some("¯£"),  // ম্ফ
        ('ম', 'ব') => Some("¤^"),  // ম্ব
        ('ম', 'ভ') => Some("¤¢"),  // ম্ভ
        ('ম', 'ম') => Some("¤§"),  // ম্ম
        ('ম', 'ল') => Some("¤ø"),  // ম্ল
        ('ল', 'ক') => Some("é"),   // ল্ক
        ('ল', 'গ') => Some("î"),   // ল্গ
        ('ল', 'ট') => Some("ë"),   // ল্ট
        ('ল', 'ড') => Some("ì"),   // ল্ড
        ('ল', 'প') => Some("í"),   // ল্প
        ('ল', 'ফ') => Some("j&d"), // ল্ফ
        ('ল', 'ব') => Some("j^"),  // ল্ব
        ('ল', 'ভ') => Some("j&f"), // ল্ভ
        ('ল', 'ম') => Some("j¥"),  // ল্ম
        ('ল', 'ল') => Some("jø"),  // ল্ল
        ('শ', 'চ') => Some("ð"),   // শ্চ
        ('শ', 'ছ') => Some("kQ"),  // শ্ছ
        ('শ', 'ন') => Some("kœ"),  // শ্ন
        ('শ', 'ব') => Some("k^"),  // শ্ব
        ('শ', 'ম') => Some("k¥"),  // শ্ম
        ('শ', 'ল') => Some("kø"),  // শ্ল
        ('ষ', 'ক') => Some("®‹"),  // ষ্ক
        ('ষ', 'ট') => Some("ó"),   // ষ্ট
        ('ষ', 'ঠ') => Some("ô"),   // ষ্ঠ
        ('ষ', 'ণ') => Some("õ"),   // ষ্ণ
        ('ষ', 'প') => Some("ö"),   // ষ্প
        ('ষ', 'ফ') => Some("®£"),  // ষ্ফ
        ('ষ', 'ব') => Some("l^"),  // ষ্ব
        ('ষ', 'ম') => Some("®§"),  // ষ্ম
        ('স', 'ক') => Some("¯‹"),  // স্ক
        ('স', 'খ') => Some("¯L"),  // স্খ
        ('স', 'ট') => Some("÷"),   // স্ট
        ('স', 'ত') => Some("¯Í"),  // স্ত
        ('স', 'থ') => Some("¯’"),  // স্থ
        ('স', 'ন') => Some("mœ"),  // স্ন
        ('স', 'প') => Some("¯ú"),  // স্প
        ('স', 'ফ') => Some("ù"),   // স্ফ
        ('স', 'ব') => Some("¯^"),  // স্ব
        ('স', 'ম') => Some("¯§"),  // স্ম
        ('স', 'ল') => Some("mø"),  // স্ল
        ('হ', 'ণ') => Some("n&Y"), // হ্ণ
        ('হ', 'ন') => Some("ý"),   // হ্ন
        ('হ', 'ব') => Some("n^"),  // হ্ব
        ('হ', 'ম') => Some("n&g"), // হ্ম
        ('হ', 'ল') => Some("n&j"), // হ্ল
        ('হ', 'য') => Some("n¨"),  // হ্য
        ('হ', 'ঋ') => Some("ü"),   // হৃ

        // Ra-fala conjuncts
        ('ক', 'র') => Some("µ"),  // ক্র
        ('গ', 'র') => Some("MÖ"), // গ্র
        ('ত', 'র') => Some("Î"),  // ত্র
        ('ধ', 'র') => Some("aÖ"), // ধ্র
        ('প', 'র') => Some("cÖ"), // প্র
        ('ব', 'র') => Some("eª"), // ব্র
        ('ভ', 'র') => Some("å"),  // ভ্র
        ('ম', 'র') => Some("gÖ"), // ম্র
        ('শ', 'র') => Some("kÖ"), // শ্র
        ('ষ', 'র') => Some("lÖ"), // ষ্র
        ('স', 'র') => Some("mÖ"), // স্র
        ('হ', 'র') => Some("nÖ"), // হ্র
        _ => None,
    }
}

// ============================================================================
// 3. Bidirectional Unicode ⇄ ANSI (SutonnyMJ) Converter Engine
// ============================================================================

/// Converts Unicode Bengali text to ANSI (SutonnyMJ encoding).
pub fn unicode_to_ansi(text: &str) -> String {
    let mut s = text.to_string();

    // 1. Common Conjuncts (যুক্তবর্ণ) — 3-consonant clusters first so 2-consonant substrings don't match prematurely
    let conjuncts: &[(&str, &str)] = &[
        ("ঙ্ক্ষ", "•¶"),
        ("ন্ত্র", "š¿"),
        ("ন্তু", "š‘"),
        ("স্ত্র", "¯¿"),
        ("স্তু", "¯‘"),
        ("স্ক্র", "¯Œ"),
        ("ন্দ্র", "›`ª"),
        ("ষ্ট্র", "óª"),
        ("ক্ষ", "¶"),
        ("জ্ঞ", "Á"),
        ("ত্র", "Î"),
        ("ক্ত", "³"),
        ("ক্ক", "°"),
        ("ক্ট", "±"),
        ("ক্স", "·"),
        ("গ্ধ", "»"),
        ("ঙ্ক", "¼"),
        ("ঙ্গ", "½"),
        ("জ্জ", "¾"),
        ("ঞ্চ", "Â"),
        ("ঞ্ছ", "Ã"),
        ("ঞ্জ", "Ä"),
        ("ট্ট", "Æ"),
        ("ড্ড", "Ç"),
        ("ণ্ট", "È"),
        ("ণ্ঠ", "É"),
        ("ণ্ড", "Ê"),
        ("ত্ত", "Ë"),
        ("ত্থ", "Ì"),
        ("ত্ন", "Zœ"),
        ("ত্ম", "Z¥"),
        ("ন্দ", "›`"),
        ("ন্ট", "›U"),
        ("ন্ড", "Û"),
        ("ন্ধ", "Ü"),
        ("ন্ন", "bœ"),
        ("ন্স", "Ý"),
        ("ন্ত", "šÍ"),
        ("প্ট", "Þ"),
        ("প্ত", "ß"),
        ("প্প", "à"),
        ("ব্দ", "ã"),
        ("ব্ধ", "ä"),
        ("ম্প", "¤ú"),
        ("ম্ব", "¤^"),
        ("ম্ভ", "¤¢"),
        ("ম্ম", "¤§"),
        ("ম্ন", "¤œ"),
        ("দ্ভ", "™¢"),
        ("ষ্ট", "ó"),
        ("ষ্ঠ", "ô"),
        ("ষ্ণ", "õ"),
        ("ষ্প", "ö"),
        ("স্ত", "¯Í"),
        ("স্থ", "¯’"),
        ("স্ট", "÷"),
        ("স্ক", "¯‹"),
        ("স্ন", "mœ"),
        ("স্প", "¯ú"),
        ("স্ফ", "ù"),
        ("স্ব", "¯^"),
        ("স্ম", "¯§"),
        ("শ্চ", "ð"),
        ("ল্প", "í"),
        ("ল্গ", "î"),
        ("ল্ট", "ë"),
        ("ল্ড", "ì"),
        ("ল্ক", "é"),
        ("ল্ল", "jø"),
        ("ন্ঠ", "›V"),
        ("ন্থ", "š’"),
        ("ন্ম", "š§"),
        ("ন্ব", "š^"),
        ("স্খ", "¯L"),
        ("চ্চ", "”P"),
        ("চ্ছ", "”Q"),
        ("ষ্ক", "®‹"),
        ("ষ্ফ", "®£"),
        ("ষ্ম", "®§"),
        ("ম্ফ", "¯£"),
        ("ক্র", "µ"),
        ("প্র", "cÖ"),
        ("গ্র", "MÖ"),
        ("ব্র", "eª"),
        ("ভ্র", "å"),
        ("শ্র", "kÖ"),
        ("স্র", "mÖ"),
        ("ধ্র", "aÖ"),
        ("ম্র", "gÖ"),
        ("হ্র", "nÖ"),
        ("দ্দ", "Ï"),
        ("দ্ধ", "×"),
        ("দ্ব", "Ø"),
        ("দ্ম", "Ù"),
        ("হ্ন", "ý"),
        ("হৃ", "ü"),
        ("্র", "Ö"),
        ("্য", "¨"),
        ("্ল", "ø"),
        ("্ন", "œ"),
        ("্ম", "¥"),
        ("্ব", "^"),
    ];

    for &(u, a) in conjuncts {
        s = s.replace(u, a);
    }

    // 2. Reph reordering: র + ্ + X -> X + ©
    let mut chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if i + 2 < len && chars[i] == 'র' && chars[i + 1] == '্' {
            let next = chars[i + 2];
            out.push(next);
            out.push('©'); // SutonnyMJ reph glyph
            i += 3;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }

    // 3. Pre-kar reordering:
    // Unicode has: Consonant + ি/ে/ৈ/ো/ৌ
    // SutonnyMJ requires:
    // ি: 'w' + Consonant
    // ে: '†' (initial) or '‡' (medial) + Consonant
    // ৈ: '‰' + Consonant
    // ো: ('†' or '‡') + Consonant + 'v'
    // ৌ: ('†' or '‡') + Consonant + 'Š'
    chars = out.chars().collect();
    out = String::new();
    i = 0;
    let len = chars.len();

    while i < len {
        // Check for 2-character conjunct cluster first (e.g. "cÖ" for প্র, "¯Í" for স্ত)
        if i + 2 < len && is_two_char_conjunct(chars[i], chars[i + 1]) {
            let next = chars[i + 2];
            let is_medial = i > 0 && !is_word_boundary(chars[i - 1]);
            let e_glyph = if is_medial { '‡' } else { '†' };
            match next {
                'ি' => {
                    out.push('w');
                    out.push(chars[i]);
                    out.push(chars[i + 1]);
                    i += 3;
                    continue;
                }
                'ে' => {
                    out.push(e_glyph);
                    out.push(chars[i]);
                    out.push(chars[i + 1]);
                    i += 3;
                    continue;
                }
                'ৈ' => {
                    out.push('‰');
                    out.push(chars[i]);
                    out.push(chars[i + 1]);
                    i += 3;
                    continue;
                }
                'ো' => {
                    out.push(e_glyph);
                    out.push(chars[i]);
                    out.push(chars[i + 1]);
                    out.push('v');
                    i += 3;
                    continue;
                }
                'ৌ' => {
                    out.push(e_glyph);
                    out.push(chars[i]);
                    out.push(chars[i + 1]);
                    out.push('Š');
                    i += 3;
                    continue;
                }
                _ => {}
            }
        }

        let ch = chars[i];
        if i + 1 < len {
            let next = chars[i + 1];
            let is_medial = i > 0 && !is_word_boundary(chars[i - 1]);
            let e_glyph = if is_medial { '‡' } else { '†' };
            match next {
                'ি' => {
                    out.push('w');
                    out.push(ch);
                    i += 2;
                    continue;
                }
                'ে' => {
                    out.push(e_glyph);
                    out.push(ch);
                    i += 2;
                    continue;
                }
                'ৈ' => {
                    out.push('‰');
                    out.push(ch);
                    i += 2;
                    continue;
                }
                'ো' => {
                    out.push(e_glyph);
                    out.push(ch);
                    out.push('v');
                    i += 2;
                    continue;
                }
                'ৌ' => {
                    out.push(e_glyph);
                    out.push(ch);
                    out.push('Š');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(ch);
        i += 1;
    }

    // 4. Character Mapping to SutonnyMJ ASCII
    let mut final_out = String::with_capacity(out.len());
    for ch in out.chars() {
        let mapped = match ch {
            // Vowels
            'অ' => "A",
            'আ' => "Av",
            'ই' => "B",
            'ঈ' => "C",
            'উ' => "D",
            'ঊ' => "E",
            'ঋ' => "F",
            'এ' => "G",
            'ঐ' => "H",
            'ও' => "I",
            'ঔ' => "J",

            // Consonants
            'ক' => "K",
            'খ' => "L",
            'গ' => "M",
            'ঘ' => "N",
            'ঙ' => "O",
            'চ' => "P",
            'ছ' => "Q",
            'জ' => "R",
            'ঝ' => "S",
            'ঞ' => "T",
            'ট' => "U",
            'ঠ' => "V",
            'ড' => "W",
            'ঢ' => "X",
            'ণ' => "Y",
            'ত' => "Z",
            'থ' => "_",
            'দ' => "`",
            'ধ' => "a",
            'ন' => "b",
            'প' => "c",
            'ফ' => "d",
            'ব' => "e",
            'ভ' => "f",
            'ম' => "g",
            'য' => "h",
            'র' => "i",
            'ল' => "j",
            'শ' => "k",
            'ষ' => "l",
            'স' => "m",
            'হ' => "n",
            'ড়' => "o",
            'ঢ়' => "p",
            'য়' => "q",
            'ৎ' => "r",
            'ং' => "s",
            'ঃ' => "t",
            'ঁ' => "u",

            // Kars
            'া' => "v",
            'ি' => "w",
            'ী' => "x",
            'ু' => "y",
            'ূ' => "~",
            'ৃ' => "„",
            'ে' => "†",
            'ৈ' => "‰",
            'ো' => "†v",
            'ৌ' => "†Š",
            '্' => "&",

            // Digits
            '০' => "0",
            '১' => "1",
            '২' => "2",
            '৩' => "3",
            '৪' => "4",
            '৫' => "5",
            '৬' => "6",
            '৭' => "7",
            '৮' => "8",
            '৯' => "9",

            // Punctuation
            '।' => ".",
            _ => "",
        };

        if !mapped.is_empty() {
            final_out.push_str(mapped);
        } else {
            final_out.push(ch);
        }
    }

    final_out
}

pub fn is_bn_consonant(c: char) -> bool {
    ('ক'..='হ').contains(&c) || ('ড়'..='য়').contains(&c) || c == 'ৎ'
}

fn is_bn_vowel_sign(c: char) -> bool {
    matches!(c, 'া' | 'ি' | 'ী' | 'ু' | 'ূ' | 'ৃ' | 'ে' | 'ৈ' | 'ো' | 'ৌ' | '্' | 'ং' | 'ঃ' | 'ঁ')
}

fn parse_bn_cluster(chars: &[char], start: usize) -> Option<usize> {
    if !chars.get(start).copied().is_some_and(is_bn_consonant) {
        return None;
    }
    let mut end = start + 1;
    while end < chars.len() {
        if chars.get(end) == Some(&'্') && chars.get(end + 1).copied().is_some_and(is_bn_consonant) {
            end += 2;
        } else {
            break;
        }
    }
    Some(end)
}

/// Returns true if `c` is a word boundary delimiter or whitespace.
pub fn is_word_boundary(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            ' ' | '\t'
                | '\n'
                | '\r'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | '"'
                | '\''
                | '‘'
                | '’'
                | '“'
                | '”'
                | '«'
                | '»'
                | '।'
                | '.'
                | ','
                | ';'
                | ':'
                | '!'
                | '?'
                | '-'
                | '—'
                | '–'
                | '/'
                | '\\'
                | '|'
                | '_'
                | '='
                | '+'
                | '*'
                | '#'
                | '@'
                | '%'
                | '^'
                | '&'
                | '~'
                | '`'
        )
}

/// Returns true if (c1, c2) is a 2-character SutonnyMJ conjunct sequence (such as "cÖ" for প্র).
pub fn is_two_char_conjunct(c1: char, c2: char) -> bool {
    matches!(
        (c1, c2),
        ('c', 'Ö')
            | ('M', 'Ö')
            | ('e', 'ª')
            | ('k', 'Ö')
            | ('m', 'Ö')
            | ('a', 'Ö')
            | ('g', 'Ö')
            | ('n', 'Ö')
            | ('¯', 'Í')
            | ('¯', '’')
            | ('¯', '‹')
            | ('¯', 'ú')
            | ('¯', '^')
            | ('¯', '§')
            | ('¯', 'L')
            | ('¯', '£')
            | ('¯', '‘')
            | ('¯', '¿')
            | ('”', 'P')
            | ('”', 'Q')
            | ('•', '¶')
            | ('®', '‹')
            | ('®', '£')
            | ('®', '§')
            | ('m', 'œ')
            | ('j', 'ø')
            | ('›', '`')
            | ('›', 'U')
            | ('›', 'V')
            | ('š', '’')
            | ('š', '§')
            | ('š', '^')
            | ('š', 'Í')
            | ('š', '‘')
            | ('š', '¿')
            | ('Z', 'œ')
            | ('Z', '¥')
            | ('b', 'œ')
            | ('K', '^')
            | ('K', '¥')
            | ('K', 'ø')
            | ('L', '^')
            | ('M', 'œ')
            | ('¤', 'ú')
            | ('¤', '^')
            | ('¤', '¢')
            | ('¤', '§')
            | ('¤', 'œ')
            | ('™', '¢')
    )
}

/// Normalizes any malformed Unicode Bengali sequence where a dependent vowel sign (kar)
/// precedes a `্` + Consonant continuation (such as `যান্তি্রক` -> `যান্ত্রিক`).
pub fn normalize_bn_cluster_order(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if matches!(ch, 'া' | 'ি' | 'ী' | 'ু' | 'ূ' | 'ৃ' | 'ে' | 'ৈ' | 'ো' | 'ৌ')
            && chars.get(i + 1) == Some(&'্')
            && chars.get(i + 2).copied().is_some_and(is_bn_consonant)
        {
            let mut end = i + 1;
            while chars.get(end) == Some(&'্') && chars.get(end + 1).copied().is_some_and(is_bn_consonant) {
                end += 2;
            }
            if let Some(tail) = chars.get(i + 1..end) {
                for &c in tail {
                    out.push(c);
                }
            }
            out.push(ch);
            i = end;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// Converts ANSI (SutonnyMJ encoding) text to Unicode Bengali with full ligature,
/// diacritic reordering, and cluster-aware vowel shaping.
pub fn ansi_to_unicode(text: &str) -> String {
    // 1. Pre-normalize special symbols, Bijoy shortcuts, and previously corrupted partial conversions
    let mut s = text
        .replace("šত্ন", "ন্ত")
        .replace("¯ত্ন", "স্ত")
        // Normalise double (accidentally repeated) vowel-diacritic glyphs
        .replace("~~", "~")
        .replace("vv", "v")
        .replace("xx", "x")
        // Ellipsis / ri-kar shorthand
        .replace("...", "\u{201E}")
        .replace('\u{2026}', "\u{201E}")
        // Precomposed ri-kar pairs: consonant + „ / … / ...
        .replace("K\u{2026}", "\u{0995}\u{09C3}")
        .replace("K...", "\u{0995}\u{09C3}")
        .replace("K\u{201E}", "\u{0995}\u{09C3}")
        .replace("M\u{2026}", "\u{0997}\u{09C3}")
        .replace("M...", "\u{0997}\u{09C3}")
        .replace("M\u{201E}", "\u{0997}\u{09C3}")
        .replace("d\u{2026}", "\u{09A6}\u{09C3}")
        .replace("d...", "\u{09A6}\u{09C3}")
        .replace("d\u{201E}", "\u{09A6}\u{09C3}")
        .replace("e\u{2026}", "\u{09AC}\u{09C3}")
        .replace("e...", "\u{09AC}\u{09C3}")
        .replace("e\u{201E}", "\u{09AC}\u{09C3}")
        .replace("k\u{2026}", "\u{09B6}\u{09C3}")
        .replace("m\u{2026}", "\u{09B8}\u{09C3}")
        .replace("n\u{2026}", "\u{09B9}\u{09C3}")
        // Dari / pipe
        .replace('|', "\u{0964}")
        // Normalise alternate e-kar glyph \u{2021} -> \u{2020}  (must happen before conjunct lookup)
        .replace('\u{2021}', "\u{2020}")
        // Collapse doubled pre-vowels (typing artefact or split-run re-join)
        .replace("\u{2020}\u{2020}", "\u{2020}")
        // ˆ is the alternate oi-kar glyph; normalise to \u{2030} before the conjunct table
        .replace('\u{02C6}', "\u{2030}")
        .replace("\u{2030}\u{2030}", "\u{2030}")
        // Collapse doubled reph (copy-paste artefact)
        .replace("\u{00A9}\u{00A9}", "\u{00A9}")
        // Precomposed \u{09AD}\u{09C2} variants
        .replace("f\u{201A}", "\u{09AD}\u{09C2}")
        .replace("f~", "\u{09AD}\u{09C2}")
        .replace("f\u{201E}", "\u{09AD}\u{09C2}")
        // \u{099E}\u{09BE} shorthand glyphs
        .replace("T\u{00C2}", "\u{099E}\u{09BE}")
        .replace("Tv", "\u{099E}\u{09BE}")
        // \u{0996}\u{09CD}\u{09B0}\u{09BF} + bisarga / visarga
        .replace("wL\u{00AA}\u{2122}", "\u{0996}\u{09CD}\u{09B0}\u{09BF}\u{0983}")
        .replace("wL\u{00AA}t", "\u{0996}\u{09CD}\u{09B0}\u{09BF}\u{0983}")
        .replace("wL\u{00AA}", "\u{0996}\u{09CD}\u{09B0}\u{09BF}")
        // Precomposed \u{09B0}\u{09C1} variants (must come BEFORE the conjunct table maps \u{00A9} and \u{2030})
        .replace("i\u{00E6}", "\u{09B0}\u{09C1}")
        .replace("i\u{201C}", "\u{09B0}\u{09C1}")
        // Precomposed \u{09B9}\u{09C1} / \u{0997}\u{09C1} / \u{09B6}\u{09C1}
        .replace('\u{00FB}', "\u{09B9}\u{09C1}")
        .replace('\u{00B8}', "\u{0997}\u{09C1}")
        .replace('\u{00EF}', "\u{09B6}\u{09C1}")
        // Collapse doubled \u{09CC} glyph
        .replace("\u{0160}\u{0160}", "\u{0160}");

    // 2. Replace Conjunct Glyphs and Phalas (longest matches first).
    // Note: Pre-kars ('w', '†', '‰') are intentionally NOT expanded here so that Step 4
    // (`parse_bn_cluster`) moves them across the ENTIRE consonant cluster (including trailing
    // phalas like `Ö`/`ª`/`«` for `্র` or `¿` for `্ত্র` in `যান্ত্রিক`).
    let conjuncts: &[(&str, &str)] = &[
        ("cÖ", "প্র"),
        ("MÖ", "গ্র"),
        ("eª", "ব্র"),
        ("eÖ", "ব্র"),
        ("¤ª", "শ্র"),
        ("¯Œ", "স্ক্র"),
        ("¯‹", "স্ক"),
        ("¯•", "স্প"),
        ("¯Í", "স্ত"),
        ("¯—", "স্ত"),
        ("¯‘", "স্তু"),
        ("¯¿", "স্ত্র"),
        ("¯’", "স্থ"),
        ("¯ú", "স্প"),
        ("¯^", "স্ব"),
        ("¯§", "স্ম"),
        ("mœ", "স্ন"),
        ("bœ", "ন্ন"),
        ("¤œ", "ম্ন"),
        ("¤ú", "ম্প"),
        ("¤^", "ম্ব"),
        ("¤¢", "ম্ভ"),
        ("¤§", "ম্ম"),
        ("™¢", "দ্ভ"),
        ("šÍ", "ন্ত"),
        ("š—", "ন্ত"),
        ("š‘", "ন্তু"),
        ("š¿", "ন্ত্র"),
        ("š^", "ন্ব"),
        ("š’", "ন্থ"),
        ("š§", "ন্ম"),
        ("š", "ন্ত"),
        ("›`", "ন্দ"),
        ("›U", "ন্ট"),
        ("›V", "ন্ঠ"),
        ("›W", "ন্ড"),
        ("›a", "ন্ধ"),
        ("›m", "ন্স"),
        ("›", "ন্দ"),
        ("®‹", "ষ্ক"),
        ("®ú", "ষ্প"),
        ("®§", "ষ্ম"),
        ("•ÿ", "ঙ্ক্ষ"),
        ("•¶", "ঙ্ক্ষ"),
        ("”P", "চ্চ"),
        ("”Q", "চ্ছ"),
        ("“P", "চ্চ"),
        ("“Q", "চ্ছ"),
        ("¶", "ক্ষ"),
        ("ÿ", "ক্ষ"),
        ("Á", "জ্ঞ"),
        ("Î", "ত্র"),
        ("°", "ক্ক"),
        ("±", "ক্ট"),
        ("³", "ক্ত"),
        ("·", "ক্স"),
        ("»", "গ্ধ"),
        ("¼", "ঙ্ক"),
        ("½", "ঙ্গ"),
        ("¾", "জ্জ"),
        ("Â", "ঞ্চ"),
        ("Ã", "ঞ্ছ"),
        ("Ä", "ঞ্জ"),
        ("Æ", "ট্ট"),
        ("Ç", "ড্ড"),
        ("È", "ণ্ট"),
        ("Ú", "ণ্ট"),
        ("É", "ণ্ঠ"),
        ("Ê", "ণ্ড"),
        ("Ð", "ণ্ড"),
        ("Û", "ন্ড"),
        ("Ë", "ত্ত"),
        ("Ì", "ত্থ"),
        ("Í", "্ত"),
        ("—", "্ত"),
        ("‘", "্তু"),
        ("¿", "্ত্র"),
        ("Ï", "দ্দ"),
        ("×", "দ্ধ"),
        ("Ø", "দ্ব"),
        ("Ù", "দ্ম"),
        ("Ü", "ন্ধ"),
        ("Ý", "ন্স"),
        ("Þ", "প্ট"),
        ("ß", "প্ত"),
        ("à", "প্প"),
        ("ã", "ব্দ"),
        ("ä", "ব্ধ"),
        ("å", "ভ্র"),
        ("é", "ল্ক"),
        ("î", "ল্গ"),
        ("ë", "ল্ট"),
        ("ì", "ল্ড"),
        ("í", "ল্প"),
        ("ñ", "ল্প"),
        ("ð", "শ্চ"),
        ("ò", "ষ্ণ"),
        ("ó", "ষ্ট"),
        ("ô", "ষ্ঠ"),
        ("õ", "ষ্ণ"),
        ("ö", "ষ্প"),
        ("÷", "স্ট"),
        ("ø", "্ল"),
        ("¬", "্ল"),
        ("ù", "স্ফ"),
        ("ú", "্প"),
        ("¢", "্ভ"),
        ("µ", "ক্র"),
        ("Œ", "্ক্র"),
        ("‹", "্ক"),
        ("þ", "হ্ন"),
        ("ý", "হ্ন"),
        ("ü", "হৃ"),
        ("œ", "্ন"),
        ("¥", "্ম"),
        ("§", "্ম"),
        ("¡", "্ব"),
        ("¦", "্ব"),
        ("Ÿ", "্ব"),
        ("^", "্ব"),
        ("¯", "স্"),
        ("¤", "ম্"),
        ("®", "ষ্"),
        ("”", "চ্"),
        ("™", "ঃ"),
    ];

    for &(a, u) in conjuncts {
        s = s.replace(a, u);
    }

    // 3. Map Independent Vowels and Consonants to Bengali Unicode
    s = s.replace("Av", "আ");
    let mut mapped_chars = String::with_capacity(s.len());
    for ch in s.chars() {
        let m = match ch {
            'A' => "অ",
            'B' => "ই",
            'C' => "ঈ",
            'D' => "উ",
            'E' => "ঊ",
            'F' => "ঋ",
            'G' => "এ",
            'H' => "ঐ",
            'I' => "ও",
            'J' => "ঔ",
            'K' => "ক",
            'L' => "খ",
            'M' => "গ",
            'N' => "ঘ",
            'O' => "ঙ",
            'P' => "চ",
            'Q' => "ছ",
            'R' => "জ",
            'S' => "ঝ",
            'T' => "ঞ",
            'U' => "ট",
            'V' => "ঠ",
            'W' => "ড",
            'X' => "ঢ",
            'Y' => "ণ",
            'Z' => "ত",
            '_' => "থ",
            '`' => "দ",
            'a' => "ধ",
            'b' => "ন",
            'c' => "প",
            'd' => "ফ",
            'e' => "ব",
            'f' => "ভ",
            'g' => "ম",
            'h' => "য",
            'i' => "র",
            'j' => "ল",
            'k' => "শ",
            'l' => "ষ",
            'm' => "স",
            'n' => "হ",
            'o' => "ড়",
            'p' => "ঢ়",
            'q' => "য়",
            'r' => "ৎ",
            '&' => "্",
            '¨' => "্য",
            'Ö' | 'ª' | '«' => "্র",
            'Ñ' => "–",
            'Ò' => "“",
            'Ó' => "”",
            'Ô' => "‘",
            'Õ' => "’",
            _ => "",
        };
        if !m.is_empty() {
            mapped_chars.push_str(m);
        } else {
            mapped_chars.push(ch);
        }
    }

    // 4. Cluster-Aware Pre-kar & Split-Vowel Reordering
    let in_chars: Vec<char> = mapped_chars.chars().collect();
    let mut out = String::with_capacity(in_chars.len());
    let mut i = 0;

    while let Some(&ch) = in_chars.get(i) {
        match ch {
            'w' => {
                if let Some(c_end) = parse_bn_cluster(&in_chars, i + 1)
                    && let Some(cluster) = in_chars.get(i + 1..c_end)
                {
                    for &c in cluster {
                        out.push(c);
                    }
                    out.push('ি');
                    i = c_end;
                    continue;
                }
            }
            '†' => {
                if let Some(c_end) = parse_bn_cluster(&in_chars, i + 1)
                    && let Some(cluster) = in_chars.get(i + 1..c_end)
                {
                    if matches!(in_chars.get(c_end), Some('v' | 'া')) {
                        for &c in cluster {
                            out.push(c);
                        }
                        out.push('ো');
                        i = c_end + 1;
                        continue;
                    } else if matches!(in_chars.get(c_end), Some('Š' | 'ৌ')) {
                        for &c in cluster {
                            out.push(c);
                        }
                        out.push('ৌ');
                        i = c_end + 1;
                        continue;
                    } else {
                        for &c in cluster {
                            out.push(c);
                        }
                        out.push('ে');
                        i = c_end;
                        continue;
                    }
                }
            }
            '‰' => {
                if let Some(c_end) = parse_bn_cluster(&in_chars, i + 1)
                    && let Some(cluster) = in_chars.get(i + 1..c_end)
                {
                    for &c in cluster {
                        out.push(c);
                    }
                    out.push('ৈ');
                    i = c_end;
                    continue;
                }
            }
            _ => {}
        }

        out.push(ch);
        i += 1;
    }

    // 5. Map Post-Diacritics, Punctuation, and Digits BEFORE Reph Reordering
    // so that C + vowel + © (e.g. KZ…©K -> কর্তৃক, mvKz©jvi -> সার্কুলার, m‡ev©”P -> সর্বোচ্চ) is recognized.
    let mut post_out = String::with_capacity(out.len());
    for ch in out.chars() {
        let mapped = match ch {
            'v' => "া",
            'x' => "ী",
            'y' | 'z' | 'æ' | '“' => "ু",
            '~' | '‚' | 'ƒ' => "ূ",
            '„' => "ৃ",
            'Š' => "ৌ",
            '&' => "্",
            's' => "ং",
            't' => "ঃ",
            'u' => "ঁ",
            '0' => "০",
            '1' => "১",
            '2' => "২",
            '3' => "৩",
            '4' => "৪",
            '5' => "৫",
            '6' => "৬",
            '7' => "৭",
            '8' => "৮",
            '9' => "৯",
            _ => "",
        };
        if !mapped.is_empty() {
            post_out.push_str(mapped);
        } else {
            post_out.push(ch);
        }
    }

    // 6. Reph (©) Reordering: C (+ vowel signs) + © -> র্ + C (+ vowel signs)
    let rchars: Vec<char> = post_out.chars().collect();
    let mut reph_out = String::with_capacity(rchars.len());
    let mut j = 0;

    while let Some(&rch) = rchars.get(j) {
        if let Some(c_end) = parse_bn_cluster(&rchars, j) {
            let mut v_end = c_end;
            while rchars.get(v_end).copied().is_some_and(is_bn_vowel_sign) {
                v_end += 1;
            }
            if rchars.get(v_end) == Some(&'©')
                && let Some(slice) = rchars.get(j..v_end)
            {
                reph_out.push_str("র্");
                for &c in slice {
                    reph_out.push(c);
                }
                j = v_end + 1;
                continue;
            }
        }
        reph_out.push(rch);
        j += 1;
    }

    // 7. Normalize any vowel sign preceding `্` + Consonant (e.g. যান্তি্রক -> যান্ত্রিক)
    // and Chandrabindu (ঁ) after vowel signs (e.g. চঁাদ -> চাঁদ).
    let mut final_out = normalize_bn_cluster_order(&reph_out);
    for (bad, good) in [("ঁা", "াঁ"), ("ঁি", "িঁ"), ("ঁী", "ীঁ"), ("ঁু", "ুঁ"), ("ঁূ", "ূঁ"), ("ঁৃ", "ৃঁ"), ("ঁে", "েঁ"), ("ঁৈ", "ৈঁ"), ("ঁো", "োঁ"), ("ঁৌ", "ৌঁ")]
    {
        if final_out.contains(bad) {
            final_out = final_out.replace(bad, good);
        }
    }

    final_out
}

// ============================================================================
// 4. Vocabulary & Phonetic Candidate Suggestions
// ============================================================================

/// High-frequency Bengali words for phonetic candidate suggestion and autocomplete.
pub use super::bangla_vocab::BANGLA_VOCAB;

/// Returns candidate Bengali transliterations/words for a phonetic syllable prefix.
pub fn phonetic_candidates(prefix: &str) -> Vec<String> {
    let p = prefix.trim();
    if p.is_empty() {
        return Vec::new();
    }
    let lower = p.to_ascii_lowercase();
    let mut candidates = Vec::with_capacity(6);

    // 1. Exact match in vocabulary (Top Priority for instant typing & suggestions)
    for &(k, v) in BANGLA_VOCAB {
        if k == lower && !candidates.iter().any(|c| c == v) {
            candidates.push(v.to_string());
        }
    }

    // 2. Direct transliteration from the engine
    let primary = phonetic_transliterate(p);
    if !primary.is_empty() && !candidates.iter().any(|c| c == &primary) {
        candidates.push(primary);
    }

    // 3. Prefix match in vocabulary (prioritizing shorter/closer matches)
    let mut prefix_matches: Vec<(&str, &str)> =
        BANGLA_VOCAB.iter().copied().filter(|&(k, v)| k.starts_with(&lower) && k != lower && !candidates.iter().any(|c| c == v)).collect();
    prefix_matches.sort_by_key(|&(k, _)| k.len());
    for (_, v) in prefix_matches {
        candidates.push(v.to_string());
        if candidates.len() >= 5 {
            break;
        }
    }

    candidates.truncate(5);
    candidates
}

// ============================================================================
// 5. Command Handlers
// ============================================================================

/// `tools.banglaMode` — sets or toggles the active Bangla typing mode with automatic font switching.
fn cmd_bangla_mode(s: &mut Session, v: &Value) -> CmdResult {
    let prev = s.bangla_mode;
    if p::bool(v, "toggle").unwrap_or(false) {
        s.bangla_mode = s.bangla_mode.next();
    } else if let Some(mode_str) = p::str(v, "mode") {
        s.bangla_mode = match mode_str.to_ascii_lowercase().as_str() {
            "phonetic" => BanglaMode::Phonetic,
            "jatiya" => BanglaMode::Jatiya,
            "ansi" => BanglaMode::Ansi,
            _ => BanglaMode::Off,
        };
    }

    // Automatic font switcher for frictionless typing
    if s.bangla_mode != prev {
        match s.bangla_mode {
            BanglaMode::Phonetic | BanglaMode::Jatiya => {
                let _ = s.run("format.font", &json!({"name": "Kalpurush"}));
            }
            BanglaMode::Ansi => {
                let _ = s.run("format.font", &json!({"name": "SutonnyMJ"}));
            }
            BanglaMode::Off => {
                let _ = s.run("format.font", &json!({"name": "Calibri"}));
            }
        }
    }

    Ok(json!({
        "mode": s.bangla_mode.name(),
        "label": s.bangla_mode.label(),
        "badge": s.bangla_mode.badge(),
    }))
}

/// `tools.banglaCandidates` — returns top candidate suggestions for a phonetic prefix.
fn cmd_bangla_candidates(_s: &mut Session, v: &Value) -> CmdResult {
    let prefix = p::req_str(v, "prefix")?;
    let candidates = phonetic_candidates(prefix);
    Ok(json!({"prefix": prefix, "candidates": candidates}))
}

/// Transliterates a Jatiya (Bijoy-layout) keystroke sequence into Unicode Bengali.
pub fn jatiya_transliterate(input: &str) -> String {
    let mut out = String::new();
    let mut linker = false;
    let mut pre_kar: Option<&'static str> = None;
    let mut reph = false;

    for c in input.chars() {
        if c == 'g' {
            if linker {
                out.push('্');
                linker = false;
            } else {
                linker = true;
                if let Some(kar @ ('ি' | 'ে' | 'ৈ')) = out.chars().next_back() {
                    out.pop();
                    out.push('্');
                    pre_kar = Some(match kar {
                        'ি' => "ি",
                        'ে' => "ে",
                        'ৈ' => "ৈ",
                        _ => "",
                    });
                } else {
                    out.push('্');
                }
            }
            continue;
        }

        if linker {
            linker = false;
            if let Some(vowel) = jatiya_linker_vowel(c) {
                if out.ends_with('্') {
                    out.pop();
                }
                if let Some(stashed) = pre_kar.take() {
                    out.push_str(stashed);
                }
                out.push_str(vowel);
                continue;
            }
            if c == '|' {
                if out.ends_with('্') {
                    out.pop();
                }
                if let Some(stashed) = pre_kar.take() {
                    out.push_str(stashed);
                }
                out.push('ঃ');
                continue;
            }
            if c == '.' || c == 'G' {
                if out.ends_with('্') {
                    out.pop();
                }
                if let Some(stashed) = pre_kar.take() {
                    out.push_str(stashed);
                }
                out.push('।');
                continue;
            }
        }

        if c == 'A' {
            reph = true;
            continue;
        }

        if c == 'd' || c == 'c' || c == 'C' {
            pre_kar = Some(match c {
                'd' => "ি",
                'c' => "ে",
                'C' => "ৈ",
                _ => "",
            });
            continue;
        }

        if (c == 'f' || c == 'X') && out.ends_with('ে') {
            out.pop();
            out.push(if c == 'f' { 'ো' } else { 'ৌ' });
            continue;
        }

        if c == 'z' || c == 'Z' {
            let fala = if c == 'z' { "্র" } else { "্য" };
            let moved_kar = match out.chars().next_back() {
                Some(kar @ ('ি' | 'ে' | 'ৈ' | 'া' | 'ী' | 'ু' | 'ূ' | 'ৃ' | 'ো' | 'ৌ')) => {
                    out.pop();
                    Some(kar)
                }
                _ => None,
            };
            if out.ends_with('্') {
                out.pop();
            }
            if reph {
                out.push_str("র্");
                reph = false;
            }
            out.push_str(fala);
            if let Some(kar) = moved_kar {
                out.push(kar);
            }
            if let Some(stashed) = pre_kar.take() {
                out.push_str(stashed);
            }
            continue;
        }

        if let Some(s) = jatiya_key(c) {
            if reph {
                out.push_str("র্");
                reph = false;
            }
            out.push_str(s);
            if let Some(kar) = pre_kar.take() {
                out.push_str(kar);
            }
        } else {
            if let Some(kar) = pre_kar.take() {
                out.push_str(kar);
            }
            if reph {
                out.push_str("র্");
                reph = false;
            }
            out.push(c);
        }
    }
    out
}

/// `tools.banglaTransliterate` — converts text using the specified engine.
fn cmd_bangla_transliterate(_s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    let mode = p::str(v, "mode").unwrap_or("phonetic");
    let result = match mode {
        "phonetic" => phonetic_transliterate(text),
        "jatiya" => jatiya_transliterate(text),
        "unicodeToAnsi" => unicode_to_ansi(text),
        "ansiToUnicode" => ansi_to_unicode(text),
        _ => text.to_string(),
    };
    Ok(json!({"result": result, "mode": mode}))
}

fn convert_blocks_recursive(blocks: &mut Vec<Arc<wordcraft_doc::Block>>, is_to_ansi: bool, target_font: &str) -> usize {
    let mut count = 0;
    for b in blocks {
        let block = Arc::make_mut(b);
        match block {
            wordcraft_doc::Block::Para(p) => {
                p.mark.font = Some(target_font.into());
                p.mark.font_cs = Some(target_font.into());
                if !p.text.is_empty() {
                    let converted = if is_to_ansi { unicode_to_ansi(&p.text) } else { ansi_to_unicode(&p.text) };
                    let mut props = p.runs.first().map(|r| r.props.clone()).unwrap_or_default();
                    props.font = Some(target_font.into());
                    props.font_cs = Some(target_font.into());
                    if props.size.is_some() {
                        props.size_cs = props.size;
                    }
                    if props.bold.is_some() {
                        props.bold_cs = props.bold;
                    }
                    if props.italic.is_some() {
                        props.italic_cs = props.italic;
                    }
                    p.text = converted.clone();
                    p.runs = vec![wordcraft_doc::Run { len: converted.len(), props }];
                    count += 1;
                }
            }
            wordcraft_doc::Block::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        count += convert_blocks_recursive(&mut cell.blocks, is_to_ansi, target_font);
                    }
                }
            }
        }
    }
    count
}

/// `tools.banglaConvert` — converts selected text or entire document between Unicode and ANSI.
fn cmd_bangla_convert(s: &mut Session, v: &Value) -> CmdResult {
    let dir = p::str(v, "direction").unwrap_or("unicodeToAnsi");
    let target = p::str(v, "target").unwrap_or("selection");
    let font = p::str(v, "font");

    let is_to_ansi = dir.eq_ignore_ascii_case("unicodeToAnsi");
    let target_font = font.unwrap_or(if is_to_ansi { "SutonnyMJ" } else { "Kalpurush" });

    if target == "selection" && !s.sel.is_collapsed() {
        let current_text = s.selected_text();
        if !current_text.is_empty() {
            let converted = if is_to_ansi { unicode_to_ansi(&current_text) } else { ansi_to_unicode(&current_text) };
            s.run("format.font", &json!({"name": target_font}))?;
            s.run("text.insert", &json!({"text": converted}))?;
            return Ok(json!({"converted": true, "count": 1, "target": "selection"}));
        }
    }

    // Convert entire document (including tables, headers, footers, text boxes)
    let mut converted_count = convert_blocks_recursive(&mut s.doc.body, is_to_ansi, target_font);
    for part in s.doc.parts.values_mut() {
        converted_count += convert_blocks_recursive(&mut part.blocks, is_to_ansi, target_font);
    }

    Ok(json!({"converted": true, "count": converted_count, "target": "document"}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("tools.banglaMode", "Bangla Input Mode", "Home › Bangla", cmd_bangla_mode)
            .key("Mod+Alt+B")
            .params(r#"{"mode"?: "off|phonetic|jatiya|ansi", "toggle"?: bool}"#)
            .pure(),
        CommandSpec::new("tools.banglaTransliterate", "Transliterate Bangla", "Tools › Bangla", cmd_bangla_transliterate)
            .params(r#"{"text": string, "mode"?: "phonetic|jatiya|unicodeToAnsi|ansiToUnicode"}"#)
            .pure(),
        CommandSpec::new("tools.banglaConvert", "Convert Bangla (Unicode ⇄ ANSI)", "Tools › Bangla", cmd_bangla_convert)
            .params(r#"{"direction"?: "unicodeToAnsi|ansiToUnicode", "target"?: "selection|document", "font"?: string}"#),
        CommandSpec::new("tools.banglaCandidates", "Bangla Phonetic Candidates", "Tools › Bangla", cmd_bangla_candidates)
            .params(r#"{"prefix": string} -> {"candidates": [string]}"#)
            .pure(),
        CommandSpec::new("tools.banglaConverter", "Bangla Converter", "Tools › Bangla", |s, _| {
            s.ui_requests.push(json!({"open": "banglaConverter"}));
            Ok(json!({"opened": true}))
        })
        .pure(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phonetic_candidates_and_vocabulary() {
        let c1 = phonetic_candidates("am");
        assert!(!c1.is_empty());
        assert!(c1.contains(&"আমি".to_string()) || c1.contains(&"আমরা".to_string()));

        let c2 = phonetic_candidates("bhalo");
        assert_eq!(c2[0], "ভালো");

        let c3 = phonetic_candidates("bangla");
        assert_eq!(c3[0], "বাংলা");
    }

    #[test]
    fn phonetic_basic_words() {
        assert_eq!(phonetic_transliterate("ami"), "আমি");
        assert_eq!(phonetic_transliterate("bangla"), "বাংলা");
        assert_eq!(phonetic_transliterate("desh"), "দেশ");
        assert_eq!(phonetic_transliterate("dhaka"), "ঢাকা");
        assert_eq!(phonetic_transliterate("ki"), "কি");
        assert_eq!(phonetic_transliterate("koi"), "কই");
    }

    #[test]
    fn phonetic_conjuncts() {
        assert_eq!(phonetic_transliterate("kkh"), "ক্ষ");
        assert_eq!(phonetic_transliterate("jng"), "জ্ঞ");
    }

    #[test]
    fn jatiya_keymap() {
        assert_eq!(jatiya_key('h'), Some("ব"));
        assert_eq!(jatiya_key('j'), Some("ক"));
        assert_eq!(jatiya_key('d'), Some("ি"));
        assert_eq!(jatiya_key('D'), Some("ী"));
        assert_eq!(jatiya_key('g'), Some("্"));
        assert_eq!(jatiya_key('f'), Some("া"));
        assert_eq!(jatiya_key('1'), Some("১"));
        assert_eq!(jatiya_key('\\'), Some("ৎ"));
        assert_eq!(jatiya_key('~'), Some("ঁ"));
    }

    #[test]
    fn bijoy_conjunct_and_vowel_mappings() {
        // Independent vowels via 'g'
        assert_eq!(jatiya_linker_vowel('f'), Some("আ"));
        assert_eq!(jatiya_linker_vowel('d'), Some("ই"));
        assert_eq!(jatiya_linker_vowel('D'), Some("ঈ"));
        assert_eq!(jatiya_linker_vowel('s'), Some("উ"));
        assert_eq!(jatiya_linker_vowel('S'), Some("ঊ"));
        assert_eq!(jatiya_linker_vowel('a'), Some("ঋ"));
        assert_eq!(jatiya_linker_vowel('c'), Some("এ"));
        assert_eq!(jatiya_linker_vowel('C'), Some("ঐ"));
        assert_eq!(jatiya_linker_vowel('X'), Some("ঔ"));

        // Major conjuncts
        assert_eq!(find_bijoy_conjunct('ক', 'ষ'), Some("¶"));
        assert_eq!(find_bijoy_conjunct('ত', 'ত'), Some("Ë"));
        assert_eq!(find_bijoy_conjunct('ন', 'ত'), Some("šÍ"));
        assert_eq!(find_bijoy_conjunct('জ', 'ঞ'), Some("Á"));
        assert_eq!(find_bijoy_conjunct('ম', 'প'), Some("¤ú"));
        assert_eq!(find_bijoy_conjunct('ষ', 'ট'), Some("ó"));
        assert_eq!(find_bijoy_conjunct('স', 'ত'), Some("¯Í"));
        assert_eq!(find_bijoy_conjunct('দ', 'ধ'), Some("×"));
        assert_eq!(find_bijoy_conjunct('হ', 'ম'), Some("n&g"));
        assert_eq!(find_bijoy_conjunct('ল', 'গ'), Some("î"));
        assert_eq!(find_bijoy_conjunct('ত', 'র'), Some("Î"));
        assert_eq!(find_bijoy_conjunct('ক', 'র'), Some("µ"));
        assert_eq!(find_bijoy_conjunct('ভ', 'র'), Some("å"));
        assert_eq!(find_bijoy_conjunct('হ', 'ঋ'), Some("ü"));
    }

    #[test]
    fn unicode_to_ansi_and_back() {
        let u = "আমার সোনার বাংলা";
        let a = unicode_to_ansi(u);
        assert_eq!(a, "Avgvi †mvbvi evsjv");
        let back = ansi_to_unicode(&a);
        assert_eq!(back, u);
    }

    #[test]
    fn phonetic_numbers_currency_and_dari() {
        assert_eq!(phonetic_transliterate("12345"), "১২৩৪৫");
        assert_eq!(phonetic_transliterate("$500"), "৳৫০০");
        assert_eq!(phonetic_transliterate("ami bhalo|"), "আমি ভালো।");
    }

    #[test]
    fn bangla_mode_toggle_and_set() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        assert_eq!(s.bangla_mode, BanglaMode::Off);

        let r1 = s.run("tools.banglaMode", &json!({"mode": "phonetic"})).unwrap();
        assert_eq!(r1["mode"], "phonetic");
        assert_eq!(s.bangla_mode, BanglaMode::Phonetic);
        assert_eq!(s.typing_props().font.as_deref(), Some("Kalpurush"));

        let r2 = s.run("tools.banglaMode", &json!({"toggle": true})).unwrap();
        assert_eq!(r2["mode"], "jatiya");
        assert_eq!(s.bangla_mode, BanglaMode::Jatiya);
        assert_eq!(s.typing_props().font.as_deref(), Some("Kalpurush"));

        let r3 = s.run("tools.banglaMode", &json!({"toggle": true})).unwrap();
        assert_eq!(r3["mode"], "ansi");
        assert_eq!(s.bangla_mode, BanglaMode::Ansi);
        assert_eq!(s.typing_props().font.as_deref(), Some("SutonnyMJ"));

        let r4 = s.run("tools.banglaMode", &json!({"toggle": true})).unwrap();
        assert_eq!(r4["mode"], "off");
        assert_eq!(s.bangla_mode, BanglaMode::Off);
        assert_eq!(s.typing_props().font.as_deref(), Some("Calibri"));
    }

    #[test]
    fn bangla_candidates_command() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        let res = s.run("tools.banglaCandidates", &json!({"prefix": "bhalo"})).unwrap();
        assert!(!res["candidates"].as_array().unwrap().is_empty());
        assert_eq!(res["candidates"][0], "ভালো");
    }

    #[test]
    fn bangla_transliterate_command() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        let res = s.run("tools.banglaTransliterate", &json!({"text": "ami bangla", "mode": "phonetic"})).unwrap();
        assert_eq!(res["result"], "আমি বাংলা");

        let res_ansi = s.run("tools.banglaTransliterate", &json!({"text": "আমার সোনার বাংলা", "mode": "unicodeToAnsi"})).unwrap();
        assert_eq!(res_ansi["result"], "Avgvi †mvbvi evsjv");

        let res_uni = s.run("tools.banglaTransliterate", &json!({"text": "Avgvi †mvbvi evsjv", "mode": "ansiToUnicode"})).unwrap();
        assert_eq!(res_uni["result"], "আমার সোনার বাংলা");
    }

    #[test]
    fn bangla_convert_document_and_selection() {
        let mut s = Session::new(wordcraft_doc::Document::from_text("আমার সোনার বাংলা"));

        // Convert document to ANSI
        let r1 = s.run("tools.banglaConvert", &json!({"direction": "unicodeToAnsi", "target": "document"})).unwrap();
        assert_eq!(r1["converted"], true);
        assert_eq!(s.doc.body[0].as_para().unwrap().text, "Avgvi †mvbvi evsjv");
        assert_eq!(s.doc.body[0].as_para().unwrap().runs[0].props.font.as_deref(), Some("SutonnyMJ"));

        // Convert document back to Unicode
        let r2 = s.run("tools.banglaConvert", &json!({"direction": "ansiToUnicode", "target": "document"})).unwrap();
        assert_eq!(r2["converted"], true);
        assert_eq!(s.doc.body[0].as_para().unwrap().text, "আমার সোনার বাংলা");
        assert_eq!(s.doc.body[0].as_para().unwrap().runs[0].props.font.as_deref(), Some("Kalpurush"));
    }

    #[test]
    fn bangla_converter_dialog_opens() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        let r = s.run("tools.banglaConverter", &json!({})).unwrap();
        assert_eq!(r["opened"], true);
        assert_eq!(s.ui_requests.len(), 1);
        assert_eq!(s.ui_requests[0]["open"], "banglaConverter");
    }

    #[test]
    fn bijoy_document_ligatures_and_diacritics() {
        // 1. Krishi Bank title with ellipsis/ri-kar
        assert_eq!(ansi_to_unicode("evsjv‡`k K…wl e¨vsK"), "বাংলাদেশ কৃষি ব্যাংক");
        assert_eq!(ansi_to_unicode("evsjv‡`k K...wl e¨vsK"), "বাংলাদেশ কৃষি ব্যাংক");

        // 2. Dari mapping
        assert_eq!(ansi_to_unicode("†cÖiY cÖm‡½|"), "প্রেরণ প্রসঙ্গে।");

        // 3. Computer / conjunct with i-kar
        assert_eq!(ansi_to_unicode("Kw¤úDUvi"), "কম্পিউটার");

        // 4. O-kar split vowel combination
        assert_eq!(ansi_to_unicode("g‡nv`q"), "মহোদয়");

        // 5. Date with bisarga / TM
        assert_eq!(ansi_to_unicode("wLª™"), "খ্রিঃ");

        // 6. Address with bhu / nya
        assert_eq!(ansi_to_unicode("`vMbf‚Tv"), "দাগনভূঞা");

        // 7. Shironame
        let shiro_ansi = unicode_to_ansi("শিরোনামে");
        assert_eq!(ansi_to_unicode(&shiro_ansi), "শিরোনামে");

        // 8. Real-world BKB Mobile Monitoring Format strings (all previously failing cases)
        assert_eq!(
            ansi_to_unicode("(RyjvB-‡m‡Þ¤^i)/2026 ˆÎgvwm‡K weZiYK…Z K…wl FY MÖnxZvi ZvwjKv"),
            "(জুলাই-সেপ্টেম্বর)/২০২৬ ত্রৈমাসিকে বিতরণকৃত কৃষি ঋণ গ্রহীতার তালিকা"
        );
        assert_eq!(ansi_to_unicode("Gbv‡qZbMi, BqvKzecyi, `vMbf‚Tv"), "এনায়েতনগর, ইয়াকুবপুর, দাগনভূঞা");
        assert_eq!(ansi_to_unicode("K…wl/cjøx, 12.5%"), "কৃষি/পল্লী, ১২.৫%");
        assert_eq!(ansi_to_unicode("DËi PÐxcyi, BqvKzecyi, `vMbf‚Tv"), "উত্তর চণ্ডীপুর, ইয়াকুবপুর, দাগনভূঞা");
        assert_eq!(ansi_to_unicode("cwðg PÐxcyi, BqvKzecyi, `vMbf‚Tv"), "পশ্চিম চণ্ডীপুর, ইয়াকুবপুর, দাগনভূঞা");
        assert_eq!(ansi_to_unicode("`w¶Y Puv`cyi, BqvKzecyi, `vMbf‚Tv"), "দক্ষিণ চাঁদপুর, ইয়াকুবপুর, দাগনভূঞা");
        assert_eq!(ansi_to_unicode("‡bRvg DwÏb 339/65"), "নেজাম উদ্দিন ৩৩৯/৬৫");
        assert_eq!(ansi_to_unicode("Avãyj gwZb 216/65"), "আব্দুল মতিন ২১৬/৬৫");
        assert_eq!(ansi_to_unicode("Av‡bvqviæj Kwig ‡gv‡k©` 06/66"), "আনোয়ারুল করিম মোর্শেদ ০৬/৬৬");
        assert_eq!(ansi_to_unicode("‡gvt wmivR Djøvn 206/65"), "মোঃ সিরাজ উল্লাহ ২০৬/৬৫");
        assert_eq!(ansi_to_unicode("‡gvt kvgmy‡Ïvnv"), "মোঃ শামসুদ্দোহা");
        assert_eq!(ansi_to_unicode("RvbœvZzj ‡di‡`Šm"), "জান্নাতুল ফেরদৌস");
        assert_eq!(ansi_to_unicode("nvwjgv LvZzb"), "হালিমা খাতুন");
        assert_eq!(ansi_to_unicode("iæûj Avwgb"), "রুহুল আমিন");
        assert_eq!(ansi_to_unicode("‡gvt ûgvqyb Kwei"), "মোঃ হুমায়ুন কবির");
        assert_eq!(ansi_to_unicode("‡gvt ‡Mvjvg ‡gvZ©zRv"), "মোঃ গোলাম মোর্তুজা");
        assert_eq!(ansi_to_unicode("gšÍe¨"), "মন্তব্য");
        assert_eq!(ansi_to_unicode("KZ…©K"), "কর্তৃক");
        assert_eq!(ansi_to_unicode("mvKz©jvi"), "সার্কুলার");
        assert_eq!(ansi_to_unicode("m‡ev©”P"), "সর্বোচ্চ");
        assert_eq!(ansi_to_unicode("wb¤œ¯^vÿiKvix"), "নিম্নস্বাক্ষরকারী");

        // 9. Double uu-kar typing artefact (g~~‡jv → মূলো, not মূূলো)
        assert_eq!(ansi_to_unicode("g~~‡jv"), "মূলো");

        // 10. Alternate u-kar after non-ra consonants (z = ু)
        assert_eq!(ansi_to_unicode("LvZzb"), "খাতুন");
        assert_eq!(ansi_to_unicode("Kzgvi"), "কুমার");
        assert_eq!(ansi_to_unicode("BqvKzecyi"), "ইয়াকুবপুর");
        assert_eq!(ansi_to_unicode("RvbœvZzj"), "জান্নাতুল");

        // 11. Alternate u-kar after ra (æ = ু)
        assert_eq!(ansi_to_unicode("byiæj"), "নুরুল");
        assert_eq!(ansi_to_unicode("Avbvqviæj"), "আনায়ারুল");
        assert_eq!(ansi_to_unicode("RvniæjvKwei"), "জাহরুলাকবির");

        // 12. Precomposed হু / শুরু
        assert_eq!(ansi_to_unicode("ûgvqyb"), "হুমায়ুন");

        // 13. ঞা shorthand Tv
        assert_eq!(ansi_to_unicode("`vMbf\u{201A}Tv"), "দাগনভূঞা");

        // 14. সর্বোচ্চ — raw Bijoy: m‡ev©"P  (‡=U+2021, ©=U+00A9, "=U+201C)
        // Note: the curly-quote U+201C is the SutonnyMJ "চ্" shorthand prefix glyph
        let sarboccho_bijoy = "m\u{2021}ev\u{00A9}\u{201C}P";
        assert_eq!(ansi_to_unicode(sarboccho_bijoy), "সর্বোচ্চ");

        // 15. যান্ত্রিক (hvwš¿K, hvwšÍÖK, and malformed যান্তি্রক normalization)
        assert_eq!(ansi_to_unicode("hvwš¿K"), "যান্ত্রিক");
        assert_eq!(ansi_to_unicode("hvwšÍÖK"), "যান্ত্রিক");
        assert_eq!(unicode_to_ansi("যান্ত্রিক"), "hvwš¿K");
        assert_eq!(normalize_bn_cluster_order("যান্তি্রক"), "যান্ত্রিক");
        assert_eq!(ansi_to_unicode("যান্তি্রক"), "যান্ত্রিক");
        assert_eq!(jatiya_transliterate("wfdbgkzj"), "যান্ত্রিক");
        assert_eq!(jatiya_transliterate("wfdbgkgzj"), "যান্ত্রিক");
        assert_eq!(jatiya_transliterate("wfdbgkgvj"), "যান্ত্রিক");
        assert_eq!(jatiya_transliterate("drzW"), "প্রিয়");
        assert_eq!(jatiya_transliterate("crzm"), "প্রেম");
    }

    #[test]
    fn test_dari_bisarga_and_middle_vowels() {
        // Dari '.' and '|'
        assert_eq!(phonetic_transliterate("bangladesh."), "বাংলাদেশ।");
        assert_eq!(phonetic_transliterate("bangladesh|"), "বাংলাদেশ।");
        assert_eq!(phonetic_transliterate("desh ."), "দেশ ।");

        // Bisarga in word vs colon ':' in punctuation/time
        assert_eq!(phonetic_transliterate("du:kho"), "দুঃখ");
        assert_eq!(phonetic_transliterate("Time: 10:30"), "টিমে: ১০:৩০");
        assert_eq!(phonetic_transliterate("10:30"), "১০:৩০");

        // Chandrabindu '^' and Khanda Ta 't`'
        assert_eq!(phonetic_transliterate("c^ad"), "চাঁদ");
        assert_eq!(phonetic_transliterate("ut`sob"), "উৎসব");
        assert_eq!(phonetic_transliterate("hot`hat"), "হঠাৎ");

        // Middle-of-word independent 'আ' and other vowels
        assert_eq!(phonetic_transliterate("k'a"), "কআ");
        assert_eq!(phonetic_transliterate("ka'a"), "কাআ");
        assert_eq!(phonetic_transliterate("kA"), "কআ");
        assert_eq!(phonetic_transliterate("naA"), "নাআ");
        assert_eq!(phonetic_transliterate("m'haAkash"), "মহাআকাশ");
        assert_eq!(phonetic_transliterate("k'k"), "কক");

        // Jatiya key mappings
        assert_eq!(jatiya_key(':'), Some(":"));
        assert_eq!(jatiya_key(';'), Some(";"));
        assert_eq!(jatiya_key('|'), Some("ঃ"));
        assert_eq!(jatiya_key('.'), Some("।"));
        assert_eq!(jatiya_key('G'), Some("।"));
        assert_eq!(jatiya_key('A'), Some("র্"));

        // Jatiya linker vowels
        assert_eq!(jatiya_linker_vowel('f'), Some("আ"));
        assert_eq!(jatiya_linker_vowel('x'), Some("ও"));
        assert_eq!(jatiya_linker_vowel('F'), Some("অ"));

        // Bijoy ANSI mappings
        assert_eq!(bijoy_ansi_key(':'), Some(":"));
        assert_eq!(bijoy_ansi_key('|'), Some("t"));
        assert_eq!(bijoy_ansi_key('.'), Some("."));
        assert_eq!(bijoy_ansi_key('G'), Some("।"));
        assert_eq!(bijoy_ansi_key('A'), Some("©"));
        assert_eq!(bijoy_linker_vowel('f'), Some("Av"));
        assert_eq!(bijoy_linker_vowel('x'), Some("I"));
        assert_eq!(bijoy_linker_vowel('F'), Some("A"));
    }

    #[test]
    fn test_medial_vs_initial_ekar_in_unicode_to_ansi() {
        // Initial E-kar / O-kar uses '†' (\u{2020}), Medial E-kar / O-kar uses '‡' (\u{2021})
        assert_eq!(unicode_to_ansi("তোমায়"), "†Zvgvq");
        assert_eq!(unicode_to_ansi("ভালোবাসি"), "fv‡jvevwm");
        assert_eq!(unicode_to_ansi("তিনি তোমায় ভালোবাসেন"), "wZwb †Zvgvq fv‡jvev‡mb");

        // 2-char conjuncts (cÖ for প্র) with initial and medial E-kar
        assert_eq!(unicode_to_ansi("প্রেম"), "†cÖg");
        assert_eq!(unicode_to_ansi("প্রেমে"), "†cÖ‡g");

        // Roundtrip verification back to Unicode
        assert_eq!(ansi_to_unicode("fv‡jvevwm"), "ভালোবাসি");
        assert_eq!(ansi_to_unicode("†Zvgvq"), "তোমায়");
        assert_eq!(ansi_to_unicode("†cÖg"), "প্রেম");
        assert_eq!(ansi_to_unicode("†cÖ‡g"), "প্রেমে");
        assert_eq!(ansi_to_unicode("wZwb †Zvgvq fv‡jvev‡mb"), "তিনি তোমায় ভালোবাসেন");
    }

    #[test]
    fn test_phonetic_transliteration_and_suggestions() {
        // Sentence transliteration parity requested by user
        assert_eq!(phonetic_transliterate("ami bangladesh krishi banke chakri kori"), "আমি বাংলাদেশ কৃষি ব্যাংকে চাকরি করি");
        assert_eq!(phonetic_transliterate("ami bangladesh krishi banke chakri kori|"), "আমি বাংলাদেশ কৃষি ব্যাংকে চাকরি করি।");
        assert_eq!(phonetic_transliterate("ami bangladesh krishi banke chakri kori."), "আমি বাংলাদেশ কৃষি ব্যাংকে চাকরি করি।");

        // Verification of top-priority dictionary candidates
        let krishi_cand = phonetic_candidates("krishi");
        assert_eq!(krishi_cand.first().map(String::as_str), Some("কৃষি"));

        let banke_cand = phonetic_candidates("banke");
        assert_eq!(banke_cand.first().map(String::as_str), Some("ব্যাংকে"));

        let chakri_cand = phonetic_candidates("chakri");
        assert_eq!(chakri_cand.first().map(String::as_str), Some("চাকরি"));

        let kori_cand = phonetic_candidates("kori");
        assert_eq!(kori_cand.first().map(String::as_str), Some("করি"));

        // Ri-kar via rri rule
        assert_eq!(phonetic_transliterate("rri"), "ঋ");
        assert_eq!(phonetic_transliterate("krri"), "কৃ");

        // Previous sentence words
        assert_eq!(phonetic_transliterate("amare"), "আমারে");
        assert_eq!(phonetic_transliterate("keu"), "কেউ");
        assert_eq!(phonetic_transliterate("ekla"), "একলা");
        assert_eq!(phonetic_transliterate("ekhane"), "এখানে");
        assert_eq!(phonetic_transliterate("boshe"), "বসে");
        assert_eq!(phonetic_transliterate("achi"), "আছি");
        assert_eq!(phonetic_transliterate("ononto"), "অনন্ত");
        assert_eq!(phonetic_transliterate("kal"), "কাল");
    }
}
