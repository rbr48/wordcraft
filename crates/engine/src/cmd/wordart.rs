//! WordArt presets and styled typography drawing objects (Insert › Text › WordArt).

use serde_json::{Value, json};
use wordcraft_doc::effects::{Glow, Shadow, ShapeEffects};
use wordcraft_doc::para::{Float, InlineObject, Paragraph, ShapeKind, Wrap};
use wordcraft_doc::props::{CharProps, Rgb, TextColor};
use wordcraft_doc::{PartKind, StoryRef, para_block};

use super::delete_selection;
use crate::{CmdResult, Selection, Session, p};

/// A WordArt style preset definition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WordArtStyle {
    pub id: &'static str,
    pub name: &'static str,
    pub font_name: &'static str,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub color: Rgb,
    pub outline: bool,
    pub shadow_char: bool,
    pub shadow_shape: Option<Shadow>,
    pub glow: Option<Glow>,
}

impl WordArtStyle {
    /// Character properties for text inside this WordArt object.
    pub fn char_props(&self) -> CharProps {
        CharProps {
            font: Some(self.font_name.to_string()),
            size: Some(self.font_size),
            bold: Some(self.bold),
            italic: Some(self.italic),
            color: Some(TextColor::Rgb(self.color)),
            outline: if self.outline { Some(true) } else { None },
            shadow: if self.shadow_char { Some(true) } else { None },
            ..Default::default()
        }
    }

    /// Shape effects (shadow, glow) on the WordArt frame.
    pub fn shape_effects(&self) -> ShapeEffects {
        ShapeEffects { shadow: self.shadow_shape, glow: self.glow, soft_edge: None }
    }
}

/// The 15 standard Microsoft Word WordArt preset styles.
pub const WORDART_STYLES: [WordArtStyle; 15] = [
    // 1: Fill - Black, Text 1; Shadow
    WordArtStyle {
        id: "style1",
        name: "Fill: Black; Shadow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0x26, 0x26, 0x26),
        outline: false,
        shadow_char: true,
        shadow_shape: Some(Shadow { color: Rgb(0x7F, 0x7F, 0x7F), transparency: 60.0, blur: 4.0, distance: 3.0, angle: 45.0, rot_with_shape: false }),
        glow: None,
    },
    // 2: Fill - Blue, Accent 1; Shadow
    WordArtStyle {
        id: "style2",
        name: "Fill: Blue, Accent 1; Shadow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0x1F, 0x4E, 0x79),
        outline: false,
        shadow_char: false,
        shadow_shape: Some(Shadow { color: Rgb(0x5B, 0x9B, 0xD5), transparency: 40.0, blur: 5.0, distance: 4.0, angle: 45.0, rot_with_shape: false }),
        glow: None,
    },
    // 3: Fill - Orange, Accent 2; Outline - Accent 2
    WordArtStyle {
        id: "style3",
        name: "Fill: Orange, Accent 2; Outline",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0xED, 0x7D, 0x31),
        outline: true,
        shadow_char: false,
        shadow_shape: Some(Shadow { color: Rgb(0x8C, 0x3B, 0x00), transparency: 65.0, blur: 3.0, distance: 2.0, angle: 60.0, rot_with_shape: false }),
        glow: None,
    },
    // 4: Fill - White; Outline - Blue, Accent 1; Shadow
    WordArtStyle {
        id: "style4",
        name: "Fill: White; Outline: Blue; Shadow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0xFF, 0xFF, 0xFF),
        outline: true,
        shadow_char: false,
        shadow_shape: Some(Shadow { color: Rgb(0x1F, 0x4E, 0x79), transparency: 50.0, blur: 6.0, distance: 4.0, angle: 45.0, rot_with_shape: false }),
        glow: None,
    },
    // 5: Fill - Gold, Accent 4; Soft Bevel / Shadow
    WordArtStyle {
        id: "style5",
        name: "Fill: Gold, Accent 4; Soft Shadow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0xFF, 0xC0, 0x00),
        outline: false,
        shadow_char: true,
        shadow_shape: Some(Shadow { color: Rgb(0xBF, 0x90, 0x00), transparency: 50.0, blur: 4.0, distance: 3.0, angle: 90.0, rot_with_shape: false }),
        glow: None,
    },
    // 6: Fill - Teal, Accent 3; Glow
    WordArtStyle {
        id: "style6",
        name: "Fill: Teal, Accent 3; Glow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0x00, 0xA8, 0x8F),
        outline: false,
        shadow_char: false,
        shadow_shape: None,
        glow: Some(Glow { color: Rgb(0x33, 0xCC, 0xB8), size: 8.0, transparency: 40.0 }),
    },
    // 7: Fill - Red, Crimson; Deep Shadow
    WordArtStyle {
        id: "style7",
        name: "Fill: Red, Crimson; Deep Shadow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0xC0, 0x00, 0x00),
        outline: false,
        shadow_char: false,
        shadow_shape: Some(Shadow { color: Rgb(0x60, 0x00, 0x00), transparency: 55.0, blur: 4.0, distance: 3.0, angle: 45.0, rot_with_shape: false }),
        glow: None,
    },
    // 8: Fill - White; Outline - Orange; Glow
    WordArtStyle {
        id: "style8",
        name: "Fill: White; Outline: Orange; Glow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0xFF, 0xFF, 0xFF),
        outline: true,
        shadow_char: false,
        shadow_shape: None,
        glow: Some(Glow { color: Rgb(0xED, 0x7D, 0x31), size: 10.0, transparency: 40.0 }),
    },
    // 9: Fill - Purple, Accent 5; Glow
    WordArtStyle {
        id: "style9",
        name: "Fill: Purple, Accent 5; Glow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0x70, 0x30, 0xA0),
        outline: false,
        shadow_char: false,
        shadow_shape: None,
        glow: Some(Glow { color: Rgb(0xB2, 0xA1, 0xC7), size: 8.0, transparency: 45.0 }),
    },
    // 10: Fill - Slate Blue; Soft Glow
    WordArtStyle {
        id: "style10",
        name: "Fill: Slate Blue; Soft Glow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0x41, 0x71, 0x9C),
        outline: false,
        shadow_char: false,
        shadow_shape: None,
        glow: Some(Glow { color: Rgb(0x5B, 0x9B, 0xD5), size: 9.0, transparency: 40.0 }),
    },
    // 11: Fill - Black; Neon Blue Glow
    WordArtStyle {
        id: "style11",
        name: "Fill: Black; Neon Blue Glow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0x1A, 0x1A, 0x1A),
        outline: false,
        shadow_char: false,
        shadow_shape: None,
        glow: Some(Glow { color: Rgb(0x00, 0xB0, 0xF0), size: 12.0, transparency: 30.0 }),
    },
    // 12: Fill - Navy Blue; Reflection
    WordArtStyle {
        id: "style12",
        name: "Fill: Navy Blue; Reflection",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0x1B, 0x36, 0x5D),
        outline: false,
        shadow_char: false,
        shadow_shape: Some(Shadow { color: Rgb(0x1B, 0x36, 0x5D), transparency: 70.0, blur: 2.0, distance: 5.0, angle: 90.0, rot_with_shape: false }),
        glow: None,
    },
    // 13: Fill - Amber; Warm Shadow
    WordArtStyle {
        id: "style13",
        name: "Fill: Amber; Warm Shadow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0xC6, 0x59, 0x11),
        outline: false,
        shadow_char: true,
        shadow_shape: Some(Shadow { color: Rgb(0x80, 0x30, 0x00), transparency: 60.0, blur: 4.0, distance: 3.5, angle: 45.0, rot_with_shape: false }),
        glow: None,
    },
    // 14: Fill - Emerald Green; Shadow
    WordArtStyle {
        id: "style14",
        name: "Fill: Emerald Green; Shadow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0x2E, 0x75, 0x43),
        outline: false,
        shadow_char: false,
        shadow_shape: Some(Shadow { color: Rgb(0x15, 0x38, 0x20), transparency: 60.0, blur: 4.0, distance: 3.5, angle: 45.0, rot_with_shape: false }),
        glow: None,
    },
    // 15: Fill - Rose Gold; Subtle Glow
    WordArtStyle {
        id: "style15",
        name: "Fill: Rose Gold; Subtle Glow",
        font_name: "Segoe UI",
        font_size: 36.0,
        bold: true,
        italic: false,
        color: Rgb(0xA6, 0x1C, 0x4C),
        outline: false,
        shadow_char: false,
        shadow_shape: None,
        glow: Some(Glow { color: Rgb(0xF0, 0xA0, 0xB8), size: 8.0, transparency: 45.0 }),
    },
];

/// Find a WordArt style preset from JSON parameters.
pub fn find_style(v: &Value) -> WordArtStyle {
    if let Some(n) = v.get("preset").and_then(Value::as_u64) {
        let idx = (n as usize).min(WORDART_STYLES.len().saturating_sub(1));
        return WORDART_STYLES[idx];
    }
    if let Some(s) = v.get("preset").and_then(Value::as_str) {
        let s_lower = s.to_ascii_lowercase();
        if let Some(style) = WORDART_STYLES.iter().find(|st| st.id.eq_ignore_ascii_case(&s_lower) || st.name.eq_ignore_ascii_case(&s_lower)) {
            return *style;
        }
        if s_lower.starts_with("style")
            && let Ok(num) = s_lower.trim_start_matches("style").parse::<usize>()
            && num >= 1
            && num <= WORDART_STYLES.len()
        {
            return WORDART_STYLES[num - 1];
        }
    }
    WORDART_STYLES[0]
}

/// Inserts a stylized WordArt object into the document.
pub fn insert_wordart(s: &mut Session, v: &Value) -> CmdResult {
    let style = find_style(v);

    let raw_text = if let Some(t) = p::str(v, "text") {
        t.to_string()
    } else {
        let sel = s.selected_text();
        if sel.is_empty() { "Your text here".to_string() } else { sel }
    };
    let text: String = raw_text.chars().take(10_000).collect();

    let char_props = style.char_props();
    let shape_effects = style.shape_effects();

    let text_len = text.chars().count().max(1) as f32;
    let w = p::f32(v, "width").unwrap_or_else(|| (text_len * 22.0).clamp(180.0, 520.0)).clamp(20.0, 2000.0);
    let h = p::f32(v, "height").unwrap_or(60.0).clamp(10.0, 2000.0);

    let wrap = match p::str(v, "wrap").unwrap_or("square") {
        "inline" => Wrap::Inline,
        "tight" => Wrap::Tight,
        "through" => Wrap::Through,
        "topAndBottom" => Wrap::TopAndBottom,
        "inFront" | "inFrontOfText" => Wrap::InFrontOfText,
        "behind" | "behindText" => Wrap::BehindText,
        _ => Wrap::Square,
    };

    let id = s.doc.add_part(PartKind::TextBox, vec![para_block(Paragraph::with_text(&text, char_props))]);
    let at = delete_selection(s)?;
    let typing_props = s.typing_props();

    let obj = InlineObject::Shape {
        kind: ShapeKind::TextBox,
        w,
        h,
        fill: None,
        stroke: None,
        stroke_width: 0.0,
        float: Float { wrap, dist: 9.0, dist_top: 4.0, dist_bottom: 4.0, ..Float::default() },
        story: Some(id),
        freeform: None,
        effects: shape_effects,
    };

    let _ = s.doc.insert_object(&at, obj, &typing_props)?;
    s.sel = Selection::caret(s.doc.end_of(StoryRef::Part(id)));

    Ok(json!({
        "story": id,
        "preset": style.id,
        "name": style.name,
        "text": text,
        "width": w,
        "height": h,
        "wrap": format!("{wrap:?}"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wordart_styles_coverage() {
        assert_eq!(WORDART_STYLES.len(), 15);
        for s in &WORDART_STYLES {
            assert!(!s.id.is_empty());
            assert!(!s.name.is_empty());
            assert!(s.font_size >= 12.0);
        }
    }

    #[test]
    fn wordart_find_style() {
        let s1 = find_style(&json!({"preset": 0}));
        assert_eq!(s1.id, "style1");

        let s3 = find_style(&json!({"preset": "style3"}));
        assert_eq!(s3.id, "style3");
        assert!(s3.outline);

        let s6 = find_style(&json!({"preset": "style6"}));
        assert_eq!(s6.id, "style6");
        assert!(s6.glow.is_some());

        let fallback = find_style(&json!({}));
        assert_eq!(fallback.id, "style1");
    }

    #[test]
    fn wordart_insert_creates_shape_textbox() {
        let mut s = Session::new(wordcraft_doc::Document::default());
        let res = s
            .run(
                "insert.wordArt",
                &json!({
                    "text": "Antigravity",
                    "preset": "style2",
                    "wrap": "square"
                }),
            )
            .expect("insert.wordArt succeeded");

        assert_eq!(res["text"], "Antigravity");
        assert_eq!(res["preset"], "style2");

        let story_id = res["story"].as_u64().expect("story id");
        let part = s.doc.parts.get(&(story_id as u32)).expect("part exists");
        assert_eq!(part.kind, PartKind::TextBox);
        assert_eq!(part.blocks.len(), 1);

        // Check text inside the part
        let para = part.blocks[0].as_para().expect("is para");
        assert_eq!(para.plain_text(), "Antigravity");
    }
}
