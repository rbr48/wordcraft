//! Bangla typing evaluation against real text: dictionary coverage, rule-vs-dictionary agreement,
//! Bijoy/ANSI round trips, Jatiya/Bijoy keyboard reach, and timing on long documents.
//!
//! The corpus-based tests are `#[ignore]` and read a UTF-8 Bangla text file from
//! `WORDCRAFT_BANGLA_CORPUS`, so no third-party text is committed. Run them in release mode:
//!
//! ```text
//! WORDCRAFT_BANGLA_CORPUS=/path/to/bangla.txt \
//!   cargo test -p wordcraft-engine --release bangla_eval -- --ignored --nocapture --test-threads=1
//! ```

use crate::cmd::bangla::{
    BANGLA_VOCAB, ansi_to_unicode, bijoy_ansi_key, bijoy_linker_vowel, jatiya_key, jatiya_linker_vowel, phonetic_candidates, phonetic_transliterate,
    unicode_to_ansi,
};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

fn corpus() -> String {
    let path = std::env::var("WORDCRAFT_BANGLA_CORPUS").expect("set WORDCRAFT_BANGLA_CORPUS to a Bangla text file");
    std::fs::read_to_string(path).expect("corpus file is readable UTF-8")
}

fn is_bangla(c: char) -> bool {
    ('\u{0980}'..='\u{09FF}').contains(&c)
}

fn bangla_tokens(text: &str) -> Vec<&str> {
    text.split(|c: char| !is_bangla(c)).filter(|s| !s.is_empty()).collect()
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 { 0.0 } else { n as f64 * 100.0 / d as f64 }
}

/// Word -> the romanised spellings in the dictionary that produce it.
fn keys_by_word() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
    for &(k, v) in BANGLA_VOCAB {
        m.entry(v).or_default().push(k);
    }
    m
}

#[test]
#[ignore = "corpus and timing evaluation; run manually (see module docs)"]
fn bangla_eval_rules_reproduce_dictionary() {
    let mut ok = 0usize;
    let mut misses = Vec::new();
    for &(k, v) in BANGLA_VOCAB {
        let got = phonetic_transliterate(k);
        if got == v {
            ok += 1;
        } else if misses.len() < 40 {
            misses.push((k, v, got));
        }
    }
    println!("rules reproduce dictionary entry: {ok}/{} = {:.2}%", BANGLA_VOCAB.len(), pct(ok, BANGLA_VOCAB.len()));
    for (k, v, got) in misses {
        println!("  miss  {k:>14}  want {v}  got {got}");
    }
}

#[test]
#[ignore = "corpus and timing evaluation; run manually (see module docs)"]
fn bangla_eval_real_text_coverage_and_top_suggestion() {
    let text = corpus();
    let tokens = bangla_tokens(&text);
    let unique: HashSet<&str> = tokens.iter().copied().collect();
    let by_word = keys_by_word();

    let covered_total = tokens.iter().filter(|t| by_word.contains_key(*t)).count();
    let covered_unique = unique.iter().filter(|t| by_word.contains_key(*t)).count();
    println!("real text: {} chars, {} tokens, {} unique", text.chars().count(), tokens.len(), unique.len());
    println!(
        "dictionary covers {}/{} tokens = {:.2}% (unique {}/{} = {:.2}%)",
        covered_total,
        tokens.len(),
        pct(covered_total, tokens.len()),
        covered_unique,
        unique.len(),
        pct(covered_unique, unique.len())
    );

    // Typing the dictionary spelling of a real word should put that word first.
    let mut top_ok = 0usize;
    let mut top_total = 0usize;
    let mut top_miss = Vec::new();
    for word in &unique {
        let Some(keys) = by_word.get(word) else { continue };
        let key = keys[0];
        top_total += 1;
        let first = phonetic_candidates(key).into_iter().next().unwrap_or_default();
        if first == *word {
            top_ok += 1;
        } else if top_miss.len() < 30 {
            top_miss.push((key, *word, first));
        }
    }
    println!("top suggestion correct for covered unique words: {top_ok}/{top_total} = {:.2}%", pct(top_ok, top_total));
    for (key, want, got) in top_miss {
        println!("  miss  {key:>14}  want {want}  top {got}");
    }
}

#[test]
#[ignore = "corpus and timing evaluation; run manually (see module docs)"]
fn bangla_eval_bijoy_round_trip_on_real_text() {
    let text = corpus();
    let mut lines = 0usize;
    let mut mismatched = 0usize;
    let mut shown = 0usize;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        lines += 1;
        let back = ansi_to_unicode(&unicode_to_ansi(line));
        if back != line {
            mismatched += 1;
            if shown < 5 {
                shown += 1;
                let first_diff = line.chars().zip(back.chars()).position(|(a, b)| a != b);
                println!("  mismatch at char {first_diff:?}");
                println!("    in : {}", line.chars().take(120).collect::<String>());
                println!("    out: {}", back.chars().take(120).collect::<String>());
            }
        }
    }
    println!("Unicode -> ANSI -> Unicode identical on {}/{lines} lines = {:.2}%", lines - mismatched, pct(lines - mismatched, lines));
}

/// Diagnostic: the smallest real-looking inputs that fail the Unicode -> ANSI -> Unicode round trip.
#[test]
#[ignore = "diagnostic for the Bijoy round-trip defect; run manually"]
fn bangla_eval_bijoy_round_trip_minimal_cases() {
    for word in ["বিশ্বের", "বৈশ্বিক", "বাংলাদেশ।", "কে", "কি", "ত্র", "মুঘল", "বাংলাদেশ", "আমার সোনার বাংলা।"]
    {
        let a = unicode_to_ansi(word);
        let back = ansi_to_unicode(&a);
        println!("  {word:<22} ansi {a:<22} back {back:<22} {}", if back == word { "ok" } else { "LOSSY" });
    }
}

#[test]
#[ignore = "corpus and timing evaluation; run manually (see module docs)"]
fn bangla_eval_keyboard_reach_on_real_text() {
    let text = corpus();
    let distinct: HashSet<char> = text.chars().filter(|c| is_bangla(*c)).collect();

    // A character is reachable by one key, or by the 'g' linker followed by a key (independent vowels).
    let jatiya_out: HashSet<String> =
        (0x21u8..0x7f).flat_map(|b| [jatiya_key(b as char), jatiya_linker_vowel(b as char)]).flatten().map(str::to_string).collect();
    let jatiya_missing: Vec<char> = distinct.iter().copied().filter(|c| !jatiya_out.contains(&c.to_string())).collect();
    println!(
        "Jatiya keys reach {}/{} distinct Bangla characters in the corpus; missing: {:?}",
        distinct.len() - jatiya_missing.len(),
        distinct.len(),
        jatiya_missing
    );

    let bijoy_out: HashSet<String> =
        (0x21u8..0x7f).flat_map(|b| [bijoy_ansi_key(b as char), bijoy_linker_vowel(b as char)]).flatten().map(str::to_string).collect();
    let ansi = unicode_to_ansi(&text);
    let ansi_glyphs: HashSet<char> = ansi.chars().filter(|c| !c.is_whitespace()).collect();
    let bijoy_missing: Vec<char> = ansi_glyphs.iter().copied().filter(|c| !bijoy_out.contains(&c.to_string())).collect();
    println!(
        "Bijoy keys reach {}/{} distinct ANSI glyphs produced from the corpus; missing: {:?}",
        ansi_glyphs.len() - bijoy_missing.len(),
        ansi_glyphs.len(),
        bijoy_missing
    );
}

#[test]
#[ignore = "corpus and timing evaluation; run manually (see module docs)"]
fn bangla_eval_timing_on_long_documents() {
    let text = corpus();
    let tokens = bangla_tokens(&text);
    let by_word = keys_by_word();
    let keys: Vec<&str> = tokens.iter().filter_map(|t| by_word.get(t).map(|k| k[0])).collect();

    // Romanised typing of a long document, one word at a time.
    let roman_doc: String = keys.join(" ");
    let t = Instant::now();
    let out = phonetic_transliterate(&roman_doc);
    let d = t.elapsed();
    println!(
        "phonetic_transliterate: {} roman chars -> {} chars in {:?} ({:.0} chars/ms)",
        roman_doc.len(),
        out.chars().count(),
        d,
        roman_doc.len() as f64 / d.as_secs_f64() / 1000.0
    );

    // Per-keystroke candidate lookup across the whole dictionary.
    let sample: Vec<&str> = BANGLA_VOCAB.iter().step_by(BANGLA_VOCAB.len() / 1000).map(|&(k, _)| k).take(1000).collect();
    let t = Instant::now();
    let mut n = 0usize;
    for k in &sample {
        n += phonetic_candidates(k).len();
    }
    let d = t.elapsed();
    println!(
        "phonetic_candidates: {} lookups in {:?}, mean {:.1} us per keystroke ({n} candidates)",
        sample.len(),
        d,
        d.as_secs_f64() * 1e6 / sample.len() as f64
    );

    // Bijoy converter scaling: 1x, 4x and 16x the corpus.
    for reps in [1usize, 4, 16] {
        let big = text.repeat(reps);
        let t = Instant::now();
        let a = unicode_to_ansi(&big);
        let to_ansi = t.elapsed();
        let t = Instant::now();
        let u = ansi_to_unicode(&a);
        let to_uni = t.elapsed();
        println!(
            "Bijoy convert {:>7} chars: to ANSI {:?}, back {:?} (round trip ok: {})",
            big.chars().count(),
            to_ansi,
            to_uni,
            u.chars().count() == big.chars().count()
        );
    }
}

/// Romanised input and the Bangla spelling expected for it. Written by hand for review, not
/// copied from Avro Keyboard, so a mismatch means either the engine or this expectation is wrong.
const PHONETIC_CHECK: &[(&str, &str)] = &[
    ("ami", "আমি"),
    ("tumi", "তুমি"),
    ("amra", "আমরা"),
    ("tomra", "তোমরা"),
    ("ekhon", "এখন"),
    ("kal", "কাল"),
    ("aj", "আজ"),
    ("bhalo", "ভালো"),
    ("kemon", "কেমন"),
    ("achho", "আছো"),
    ("jacchi", "যাচ্ছি"),
    ("shonar", "সোনার"),
    ("bangla", "বাংলা"),
    ("desh", "দেশ"),
    ("boi", "বই"),
    ("jol", "জল"),
    ("pani", "পানি"),
    ("bhasha", "ভাষা"),
    ("shikkha", "শিক্ষা"),
    ("shomoy", "সময়"),
    ("kotha", "কথা"),
    ("bhalobasha", "ভালোবাসা"),
    ("ghum", "ঘুম"),
    ("shundor", "সুন্দর"),
    ("prothom", "প্রথম"),
    ("shohor", "শহর"),
    ("rastra", "রাষ্ট্র"),
    ("bissho", "বিশ্ব"),
    ("kaj", "কাজ"),
    ("chhele", "ছেলে"),
    ("meye", "মেয়ে"),
    ("bari", "বাড়ি"),
    ("gach", "গাছ"),
    ("nodi", "নদী"),
    ("akash", "আকাশ"),
    ("rat", "রাত"),
    ("din", "দিন"),
    ("sukh", "সুখ"),
    ("dukkho", "দুঃখ"),
    ("sotyi", "সত্যি"),
    ("bhai", "ভাই"),
    ("bon", "বোন"),
    ("ma", "মা"),
    ("baba", "বাবা"),
    ("ghor", "ঘর"),
    ("sokal", "সকাল"),
    ("bikel", "বিকেল"),
    ("ratri", "রাত্রি"),
    ("surjo", "সূর্য"),
];

/// Typed word by word, as a user would: the top suggestion must be the expected spelling.
#[test]
#[ignore = "phonetic check list for review; run manually (see module docs)"]
fn bangla_eval_phonetic_check_list() {
    let mut ok = 0usize;
    let mut misses = Vec::new();
    for &(roman, want) in PHONETIC_CHECK {
        let top = phonetic_candidates(roman).into_iter().next().unwrap_or_default();
        let rules = phonetic_transliterate(roman);
        if top == want {
            ok += 1;
        } else {
            misses.push((roman, want, top, rules));
        }
    }
    println!("top suggestion matches expected spelling: {ok}/{}", PHONETIC_CHECK.len());
    for (roman, want, top, rules) in misses {
        println!("  {roman:<12} expected {want:<14} top {top:<14} rules {rules}");
    }
}
