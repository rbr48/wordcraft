//! Bibliography sources storage (`customXml/item1.xml`, ECMA-376 Part 4 / ISO 29500).
//!
//! Word stores references and bibliography sources in custom XML parts within the package.
//! Reading and writing these parts ensures bibliography sources survive round trips intact.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use wordcraft_doc::Source;

use crate::xml::{El, W};

/// Map Word's `SourceType` string to WordCraft's lowercase kind.
pub fn parse_kind(s: &str) -> String {
    match s.trim().to_ascii_lowercase().as_str() {
        "book" | "booksection" => "book".into(),
        "journalarticle" | "articleinaperiodical" | "conferenceproceedings" | "article" => "article".into(),
        "internetsite" | "documentfrominternetsite" | "electronicsource" | "website" => "website".into(),
        "report" => "report".into(),
        "film" | "soundrecording" => "film".into(),
        "misc" => "other".into(),
        other => {
            if other.is_empty() {
                "book".into()
            } else {
                other.to_string()
            }
        }
    }
}

/// Map WordCraft's kind to Word's PascalCase `SourceType`.
pub fn source_type(kind: &str) -> &'static str {
    match kind.trim().to_ascii_lowercase().as_str() {
        "book" => "Book",
        "booksection" => "BookSection",
        "article" | "journalarticle" => "JournalArticle",
        "articleinaperiodical" => "ArticleInAPeriodical",
        "conferenceproceedings" => "ConferenceProceedings",
        "report" => "Report",
        "website" | "internetsite" => "InternetSite",
        "documentfrominternetsite" => "DocumentFromInternetSite",
        "film" => "Film",
        "soundrecording" => "SoundRecording",
        "patent" => "Patent",
        "case" => "Case",
        "interview" => "Interview",
        _ => "Misc",
    }
}

/// Split author string ("Last, First; Last, First") into (last, first) tuples.
fn split_authors(s: &str) -> Vec<(String, String)> {
    s.split(';')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(|x| match x.split_once(',') {
            Some((last, first)) => (last.trim().to_string(), first.trim().to_string()),
            None => match x.rsplit_once(' ') {
                Some((first, last)) => (last.trim().to_string(), first.trim().to_string()),
                None => (x.to_string(), String::new()),
            },
        })
        .collect()
}

/// Deterministic pseudo-GUID for a source.
fn make_guid(tag: &str, title: &str) -> String {
    let mut h1 = DefaultHasher::new();
    tag.hash(&mut h1);
    let v1 = h1.finish();
    let mut h2 = DefaultHasher::new();
    title.hash(&mut h2);
    let v2 = h2.finish();
    let p1 = (v1 >> 32) as u32;
    let p2 = (v1 & 0xFFFF_FFFF) as u32;
    let p3 = (v2 >> 32) as u32;
    let p4 = (v2 & 0xFFFF_FFFF) as u32;
    format!("{{{:08X}-{:04X}-4{:03X}-8{:03X}-{:08X}{:04X}}}", p1, (p2 >> 16) as u16, p2 as u16 & 0x0FFF, (p3 >> 16) as u16 & 0x0FFF, p4, p3 as u16,)
}

/// Serialize bibliography sources to `customXml/item1.xml` bytes.
pub fn write(sources: &[Source]) -> Vec<u8> {
    let mut w = W::new();
    w.open(
        "b:Sources",
        &[
            ("xmlns:b", "http://schemas.openxmlformats.org/officeDocument/2006/bibliography"),
            ("xmlns", "http://schemas.openxmlformats.org/officeDocument/2006/bibliography"),
            ("SelectedStyle", ""),
        ],
    );
    for src in sources {
        if src.tag.is_empty() && src.title.is_empty() {
            continue;
        }
        w.open("b:Source", &[]);
        let tag = if src.tag.is_empty() { "Source" } else { &src.tag };
        w.leaf("b:Tag", &[], tag);
        w.leaf("b:SourceType", &[], source_type(&src.kind));
        let guid = make_guid(&src.tag, &src.title);
        w.leaf("b:Guid", &[], &guid);
        if !src.title.is_empty() {
            w.leaf("b:Title", &[], &src.title);
        }
        if !src.year.is_empty() {
            w.leaf("b:Year", &[], &src.year);
        }
        if !src.city.is_empty() {
            w.leaf("b:City", &[], &src.city);
        }
        if !src.publisher.is_empty() {
            w.leaf("b:Publisher", &[], &src.publisher);
        }
        if !src.journal.is_empty() {
            w.leaf("b:JournalName", &[], &src.journal);
        }
        if !src.volume.is_empty() {
            w.leaf("b:VolumeNumber", &[], &src.volume);
        }
        if !src.pages.is_empty() {
            w.leaf("b:Pages", &[], &src.pages);
        }
        if !src.url.is_empty() {
            w.leaf("b:URL", &[], &src.url);
        }
        if !src.author.is_empty() {
            w.open("b:Author", &[]);
            w.open("b:Author", &[]);
            w.open("b:NameList", &[]);
            for (last, first) in split_authors(&src.author) {
                w.open("b:Person", &[]);
                if !last.is_empty() {
                    w.leaf("b:Last", &[], &last);
                }
                if !first.is_empty() {
                    w.leaf("b:First", &[], &first);
                }
                w.close("b:Person");
            }
            w.close("b:NameList");
            w.close("b:Author");
            w.close("b:Author");
        }
        w.close("b:Source");
    }
    w.close("b:Sources");
    w.into_bytes()
}

/// Serialize customXml item properties for the bibliography part (`customXml/itemProps1.xml`).
pub fn write_props() -> Vec<u8> {
    let mut w = W::new();
    w.open(
        "ds:datastoreItem",
        &[("xmlns:ds", "http://schemas.openxmlformats.org/officeDocument/2006/customXml"), ("ds:itemID", "{B9E81310-9686-4545-983A-1B05AE54176A}")],
    );
    w.open("ds:schemaRefs", &[]);
    w.empty("ds:schemaRef", &[("ds:uri", "http://schemas.openxmlformats.org/officeDocument/2006/bibliography")]);
    w.close("ds:schemaRefs");
    w.close("ds:datastoreItem");
    w.into_bytes()
}

/// Parse bibliography sources from a `<b:Sources>` element.
pub fn read(root: &El) -> Vec<Source> {
    let mut out = Vec::new();
    for s in root.els().filter(|e| e.local() == "Source").take(10_000) {
        let mut src = Source::default();
        for c in s.els() {
            match c.local() {
                "Tag" => src.tag = c.text().trim().to_string(),
                "SourceType" => src.kind = parse_kind(c.text().trim()),
                "Title" => src.title = c.text().trim().to_string(),
                "Year" => src.year = c.text().trim().to_string(),
                "City" => src.city = c.text().trim().to_string(),
                "Publisher" => src.publisher = c.text().trim().to_string(),
                "JournalName" => src.journal = c.text().trim().to_string(),
                "VolumeNumber" | "Volume" => src.volume = c.text().trim().to_string(),
                "Pages" => src.pages = c.text().trim().to_string(),
                "URL" => src.url = c.text().trim().to_string(),
                "Author" => {
                    let mut author_names = Vec::new();
                    collect_persons(c, &mut author_names, 0);
                    if author_names.is_empty()
                        && let Some(corp) = find_local(c, "Corporate", 0)
                    {
                        let t = corp.text().trim().to_string();
                        if !t.is_empty() {
                            author_names.push(t);
                        }
                    }
                    if author_names.is_empty() {
                        let direct = c.text().trim().to_string();
                        if !direct.is_empty() {
                            author_names.push(direct);
                        }
                    }
                    if !author_names.is_empty() {
                        src.author = author_names.join("; ");
                    }
                }
                _ => {}
            }
        }
        if !src.tag.is_empty() || !src.title.is_empty() {
            if src.tag.is_empty() {
                src.tag = format!("Source{}", out.len().saturating_add(1));
            }
            out.push(src);
        }
    }
    out
}

fn collect_persons(e: &El, out: &mut Vec<String>, depth: usize) {
    if depth > 10 {
        return;
    }
    if e.local() == "Person" {
        let last = find_local_text(e, "Last");
        let first = find_local_text(e, "First");
        let middle = find_local_text(e, "Middle");
        let first_full = match (first.is_empty(), middle.is_empty()) {
            (false, false) => format!("{first} {middle}"),
            (false, true) => first,
            (true, false) => middle,
            (true, true) => String::new(),
        };
        let full = if !first_full.is_empty() && !last.is_empty() {
            format!("{last}, {first_full}")
        } else if !last.is_empty() {
            last
        } else {
            first_full
        };
        if !full.is_empty() {
            out.push(full);
        }
        return;
    }
    for kid in e.els() {
        collect_persons(kid, out, depth.saturating_add(1));
    }
}

fn find_local<'a>(e: &'a El, name: &str, depth: usize) -> Option<&'a El> {
    if depth > 10 {
        return None;
    }
    if e.local() == name {
        return Some(e);
    }
    for kid in e.els() {
        if let Some(found) = find_local(kid, name, depth.saturating_add(1)) {
            return Some(found);
        }
    }
    None
}

fn find_local_text(e: &El, name: &str) -> String {
    find_local(e, name, 0).map(|x| x.text().trim().to_string()).unwrap_or_default()
}
