//! Insert tab: pages, tables, pictures, shapes, links, bookmarks, headers/footers, text, symbols.

use serde_json::{Value, json};
use wordcraft_doc::para::{Float, InlineObject, ShapeKind, Wrap};
use wordcraft_doc::props::{
    Align, Border, BorderStyle, Borders, CellProps, CharProps, Rgb, RowProps, TabAlign, TabStop, TableLook, TableProps, TextColor, VAlign,
};
use wordcraft_doc::{Block, Cell, Paragraph, PartKind, Path, Pos, Row, StoryRef, Table, para_block};

use super::{delete_selection, sel_result, split_para, type_text};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("insert.pageBreak", "Page Break", "Insert › Pages", |s, _| {
            type_text(s, "\u{000C}")?;
            sel_result(s)
        }),
        CommandSpec::new("insert.blankPage", "Blank Page", "Insert › Pages", |s, _| {
            type_text(s, "\u{000C}")?;
            let at = s.sel.focus.clone();
            let at = split_para(s, &at)?;
            s.doc.insert_text(&at, "\u{000C}", &CharProps::default())?;
            s.sel = Selection::caret(at);
            sel_result(s)
        }),
        CommandSpec::new("insert.coverPage", "Cover Page", "Insert › Pages", cover_page).params(r#"{"title"?: string, "subtitle"?: string, "author"?: string}"#),
        CommandSpec::new("insert.table", "Table", "Insert › Tables", table).params(r#"{"rows": n, "cols": n, "style"?: string}"#),
        CommandSpec::new("insert.picture", "Pictures", "Insert › Illustrations", picture).params(r#"{"path"?: string, "data"?: base64, "width"?: pt, "alt"?: string}"#),
        CommandSpec::new("insert.shape", "Shapes", "Insert › Illustrations", shape)
            .params(r#"{"kind": "rectangle|roundedRectangle|ellipse|triangle|diamond|line|elbowConnector|curvedConnector|arrow|star|heart", "width"?: pt, "height"?: pt, "fill"?: "RRGGBB", "stroke"?: "RRGGBB"}"#),
        CommandSpec::new("insert.icon", "Icons", "Insert › Illustrations", icon)
            .params(r#"{"name"?: "arrow|star|heart|ellipse|diamond|triangle", "size"?: pt, "color"?: "RRGGBB"}"#),
        CommandSpec::new("insert.chart", "Chart", "Insert › Illustrations", chart)
            .params(r#"{"type"?: "bar|column|line|pie", "title"?: string, "width"?: pt, "height"?: pt}"#),
        CommandSpec::new("insert.screenshot", "Screenshot", "Insert › Illustrations", screenshot)
            .params(r#"{"path"?: string, "data"?: base64}"#),
        CommandSpec::new("insert.textBox", "Text Box", "Insert › Text", text_box)
            .params(r#"{"text"?: string, "width"?: pt, "height"?: pt}"#)
            .when(|s| {
                // As in Word: no text box inside a text box.
                matches!(s.sel.focus.story, StoryRef::Part(id) if s.doc.parts.get(&id).is_some_and(|p| p.kind == PartKind::TextBox))
                    .then_some("text boxes can't go inside a text box")
            }),
        CommandSpec::new("insert.link", "Link", "Insert › Links", link).key("Mod+K").params(r#"{"url": string, "text"?: string}"#),
        CommandSpec::new("insert.removeLink", "Remove Hyperlink", "Insert › Links", |s, _| super::format::apply(s, &|c| {
            c.link = None;
            if c.style.as_deref() == Some("Hyperlink") {
                c.style = None;
            }
        })),
        CommandSpec::new("insert.bookmark", "Bookmark", "Insert › Links", bookmark).params(r#"{"name": string}"#),
        CommandSpec::new("insert.header", "Header", "Insert › Header & Footer", |s, v| header_footer(s, v, true)).params(r#"{"text"?: string, "preset"?: "blank|blankThree|title"}"#),
        CommandSpec::new("insert.footer", "Footer", "Insert › Header & Footer", |s, v| header_footer(s, v, false)).params(r#"{"text"?: string, "preset"?: "blank|blankThree|pageNumber"}"#),
        CommandSpec::new("insert.pageNumber", "Page Number", "Insert › Header & Footer", page_number).params(r#"{"position"?: "top|bottom|current", "align"?: "left|center|right", "format"?: "x of y"}"#),
        CommandSpec::new("insert.editHeader", "Edit Header", "Insert › Header & Footer › Header", |s, _| edit_hf(s, true)).pure(),
        CommandSpec::new("insert.editFooter", "Edit Footer", "Insert › Header & Footer › Footer", |s, _| edit_hf(s, false)).pure(),
        CommandSpec::new("insert.closeHeader", "Close Header and Footer", "Header & Footer", |s, _| {
            s.sel = Selection::caret(s.doc.start_of(StoryRef::Body));
            sel_result(s)
        })
        .pure(),
        CommandSpec::new("insert.removeHeader", "Remove Header", "Insert › Header & Footer › Header", |s, _| {
            s.doc.last_section.headers = Default::default();
            for path in s.doc.para_paths(StoryRef::Body) {
                if s.doc.para(StoryRef::Body, &path).is_some_and(|p| p.section.is_some())
                    && let Ok(p) = s.doc.para_mut(StoryRef::Body, &path)
                    && let Some(sec) = p.section.as_mut()
                {
                    sec.headers = Default::default();
                }
            }
            sel_result(s)
        }),
        CommandSpec::new("insert.removeFooter", "Remove Footer", "Insert › Header & Footer › Footer", |s, _| {
            s.doc.last_section.footers = Default::default();
            sel_result(s)
        }),
        CommandSpec::new("insert.dateTime", "Date & Time", "Insert › Text", date_time).params(r#"{"format"?: "M/d/yyyy", "update"?: bool}"#),
        CommandSpec::new("insert.symbol", "Symbol", "Insert › Symbols", symbol).params(r#"{"char": string, "font"?: family (the symbol's own font)}"#),
        CommandSpec::new("insert.onlinePicture", "Online Pictures", "Insert › Illustrations", online_picture)
            .params(r#"{"url"?: string, "alt"?: string}"#),
        CommandSpec::new("insert.model3d", "3D Models", "Insert › Illustrations", model_3d)
            .params(r#"{"path"?: string, "alt"?: string}"#),
        CommandSpec::new("insert.smartArt", "SmartArt", "Insert › Illustrations", smart_art)
            .params(r#"{"layout"?: "process|cycle|hierarchy|matrix|list", "items"?: string[]}"#),
        CommandSpec::new("insert.canvas", "Drawing Canvas", "Insert › Illustrations", drawing_canvas)
            .params(r#"{"width"?: pt, "height"?: pt}"#),
        CommandSpec::new("insert.video", "Online Video", "Insert › Media", online_video)
            .params(r#"{"url"?: string, "width"?: pt, "height"?: pt}"#),
        CommandSpec::new("insert.inkEquation", "Ink Equation", "Insert › Symbols", ink_equation)
            .params(r#"{"latex"?: string}"#),
        CommandSpec::new("insert.field", "Field", "Insert › Text › Quick Parts", field).key("Mod+F9").params(r#"{"instr": string, "result"?: string}"#),
        CommandSpec::new("insert.dropCap", "Drop Cap", "Insert › Text", |s, v| {
            if v.as_object().is_none_or(|m| m.is_empty()) {
                s.ui_requests.push(json!({"open": "dropCap"}));
                let cur = s.doc.para_at(&s.sel.focus).and_then(|p| p.props.drop_cap).unwrap_or(0);
                return Ok(json!({"lines": cur}));
            }
            let lines = p::u64(v, "lines").unwrap_or(3).min(10) as u8;
            super::para::fmt(s, &|p| p.drop_cap = if lines == 0 { None } else { Some(lines) })
        })
        .params(r#"{"lines"?: n (0 = none)}"#),
        CommandSpec::new("insert.horizontalLine", "Horizontal Line", "Home › Paragraph › Borders", |s, _| {
            super::para::fmt(s, &|p| {
                p.borders = Some(wordcraft_doc::props::Borders { bottom: Some(wordcraft_doc::props::Border::single(1.5)), ..Default::default() })
            })
        }),
        CommandSpec::new("insert.wordArt", "WordArt", "Insert › Text", |s, v| {
            let text = p::str(v, "text").unwrap_or("Your text here").to_string();
            let props = CharProps { size: Some(36.0), bold: Some(true), color: Some(TextColor::Rgb(Rgb(0x15, 0x60, 0x82))), outline: Some(false), ..Default::default() };
            let at = delete_selection(s)?;
            let end = s.doc.insert_text(&at, &text, &props)?;
            s.sel = Selection { anchor: at, focus: end };
            sel_result(s)
        }),
        CommandSpec::new("insert.textFromFile", "Text from File", "Insert › Text › Object", |s, v| {
            let path = p::req_str(v, "path")?;
            let doc = crate::io::open_path(std::path::Path::new(path)).map_err(CmdError::Failed)?;
            let frag = doc.body_fragment();
            let at = delete_selection(s)?;
            let end = s.doc.insert_fragment(&at, &frag)?;
            s.sel = Selection::caret(end);
            sel_result(s)
        })
        .params(r#"{"path": string}"#),
    ]
}

/// Formats a linear or LaTeX mathematical string into Unicode mathematical typography.
pub fn format_math_to_unicode(src: &str) -> String {
    let mut s = src.to_string();

    let greeks = [
        (r"\alpha", "α"),
        (r"\beta", "β"),
        (r"\gamma", "γ"),
        (r"\delta", "δ"),
        (r"\epsilon", "ε"),
        (r"\zeta", "ζ"),
        (r"\eta", "η"),
        (r"\theta", "θ"),
        (r"\iota", "ι"),
        (r"\kappa", "κ"),
        (r"\lambda", "λ"),
        (r"\mu", "μ"),
        (r"\nu", "ν"),
        (r"\xi", "ξ"),
        (r"\pi", "π"),
        (r"\rho", "ρ"),
        (r"\sigma", "σ"),
        (r"\tau", "τ"),
        (r"\phi", "φ"),
        (r"\chi", "χ"),
        (r"\psi", "ψ"),
        (r"\omega", "ω"),
        (r"\Gamma", "Γ"),
        (r"\Delta", "Δ"),
        (r"\Theta", "Θ"),
        (r"\Lambda", "Λ"),
        (r"\Xi", "Ξ"),
        (r"\Pi", "Π"),
        (r"\Sigma", "Σ"),
        (r"\Phi", "Φ"),
        (r"\Psi", "Ψ"),
        (r"\Omega", "Ω"),
    ];
    for (k, v) in greeks {
        s = s.replace(k, v);
    }

    let ops = [
        (r"\pm", "±"),
        (r"\mp", "∓"),
        (r"\times", "×"),
        (r"\div", "÷"),
        (r"\cdot", "·"),
        (r"\leq", "≤"),
        (r"\le", "≤"),
        (r"\geq", "≥"),
        (r"\ge", "≥"),
        (r"\neq", "≠"),
        (r"\ne", "≠"),
        (r"\approx", "≈"),
        (r"\equiv", "≡"),
        (r"\infty", "∞"),
        (r"\partial", "∂"),
        (r"\nabla", "∇"),
        (r"\int", "∫"),
        (r"\sum", "∑"),
        (r"\prod", "∏"),
        (r"\in", "∈"),
        (r"\notin", "∉"),
        (r"\subset", "⊂"),
        (r"\subseteq", "⊆"),
        (r"\cup", "∪"),
        (r"\cap", "∩"),
        (r"\to", "→"),
        (r"\leftarrow", "←"),
        (r"\Rightarrow", "⇒"),
        (r"\Leftarrow", "⇐"),
        (r"\leftrightarrow", "↔"),
        (r"\forall", "∀"),
        (r"\exists", "∃"),
        (r"\degree", "°"),
    ];
    for (k, v) in ops {
        s = s.replace(k, v);
    }

    let sups = [
        ("^0", "⁰"),
        ("^1", "¹"),
        ("^2", "²"),
        ("^3", "³"),
        ("^4", "⁴"),
        ("^5", "⁵"),
        ("^6", "⁶"),
        ("^7", "⁷"),
        ("^8", "⁸"),
        ("^9", "⁹"),
        ("^+", "⁺"),
        ("^-", "⁻"),
        ("^n", "ⁿ"),
        ("^x", "ˣ"),
        ("^y", "ʸ"),
        ("^{0}", "⁰"),
        ("^{1}", "¹"),
        ("^{2}", "²"),
        ("^{3}", "³"),
        ("^{4}", "⁴"),
        ("^{5}", "⁵"),
        ("^{6}", "⁶"),
        ("^{7}", "⁷"),
        ("^{8}", "⁸"),
        ("^{9}", "⁹"),
        ("^{+}", "⁺"),
        ("^{-}", "⁻"),
        ("^{n}", "ⁿ"),
        ("^{x}", "ˣ"),
    ];
    for (k, v) in sups {
        s = s.replace(k, v);
    }

    let subs = [
        ("_0", "₀"),
        ("_1", "₁"),
        ("_2", "₂"),
        ("_3", "₃"),
        ("_4", "₄"),
        ("_5", "₅"),
        ("_6", "₆"),
        ("_7", "₇"),
        ("_8", "₈"),
        ("_9", "₉"),
        ("_+", "₊"),
        ("_-", "₋"),
        ("_i", "ᵢ"),
        ("_j", "ⱼ"),
        ("_k", "ₖ"),
        ("_n", "ₙ"),
        ("_m", "ₘ"),
        ("_x", "ₓ"),
        ("_y", "ᵧ"),
        ("_{0}", "₀"),
        ("_{1}", "₁"),
        ("_{2}", "₂"),
        ("_{3}", "₃"),
        ("_{4}", "₄"),
        ("_{5}", "₅"),
        ("_{6}", "₆"),
        ("_{7}", "₇"),
        ("_{8}", "₈"),
        ("_{9}", "₉"),
        ("_{i}", "ᵢ"),
        ("_{j}", "ⱼ"),
        ("_{k}", "ₖ"),
        ("_{n}", "ₙ"),
    ];
    for (k, v) in subs {
        s = s.replace(k, v);
    }

    while let Some(frac_idx) = s.find(r"\frac") {
        if let Some(open1) = s[frac_idx..].find('{') {
            let o1 = frac_idx + open1;
            if let Some(close1) = s[o1..].find('}') {
                let c1 = o1 + close1;
                let num = &s[o1 + 1..c1];
                if let Some(open2) = s[c1..].find('{') {
                    let o2 = c1 + open2;
                    if let Some(close2) = s[o2..].find('}') {
                        let c2 = o2 + close2;
                        let den = &s[o2 + 1..c2];
                        let repl = format!("({num})/({den})");
                        s.replace_range(frac_idx..=c2, &repl);
                        continue;
                    }
                }
            }
        }
        break;
    }

    while let Some(sqrt_idx) = s.find(r"\sqrt") {
        let after_sqrt = &s[sqrt_idx + 5..];
        let trimmed = after_sqrt.trim_start();
        if trimmed.starts_with('{') {
            let open_offset = (sqrt_idx + 5) + (after_sqrt.len() - trimmed.len());
            if let Some(close_offset) = s[open_offset..].find('}') {
                let c = open_offset + close_offset;
                let inner = &s[open_offset + 1..c];
                let repl = format!("√({inner})");
                s.replace_range(sqrt_idx..=c, &repl);
                continue;
            }
        }
        s.replace_range(sqrt_idx..sqrt_idx + 5, "√");
    }

    s
}

fn online_picture(s: &mut Session, v: &Value) -> CmdResult {
    let url = p::str(v, "url").unwrap_or("");
    let alt = p::str(v, "alt").unwrap_or("Online Picture");
    picture(s, &json!({"path": url, "alt": alt}))
}

fn model_3d(s: &mut Session, v: &Value) -> CmdResult {
    let path = p::str(v, "path").unwrap_or("model.glb");
    let alt = p::str(v, "alt").unwrap_or("3D Model");
    picture(s, &json!({"path": path, "alt": alt}))
}

fn smart_art(s: &mut Session, v: &Value) -> CmdResult {
    let layout = p::str(v, "layout").unwrap_or("process");
    let items_arg = v.get("items").and_then(Value::as_array);

    let t = match layout {
        "cycle" => {
            let mut tbl = Table {
                props: TableProps {
                    borders: Some(Borders::default()),
                    align: Some(Align::Center),
                    look: TableLook {
                        header_row: false,
                        total_row: false,
                        banded_rows: false,
                        first_column: false,
                        last_column: false,
                        banded_columns: false,
                    },
                    ..Default::default()
                },
                grid: vec![130.0, 45.0, 130.0],
                rows: Vec::new(),
            };

            let corner_cell = |title: &str, sub: &str, bg: Rgb, border: Rgb| -> Cell {
                Cell {
                    props: CellProps {
                        span: 1,
                        width: Some(130.0),
                        shading: Some(bg),
                        borders: Some(Borders::box_(Border { style: BorderStyle::Single, width: 1.0, color: Some(border), space: 0.0 })),
                        margins: Some([8.0, 10.0, 8.0, 10.0]),
                        valign: VAlign::Center,
                        ..Default::default()
                    },
                    blocks: vec![
                        para_block({
                            let mut p = Paragraph::with_text(
                                title,
                                CharProps {
                                    bold: Some(true),
                                    size: Some(11.0),
                                    color: Some(TextColor::Rgb(Rgb(255, 255, 255))),
                                    ..Default::default()
                                },
                            );
                            p.props.align = Some(Align::Center);
                            p
                        }),
                        para_block({
                            let mut p = Paragraph::with_text(
                                sub,
                                CharProps { size: Some(9.5), color: Some(TextColor::Rgb(Rgb(240, 240, 240))), ..Default::default() },
                            );
                            p.props.align = Some(Align::Center);
                            p
                        }),
                    ],
                }
            };

            let sym_cell = |sym: &str, sz: f32, col: Rgb, w: f32| -> Cell {
                Cell {
                    props: CellProps { span: 1, width: Some(w), margins: Some([4.0, 2.0, 4.0, 2.0]), valign: VAlign::Center, ..Default::default() },
                    blocks: vec![para_block({
                        let mut p = Paragraph::with_text(
                            sym,
                            CharProps { bold: Some(true), size: Some(sz), color: Some(TextColor::Rgb(col)), ..Default::default() },
                        );
                        p.props.align = Some(Align::Center);
                        p
                    })],
                }
            };

            let r0 = Row {
                props: RowProps::default(),
                cells: vec![
                    corner_cell("1. Plan", "Requirements & Scope", Rgb(0x2B, 0x57, 0x9A), Rgb(0x1F, 0x38, 0x64)),
                    sym_cell("➔", 16.0, Rgb(0x59, 0x59, 0x59), 45.0),
                    corner_cell("2. Build", "Design & Prototype", Rgb(0x41, 0x71, 0x9C), Rgb(0x2E, 0x4E, 0x6D)),
                ],
            };
            let r1 = Row {
                props: RowProps::default(),
                cells: vec![
                    sym_cell("↑", 16.0, Rgb(0x59, 0x59, 0x59), 130.0),
                    sym_cell("↺", 24.0, Rgb(0x2B, 0x57, 0x9A), 45.0),
                    sym_cell("↓", 16.0, Rgb(0x59, 0x59, 0x59), 130.0),
                ],
            };
            let r2 = Row {
                props: RowProps::default(),
                cells: vec![
                    corner_cell("4. Deploy", "Launch & Monitor", Rgb(0x10, 0x7C, 0x41), Rgb(0x0A, 0x4E, 0x29)),
                    sym_cell("←", 16.0, Rgb(0x59, 0x59, 0x59), 45.0),
                    corner_cell("3. Test", "Verification & QA", Rgb(0x70, 0xAD, 0x47), Rgb(0x50, 0x7E, 0x32)),
                ],
            };
            tbl.rows = vec![r0, r1, r2];
            tbl
        }
        "hierarchy" => {
            let mut tbl = Table {
                props: TableProps {
                    borders: Some(Borders::default()),
                    align: Some(Align::Center),
                    look: TableLook {
                        header_row: false,
                        total_row: false,
                        banded_rows: false,
                        first_column: false,
                        last_column: false,
                        banded_columns: false,
                    },
                    ..Default::default()
                },
                grid: vec![125.0, 125.0, 125.0],
                rows: Vec::new(),
            };

            let node_cell = |title: &str, sub: &str, bg: Rgb, border: Rgb| -> Cell {
                Cell {
                    props: CellProps {
                        span: 1,
                        width: Some(125.0),
                        shading: Some(bg),
                        borders: Some(Borders::box_(Border { style: BorderStyle::Single, width: 1.0, color: Some(border), space: 0.0 })),
                        margins: Some([8.0, 10.0, 8.0, 10.0]),
                        valign: VAlign::Center,
                        ..Default::default()
                    },
                    blocks: vec![
                        para_block({
                            let mut p = Paragraph::with_text(
                                title,
                                CharProps {
                                    bold: Some(true),
                                    size: Some(11.0),
                                    color: Some(TextColor::Rgb(Rgb(255, 255, 255))),
                                    ..Default::default()
                                },
                            );
                            p.props.align = Some(Align::Center);
                            p
                        }),
                        para_block({
                            let mut p = Paragraph::with_text(
                                sub,
                                CharProps { size: Some(9.5), color: Some(TextColor::Rgb(Rgb(240, 240, 240))), ..Default::default() },
                            );
                            p.props.align = Some(Align::Center);
                            p
                        }),
                    ],
                }
            };
            let empty_cell =
                || Cell { props: CellProps { span: 1, width: Some(125.0), ..Default::default() }, blocks: vec![para_block(Paragraph::new())] };
            let sym_cell = |sym: &str| Cell {
                props: CellProps { span: 1, width: Some(125.0), valign: VAlign::Center, ..Default::default() },
                blocks: vec![para_block({
                    let mut p = Paragraph::with_text(
                        sym,
                        CharProps { bold: Some(true), size: Some(14.0), color: Some(TextColor::Rgb(Rgb(0x7F, 0x7F, 0x7F))), ..Default::default() },
                    );
                    p.props.align = Some(Align::Center);
                    p
                })],
            };

            let r0 = Row {
                props: RowProps::default(),
                cells: vec![empty_cell(), node_cell("Leadership", "Executive Director", Rgb(0x1B, 0x36, 0x5D), Rgb(0x10, 0x24, 0x3E)), empty_cell()],
            };
            let r1 = Row { props: RowProps::default(), cells: vec![sym_cell("│"), sym_cell("┬"), sym_cell("│")] };
            let r2 = Row {
                props: RowProps::default(),
                cells: vec![
                    node_cell("Engineering", "Product & Infra", Rgb(0x2E, 0x75, 0xB6), Rgb(0x1F, 0x4E, 0x79)),
                    node_cell("Operations", "Strategy & Scale", Rgb(0x2E, 0x75, 0xB6), Rgb(0x1F, 0x4E, 0x79)),
                    node_cell("Marketing", "Community & Growth", Rgb(0x2E, 0x75, 0xB6), Rgb(0x1F, 0x4E, 0x79)),
                ],
            };
            tbl.rows = vec![r0, r1, r2];
            tbl
        }
        "matrix" => {
            let mut tbl = Table {
                props: TableProps {
                    borders: Some(Borders::default()),
                    align: Some(Align::Center),
                    look: TableLook {
                        header_row: false,
                        total_row: false,
                        banded_rows: false,
                        first_column: false,
                        last_column: false,
                        banded_columns: false,
                    },
                    ..Default::default()
                },
                grid: vec![185.0, 185.0],
                rows: Vec::new(),
            };

            let quad_cell = |title: &str, bullets: &[&str], bg: Rgb, border: Rgb, text_col: Rgb| -> Cell {
                let mut blocks = vec![para_block({
                    let mut p = Paragraph::with_text(
                        title,
                        CharProps { bold: Some(true), size: Some(11.0), color: Some(TextColor::Rgb(text_col)), ..Default::default() },
                    );
                    p.props.align = Some(Align::Left);
                    p
                })];
                for b in bullets {
                    blocks.push(para_block({
                        let mut p = Paragraph::with_text(
                            &format!("• {b}"),
                            CharProps { size: Some(9.5), color: Some(TextColor::Rgb(Rgb(0x33, 0x33, 0x33))), ..Default::default() },
                        );
                        p.props.align = Some(Align::Left);
                        p
                    }));
                }
                Cell {
                    props: CellProps {
                        span: 1,
                        width: Some(185.0),
                        shading: Some(bg),
                        borders: Some(Borders::box_(Border { style: BorderStyle::Single, width: 1.0, color: Some(border), space: 0.0 })),
                        margins: Some([10.0, 12.0, 10.0, 12.0]),
                        valign: VAlign::Top,
                        ..Default::default()
                    },
                    blocks,
                }
            };

            let r0 = Row {
                props: RowProps::default(),
                cells: vec![
                    quad_cell(
                        "QUADRANT I: Urgent & Important",
                        &["Critical client issues", "Production deadlines"],
                        Rgb(0xDE, 0xEB, 0xF7),
                        Rgb(0x8E, 0xA9, 0xDB),
                        Rgb(0x1F, 0x49, 0x7D),
                    ),
                    quad_cell(
                        "QUADRANT II: Important, Not Urgent",
                        &["Strategic planning", "Capability development"],
                        Rgb(0xE2, 0xEF, 0xDA),
                        Rgb(0xA9, 0xD0, 0x8E),
                        Rgb(0x38, 0x57, 0x23),
                    ),
                ],
            };
            let r1 = Row {
                props: RowProps::default(),
                cells: vec![
                    quad_cell(
                        "QUADRANT III: Urgent, Not Important",
                        &["Frequent interruptions", "Routine reporting"],
                        Rgb(0xFF, 0xF2, 0xCC),
                        Rgb(0xFF, 0xD9, 0x66),
                        Rgb(0x7F, 0x60, 0x00),
                    ),
                    quad_cell(
                        "QUADRANT IV: Low Value",
                        &["Busy activities", "Outdated workflows"],
                        Rgb(0xFC, 0xE4, 0xD6),
                        Rgb(0xF4, 0xB0, 0x84),
                        Rgb(0xC6, 0x59, 0x11),
                    ),
                ],
            };
            tbl.rows = vec![r0, r1];
            tbl
        }
        "list" => {
            let mut tbl = Table {
                props: TableProps {
                    borders: Some(Borders::default()),
                    align: Some(Align::Center),
                    look: TableLook {
                        header_row: false,
                        total_row: false,
                        banded_rows: false,
                        first_column: false,
                        last_column: false,
                        banded_columns: false,
                    },
                    ..Default::default()
                },
                grid: vec![45.0, 320.0],
                rows: Vec::new(),
            };

            let items = [
                ("01", "Research & Feasibility", "Market survey, architecture exploration, and technical spikes.", Rgb(0x2B, 0x57, 0x9A)),
                ("02", "Core Implementation", "Feature engineering, unit tests, automated CI verification.", Rgb(0x1F, 0x4E, 0x79)),
                ("03", "Release & Monitoring", "Packaging, distribution, feedback loops, and metrics.", Rgb(0x10, 0x7C, 0x41)),
            ];

            for (num, title, desc, col) in items {
                let badge = Cell {
                    props: CellProps {
                        span: 1,
                        width: Some(45.0),
                        shading: Some(col),
                        borders: Some(Borders::box_(Border { style: BorderStyle::Single, width: 1.0, color: Some(col), space: 0.0 })),
                        margins: Some([10.0, 6.0, 10.0, 6.0]),
                        valign: VAlign::Center,
                        ..Default::default()
                    },
                    blocks: vec![para_block({
                        let mut p = Paragraph::with_text(
                            num,
                            CharProps { bold: Some(true), size: Some(13.0), color: Some(TextColor::Rgb(Rgb(255, 255, 255))), ..Default::default() },
                        );
                        p.props.align = Some(Align::Center);
                        p
                    })],
                };
                let content = Cell {
                    props: CellProps {
                        span: 1,
                        width: Some(320.0),
                        shading: Some(Rgb(0xF7, 0xF9, 0xFA)),
                        borders: Some(Borders::box_(Border {
                            style: BorderStyle::Single,
                            width: 0.5,
                            color: Some(Rgb(0xDC, 0xDE, 0xE0)),
                            space: 0.0,
                        })),
                        margins: Some([8.0, 12.0, 8.0, 12.0]),
                        valign: VAlign::Center,
                        ..Default::default()
                    },
                    blocks: vec![
                        para_block({
                            let mut p = Paragraph::with_text(
                                title,
                                CharProps {
                                    bold: Some(true),
                                    size: Some(10.5),
                                    color: Some(TextColor::Rgb(Rgb(0x1F, 0x2A, 0x37))),
                                    ..Default::default()
                                },
                            );
                            p.props.align = Some(Align::Left);
                            p
                        }),
                        para_block({
                            let mut p = Paragraph::with_text(
                                desc,
                                CharProps { size: Some(9.0), color: Some(TextColor::Rgb(Rgb(0x4B, 0x55, 0x63))), ..Default::default() },
                            );
                            p.props.align = Some(Align::Left);
                            p
                        }),
                    ],
                };
                tbl.rows.push(Row { props: RowProps::default(), cells: vec![badge, content] });
            }
            tbl
        }
        _ => {
            // "process" default
            let mut tbl = Table {
                props: TableProps {
                    borders: Some(Borders::default()),
                    align: Some(Align::Center),
                    look: TableLook {
                        header_row: false,
                        total_row: false,
                        banded_rows: false,
                        first_column: false,
                        last_column: false,
                        banded_columns: false,
                    },
                    ..Default::default()
                },
                grid: vec![110.0, 30.0, 110.0, 30.0, 110.0],
                rows: Vec::new(),
            };

            let step_cell = |title: &str, sub: &str, bg: Rgb, border: Rgb| -> Cell {
                Cell {
                    props: CellProps {
                        span: 1,
                        width: Some(110.0),
                        shading: Some(bg),
                        borders: Some(Borders::box_(Border { style: BorderStyle::Single, width: 1.0, color: Some(border), space: 0.0 })),
                        margins: Some([8.0, 8.0, 8.0, 8.0]),
                        valign: VAlign::Center,
                        ..Default::default()
                    },
                    blocks: vec![
                        para_block({
                            let mut p = Paragraph::with_text(
                                title,
                                CharProps {
                                    bold: Some(true),
                                    size: Some(11.0),
                                    color: Some(TextColor::Rgb(Rgb(255, 255, 255))),
                                    ..Default::default()
                                },
                            );
                            p.props.align = Some(Align::Center);
                            p
                        }),
                        para_block({
                            let mut p = Paragraph::with_text(
                                sub,
                                CharProps { size: Some(9.5), color: Some(TextColor::Rgb(Rgb(240, 240, 240))), ..Default::default() },
                            );
                            p.props.align = Some(Align::Center);
                            p
                        }),
                    ],
                }
            };
            let arrow_cell = || Cell {
                props: CellProps { span: 1, width: Some(30.0), valign: VAlign::Center, ..Default::default() },
                blocks: vec![para_block({
                    let mut p = Paragraph::with_text(
                        "➔",
                        CharProps { bold: Some(true), size: Some(16.0), color: Some(TextColor::Rgb(Rgb(0x59, 0x59, 0x59))), ..Default::default() },
                    );
                    p.props.align = Some(Align::Center);
                    p
                })],
            };

            let s1 = items_arg.and_then(|a| a.first()).and_then(Value::as_str).unwrap_or("1. Research");
            let s2 = items_arg.and_then(|a| a.get(1)).and_then(Value::as_str).unwrap_or("2. Design");
            let s3 = items_arg.and_then(|a| a.get(2)).and_then(Value::as_str).unwrap_or("3. Deploy");

            let r = Row {
                props: RowProps::default(),
                cells: vec![
                    step_cell(s1, "Requirements & Scope", Rgb(0x2B, 0x57, 0x9A), Rgb(0x1F, 0x38, 0x64)),
                    arrow_cell(),
                    step_cell(s2, "Architecture & Proto", Rgb(0x1F, 0x4E, 0x79), Rgb(0x15, 0x33, 0x4F)),
                    arrow_cell(),
                    step_cell(s3, "Launch & Verification", Rgb(0x10, 0x7C, 0x41), Rgb(0x0A, 0x4E, 0x29)),
                ],
            };
            tbl.rows = vec![r];
            tbl
        }
    };

    let story = s.sel.focus.story;
    let path = insert_block_at_caret(s, Block::Table(t))?;
    let mut first = path.0.clone();
    first.extend([0, 0, 0]);
    s.sel = Selection::caret(Pos { story, path: Path(first), off: 0 });
    Ok(json!({"smartArt": layout, "table": path.0}))
}

fn drawing_canvas(s: &mut Session, v: &Value) -> CmdResult {
    let w = p::f32(v, "width").unwrap_or(400.0).clamp(10.0, 2000.0);
    let h = p::f32(v, "height").unwrap_or(250.0).clamp(10.0, 2000.0);
    let fill = p::str(v, "fill").and_then(Rgb::parse).or(Some(Rgb(0xFA, 0xFA, 0xFA)));
    let stroke = p::str(v, "stroke").and_then(Rgb::parse).or(Some(Rgb(0xCC, 0xCC, 0xCC)));
    let props = s.typing_props();
    let at = delete_selection(s)?;
    let bg_shape = InlineObject::Shape {
        kind: ShapeKind::Rectangle,
        w,
        h,
        fill,
        stroke,
        stroke_width: 1.0,
        float: Float::default(),
        story: None,
        freeform: None,
        effects: Default::default(),
    };
    let obj = InlineObject::Group {
        w,
        h,
        float: Float { wrap: Wrap::TopAndBottom, ..Default::default() },
        ch_w: w,
        ch_h: h,
        children: vec![wordcraft_doc::para::GroupChild { x: 0.0, y: 0.0, obj: bg_shape }],
        is_canvas: true,
    };
    let end = s.doc.insert_object(&at, obj, &props)?;
    s.sel = Selection { anchor: at, focus: end };
    sel_result(s)
}

fn online_video(s: &mut Session, v: &Value) -> CmdResult {
    let w = p::f32(v, "width").unwrap_or(360.0);
    let h = p::f32(v, "height").unwrap_or(200.0);
    shape(s, &json!({"kind": "rectangle", "width": w, "height": h, "fill": "1A1A1A"}))
}

fn ink_equation(s: &mut Session, v: &Value) -> CmdResult {
    let lin = p::str(v, "latex").or_else(|| p::str(v, "linear")).unwrap_or(r"\int x dx");
    s.run("insert.equation", &json!({"latex": lin, "edit": false}))
}

/// Insert a block after the caret's paragraph (splitting it if the caret is mid-paragraph).
pub(crate) fn insert_block_at_caret(s: &mut Session, block: Block) -> Result<Path, CmdError> {
    let at = delete_selection(s)?;
    let len = s.doc.para_at(&at).map(|p| p.len()).unwrap_or(0);
    let insert_at = if at.off == 0 {
        at.path.clone()
    } else {
        let new = s.doc.split_paragraph(&at)?;
        if at.off >= len { at.path.with_last(at.path.last() + 1) } else { new.path }
    };
    s.doc.insert_block(at.story, &insert_at, block)?;
    // Word keeps a paragraph after a table.
    let next = insert_at.with_last(insert_at.last() + 1);
    if s.doc.block(at.story, &next).is_none() {
        s.doc.insert_block(at.story, &next, Block::Para(Paragraph::new()))?;
    }
    // An empty paragraph we inserted before? Remove if the table took its place at the start.
    Ok(insert_at)
}

fn table(s: &mut Session, v: &Value) -> CmdResult {
    let rows = p::u64(v, "rows").unwrap_or(2).clamp(1, 1000) as usize;
    let cols = p::u64(v, "cols").unwrap_or(2).clamp(1, 63) as usize;
    // Inside a table cell the new table fits the cell, as in Word.
    let width = super::table::room_at(s, &s.sel.focus.path);
    let mut t = Table::new(rows, cols, width);
    if let Some(st) = p::str(v, "style") {
        let id = s.doc.styles.find(st).map(|x| x.id.clone()).ok_or_else(|| CmdError::Params(format!("no table style `{st}`")))?;
        t.props.style = Some(id);
    }
    let story = s.sel.focus.story;
    let path = insert_block_at_caret(s, Block::Table(t))?;
    let mut first = path.0.clone();
    first.extend([0, 0, 0]);
    s.sel = Selection::caret(Pos { story, path: Path(first), off: 0 });
    Ok(json!({"table": path.0}))
}

/// Minimal base64 (standard alphabet, padding optional, whitespace ignored).
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut n = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        } as u32;
        buf = (buf << 6) | v;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((buf >> n) as u8);
            buf &= (1 << n) - 1;
        }
    }
    Some(out)
}

pub fn base64_encode(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    for ch in b.chunks(3) {
        let n =
            (ch.first().copied().unwrap_or(0) as u32) << 16 | (ch.get(1).copied().unwrap_or(0) as u32) << 8 | ch.get(2).copied().unwrap_or(0) as u32;
        for i in 0..4 {
            if i <= ch.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub(crate) fn picture(s: &mut Session, v: &Value) -> CmdResult {
    let bytes = if let Some(d) = p::str(v, "data") {
        base64_decode(d).ok_or_else(|| CmdError::Params("bad base64 data".into()))?
    } else if let Some(path) = p::str(v, "path") {
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::fs::read(path).map_err(|e| CmdError::Failed(format!("{path}: {e}")))?
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = path;
            return Err(CmdError::Failed("reading files by path isn't available on the web".into()));
        }
    } else {
        s.ui_requests.push(json!({"open": "insertPicture"}));
        return sel_result(s);
    };
    if bytes.len() > 200 << 20 {
        return Err(CmdError::Failed("image is larger than 200 MB".into()));
    }
    let (pw, ph) =
        wordcraft_render::image_size(&bytes).ok_or_else(|| CmdError::Failed("not a supported image (PNG, JPEG, GIF, WebP, BMP)".into()))?;
    let ext = match bytes.get(..4) {
        Some([0x89, b'P', b'N', b'G']) => "png",
        Some([0xFF, 0xD8, ..]) => "jpeg",
        Some([b'G', b'I', b'F', _]) => "gif",
        Some([b'R', b'I', b'F', b'F']) => "webp",
        Some([b'B', b'M', ..]) => "bmp",
        _ => "png",
    };
    let key = s.doc.add_media(bytes, ext);
    // Natural size at 96 ppi, fitted to the text width.
    let max_w = s.doc.sections().first().map(|(_, sp)| sp.text_width()).unwrap_or(468.0);
    let (mut w, mut h) = (pw as f32 * 0.75, ph as f32 * 0.75);
    if let Some(want) = p::f32(v, "width") {
        let k = want.max(4.0) / w.max(1.0);
        w *= k;
        h *= k;
    }
    if w > max_w {
        h *= max_w / w;
        w = max_w;
    }
    let props = s.typing_props();
    let at = delete_selection(s)?;
    let obj = InlineObject::Image {
        media: key.clone(),
        w,
        h,
        alt: p::str(v, "alt").unwrap_or("").to_string(),
        float: Float::default(),
        crop: [0.0; 4],
        ole: None,
    };
    let end = s.doc.insert_object(&at, obj, &props)?;
    s.sel = Selection { anchor: at, focus: end };
    Ok(json!({"media": key, "width": w, "height": h}))
}

pub(crate) fn shape(s: &mut Session, v: &Value) -> CmdResult {
    let kind: ShapeKind =
        serde_json::from_value(v.get("kind").cloned().unwrap_or(json!("rectangle"))).map_err(|e| CmdError::Params(e.to_string()))?;
    let w = p::f32(v, "width").unwrap_or(108.0).clamp(4.0, 2000.0);
    let is_line = matches!(kind, ShapeKind::Line | ShapeKind::ElbowConnector | ShapeKind::CurvedConnector);
    let h = p::f32(v, "height").unwrap_or(if is_line { 36.0 } else { 72.0 }).clamp(1.0, 2000.0);
    let fill = match p::str(v, "fill") {
        Some(c) => Rgb::parse(c),
        None if is_line => None,
        None => Some(Rgb(0x15, 0x60, 0x82)),
    };
    let stroke = p::str(v, "stroke").and_then(Rgb::parse).or(Some(Rgb(0x0E, 0x40, 0x5A)));
    let props = s.typing_props();
    let at = delete_selection(s)?;
    let obj = InlineObject::Shape {
        kind,
        w,
        h,
        fill,
        stroke,
        stroke_width: 1.0,
        float: Float { wrap: Wrap::Inline, ..Default::default() },
        story: None,
        freeform: None,
        effects: Default::default(),
    };
    let end = s.doc.insert_object(&at, obj, &props)?;
    s.sel = Selection { anchor: at, focus: end };
    sel_result(s)
}

fn icon(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::str(v, "name").unwrap_or("star");
    let kind = match name.to_lowercase().as_str() {
        "arrow" => ShapeKind::Arrow,
        "heart" => ShapeKind::Heart,
        "ellipse" | "circle" => ShapeKind::Ellipse,
        "diamond" => ShapeKind::Diamond,
        "triangle" => ShapeKind::Triangle,
        _ => ShapeKind::Star,
    };
    let sz = p::f32(v, "size").unwrap_or(24.0).clamp(8.0, 500.0);
    let fill = p::str(v, "color").and_then(Rgb::parse).or(Some(Rgb(0x15, 0x60, 0x82)));
    let props = s.typing_props();
    let at = delete_selection(s)?;
    let obj = InlineObject::Shape {
        kind,
        w: sz,
        h: sz,
        fill,
        stroke: None,
        stroke_width: 0.0,
        float: Float { wrap: Wrap::Inline, ..Default::default() },
        story: None,
        freeform: None,
        effects: Default::default(),
    };
    let end = s.doc.insert_object(&at, obj, &props)?;
    s.sel = Selection { anchor: at, focus: end };
    sel_result(s)
}

fn chart(s: &mut Session, v: &Value) -> CmdResult {
    let chart_type = p::str(v, "type").unwrap_or("bar").to_lowercase();
    let title = p::str(v, "title").unwrap_or("Sales Summary");
    let w = p::f32(v, "width").unwrap_or(288.0).clamp(72.0, 1000.0);
    let h = p::f32(v, "height").unwrap_or(180.0).clamp(72.0, 1000.0);

    let default_cats = ["Q1", "Q2", "Q3", "Q4"];
    let default_vals = [24.0, 48.0, 36.0, 60.0];

    let cats: Vec<String> = v
        .get("categories")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .filter(|v: &Vec<String>| !v.is_empty())
        .unwrap_or_else(|| default_cats.iter().map(|s| s.to_string()).collect());

    let vals: Vec<f64> = v
        .get("values")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .filter(|v: &Vec<f64>| !v.is_empty())
        .unwrap_or_else(|| default_vals.to_vec());

    let max_val = vals.iter().copied().fold(1.0f64, f64::max);
    let total: f64 = vals.iter().sum::<f64>().max(1.0);

    let mut blocks = Vec::new();

    // Chart title
    let title_para = Paragraph::with_text(title, CharProps { bold: Some(true), size: Some(13.0), ..Default::default() });
    blocks.push(para_block(title_para));

    match chart_type.as_str() {
        "pie" => {
            let symbols = ["●", "■", "▲", "◆", "★", "▼"];
            for (i, (cat, val)) in cats.iter().zip(vals.iter()).enumerate() {
                let pct = (val / total * 100.0).round() as u64;
                let sym = symbols.get(i % symbols.len()).copied().unwrap_or("●");
                let line = format!("{} {} : {:.0} ({}%)", sym, cat, val, pct);
                blocks.push(para_block(Paragraph::with_text(&line, CharProps { size: Some(10.5), ..Default::default() })));
            }
        }
        "line" => {
            for (cat, val) in cats.iter().zip(vals.iter()) {
                let ratio = (val / max_val).clamp(0.0, 1.0);
                let bars_len = (ratio * 16.0).round() as usize;
                let line = format!("{:<6} ╶{}● {:.0}", cat, "─".repeat(bars_len), val);
                blocks.push(para_block(Paragraph::with_text(&line, CharProps { size: Some(10.5), ..Default::default() })));
            }
        }
        _ => {
            // "bar" / "column"
            for (cat, val) in cats.iter().zip(vals.iter()) {
                let ratio = (val / max_val).clamp(0.0, 1.0);
                let bars_len = (ratio * 16.0).round() as usize;
                let line = format!("{:<6} {} {:.0}", cat, "█".repeat(bars_len.max(1)), val);
                blocks.push(para_block(Paragraph::with_text(&line, CharProps { size: Some(10.5), ..Default::default() })));
            }
        }
    }

    let id = s.doc.add_part(PartKind::TextBox, blocks);
    let props = s.typing_props();
    let at = delete_selection(s)?;
    let obj = InlineObject::Shape {
        kind: ShapeKind::TextBox,
        w,
        h,
        fill: Some(Rgb(0xFB, 0xFB, 0xFD)),
        stroke: Some(Rgb(0x4A, 0x6E, 0xA9)),
        stroke_width: 1.5,
        float: Float::default(),
        story: Some(id),
        freeform: None,
        effects: Default::default(),
    };
    let end = s.doc.insert_object(&at, obj, &props)?;
    s.sel = Selection { anchor: at, focus: end };
    Ok(json!({"chart": chart_type, "title": title, "width": w, "height": h, "categories": cats, "values": vals}))
}

fn screenshot(s: &mut Session, v: &Value) -> CmdResult {
    if v.get("data").is_some() || v.get("path").is_some() {
        picture(s, v)
    } else {
        s.ui_requests.push(json!({"open": "screenshot"}));
        sel_result(s)
    }
}

fn text_box(s: &mut Session, v: &Value) -> CmdResult {
    let text = p::str(v, "text").unwrap_or("").to_string();
    let w = p::f32(v, "width").unwrap_or(144.0).clamp(18.0, 2000.0);
    let h = p::f32(v, "height").unwrap_or(72.0).clamp(18.0, 2000.0);
    let id = s.doc.add_part(PartKind::TextBox, vec![para_block(Paragraph::with_text(&text, CharProps::default()))]);
    let props = s.typing_props();
    let at = delete_selection(s)?;
    let obj = InlineObject::Shape {
        kind: ShapeKind::TextBox,
        w,
        h,
        fill: Some(Rgb::WHITE),
        stroke: Some(Rgb::BLACK),
        stroke_width: 0.75,
        float: Float::default(),
        story: Some(id),
        freeform: None,
        effects: Default::default(),
    };
    s.doc.insert_object(&at, obj, &props)?;
    // Like Word, type straight into the new box.
    s.sel = Selection::caret(s.doc.end_of(StoryRef::Part(id)));
    Ok(json!({"story": id}))
}

fn link(s: &mut Session, v: &Value) -> CmdResult {
    let Some(url) = p::str(v, "url").map(str::to_string) else {
        s.ui_requests.push(json!({"open": "link"}));
        return sel_result(s);
    };
    if url.is_empty() || url.len() > 4096 {
        return Err(CmdError::Params("bad url".into()));
    }
    let style = Some("Hyperlink".to_string());
    if s.sel.is_collapsed() {
        let text = p::str(v, "text").unwrap_or(&url).to_string();
        let mut props = s.typing_props();
        props.link = Some(url.clone());
        props.style = style;
        let at = s.sel.focus.clone();
        let end = s.doc.insert_text(&at, &text, &props)?;
        s.sel = Selection::caret(end);
        s.pending = Some(CharProps::default());
    } else {
        let (a, b) = s.sel.ordered();
        s.doc.format_range(&a, &b, &|c| {
            c.link = Some(url.clone());
            c.style = style.clone();
        })?;
    }
    sel_result(s)
}

fn bookmark(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "name")?.trim().to_string();
    if name.is_empty() || name.contains(' ') || name.len() > 40 || name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err(CmdError::Params("bookmark names start with a letter, have no spaces and at most 40 characters".into()));
    }
    // Replace an existing bookmark of the same name.
    for (_, pos) in s.doc.bookmarks().into_iter().filter(|(n, _)| *n == name).rev() {
        let para = s.doc.para_mut(pos.story, &pos.path)?;
        para.delete(pos.off, pos.off + wordcraft_doc::para::OBJ.len_utf8())?;
        let offs: Vec<usize> = para.object_offsets();
        if let Some(e) = offs.into_iter().find(|o| matches!(para.object_at(*o), Some(InlineObject::BookmarkEnd { name: n }) if *n == name)) {
            para.delete(e, e + wordcraft_doc::para::OBJ.len_utf8())?;
        }
    }
    let (a, b) = s.sel.ordered();
    let props = CharProps::default();
    let end = s.doc.insert_object(&b, InlineObject::BookmarkEnd { name: name.clone() }, &props)?;
    s.doc.insert_object(&a, InlineObject::BookmarkStart { name: name.clone() }, &props)?;
    let _ = end;
    sel_result(s)
}

fn hf_part(s: &mut Session, header: bool) -> u32 {
    let block = s.sel.focus.path.0.first().copied().unwrap_or(0) as usize;
    let sect = s.doc.section_mut(block).clone();
    let existing = if header { sect.headers.default } else { sect.footers.default };
    if let Some(id) = existing.filter(|id| s.doc.parts.contains_key(id)) {
        return id;
    }
    let style = if header { "Header" } else { "Footer" };
    let id = s.doc.add_part(if header { PartKind::Header } else { PartKind::Footer }, vec![para_block(Paragraph::new().styled(style))]);
    let sect = s.doc.section_mut(block);
    if header {
        sect.headers.default = Some(id);
    } else {
        sect.footers.default = Some(id);
    }
    id
}

fn header_footer(s: &mut Session, v: &Value, header: bool) -> CmdResult {
    let id = hf_part(s, header);
    let style = if header { "Header" } else { "Footer" };
    if let Some(text) = p::str(v, "text") {
        let blocks = text.split('\n').map(|l| para_block(Paragraph::with_text(l, CharProps::default()).styled(style))).collect();
        s.doc.set_story(StoryRef::Part(id), blocks)?;
    } else if let Some(preset) = p::str(v, "preset") {
        let mut para = Paragraph::new().styled(style);
        match preset {
            "blankThree" => {
                para.insert_text(0, "[Type here]\t[Type here]\t[Type here]", &CharProps::default())?;
            }
            "title" => {
                para.insert_text(0, if s.doc.core.title.is_empty() { "[Document title]" } else { &s.doc.core.title }, &CharProps::default())?;
                para.props.align = Some(Align::Center);
            }
            "pageNumber" => {
                para.props.align = Some(Align::Center);
                para.insert_object(0, InlineObject::Field { instr: "PAGE".into(), result: "1".into(), locked: false }, &CharProps::default())?;
            }
            _ => {
                para.insert_text(0, "[Type here]", &CharProps::default())?;
            }
        }
        s.doc.set_story(StoryRef::Part(id), vec![para_block(para)])?;
    }
    s.sel = Selection::caret(s.doc.start_of(StoryRef::Part(id)));
    Ok(json!({"story": id}))
}

fn edit_hf(s: &mut Session, header: bool) -> CmdResult {
    let id = hf_part(s, header);
    s.touch();
    s.sel = Selection::caret(s.doc.end_of(StoryRef::Part(id)));
    Ok(json!({"story": id}))
}

fn page_number(s: &mut Session, v: &Value) -> CmdResult {
    let position = p::str(v, "position").unwrap_or("bottom");
    let align = match p::str(v, "align").unwrap_or("center") {
        "left" => Align::Left,
        "right" => Align::Right,
        _ => Align::Center,
    };
    let x_of_y = p::str(v, "format").is_some_and(|f| f.contains("of"));
    let field = InlineObject::Field { instr: "PAGE".into(), result: "1".into(), locked: false };
    if position == "current" {
        let props = s.typing_props();
        let at = delete_selection(s)?;
        let end = s.doc.insert_object(&at, field, &props)?;
        s.sel = Selection::caret(end);
        return sel_result(s);
    }
    let header = position == "top";
    let id = hf_part(s, header);
    let mut para = Paragraph::new().styled(if header { "Header" } else { "Footer" });
    para.props.align = Some(align);
    para.props.tabs = Some(vec![TabStop { pos: 0.0, align: TabAlign::Clear, leader: Default::default() }]);
    if x_of_y {
        para.insert_text(0, "Page  of ", &CharProps::default())?;
        para.insert_object(5, field, &CharProps { bold: Some(true), ..Default::default() })?;
        let end = para.len();
        para.insert_object(
            end,
            InlineObject::Field { instr: "NUMPAGES".into(), result: "1".into(), locked: false },
            &CharProps { bold: Some(true), ..Default::default() },
        )?;
    } else {
        para.insert_object(0, field, &CharProps::default())?;
    }
    s.doc.set_story(StoryRef::Part(id), vec![para_block(para)])?;
    Ok(json!({"story": id}))
}

fn date_time(s: &mut Session, v: &Value) -> CmdResult {
    let fmt = p::str(v, "format").unwrap_or("M/d/yyyy");
    let text = format_date(fmt);
    if p::bool(v, "update").unwrap_or(false) {
        let props = s.typing_props();
        let at = delete_selection(s)?;
        let end = s.doc.insert_object(&at, InlineObject::Field { instr: format!("DATE \\@ \"{fmt}\""), result: text, locked: false }, &props)?;
        s.sel = Selection::caret(end);
    } else {
        type_text(s, &text)?;
    }
    sel_result(s)
}

/// Format today's date with a Word picture (`M/d/yyyy`, `MMMM d, yyyy`, `yyyy-MM-dd`, `dddd`…).
pub fn format_date(fmt: &str) -> String {
    let iso = super::now_iso();
    let y: i64 = iso.get(0..4).and_then(|x| x.parse().ok()).unwrap_or(2026);
    let m: usize = iso.get(5..7).and_then(|x| x.parse().ok()).unwrap_or(1);
    let d: u32 = iso.get(8..10).and_then(|x| x.parse().ok()).unwrap_or(1);
    let hh: u32 = iso.get(11..13).and_then(|x| x.parse().ok()).unwrap_or(0);
    let mm: u32 = iso.get(14..16).and_then(|x| x.parse().ok()).unwrap_or(0);
    let ss: u32 = iso.get(17..19).and_then(|x| x.parse().ok()).unwrap_or(0);
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    const DAYS: [&str; 7] = ["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"];
    // Days since the epoch, for the weekday.
    let days_since = (super::now_unix() / 86_400) as usize;
    let month = MONTHS.get(m.saturating_sub(1)).copied().unwrap_or("January");
    let day = DAYS.get(days_since % 7).copied().unwrap_or("Monday");
    let mut out = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars.get(i).copied().unwrap_or(' ');
        // `am/pm` (any case) is one token: AM or PM.
        if chars.get(i..i + 5).is_some_and(|w| w.iter().collect::<String>().eq_ignore_ascii_case("am/pm")) {
            out.push_str(if hh < 12 { "AM" } else { "PM" });
            i += 5;
            continue;
        }
        let mut n = 1;
        while chars.get(i + n) == Some(&c) {
            n += 1;
        }
        match (c, n) {
            ('y', 4..) => out.push_str(&format!("{y:04}")),
            ('y', _) => out.push_str(&format!("{:02}", y % 100)),
            ('M', 4..) => out.push_str(month),
            ('M', 3) => out.push_str(month.get(..3).unwrap_or(month)),
            ('M', 2) => out.push_str(&format!("{m:02}")),
            ('M', 1) => out.push_str(&m.to_string()),
            ('d', 4..) => out.push_str(day),
            ('d', 3) => out.push_str(day.get(..3).unwrap_or(day)),
            ('d', 2) => out.push_str(&format!("{d:02}")),
            ('d', 1) => out.push_str(&d.to_string()),
            ('H', 2..) => out.push_str(&format!("{hh:02}")),
            ('H', _) => out.push_str(&hh.to_string()),
            ('h', _) => out.push_str(&(if hh.is_multiple_of(12) { 12 } else { hh % 12 }).to_string()),
            ('m', 2..) => out.push_str(&format!("{mm:02}")),
            ('m', _) => out.push_str(&mm.to_string()),
            ('s', 2..) => out.push_str(&format!("{ss:02}")),
            ('s', _) => out.push_str(&ss.to_string()),
            ('a' | 'A', _) => out.push_str(if hh < 12 { "AM" } else { "PM" }),
            _ => {
                for _ in 0..n {
                    out.push(c);
                }
            }
        }
        i += n;
    }
    out
}

/// Insert › Symbol: the character at the caret, in its own font when `font` names one (the rest of
/// the text keeps the caret's font).
fn symbol(s: &mut Session, v: &Value) -> CmdResult {
    let c = p::req_str(v, "char")?;
    if c.is_empty() || c.chars().count() > 64 {
        return Err(CmdError::Params("char: 1–64 characters".into()));
    }
    let font = p::str(v, "font").map(str::trim).filter(|f| !f.is_empty());
    let Some(font) = font else {
        type_text(s, c)?;
        return sel_result(s);
    };
    if font.len() > 256 {
        return Err(CmdError::Params("font: a family name".into()));
    }
    let base = s.typing_props();
    let mut props = base.clone();
    props.font = Some(font.to_string());
    s.pending = Some(props);
    let r = type_text(s, c);
    // Typing on after the symbol goes on in the text's own font.
    s.pending = Some(base);
    r?;
    sel_result(s)
}

fn field(s: &mut Session, v: &Value) -> CmdResult {
    let instr = p::req_str(v, "instr")?.trim().to_string();
    if instr.is_empty() || instr.len() > 4096 {
        return Err(CmdError::Params("bad field code".into()));
    }
    let result = p::str(v, "result").unwrap_or("").to_string();
    let props = s.typing_props();
    let at = delete_selection(s)?;
    let end = s.doc.insert_object(&at, InlineObject::Field { instr, result, locked: false }, &props)?;
    s.sel = Selection::caret(end);
    let _ = super::references::update_fields(s);
    sel_result(s)
}

fn cover_page(s: &mut Session, v: &Value) -> CmdResult {
    let title = p::str(v, "title").unwrap_or("Document Title").to_string();
    let subtitle = p::str(v, "subtitle").unwrap_or("Document subtitle").to_string();
    let author = p::str(v, "author").unwrap_or(&s.author).to_string();
    let accent = Rgb(0x15, 0x60, 0x82);
    let mut blocks = Vec::new();
    for _ in 0..8 {
        blocks.push(Block::Para(Paragraph::new()));
    }
    let mut t =
        Paragraph::with_text(&title, CharProps { size: Some(44.0), color: Some(TextColor::Rgb(accent)), ..Default::default() }).styled("Title");
    t.props.borders = Some(wordcraft_doc::props::Borders {
        bottom: Some(wordcraft_doc::props::Border { style: wordcraft_doc::props::BorderStyle::Single, width: 2.0, color: Some(accent), space: 6.0 }),
        ..Default::default()
    });
    blocks.push(Block::Para(t));
    blocks.push(Block::Para(Paragraph::with_text(&subtitle, CharProps::default()).styled("Subtitle")));
    for _ in 0..14 {
        blocks.push(Block::Para(Paragraph::new()));
    }
    blocks
        .push(Block::Para(Paragraph::with_text(&author, CharProps { bold: Some(true), color: Some(TextColor::Rgb(accent)), ..Default::default() })));
    let mut date = Paragraph::with_text(&format_date("MMMM d, yyyy"), CharProps::default());
    date.insert_text(date.len(), "\u{000C}", &CharProps::default())?;
    blocks.push(Block::Para(date));
    let frag = wordcraft_doc::edit::Fragment { blocks, ..Default::default() };
    let start = s.doc.start_of(StoryRef::Body);
    let at = s.doc.split_paragraph(&start)?;
    let _ = at;
    s.doc.insert_fragment(&start, &frag)?;
    s.sel = Selection::caret(s.doc.start_of(StoryRef::Body));
    sel_result(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        let data = b"hello world!?";
        assert_eq!(base64_decode(&base64_encode(data)).unwrap(), data);
        assert!(base64_decode("!!!").is_none());
    }

    /// Insert › Symbol with a font: only the symbol takes it, the caret's font carries on after.
    #[test]
    fn symbol_takes_its_own_font() {
        let mut s = Session::new(wordcraft_doc::Document::from_text("ab"));
        s.run("caret.docEnd", &json!({})).unwrap();
        s.run("insert.symbol", &json!({"char": "\u{2665}", "font": "DejaVu Sans"})).unwrap();
        s.run("text.insert", &json!({"text": "c"})).unwrap();
        let p = s.doc.para(StoryRef::Body, &Path::top(0)).unwrap();
        assert_eq!(p.text, "ab\u{2665}c");
        assert_eq!(p.props_of_char(2).font.as_deref(), Some("DejaVu Sans"), "the symbol");
        assert_eq!(p.props_of_char(5).font, None, "typing after it (byte offsets)");
        assert!(s.run("insert.symbol", &json!({"char": ""})).is_err());
    }

    #[test]
    fn date_pictures() {
        let d = format_date("yyyy-MM-dd");
        assert_eq!(d.len(), 10);
        assert!(format_date("MMMM d, yyyy").contains(", "));
        let t = format_date("h:mm:ss am/pm");
        assert!(t.ends_with(" AM") || t.ends_with(" PM"), "{t}");
        assert_eq!(t.matches(':').count(), 2, "{t}");
        use super::super::references::date_picture;
        assert_eq!(date_picture(r#"DATE \@ "MMMM d, yyyy" \* MERGEFORMAT"#).as_deref(), Some("MMMM d, yyyy"));
        assert_eq!(date_picture(r#"TIME \@ HH:mm"#).as_deref(), Some("HH:mm"));
        assert_eq!(date_picture("DATE"), None);
    }

    #[test]
    fn smart_art_diagrams() {
        use wordcraft_doc::Document;
        let mut s = Session::new(Document::new());
        for layout in ["process", "cycle", "hierarchy", "matrix", "list"] {
            let res = s.run("insert.smartArt", &json!({"layout": layout})).unwrap();
            assert_eq!(res["smartArt"].as_str().unwrap(), layout);
            assert!(res["table"].as_array().is_some());
        }
    }

    #[test]
    fn drop_cap_presets() {
        use wordcraft_doc::Document;
        let mut s = Session::new(Document::from_text("This is a paragraph with drop cap."));
        s.run("insert.dropCap", &json!({"lines": 3})).unwrap();
        let path = s.sel.focus.path.clone();
        let p = s.doc.para(StoryRef::Body, &path).unwrap();
        assert_eq!(p.props.drop_cap, Some(3));

        s.run("insert.dropCap", &json!({"lines": 0})).unwrap();
        let p = s.doc.para(StoryRef::Body, &path).unwrap();
        assert_eq!(p.props.drop_cap, None);
    }

    #[test]
    fn equation_math_formatting() {
        assert_eq!(format_math_to_unicode(r"\alpha+\beta=\gamma"), "α+β=γ");
        assert_eq!(format_math_to_unicode(r"x^2+y_0=z"), "x²+y₀=z");
        assert_eq!(format_math_to_unicode(r"\sqrt{x}\leq\infty"), "√(x)≤∞");
        assert_eq!(format_math_to_unicode(r"\frac{a}{b}"), "(a)/(b)");
    }

    #[test]
    fn chart_generation_with_data() {
        use wordcraft_doc::Document;
        let mut s = Session::new(Document::new());
        let res = s
            .run(
                "insert.chart",
                &json!({
                    "type": "bar",
                    "title": "Quarterly Growth",
                    "categories": ["Q1", "Q2", "Q3", "Q4"],
                    "values": [25.0, 50.0, 75.0, 100.0]
                }),
            )
            .unwrap();
        assert_eq!(res["chart"], "bar");
        assert_eq!(res["title"], "Quarterly Growth");
        assert_eq!(res["categories"].as_array().unwrap().len(), 4);
    }
}
