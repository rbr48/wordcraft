//! Arabic kashida (tatweel) justification: identifies cursive joining opportunities
//! between Arabic script letters and manages elongation points for justified lines.

use crate::para::{ClKind, ParaLayout};

/// A kashida elongation point on a justified line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kashida {
    /// Logical cluster index on the line after which the elongation occurs.
    pub cluster: usize,
    /// Style run index for font face, size, and color.
    pub style: u16,
    /// Width of the elongation in points.
    pub width: f32,
}

/// Is `c` an Arabic script character?
pub fn is_arabic(c: char) -> bool {
    matches!(c as u32,
        0x0600..=0x06FF // Basic Arabic
        | 0x0750..=0x077F // Arabic Supplement
        | 0x08A0..=0x08FF // Arabic Extended-A
        | 0xFB50..=0xFDFF // Arabic Presentation Forms-A
        | 0xFE70..=0xFEFF // Arabic Presentation Forms-B
    )
}

/// Is `c` an Arabic combining mark (harakat, tashkeel)?
pub fn is_arabic_mark(c: char) -> bool {
    matches!(c as u32,
        0x0610..=0x061A
        | 0x064B..=0x065F
        | 0x0670
        | 0x06D6..=0x06DC
        | 0x06DF..=0x06E4
        | 0x06E7..=0x06E8
        | 0x06EA..=0x06ED
        | 0x08D3..=0x08E1
        | 0x08E3..=0x08FF
    )
}

/// Does Arabic letter `c` connect to the character that follows it (left-joining in logical order)?
pub fn arabic_connects_next(c: char) -> bool {
    if !is_arabic(c) && c != '\u{200D}' {
        return false;
    }
    if c == '\u{200D}' || c == '\u{0640}' {
        return true; // ZWJ and Tatweel connect
    }
    if c == '\u{200C}' {
        return false; // ZWNJ prevents connection
    }
    // Right-joining and non-joining letters do NOT connect forward to next letter:
    !matches!(c,
        // Alef forms
        '\u{0622}' | '\u{0623}' | '\u{0625}' | '\u{0627}' | '\u{0671}'..='\u{0673}' | '\u{0675}'
        // Dal forms
        | '\u{062F}' | '\u{0630}' | '\u{0688}'..='\u{0690}' | '\u{06EE}'
        // Ra forms
        | '\u{0631}' | '\u{0632}' | '\u{0691}'..='\u{0699}' | '\u{06EF}'
        // Waw forms
        | '\u{0624}' | '\u{0648}' | '\u{0676}' | '\u{0677}' | '\u{06C4}'..='\u{06CB}' | '\u{06CF}'
        // Taa Marbuta
        | '\u{0629}' | '\u{06C0}' | '\u{06D5}'
        // Non-joining Hamza & symbols
        | '\u{0621}' | '\u{0674}'
    )
}

/// Does Arabic letter `c` connect to the character that precedes it (right-joining in logical order)?
pub fn arabic_connects_prev(c: char) -> bool {
    if !is_arabic(c) && c != '\u{200D}' {
        return false;
    }
    if c == '\u{200D}' || c == '\u{0640}' {
        return true; // ZWJ and Tatweel connect
    }
    if c == '\u{200C}' {
        return false; // ZWNJ prevents connection
    }
    // Only non-joining Hamza does NOT connect backward:
    !matches!(c, '\u{0621}' | '\u{0674}')
}

/// Is `c` an Alef letter?
pub fn is_alef(c: char) -> bool {
    matches!(c, '\u{0622}' | '\u{0623}' | '\u{0625}' | '\u{0627}' | '\u{0671}'..='\u{0673}' | '\u{0675}')
}

/// Is `c` a Laam letter?
pub fn is_laam(c: char) -> bool {
    matches!(c, '\u{0644}' | '\u{06B5}'..='\u{06B8}')
}

/// Can a kashida (tatweel elongation) be inserted between character `c1` and `c2`?
pub fn is_kashida_opportunity(c1: char, c2: char) -> bool {
    if !is_arabic(c1) || !is_arabic(c2) {
        return false;
    }
    if !arabic_connects_next(c1) || !arabic_connects_prev(c2) {
        return false;
    }
    // Mandatory Lam-Alef ligature must not be split by kashida
    if is_laam(c1) && is_alef(c2) {
        return false;
    }
    true
}

/// Finds the base (non-mark) character from the cluster's text span.
fn cluster_base_char(text: &str) -> Option<char> {
    text.chars().find(|c| !is_arabic_mark(*c))
}

/// Scan a line's clusters for valid Arabic kashida insertion points.
pub fn find_kashida_points(pl: &ParaLayout, para_text: &str, align_from: usize, c1: usize) -> Vec<usize> {
    if c1 <= align_from + 1 {
        return Vec::new();
    }
    let mut points = Vec::new();
    for k in align_from..c1 - 1 {
        let (Some(c1_cl), Some(c2_cl)) = (pl.clusters.get(k), pl.clusters.get(k + 1)) else { continue };
        if c1_cl.kind != ClKind::Text || c2_cl.kind != ClKind::Text {
            continue;
        }
        let Some(t1) = para_text.get(c1_cl.start..c1_cl.end) else { continue };
        let Some(t2) = para_text.get(c2_cl.start..c2_cl.end) else { continue };
        let (Some(ch1), Some(ch2)) = (cluster_base_char(t1), cluster_base_char(t2)) else { continue };
        if is_kashida_opportunity(ch1, ch2) {
            points.push(k);
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_joining_connectivity_rules() {
        // Dual-joining letters connect both ways
        for c in ['ب', 'ت', 'ث', 'ج', 'ح', 'خ', 'س', 'ش', 'ص', 'ض', 'ط', 'ظ', 'ع', 'غ', 'ف', 'ق', 'ك', 'ل', 'م', 'ن', 'ه', 'ي']
        {
            assert!(arabic_connects_next(c), "{c} should connect next");
            assert!(arabic_connects_prev(c), "{c} should connect prev");
        }
        // Right-joining letters only connect to previous
        for c in ['ا', 'د', 'ذ', 'ر', 'ز', 'و', 'ة'] {
            assert!(!arabic_connects_next(c), "{c} must NOT connect next");
            assert!(arabic_connects_prev(c), "{c} should connect prev");
        }
        // Hamza does not connect either way
        assert!(!arabic_connects_next('ء'));
        assert!(!arabic_connects_prev('ء'));
    }

    #[test]
    fn kashida_opportunities_in_words() {
        // "سلام": 'س' + 'ل' is valid; 'ل' + 'ا' is Lam-Alef (invalid); 'ا' does not connect forward
        assert!(is_kashida_opportunity('س', 'ل'));
        assert!(!is_kashida_opportunity('ل', 'ا'));
        assert!(!is_kashida_opportunity('ا', 'م'));

        // "کتاب": 'ک' + 'ت' valid; 'ت' + 'ا' valid; 'ا' + 'ب' invalid
        assert!(is_kashida_opportunity('ک', 'ت'));
        assert!(is_kashida_opportunity('ت', 'ا'));
        assert!(!is_kashida_opportunity('ا', 'ب'));

        // Non-Arabic letters
        assert!(!is_kashida_opportunity('A', 'B'));
        assert!(!is_kashida_opportunity('س', ' '));
    }
}
