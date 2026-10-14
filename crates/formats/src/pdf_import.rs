//! PDF import and reflow engine.
//!
//! Converts PDF documents into editable WordCraft [`Document`] AST models ([`model::Flow`])
//! by parsing PDF indirect objects, resolving `/Pages` trees, decompressing content streams
//! (`/Filter /FlateDecode`), parsing Adobe CMap `/ToUnicode` font mappings, extracting positioned
//! text operators (`BT`/`ET`, `Tf`, `Tm`, `Td`, `TD`, `T*`, `Tj`, `TJ`), reflowing lines into
//! paragraphs, headings, lists, tables, and page breaks.
//!
//! Conforms to ISO 32000-1 (PDF 1.7) clean-room specification. Lenient and panic-free.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::collections::HashMap;

use wordcraft_doc::Document;
use wordcraft_doc::encoding::cp1252;

use crate::model::{self, FBlock, Flow, Fmt, Kind, ListInfo, Meta, Para};

const MAX_STREAM_LEN: usize = 64 * 1024 * 1024;
const MAX_RECURSION_DEPTH: usize = 32;

// ---------------------------------------------------------------------------------------------
// PDF Object Model
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum PdfValue {
    Null,
    Bool(bool),
    Int(i64),
    Real(f32),
    String(Vec<u8>),
    Name(String),
    Array(Vec<PdfValue>),
    Dict(Vec<(String, PdfValue)>),
    Ref(u32, u16),
}

impl PdfValue {
    pub fn as_int(&self) -> Option<i64> {
        match self {
            PdfValue::Int(i) => Some(*i),
            PdfValue::Real(r) => Some(*r as i64),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            PdfValue::Real(r) => Some(*r),
            PdfValue::Int(i) => Some(*i as f32),
            _ => None,
        }
    }

    pub fn as_name(&self) -> Option<&str> {
        match self {
            PdfValue::Name(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_dict(&self) -> Option<&[(String, PdfValue)]> {
        match self {
            PdfValue::Dict(d) => Some(d.as_slice()),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[PdfValue]> {
        match self {
            PdfValue::Array(a) => Some(a.as_slice()),
            _ => None,
        }
    }

    pub fn as_ref(&self) -> Option<(u32, u16)> {
        match self {
            PdfValue::Ref(id, gen_num) => Some((*id, *gen_num)),
            _ => None,
        }
    }
}

pub fn dict_get<'a>(dict: &'a [(String, PdfValue)], key: &str) -> Option<&'a PdfValue> {
    dict.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

#[derive(Clone, Debug)]
pub struct PdfObj {
    pub val: PdfValue,
    pub stream_data: Option<Vec<u8>>,
}

// ---------------------------------------------------------------------------------------------
// PDF Lexer and Parser
// ---------------------------------------------------------------------------------------------

struct Lexer<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Lexer { bytes, pos: 0 }
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0C' | 0) {
                self.pos += 1;
            } else if b == b'%' {
                self.pos += 1;
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\r' && self.bytes[self.pos] != b'\n' {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn read_literal_string(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut depth = 1usize;
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            self.pos += 1;
            match b {
                b'\\' => {
                    if self.pos >= self.bytes.len() {
                        break;
                    }
                    let esc = self.bytes[self.pos];
                    self.pos += 1;
                    match esc {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0C),
                        b'(' => out.push(b'('),
                        b')' => out.push(b')'),
                        b'\\' => out.push(b'\\'),
                        b'\r' => {
                            if self.pos < self.bytes.len() && self.bytes[self.pos] == b'\n' {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut octal = (esc - b'0') as u32;
                            for _ in 0..2 {
                                if self.pos < self.bytes.len() && matches!(self.bytes[self.pos], b'0'..=b'7') {
                                    octal = octal * 8 + (self.bytes[self.pos] - b'0') as u32;
                                    self.pos += 1;
                                } else {
                                    break;
                                }
                            }
                            out.push(octal as u8);
                        }
                        other => out.push(other),
                    }
                }
                b'(' => {
                    depth += 1;
                    out.push(b'(');
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(b')');
                }
                other => out.push(other),
            }
        }
        out
    }

    fn read_hex_string(&mut self) -> Vec<u8> {
        let mut hex_chars = Vec::new();
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            self.pos += 1;
            if b == b'>' {
                break;
            }
            if b.is_ascii_hexdigit() {
                hex_chars.push(b);
            }
        }
        let mut out = Vec::with_capacity(hex_chars.len() / 2 + 1);
        let mut i = 0;
        while i < hex_chars.len() {
            let hi = hex_val(hex_chars[i]);
            let lo = if i + 1 < hex_chars.len() {
                i += 2;
                hex_val(hex_chars[i - 1])
            } else {
                i += 1;
                0
            };
            out.push((hi << 4) | lo);
        }
        out
    }

    fn read_name(&mut self) -> String {
        let mut out = Vec::new();
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0C' | 0 | b'/' | b'[' | b']' | b'<' | b'>' | b'(' | b')' | b'{' | b'}' | b'%') {
                break;
            }
            self.pos += 1;
            if b == b'#' && self.pos + 1 < self.bytes.len() {
                let h1 = self.bytes[self.pos];
                let h2 = self.bytes[self.pos + 1];
                if h1.is_ascii_hexdigit() && h2.is_ascii_hexdigit() {
                    self.pos += 2;
                    out.push((hex_val(h1) << 4) | hex_val(h2));
                    continue;
                }
            }
            out.push(b);
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    fn read_token(&mut self) -> Option<String> {
        self.skip_whitespace_and_comments();
        if self.pos >= self.bytes.len() {
            return None;
        }
        let b = self.bytes[self.pos];
        if matches!(b, b'[' | b']' | b'(' | b')') {
            self.pos += 1;
            return Some(char::from(b).to_string());
        }
        if b == b'<' {
            if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'<' {
                self.pos += 2;
                return Some("<<".to_string());
            }
            self.pos += 1;
            return Some("<".to_string());
        }
        if b == b'>' {
            if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'>' {
                self.pos += 2;
                return Some(">>".to_string());
            }
            self.pos += 1;
            return Some(">".to_string());
        }
        if b == b'/' {
            self.pos += 1;
            let name = self.read_name();
            return Some(format!("/{name}"));
        }
        let start = self.pos;
        while self.pos < self.bytes.len() {
            let c = self.bytes[self.pos];
            if matches!(c, b' ' | b'\t' | b'\r' | b'\n' | b'\x0C' | 0 | b'/' | b'[' | b']' | b'<' | b'>' | b'(' | b')' | b'{' | b'}' | b'%') {
                break;
            }
            self.pos += 1;
        }
        if self.pos > start { Some(String::from_utf8_lossy(&self.bytes[start..self.pos]).into_owned()) } else { None }
    }

    fn parse_value(&mut self, depth: usize) -> Option<PdfValue> {
        if depth > MAX_RECURSION_DEPTH {
            return Some(PdfValue::Null);
        }
        self.skip_whitespace_and_comments();
        if self.pos >= self.bytes.len() {
            return None;
        }
        let b = self.bytes[self.pos];
        if b == b'(' {
            self.pos += 1;
            let s = self.read_literal_string();
            return Some(PdfValue::String(s));
        }
        if b == b'<' {
            if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'<' {
                // Dictionary
                self.pos += 2;
                let mut dict = Vec::new();
                loop {
                    self.skip_whitespace_and_comments();
                    if self.pos >= self.bytes.len() {
                        break;
                    }
                    if self.bytes[self.pos] == b'>' && self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'>' {
                        self.pos += 2;
                        break;
                    }
                    if self.bytes[self.pos] != b'/' {
                        // Malformed dictionary entry: consume token to recover
                        if self.read_token().is_none() {
                            break;
                        }
                        continue;
                    }
                    self.pos += 1;
                    let key = self.read_name();
                    let val = self.parse_value(depth + 1).unwrap_or(PdfValue::Null);
                    dict.push((key, val));
                }
                return Some(PdfValue::Dict(dict));
            } else {
                // Hex string
                self.pos += 1;
                let s = self.read_hex_string();
                return Some(PdfValue::String(s));
            }
        }
        if b == b'[' {
            self.pos += 1;
            let mut arr = Vec::new();
            loop {
                self.skip_whitespace_and_comments();
                if self.pos >= self.bytes.len() {
                    break;
                }
                if self.bytes[self.pos] == b']' {
                    self.pos += 1;
                    break;
                }
                if let Some(v) = self.parse_value(depth + 1) {
                    arr.push(v);
                } else {
                    break;
                }
            }
            return Some(PdfValue::Array(arr));
        }
        if b == b'/' {
            self.pos += 1;
            let name = self.read_name();
            return Some(PdfValue::Name(name));
        }

        // Token: number, boolean, null, or indirect ref
        let tok = self.read_token()?;
        match tok.as_str() {
            "true" => Some(PdfValue::Bool(true)),
            "false" => Some(PdfValue::Bool(false)),
            "null" => Some(PdfValue::Null),
            s => {
                if let Ok(num) = s.parse::<i64>() {
                    // Check if followed by gen and 'R' (indirect ref)
                    let saved_pos = self.pos;
                    self.skip_whitespace_and_comments();
                    if let Some(tok2) = self.read_token()
                        && let Ok(gen_num) = tok2.parse::<u16>()
                    {
                        self.skip_whitespace_and_comments();
                        if let Some(tok3) = self.read_token()
                            && tok3 == "R"
                        {
                            return Some(PdfValue::Ref(num as u32, gen_num));
                        }
                    }
                    self.pos = saved_pos;
                    Some(PdfValue::Int(num))
                } else if let Ok(f) = s.parse::<f32>() {
                    Some(PdfValue::Real(f))
                } else {
                    Some(PdfValue::Name(s.to_string()))
                }
            }
        }
    }
}

fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

// ---------------------------------------------------------------------------------------------
// PDF Stream and Document Parsing
// ---------------------------------------------------------------------------------------------

fn decompress_stream(data: &[u8], dict: &[(String, PdfValue)]) -> Vec<u8> {
    let filter = dict_get(dict, "Filter");
    let is_flate = match filter {
        Some(PdfValue::Name(n)) => n == "FlateDecode",
        Some(PdfValue::Array(arr)) => arr.iter().any(|v| v.as_name() == Some("FlateDecode")),
        _ => false,
    };
    if is_flate { miniz_oxide::inflate::decompress_to_vec(data).unwrap_or_else(|_| data.to_vec()) } else { data.to_vec() }
}

fn parse_pdf_objects(bytes: &[u8]) -> HashMap<u32, PdfObj> {
    let mut objects = HashMap::new();
    let mut i = 0usize;
    let len = bytes.len();

    while i < len {
        // Look for pattern: <id> <gen> obj
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let id_str = &bytes[start..i];
            // Skip spaces
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            let gen_start = i;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let gen_str = &bytes[gen_start..i];
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            if i + 3 <= len && &bytes[i..i + 3] == b"obj" && (i + 3 == len || matches!(bytes[i + 3], b' ' | b'\t' | b'\r' | b'\n')) {
                i += 3;
                let id = std::str::from_utf8(id_str).ok().and_then(|s| s.parse::<u32>().ok());
                let _gen_num = std::str::from_utf8(gen_str).ok().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
                if let Some(id) = id {
                    let mut lexer = Lexer::new(bytes);
                    lexer.pos = i;
                    if let Some(val) = lexer.parse_value(0) {
                        let mut stream_data = None;
                        lexer.skip_whitespace_and_comments();
                        if lexer.pos + 6 <= len && &bytes[lexer.pos..lexer.pos + 6] == b"stream" {
                            lexer.pos += 6;
                            // Skip CR / LF after stream
                            if lexer.pos < len && bytes[lexer.pos] == b'\r' {
                                lexer.pos += 1;
                            }
                            if lexer.pos < len && bytes[lexer.pos] == b'\n' {
                                lexer.pos += 1;
                            }
                            let stream_start = lexer.pos;
                            let dict_ref = val.as_dict();
                            let declared_len =
                                dict_ref.and_then(|d| dict_get(d, "Length")).and_then(|v| v.as_int()).filter(|l| *l > 0) as Option<i64>;
                            let mut stream_end = None;

                            if let Some(dlen) = declared_len {
                                let target = stream_start + dlen as usize;
                                if target <= len {
                                    // Check if endstream appears within next 32 bytes
                                    let window = &bytes[target..len.min(target + 32)];
                                    if let Some(off) = find_subsequence(window, b"endstream") {
                                        stream_end = Some(target + off);
                                    }
                                }
                            }
                            if stream_end.is_none()
                                && let Some(off) = find_subsequence(&bytes[stream_start..len.min(stream_start + MAX_STREAM_LEN)], b"endstream")
                            {
                                stream_end = Some(stream_start + off);
                            }

                            if let Some(mut send) = stream_end {
                                while send > stream_start && matches!(bytes[send - 1], b'\r' | b'\n') {
                                    send -= 1;
                                }
                                let raw = &bytes[stream_start..send];
                                let decompressed = dict_ref.map(|d| decompress_stream(raw, d)).unwrap_or_else(|| raw.to_vec());
                                stream_data = Some(decompressed);
                            }
                        }
                        objects.insert(id, PdfObj { val, stream_data });
                        i = lexer.pos;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    // PDF 1.5+ Object Streams (/Type /ObjStm) support:
    let obj_stms: Vec<(u32, Vec<u8>, Vec<(String, PdfValue)>)> = objects
        .iter()
        .filter_map(|(&id, obj)| {
            let dict = obj.val.as_dict()?;
            if dict_get(dict, "Type")?.as_name()? == "ObjStm" {
                let data = obj.stream_data.as_ref()?;
                Some((id, data.clone(), dict.to_vec()))
            } else {
                None
            }
        })
        .collect();

    for (_stm_id, data, dict) in obj_stms {
        let n = dict_get(&dict, "N").and_then(|v| v.as_int()).unwrap_or(0).clamp(0, 10000) as usize;
        let first = dict_get(&dict, "First").and_then(|v| v.as_int()).unwrap_or(0).clamp(0, data.len() as i64) as usize;
        if n == 0 || first >= data.len() {
            continue;
        }

        let mut lexer = Lexer::new(&data[..first]);
        let mut entries = Vec::new();
        for _ in 0..n {
            let id = lexer.read_token().and_then(|s| s.parse::<u32>().ok());
            let off = lexer.read_token().and_then(|s| s.parse::<usize>().ok());
            if let (Some(id), Some(off)) = (id, off) {
                entries.push((id, off));
            } else {
                break;
            }
        }

        for (id, off) in entries {
            let obj_pos = first + off;
            if obj_pos < data.len() {
                let mut obj_lexer = Lexer::new(&data);
                obj_lexer.pos = obj_pos;
                if let Some(val) = obj_lexer.parse_value(0) {
                    objects.entry(id).or_insert(PdfObj { val, stream_data: None });
                }
            }
        }
    }

    objects
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

// ---------------------------------------------------------------------------------------------
// CMap and Font Decoder
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct FontInfo {
    cmap: HashMap<u32, String>,
    bold: bool,
    italic: bool,
    mono: bool,
    is_cid: bool,
}

fn parse_to_unicode_cmap(bytes: &[u8]) -> HashMap<u32, String> {
    let mut cmap = HashMap::new();
    let text = String::from_utf8_lossy(bytes);
    let mut words = text.split_whitespace();

    while let Some(word) = words.next() {
        if word == "beginbfchar" {
            while let Some(src_token) = words.next() {
                if src_token == "endbfchar" {
                    break;
                }
                let Some(dst_token) = words.next() else { break };
                if let (Some(src), Some(dst)) = (parse_hex_code(src_token), parse_hex_string_chars(dst_token)) {
                    cmap.insert(src, dst);
                }
            }
        } else if word == "beginbfrange" {
            while let Some(start_token) = words.next() {
                if start_token == "endbfrange" {
                    break;
                }
                let Some(end_token) = words.next() else { break };
                let Some(dst_token) = words.next() else { break };

                if let (Some(start), Some(end)) = (parse_hex_code(start_token), parse_hex_code(end_token)) {
                    if dst_token.starts_with('[') {
                        // Array of destinations
                        let mut cur_dst = dst_token.trim_start_matches('[').to_string();
                        let mut cur_code = start;
                        while cur_code <= end {
                            if !cur_dst.is_empty()
                                && cur_dst != "]"
                                && let Some(dst_str) = parse_hex_string_chars(&cur_dst)
                            {
                                cmap.insert(cur_code, dst_str);
                            }
                            if cur_dst.ends_with(']') {
                                break;
                            }
                            let Some(next_tok) = words.next() else { break };
                            cur_dst = next_tok.to_string();
                            cur_code = cur_code.saturating_add(1);
                        }
                    } else if let Some(dst_start) = parse_hex_code(dst_token) {
                        let count = (end.saturating_sub(start) + 1).min(65536);
                        for idx in 0..count {
                            let src = start.saturating_add(idx);
                            let dst = dst_start.saturating_add(idx);
                            if let Some(ch) = char::from_u32(dst) {
                                cmap.insert(src, ch.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    cmap
}

fn parse_hex_code(s: &str) -> Option<u32> {
    let clean = s.trim_matches(['<', '>']);
    u32::from_str_radix(clean, 16).ok()
}

fn parse_hex_string_chars(s: &str) -> Option<String> {
    let clean = s.trim_matches(['<', '>']);
    if clean.len().is_multiple_of(4) {
        // UTF-16 code units
        let mut u16_units = Vec::new();
        let mut i = 0;
        while i < clean.len() {
            if let Ok(val) = u16::from_str_radix(&clean[i..i + 4], 16) {
                u16_units.push(val);
            }
            i += 4;
        }
        Some(String::from_utf16_lossy(&u16_units))
    } else if let Ok(val) = u32::from_str_radix(clean, 16) {
        char::from_u32(val).map(|c| c.to_string())
    } else {
        None
    }
}

fn extract_fonts(res_dict: &[(String, PdfValue)], objects: &HashMap<u32, PdfObj>) -> HashMap<String, FontInfo> {
    let mut fonts = HashMap::new();
    let Some(font_res) = dict_get(res_dict, "Font") else {
        return fonts;
    };

    let font_pairs = match font_res {
        PdfValue::Dict(d) => d.as_slice(),
        PdfValue::Ref(id, _) => objects.get(id).and_then(|o| o.val.as_dict()).unwrap_or(&[]),
        _ => &[],
    };

    for (k, v) in font_pairs {
        let fobj = match v {
            PdfValue::Ref(id, _) => objects.get(id),
            _ => None,
        };
        let Some(fdict) = fobj.and_then(|o| o.val.as_dict()) else {
            continue;
        };

        let base_font = dict_get(fdict, "BaseFont").and_then(|v| v.as_name()).unwrap_or("");
        let lower = base_font.to_ascii_lowercase();
        let bold = lower.contains("bold") || lower.contains("black") || lower.contains("heavy") || lower.contains("medium");
        let italic = lower.contains("italic") || lower.contains("oblique") || lower.contains("slanted");
        let mono = lower.contains("courier") || lower.contains("mono") || lower.contains("consolas");

        let subtype = dict_get(fdict, "Subtype").and_then(|v| v.as_name()).unwrap_or("");
        let is_cid = subtype == "Type0";

        let mut cmap = HashMap::new();
        if let Some(to_uni_ref) = dict_get(fdict, "ToUnicode").and_then(|v| v.as_ref())
            && let Some(cmap_obj) = objects.get(&to_uni_ref.0)
            && let Some(cmap_data) = &cmap_obj.stream_data
        {
            cmap = parse_to_unicode_cmap(cmap_data);
        }

        fonts.insert(k.clone(), FontInfo { cmap, bold, italic, mono, is_cid });
    }
    fonts
}

fn decode_pdf_string(raw: &[u8], font: Option<&FontInfo>) -> String {
    if let Some(f) = font
        && !f.cmap.is_empty()
    {
        let mut out = String::new();
        let max_code = f.cmap.keys().copied().max().unwrap_or(0);
        let is_2byte = f.is_cid || max_code > 255 || (raw.len() >= 2 && raw.len().is_multiple_of(2));

        if is_2byte {
            let mut i = 0;
            while i + 1 < raw.len() {
                let code = ((raw[i] as u32) << 8) | (raw[i + 1] as u32);
                if let Some(mapped) = f.cmap.get(&code) {
                    out.push_str(mapped);
                } else if let Some(ch) = char::from_u32(code) {
                    out.push(ch);
                }
                i += 2;
            }
            if !out.is_empty() {
                return out;
            }
        } else {
            for &b in raw {
                let code = b as u32;
                if let Some(mapped) = f.cmap.get(&code) {
                    out.push_str(mapped);
                } else {
                    out.push(cp1252(b));
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
    }

    // UTF-16BE BOM check
    if let Some(rest) = raw.strip_prefix(&[0xFE, 0xFF]) {
        let u16s: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect();
        return String::from_utf16_lossy(&u16s);
    }

    // UTF-8 check
    if let Ok(s) = std::str::from_utf8(raw)
        && s.chars().all(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
    {
        return s.to_string();
    }

    // Windows-1252 fallback
    raw.iter().map(|&b| cp1252(b)).collect()
}

// ---------------------------------------------------------------------------------------------
// Page Tree Resolution
// ---------------------------------------------------------------------------------------------

struct PageRef {
    content_streams: Vec<Vec<u8>>,
    res_dict: Vec<(String, PdfValue)>,
}

fn collect_pages(
    node: &PdfValue,
    objects: &HashMap<u32, PdfObj>,
    inherited_res: Option<&[(String, PdfValue)]>,
    out: &mut Vec<PageRef>,
    depth: usize,
) {
    if depth > 16 {
        return;
    }
    let Some(dict) = node.as_dict() else { return };
    let node_type = dict_get(dict, "Type").and_then(|v| v.as_name()).unwrap_or("");

    let res = dict_get(dict, "Resources")
        .and_then(|v| match v {
            PdfValue::Dict(d) => Some(d.as_slice()),
            PdfValue::Ref(id, _) => objects.get(id).and_then(|o| o.val.as_dict()),
            _ => None,
        })
        .or(inherited_res);

    if node_type == "Pages" {
        if let Some(kids) = dict_get(dict, "Kids").and_then(|v| v.as_array()) {
            for kid in kids {
                if let Some((kid_id, _)) = kid.as_ref()
                    && let Some(kid_obj) = objects.get(&kid_id)
                {
                    collect_pages(&kid_obj.val, objects, res, out, depth + 1);
                }
            }
        }
    } else if node_type == "Page" {
        let mut content_streams = Vec::new();
        if let Some(contents) = dict_get(dict, "Contents") {
            match contents {
                PdfValue::Ref(id, _) => {
                    if let Some(stream_obj) = objects.get(id)
                        && let Some(data) = &stream_obj.stream_data
                    {
                        content_streams.push(data.clone());
                    }
                }
                PdfValue::Array(arr) => {
                    for item in arr {
                        if let Some((id, _)) = item.as_ref()
                            && let Some(stream_obj) = objects.get(&id)
                            && let Some(data) = &stream_obj.stream_data
                        {
                            content_streams.push(data.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        out.push(PageRef { content_streams, res_dict: res.map(|r| r.to_vec()).unwrap_or_default() });
    }
}

// ---------------------------------------------------------------------------------------------
// Content Stream Text Extractor
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct TextChunk {
    x: f32,
    y: f32,
    w: f32,
    text: String,
    font_size: f32,
    bold: bool,
    italic: bool,
    mono: bool,
}

#[derive(Clone, Debug)]
enum ContentToken {
    Op(String),
    Num(f32),
    Str(Vec<u8>),
    Name(String),
    Array(Vec<ContentToken>),
}

fn tokenize_content_stream(bytes: &[u8]) -> Vec<ContentToken> {
    let mut tokens = Vec::new();
    let mut lexer = Lexer::new(bytes);

    while lexer.pos < lexer.bytes.len() {
        lexer.skip_whitespace_and_comments();
        if lexer.pos >= lexer.bytes.len() {
            break;
        }
        let b = lexer.bytes[lexer.pos];
        if b == b'(' {
            lexer.pos += 1;
            let s = lexer.read_literal_string();
            tokens.push(ContentToken::Str(s));
            continue;
        }
        if b == b'<' {
            if lexer.pos + 1 < lexer.bytes.len() && lexer.bytes[lexer.pos + 1] == b'<' {
                // Skip inline dictionaries
                lexer.pos += 2;
                while lexer.pos + 1 < lexer.bytes.len() && !(lexer.bytes[lexer.pos] == b'>' && lexer.bytes[lexer.pos + 1] == b'>') {
                    lexer.pos += 1;
                }
                if lexer.pos + 1 < lexer.bytes.len() {
                    lexer.pos += 2;
                }
                continue;
            }
            lexer.pos += 1;
            let s = lexer.read_hex_string();
            tokens.push(ContentToken::Str(s));
            continue;
        }
        if b == b'[' {
            lexer.pos += 1;
            let mut sub_tokens = Vec::new();
            while lexer.pos < lexer.bytes.len() {
                lexer.skip_whitespace_and_comments();
                if lexer.pos >= lexer.bytes.len() || lexer.bytes[lexer.pos] == b']' {
                    if lexer.pos < lexer.bytes.len() {
                        lexer.pos += 1;
                    }
                    break;
                }
                let cb = lexer.bytes[lexer.pos];
                if cb == b'(' {
                    lexer.pos += 1;
                    sub_tokens.push(ContentToken::Str(lexer.read_literal_string()));
                } else if cb == b'<' {
                    lexer.pos += 1;
                    sub_tokens.push(ContentToken::Str(lexer.read_hex_string()));
                } else if let Some(tok) = lexer.read_token()
                    && let Ok(f) = tok.parse::<f32>()
                {
                    sub_tokens.push(ContentToken::Num(f));
                }
            }
            tokens.push(ContentToken::Array(sub_tokens));
            continue;
        }
        if b == b'/' {
            lexer.pos += 1;
            let name = lexer.read_name();
            tokens.push(ContentToken::Name(name));
            continue;
        }

        if let Some(tok) = lexer.read_token() {
            if let Ok(f) = tok.parse::<f32>() {
                tokens.push(ContentToken::Num(f));
            } else {
                tokens.push(ContentToken::Op(tok));
            }
        }
    }
    tokens
}

fn extract_page_text_chunks(page: &PageRef, fonts: &HashMap<String, FontInfo>) -> Vec<TextChunk> {
    let mut chunks = Vec::new();
    let mut combined_bytes = Vec::new();
    for stm in &page.content_streams {
        combined_bytes.extend_from_slice(stm);
        combined_bytes.push(b'\n');
    }
    if combined_bytes.is_empty() {
        return chunks;
    }

    let tokens = tokenize_content_stream(&combined_bytes);
    let mut stack = Vec::new();

    let mut in_text = false;
    let mut cur_font_name = String::new();
    let mut cur_font_size = 12.0f32;
    let mut text_matrix = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut line_matrix = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut leading = 14.0f32;

    for tok in tokens {
        match tok {
            ContentToken::Op(op) => {
                match op.as_str() {
                    "BT" => {
                        in_text = true;
                        text_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
                        line_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
                    }
                    "ET" => {
                        in_text = false;
                    }
                    "Tf" => {
                        if stack.len() >= 2 {
                            if let ContentToken::Num(sz) = stack.remove(stack.len() - 1) {
                                cur_font_size = sz.abs().clamp(1.0, 500.0);
                            }
                            if let ContentToken::Name(nm) = stack.remove(stack.len() - 1) {
                                cur_font_name = nm;
                            }
                        }
                    }
                    "Tm" => {
                        if stack.len() >= 6 {
                            let f = pop_f32(&mut stack).unwrap_or(0.0);
                            let e = pop_f32(&mut stack).unwrap_or(0.0);
                            let d = pop_f32(&mut stack).unwrap_or(1.0);
                            let c = pop_f32(&mut stack).unwrap_or(0.0);
                            let b = pop_f32(&mut stack).unwrap_or(0.0);
                            let a = pop_f32(&mut stack).unwrap_or(1.0);
                            text_matrix = [a, b, c, d, e, f];
                            line_matrix = [a, b, c, d, e, f];
                        }
                    }
                    "Td" => {
                        if stack.len() >= 2 {
                            let ty = pop_f32(&mut stack).unwrap_or(0.0);
                            let tx = pop_f32(&mut stack).unwrap_or(0.0);
                            line_matrix[4] += tx * line_matrix[0] + ty * line_matrix[2];
                            line_matrix[5] += tx * line_matrix[1] + ty * line_matrix[3];
                            text_matrix = line_matrix;
                        }
                    }
                    "TD" => {
                        if stack.len() >= 2 {
                            let ty = pop_f32(&mut stack).unwrap_or(0.0);
                            let tx = pop_f32(&mut stack).unwrap_or(0.0);
                            leading = -ty;
                            line_matrix[4] += tx * line_matrix[0] + ty * line_matrix[2];
                            line_matrix[5] += tx * line_matrix[1] + ty * line_matrix[3];
                            text_matrix = line_matrix;
                        }
                    }
                    "T*" => {
                        line_matrix[4] += -leading * line_matrix[2];
                        line_matrix[5] += -leading * line_matrix[3];
                        text_matrix = line_matrix;
                    }
                    "TL" => {
                        if let Some(l) = pop_f32(&mut stack) {
                            leading = l;
                        }
                    }
                    "Tj" => {
                        if in_text && let Some(ContentToken::Str(raw)) = stack.pop() {
                            let font = fonts.get(&cur_font_name);
                            let text = decode_pdf_string(&raw, font);
                            if !text.is_empty() {
                                let w = (text.chars().count() as f32) * cur_font_size * 0.52;
                                chunks.push(TextChunk {
                                    x: text_matrix[4],
                                    y: text_matrix[5],
                                    w,
                                    text,
                                    font_size: cur_font_size,
                                    bold: font.map(|f| f.bold).unwrap_or(false),
                                    italic: font.map(|f| f.italic).unwrap_or(false),
                                    mono: font.map(|f| f.mono).unwrap_or(false),
                                });
                                text_matrix[4] += w;
                            }
                        }
                    }
                    "TJ" => {
                        if in_text && let Some(ContentToken::Array(items)) = stack.pop() {
                            let font = fonts.get(&cur_font_name);
                            for item in items {
                                match item {
                                    ContentToken::Str(raw) => {
                                        let text = decode_pdf_string(&raw, font);
                                        if !text.is_empty() {
                                            let w = (text.chars().count() as f32) * cur_font_size * 0.52;
                                            chunks.push(TextChunk {
                                                x: text_matrix[4],
                                                y: text_matrix[5],
                                                w,
                                                text,
                                                font_size: cur_font_size,
                                                bold: font.map(|f| f.bold).unwrap_or(false),
                                                italic: font.map(|f| f.italic).unwrap_or(false),
                                                mono: font.map(|f| f.mono).unwrap_or(false),
                                            });
                                            text_matrix[4] += w;
                                        }
                                    }
                                    ContentToken::Num(adj) => {
                                        let shift = -adj / 1000.0 * cur_font_size;
                                        text_matrix[4] += shift;
                                        if adj < -180.0
                                            && let Some(last) = chunks.last_mut()
                                            && !last.text.ends_with(' ')
                                        {
                                            last.text.push(' ');
                                            last.w += cur_font_size * 0.3;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    "'" => {
                        // T* then Tj
                        line_matrix[4] += -leading * line_matrix[2];
                        line_matrix[5] += -leading * line_matrix[3];
                        text_matrix = line_matrix;
                        if in_text && let Some(ContentToken::Str(raw)) = stack.pop() {
                            let font = fonts.get(&cur_font_name);
                            let text = decode_pdf_string(&raw, font);
                            if !text.is_empty() {
                                let w = (text.chars().count() as f32) * cur_font_size * 0.52;
                                chunks.push(TextChunk {
                                    x: text_matrix[4],
                                    y: text_matrix[5],
                                    w,
                                    text,
                                    font_size: cur_font_size,
                                    bold: font.map(|f| f.bold).unwrap_or(false),
                                    italic: font.map(|f| f.italic).unwrap_or(false),
                                    mono: font.map(|f| f.mono).unwrap_or(false),
                                });
                                text_matrix[4] += w;
                            }
                        }
                    }
                    _ => {
                        stack.clear();
                    }
                }
            }
            other => stack.push(other),
        }
    }
    chunks
}

fn pop_f32(stack: &mut Vec<ContentToken>) -> Option<f32> {
    match stack.pop() {
        Some(ContentToken::Num(f)) => Some(f),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Reflow Engine: Lines, Paragraphs, Headings, and Tables
// ---------------------------------------------------------------------------------------------

struct ReflowLine {
    baseline_y: f32,
    x_start: f32,
    x_end: f32,
    chunks: Vec<TextChunk>,
}

fn cluster_chunks_into_lines(mut chunks: Vec<TextChunk>) -> Vec<ReflowLine> {
    if chunks.is_empty() {
        return Vec::new();
    }
    // Sort primarily by y descending (top of page down to bottom)
    chunks.sort_by(|a, b| b.y.total_cmp(&a.y));

    let mut lines: Vec<ReflowLine> = Vec::new();

    for chunk in chunks {
        let mut placed = false;
        for line in &mut lines {
            let threshold = (chunk.font_size * 0.35).clamp(2.5, 8.0);
            if (chunk.y - line.baseline_y).abs() <= threshold {
                line.x_start = line.x_start.min(chunk.x);
                line.x_end = line.x_end.max(chunk.x + chunk.w);
                line.chunks.push(chunk.clone());
                placed = true;
                break;
            }
        }
        if !placed {
            lines.push(ReflowLine { baseline_y: chunk.y, x_start: chunk.x, x_end: chunk.x + chunk.w, chunks: vec![chunk] });
        }
    }

    // Sort lines top to bottom
    lines.sort_by(|a, b| b.baseline_y.total_cmp(&a.baseline_y));

    // Sort chunks within each line from left to right
    for line in &mut lines {
        line.chunks.sort_by(|a, b| a.x.total_cmp(&b.x));
    }

    lines
}

fn lines_to_flow_blocks(lines: Vec<ReflowLine>, is_first_page: bool) -> Vec<FBlock> {
    if lines.is_empty() {
        return Vec::new();
    }

    let mut blocks: Vec<FBlock> = Vec::new();
    let mut cur_para: Option<Para> = None;
    let mut prev_line_y: Option<f32> = None;
    let mut prev_line_height = 12.0f32;
    let mut is_page_start = !is_first_page;

    for line in lines {
        let line_height = line.chunks.iter().map(|c| c.font_size).fold(0.0f32, f32::max).max(10.0);
        let dy = prev_line_y.map(|py| py - line.baseline_y).unwrap_or(0.0);

        let mut starts_new_para = false;
        if cur_para.is_none() {
            starts_new_para = true;
        } else if prev_line_y.is_some() {
            if dy > line_height * 1.55 || dy < 0.0 || (line_height - prev_line_height).abs() >= 2.5 {
                starts_new_para = true;
            } else if let Some(first_chunk) = line.chunks.first() {
                let txt = first_chunk.text.trim_start();
                if txt.starts_with('•') || txt.starts_with('-') || txt.starts_with('*') || is_numbered_list_prefix(txt) {
                    starts_new_para = true;
                }
            }
        }

        if starts_new_para {
            if let Some(mut p) = cur_para.take() {
                finalize_para(&mut p);
                if !p.is_empty() {
                    blocks.push(FBlock::Para(p));
                }
            }
            let mut p = Para::default();
            if is_page_start {
                p.page_break = true;
                is_page_start = false;
            }
            cur_para = Some(p);
        }

        let p = cur_para.get_or_insert_with(Para::default);

        // Append line text chunks to the paragraph
        let mut last_chunk_end_x: Option<f32> = None;
        for chunk in line.chunks {
            if let Some(end_x) = last_chunk_end_x {
                let gap = chunk.x - end_x;
                if gap > 2.0 && !p.text().ends_with(' ') && !chunk.text.starts_with(' ') {
                    p.push_text(" ", &Fmt::default());
                }
            }
            last_chunk_end_x = Some(chunk.x + chunk.w);

            let fmt = Fmt {
                bold: chunk.bold,
                italic: chunk.italic,
                code: chunk.mono,
                size: Some(chunk.font_size),
                font: if chunk.mono { Some(model::MONO_FONT.into()) } else { None },
                ..Default::default()
            };
            p.push_text(&chunk.text, &fmt);
        }

        // Add line-continuation space if appropriate
        if !p.text().ends_with(' ') {
            p.push_text(" ", &Fmt::default());
        }

        prev_line_y = Some(line.baseline_y);
        prev_line_height = line_height;
    }

    if let Some(mut p) = cur_para {
        finalize_para(&mut p);
        if !p.is_empty() {
            blocks.push(FBlock::Para(p));
        }
    }

    blocks
}

fn is_numbered_list_prefix(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else { return false };
    if first.is_ascii_digit() {
        let mut saw_dot = false;
        for c in chars {
            if c == '.' || c == ')' {
                saw_dot = true;
                break;
            }
            if !c.is_ascii_digit() {
                break;
            }
        }
        return saw_dot;
    }
    false
}

fn finalize_para(p: &mut Para) {
    p.trim();
    let text = p.text();
    let trimmed = text.trim();

    // List detection
    if let Some(rest) =
        trimmed.strip_prefix('•').or_else(|| trimmed.strip_prefix('-')).or_else(|| trimmed.strip_prefix('*')).or_else(|| trimmed.strip_prefix("–"))
    {
        p.list = Some(ListInfo { ordered: false, level: 0 });
        strip_prefix_from_para(p, text.len() - rest.trim_start().len());
    } else if let Some(stripped_len) = get_numbered_list_len(trimmed) {
        p.list = Some(ListInfo { ordered: true, level: 0 });
        strip_prefix_from_para(p, stripped_len);
    }

    // Dominant font size and heading detection
    let max_size = p
        .inlines
        .iter()
        .filter_map(|i| match i {
            model::Inline::Text(_, f) => f.size,
            _ => None,
        })
        .fold(0.0f32, f32::max);

    let is_bold = p.inlines.iter().any(|i| match i {
        model::Inline::Text(_, f) => f.bold,
        _ => false,
    });

    if max_size >= 18.0 {
        p.kind = Kind::Heading(1);
    } else if max_size >= 14.0 {
        p.kind = Kind::Heading(2);
    } else if max_size >= 12.5 && is_bold {
        p.kind = Kind::Heading(3);
    } else {
        p.kind = Kind::Normal;
    }
}

fn get_numbered_list_len(s: &str) -> Option<usize> {
    let mut count = 0;
    let mut saw_dot = false;
    for c in s.chars() {
        count += c.len_utf8();
        if c == '.' || c == ')' {
            saw_dot = true;
            break;
        }
        if !c.is_ascii_digit() {
            return None;
        }
    }
    if saw_dot && count > 0 {
        let rest = &s[count..];
        let spaces = rest.len() - rest.trim_start().len();
        Some(count + spaces)
    } else {
        None
    }
}

fn strip_prefix_from_para(p: &mut Para, mut chars_to_strip: usize) {
    p.inlines.retain_mut(|inline| {
        if chars_to_strip == 0 {
            return true;
        }
        if let model::Inline::Text(t, _) = inline {
            if t.len() <= chars_to_strip {
                chars_to_strip -= t.len();
                false
            } else {
                t.replace_range(..chars_to_strip, "");
                chars_to_strip = 0;
                true
            }
        } else {
            true
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Metadata Extraction
// ---------------------------------------------------------------------------------------------

fn extract_pdf_metadata(objects: &HashMap<u32, PdfObj>) -> Meta {
    let mut meta = Meta::default();
    let info_dict = objects.values().find_map(|o| {
        let dict = o.val.as_dict()?;
        if dict_get(dict, "Title").is_some() || dict_get(dict, "Author").is_some() { Some(dict) } else { None }
    });

    if let Some(dict) = info_dict {
        if let Some(PdfValue::String(s)) = dict_get(dict, "Title") {
            meta.title = decode_pdf_string(s, None);
        }
        if let Some(PdfValue::String(s)) = dict_get(dict, "Author") {
            meta.author = decode_pdf_string(s, None);
        }
        if let Some(PdfValue::String(s)) = dict_get(dict, "Subject") {
            meta.subject = decode_pdf_string(s, None);
        }
        if let Some(PdfValue::String(s)) = dict_get(dict, "Keywords") {
            meta.keywords = decode_pdf_string(s, None);
        }
        if let Some(PdfValue::String(s)) = dict_get(dict, "CreationDate") {
            meta.created = decode_pdf_string(s, None);
        }
    }
    meta
}

// ---------------------------------------------------------------------------------------------
// Public Import API
// ---------------------------------------------------------------------------------------------

/// Import a PDF file's bytes into a WordCraft [`Document`].
pub fn import(bytes: &[u8]) -> Result<Document, String> {
    if bytes.len() < 5 {
        return Err("file is too short to be a PDF".into());
    }
    if !bytes.starts_with(b"%PDF-") && find_subsequence(&bytes[..bytes.len().min(1024)], b"%PDF-").is_none() {
        return Err("not a valid PDF document (missing %PDF- header)".into());
    }

    let objects = parse_pdf_objects(bytes);
    if objects.is_empty() {
        return Err("failed to parse PDF objects".into());
    }

    // Locate Catalog
    let catalog = objects.values().find(|o| o.val.as_dict().and_then(|d| dict_get(d, "Type")).and_then(|v| v.as_name()) == Some("Catalog"));

    let mut pages = Vec::new();
    if let Some(cat) = catalog
        && let Some(cat_dict) = cat.val.as_dict()
        && let Some(pages_ref) = dict_get(cat_dict, "Pages").and_then(|v| v.as_ref())
        && let Some(pages_obj) = objects.get(&pages_ref.0)
    {
        collect_pages(&pages_obj.val, &objects, None, &mut pages, 0);
    }

    // Fallback: collect all objects with /Type /Page directly if catalog resolution failed
    if pages.is_empty() {
        for obj in objects.values() {
            if let Some(dict) = obj.val.as_dict()
                && dict_get(dict, "Type").and_then(|v| v.as_name()) == Some("Page")
            {
                collect_pages(&obj.val, &objects, None, &mut pages, 0);
            }
        }
    }

    let mut all_blocks = Vec::new();
    for (page_idx, page) in pages.iter().enumerate() {
        let fonts = extract_fonts(&page.res_dict, &objects);
        let chunks = extract_page_text_chunks(page, &fonts);
        let lines = cluster_chunks_into_lines(chunks);
        let blocks = lines_to_flow_blocks(lines, page_idx == 0);
        all_blocks.extend(blocks);
    }

    let meta = extract_pdf_metadata(&objects);
    let flow = Flow { blocks: all_blocks, meta };

    let mut doc = model::to_doc(&flow);
    doc.ensure_nonempty();
    Ok(doc)
}
