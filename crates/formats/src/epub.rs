//! EPUB 3 export.
//!
//! Produces standard EPUB 3 e-books conforming to IDPF EPUB 3 specification.
//! Packages mimetype, container.xml, content.opf, nav.xhtml, stylesheet,
//! and XHTML document flow.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::io::{Cursor, Write};
use wordcraft_doc::Document;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::html;
use crate::model;

fn xml_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            _ => o.push(c),
        }
    }
    o
}

fn fnv1a(b: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &byte in b {
        h ^= byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Export a [`Document`] to an EPUB 3 binary package.
pub fn export(doc: &Document) -> Result<Vec<u8>, String> {
    let flow = model::from_doc(doc);
    let title = if flow.meta.title.is_empty() { "Untitled Document".to_string() } else { flow.meta.title.clone() };
    let author = if flow.meta.author.is_empty() { "WordCraft Author".to_string() } else { flow.meta.author.clone() };
    let lang = doc.styles.default_chr.lang.as_deref().unwrap_or("en");

    let xhtml_content = html::export_flow(&flow, lang);

    let mut buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buf);

    // 1. mimetype (must be uncompressed, first entry)
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("mimetype", stored).map_err(|e| e.to_string())?;
    zip.write_all(b"application/epub+zip").map_err(|e| e.to_string())?;

    let def = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    // 2. META-INF/container.xml
    zip.start_file("META-INF/container.xml", def).map_err(|e| e.to_string())?;
    let container_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
    zip.write_all(container_xml.as_bytes()).map_err(|e| e.to_string())?;

    // 3. OEBPS/style.css
    zip.start_file("OEBPS/style.css", def).map_err(|e| e.to_string())?;
    let css = r#"body { font-family: sans-serif; line-height: 1.5; margin: 5%; color: #111; }
h1, h2, h3, h4, h5, h6 { color: #156082; margin-top: 1.2em; margin-bottom: 0.5em; }
table { border-collapse: collapse; width: 100%; margin: 1em 0; }
th, td { border: 1px solid #ccc; padding: 6px 10px; }
blockquote { border-left: 3px solid #156082; margin: 1em 0; padding-left: 1em; color: #555; }
pre { background: #f4f4f4; padding: 0.8em; border-radius: 4px; overflow-x: auto; }
"#;
    zip.write_all(css.as_bytes()).map_err(|e| e.to_string())?;

    // 4. OEBPS/nav.xhtml
    zip.start_file("OEBPS/nav.xhtml", def).map_err(|e| e.to_string())?;
    let nav_xhtml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="{lang}">
<head>
  <meta charset="utf-8"/>
  <title>Table of Contents</title>
  <link rel="stylesheet" type="text/css" href="style.css"/>
</head>
<body>
  <nav epub:type="toc" id="toc">
    <h1>Table of Contents</h1>
    <ol>
      <li><a href="content.xhtml">{}</a></li>
    </ol>
  </nav>
</body>
</html>"#,
        xml_escape(&title)
    );
    zip.write_all(nav_xhtml.as_bytes()).map_err(|e| e.to_string())?;

    // 5. OEBPS/content.xhtml
    zip.start_file("OEBPS/content.xhtml", def).map_err(|e| e.to_string())?;
    zip.write_all(xhtml_content.as_bytes()).map_err(|e| e.to_string())?;

    // 6. OEBPS/content.opf
    zip.start_file("OEBPS/content.opf", def).map_err(|e| e.to_string())?;
    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="pub-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="pub-id">urn:uuid:wordcraft-export-{}</dc:identifier>
    <dc:title>{}</dc:title>
    <dc:language>{}</dc:language>
    <dc:creator>{}</dc:creator>
  </metadata>
  <manifest>
    <item id="content" href="content.xhtml" media-type="application/xhtml+xml"/>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="css" href="style.css" media-type="text/css"/>
  </manifest>
  <spine>
    <itemref idref="content"/>
  </spine>
</package>"#,
        fnv1a(title.as_bytes()),
        xml_escape(&title),
        xml_escape(lang),
        xml_escape(&author)
    );
    zip.write_all(opf.as_bytes()).map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}
