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
    let mut out = String::with_capacity(input.len().saturating_mul(2));
    let mut words = Vec::new();
    let mut current_word = String::new();
    let mut is_alpha = false;

    // Split text into tokens (alphabetic word tokens vs whitespace/punctuation)
    for ch in input.chars() {
        let alpha = ch.is_ascii_alphabetic();
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
                    _ => out.push(ch),
                }
            }
        }
    }

    out
}

/// Transliterates a single alphabetic word phonetically.
fn phonetic_word(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    // Common word overrides for frictionless typing
    match lower.as_str() {
        "ami" => return "আমি".into(),
        "amra" => return "আমরা".into(),
        "amader" => return "আমাদের".into(),
        "amar" => return "আমার".into(),
        "tumi" => return "তুমি".into(),
        "tomra" => return "তোমরা".into(),
        "tomader" => return "তোমাদের".into(),
        "tomar" => return "তোমার".into(),
        "apni" => return "আপনি".into(),
        "apnader" => return "আপনাদের".into(),
        "apnar" => return "আপনার".into(),
        "bangla" => return "বাংলা".into(),
        "bangladesh" => return "বাংলাদেশ".into(),
        "dhaka" => return "ঢাকা".into(),
        _ => {}
    }

    let mut res = String::new();
    let chars: Vec<char> = word.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut last_was_consonant = false;

    while i < len {
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
            }
        }

        // 2. Check for 2-letter vowel matches
        if i + 1 < len {
            let pair: String = chars[i..i + 2].iter().collect();
            let lower_pair = pair.to_ascii_lowercase();
            let vowel_match = match lower_pair.as_str() {
                "aa" => Some(('আ', 'া')),
                "ee" => Some(('ঈ', 'ী')),
                "oo" => Some(('ঊ', 'ূ')),
                "oi" => Some(('ঐ', 'ৈ')),
                "ou" => Some(('ঔ', 'ৌ')),
                _ => None,
            };
            if let Some((ind, dep)) = vowel_match {
                if last_was_consonant {
                    res.push(dep);
                } else {
                    res.push(ind);
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
                _ => None,
            };
            if let Some(bn) = con_match {
                if last_was_consonant {
                    res.push('্');
                }
                res.push_str(bn);
                last_was_consonant = true;
                i += 2;
                continue;
            }
        }

        // 3. Single-character checks
        let c = chars[i];
        let c_lower = c.to_ascii_lowercase();

        if is_vowel(c) {
            let is_upper = c.is_ascii_uppercase();
            let (ind, dep, is_inherent) = match c_lower {
                'a' => {
                    if is_upper {
                        ('আ', 'া', false)
                    } else if last_was_consonant {
                        // Inherent vowel after consonant, doesn't add kar
                        ('অ', ' ', true)
                    } else {
                        ('অ', 'া', false)
                    }
                }
                'i' => {
                    if is_upper {
                        ('ঈ', 'ী', false)
                    } else {
                        ('ই', 'ি', false)
                    }
                }
                'u' => {
                    if is_upper {
                        ('ঊ', 'ূ', false)
                    } else {
                        ('উ', 'ু', false)
                    }
                }
                'e' => ('এ', 'ে', false),
                'o' => {
                    if is_upper {
                        ('ও', 'ো', false)
                    } else if last_was_consonant {
                        ('ও', ' ', true)
                    } else {
                        ('ও', 'ো', false)
                    }
                }
                _ => ('অ', ' ', true),
            };

            if !is_inherent {
                if last_was_consonant {
                    res.push(dep);
                } else {
                    res.push(ind);
                }
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
            if last_was_consonant && bn_c != "্য" && bn_c != "্ব" {
                // Join consecutive consonants with virama (্)
                res.push('্');
            }
            res.push_str(bn_c);
            last_was_consonant = true;
        } else {
            res.push(c);
            last_was_consonant = false;
        }

        i += 1;
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
        'd' => Some("্"), // Hasanta / Virama
        'f' => Some("া"),
        'g' => Some("্"), // Linker / Hasanta
        'h' => Some("ব"),
        'j' => Some("ক"),
        'k' => Some("ত"),
        'l' => Some("দ"),
        'z' => Some("্র"), // Rafala
        'x' => Some("ও"),
        'c' => Some("ে"),
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
        'S' => Some("ূ"),
        'D' => Some("ৎ"),
        'F' => Some("অ"),
        'G' => Some("জ্ঞ"),
        'H' => Some("ভ"),
        'J' => Some("খ"),
        'K' => Some("থ"),
        'L' => Some("ধ"),
        'Z' => Some("্য"), // Yafala
        'X' => Some("ৌ"),
        'C' => Some("ৈ"),
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
        ':' => Some("ঃ"),
        ';' => Some("；"),
        '$' => Some("৳"),
        '&' | '^' => Some("ঁ"),
        '|' => Some("।"),

        _ => None,
    }
}

// ============================================================================
// 3. Bidirectional Unicode ⇄ ANSI (SutonnyMJ) Converter Engine
// ============================================================================

/// Converts Unicode Bengali text to ANSI (SutonnyMJ encoding).
pub fn unicode_to_ansi(text: &str) -> String {
    let mut s = text.to_string();

    // 1. Common Conjuncts (যুক্তবর্ণ)
    let conjuncts: &[(&str, &str)] = &[
        ("ক্ষ", "¶"),
        ("জ্ঞ", "Á"),
        ("ত্র", "Î"),
        ("ক্ত", "³"),
        ("ঙ্ক", "¼"),
        ("ঙ্গ", "½"),
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
        ("ত্ন", "Í"),
        ("ত্ম", "Î"),
        ("ন্দ", "›"),
        ("ন্ধ", "Ü"),
        ("ন্ন", "bœ"),
        ("ম্প", "¤ú"),
        ("ম্ব", "¤^"),
        ("ম্ভ", "¤¢"),
        ("ম্ম", "¤§"),
        ("ষ্ট", "ó"),
        ("ষ্ঠ", "ô"),
        ("ষ্ণ", "õ"),
        ("ষ্প", "ö"),
        ("ষ্ফ", "÷"),
        ("স্ত", "¯Í"),
        ("স্থ", "¯’"),
        ("স্ন", "mœ"),
        ("স্প", "¯ú"),
        ("স্ফ", "ù"),
        ("স্ব", "¯^"),
        ("স্ম", "¯§"),
        ("ক্র", "µ"),
        ("প্র", "cÖ"),
        ("গ্র", "MÖ"),
        ("ভ্র", "å"),
        ("দ্ধ", "×"),
        ("দ্ব", "Ø"),
        ("দ্ম", "Ù"),
        ("হ্ন", "þ"),
        ("হ্ম", "ÿ"),
        ("হৃ", "ü"),
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
    // ে: '†' + Consonant
    // ৈ: '‰' + Consonant
    // ো: '†' + Consonant + 'v'
    // ৌ: '†' + Consonant + 'Š'
    chars = out.chars().collect();
    out = String::new();
    i = 0;
    let len = chars.len();

    while i < len {
        let ch = chars[i];
        if i + 1 < len {
            let next = chars[i + 1];
            match next {
                'ি' => {
                    out.push('w');
                    out.push(ch);
                    i += 2;
                    continue;
                }
                'ে' => {
                    out.push('†');
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
                    out.push('†');
                    out.push(ch);
                    out.push('v');
                    i += 2;
                    continue;
                }
                'ৌ' => {
                    out.push('†');
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
            'ী' => "x",
            'ু' => "y",
            'ূ' => "~",
            'ৃ' => "„",
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

/// Converts ANSI (SutonnyMJ encoding) text to Unicode Bengali.
pub fn ansi_to_unicode(text: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    // 1. Re-position pre-kars:
    // 'w' + C -> C + ি
    // '†' + C + 'v' -> C + ো
    // '†' + C + 'Š' -> C + ৌ
    // '†' + C -> C + ে
    // '‰' + C -> C + ৈ
    while i < len {
        let ch = chars[i];
        if ch == 'w' && i + 1 < len {
            let c = chars[i + 1];
            out.push(c);
            out.push('ি');
            i += 2;
            continue;
        } else if ch == '†' && i + 1 < len {
            let c = chars[i + 1];
            if i + 2 < len && chars[i + 2] == 'v' {
                out.push(c);
                out.push('ো');
                i += 3;
                continue;
            } else if i + 2 < len && chars[i + 2] == 'Š' {
                out.push(c);
                out.push('ৌ');
                i += 3;
                continue;
            } else {
                out.push(c);
                out.push('ে');
                i += 2;
                continue;
            }
        } else if ch == '‰' && i + 1 < len {
            let c = chars[i + 1];
            out.push(c);
            out.push('ৈ');
            i += 2;
            continue;
        }
        out.push(ch);
        i += 1;
    }

    // 2. Re-position Reph:
    // C + '©' -> র্ + C
    let mut temp = String::new();
    let chars2: Vec<char> = out.chars().collect();
    let len2 = chars2.len();
    i = 0;

    while i < len2 {
        if i + 1 < len2 && chars2[i + 1] == '©' {
            temp.push_str("র্");
            temp.push(chars2[i]);
            i += 2;
            continue;
        }
        temp.push(chars2[i]);
        i += 1;
    }

    // 3. Replace Conjunct Glyphs
    let conjuncts: &[(&str, &str)] = &[
        ("¶", "ক্ষ"),
        ("Á", "জ্ঞ"),
        ("Î", "ত্র"),
        ("³", "ক্ত"),
        ("¼", "ঙ্ক"),
        ("½", "ঙ্গ"),
        ("Â", "ঞ্চ"),
        ("Ã", "ঞ্ছ"),
        ("Ä", "ঞ্জ"),
        ("Æ", "ট্ট"),
        ("Ç", "ড্ড"),
        ("È", "ণ্ট"),
        ("É", "ণ্ঠ"),
        ("Ê", "ণ্ড"),
        ("Ë", "ত্ত"),
        ("Ì", "ত্থ"),
        ("Í", "ত্ন"),
        ("Î", "ত্ম"),
        ("›", "ন্দ"),
        ("Ü", "ন্ধ"),
        ("bœ", "ন্ন"),
        ("¤ú", "ম্প"),
        ("¤^", "ম্ব"),
        ("¤¢", "ম্ভ"),
        ("¤§", "ম্ম"),
        ("ó", "ষ্ট"),
        ("ô", "ষ্ঠ"),
        ("õ", "ষ্ণ"),
        ("ö", "ষ্প"),
        ("÷", "ষ্ফ"),
        ("¯Í", "স্ত"),
        ("¯’", "স্থ"),
        ("mœ", "স্ন"),
        ("¯ú", "স্প"),
        ("ù", "স্ফ"),
        ("¯^", "স্ব"),
        ("¯§", "স্ম"),
        ("µ", "ক্র"),
        ("cÖ", "প্র"),
        ("MÖ", "গ্র"),
        ("å", "ভ্র"),
        ("×", "দ্ধ"),
        ("Ø", "দ্ব"),
        ("Ù", "দ্ম"),
        ("þ", "হ্ন"),
        ("ÿ", "হ্ম"),
        ("ü", "হৃ"),
    ];

    for &(a, u) in conjuncts {
        temp = temp.replace(a, u);
    }

    // 4. Map Remaining Basic Characters
    let mut final_out = String::with_capacity(temp.len());
    let mut char_iter = temp.chars().peekable();

    while let Some(c) = char_iter.next() {
        if c == 'A' && char_iter.peek() == Some(&'v') {
            char_iter.next();
            final_out.push('আ');
            continue;
        }

        let mapped = match c {
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
            's' => "ং",
            't' => "ঃ",
            'u' => "ঁ",
            'v' => "া",
            'x' => "ী",
            'y' => "ু",
            '~' => "ূ",
            '„' => "ৃ",
            '&' => "্",
            'Ö' => "্র",
            '¨' => "্য",
            _ => "",
        };

        if !mapped.is_empty() {
            final_out.push_str(mapped);
        } else {
            final_out.push(c);
        }
    }

    final_out
}

// ============================================================================
// 4. Command Handlers
// ============================================================================

/// `tools.banglaMode` — sets or toggles the active Bangla typing mode.
fn cmd_bangla_mode(s: &mut Session, v: &Value) -> CmdResult {
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
    Ok(json!({
        "mode": s.bangla_mode.name(),
        "label": s.bangla_mode.label(),
        "badge": s.bangla_mode.badge(),
    }))
}

/// `tools.banglaTransliterate` — converts text using the specified engine.
fn cmd_bangla_transliterate(_s: &mut Session, v: &Value) -> CmdResult {
    let text = p::req_str(v, "text")?;
    let mode = p::str(v, "mode").unwrap_or("phonetic");
    let result = match mode {
        "phonetic" => phonetic_transliterate(text),
        "jatiya" => {
            let mut out = String::new();
            for c in text.chars() {
                if let Some(bn) = jatiya_key(c) {
                    out.push_str(bn);
                } else {
                    out.push(c);
                }
            }
            out
        }
        "unicodeToAnsi" => unicode_to_ansi(text),
        "ansiToUnicode" => ansi_to_unicode(text),
        _ => text.to_string(),
    };
    Ok(json!({"result": result, "mode": mode}))
}

/// `tools.banglaConvert` — converts selected text or entire document between Unicode and ANSI.
fn cmd_bangla_convert(s: &mut Session, v: &Value) -> CmdResult {
    let dir = p::str(v, "direction").unwrap_or("unicodeToAnsi");
    let target = p::str(v, "target").unwrap_or("selection");
    let font = p::str(v, "font");

    let is_to_ansi = dir.eq_ignore_ascii_case("unicodeToAnsi");

    if target == "selection" && !s.sel.is_collapsed() {
        let (a, b) = s.sel.ordered();
        let current_text = s.doc.para_at(&a).and_then(|p| p.text.get(a.off..b.off)).unwrap_or("").to_string();
        if !current_text.is_empty() {
            let converted = if is_to_ansi { unicode_to_ansi(&current_text) } else { ansi_to_unicode(&current_text) };
            s.run("text.insert", &json!({"text": converted}))?;

            // Optionally apply the appropriate font
            let target_font = font.unwrap_or(if is_to_ansi { "SutonnyMJ" } else { "Kalpurush" });
            s.run("format.font", &json!({"name": target_font}))?;
            return Ok(json!({"converted": true, "count": 1, "target": "selection"}));
        }
    }

    // Convert entire document
    let mut converted_count = 0;
    for b in &mut s.doc.body {
        let block = Arc::make_mut(b);
        if let wordcraft_doc::Block::Para(p) = block
            && !p.text.is_empty()
        {
            let converted = if is_to_ansi { unicode_to_ansi(&p.text) } else { ansi_to_unicode(&p.text) };
            let target_font = if is_to_ansi { "SutonnyMJ" } else { "Kalpurush" };
            p.text = converted.clone();
            p.runs = vec![wordcraft_doc::Run {
                len: converted.len(),
                props: wordcraft_doc::CharProps { font: Some(target_font.into()), ..Default::default() },
            }];
            converted_count += 1;
        }
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
    fn phonetic_basic_words() {
        assert_eq!(phonetic_transliterate("ami"), "আমি");
        assert_eq!(phonetic_transliterate("bangla"), "বাংলা");
        assert_eq!(phonetic_transliterate("desh"), "দেশ");
        assert_eq!(phonetic_transliterate("dhaka"), "ঢাকা");
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
        assert_eq!(jatiya_key('d'), Some("্"));
        assert_eq!(jatiya_key('f'), Some("া"));
        assert_eq!(jatiya_key('1'), Some("১"));
    }

    #[test]
    fn unicode_to_ansi_and_back() {
        let u = "আমার সোনার বাংলা";
        let a = unicode_to_ansi(u);
        assert_eq!(a, "Avgvi †mvbvi evsjv");
        let back = ansi_to_unicode(&a);
        assert_eq!(back, u);
    }
}
