//! Draw tab: ink pens, highlighters, eraser, lasso, ink-to-shape, ink-to-math, replay.

use serde_json::json;
use wordcraft_doc::para::ShapeKind;

use crate::{CommandSpec, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("draw.select", "Select", "Draw › Tools", |s, _| {
            s.status = "Draw: Select tool active".into();
            Ok(json!({"tool": "select"}))
        })
        .pure(),
        CommandSpec::new("draw.lasso", "Lasso Select", "Draw › Tools", |s, _| {
            s.status = "Draw: Lasso select active".into();
            Ok(json!({"tool": "lasso"}))
        })
        .pure(),
        CommandSpec::new("draw.eraser", "Eraser", "Draw › Tools", |s, v| {
            let mode = p::str(v, "mode").unwrap_or("stroke");
            s.status = format!("Draw: Eraser ({mode}) active");
            Ok(json!({"tool": "eraser", "mode": mode}))
        })
        .params(r#"{"mode"?: "stroke|point"}"#)
        .pure(),
        CommandSpec::new("draw.pen", "Pen", "Draw › Pens", |s, v| {
            let color = p::str(v, "color").unwrap_or("000000");
            let width = p::f32(v, "width").unwrap_or(1.5).clamp(0.25, 36.0);
            s.status = format!("Draw: Pen ({color}, {width}pt)");
            Ok(json!({"tool": "pen", "color": color, "width": width}))
        })
        .params(r#"{"color"?: "RRGGBB", "width"?: pt}"#),
        CommandSpec::new("draw.pencil", "Pencil", "Draw › Pens", |s, v| {
            let color = p::str(v, "color").unwrap_or("666666");
            let width = p::f32(v, "width").unwrap_or(1.0).clamp(0.25, 18.0);
            s.status = format!("Draw: Pencil ({color}, {width}pt)");
            Ok(json!({"tool": "pencil", "color": color, "width": width}))
        })
        .params(r#"{"color"?: "RRGGBB", "width"?: pt}"#),
        CommandSpec::new("draw.highlighter", "Highlighter", "Draw › Pens", |s, v| {
            let color = p::str(v, "color").unwrap_or("FFFF00");
            let width = p::f32(v, "width").unwrap_or(12.0).clamp(4.0, 72.0);
            s.status = format!("Draw: Highlighter ({color}, {width}pt)");
            Ok(json!({"tool": "highlighter", "color": color, "width": width}))
        })
        .params(r#"{"color"?: "RRGGBB", "width"?: pt}"#),
        CommandSpec::new("draw.addPen", "Add Pen", "Draw › Pens", |s, v| {
            let kind = p::str(v, "kind").unwrap_or("pen");
            let color = p::str(v, "color").unwrap_or("0000FF");
            let width = p::f32(v, "width").unwrap_or(2.0);
            s.status = format!("Draw: Added {kind} ({color})");
            Ok(json!({"added": true, "kind": kind, "color": color, "width": width}))
        })
        .params(r#"{"kind"?: "pen|pencil|highlighter", "color"?: "RRGGBB", "width"?: pt}"#),
        CommandSpec::new("draw.inkToShape", "Ink to Shape", "Draw › Convert", |s, v| {
            let shape = p::str(v, "shape").unwrap_or("rectangle");
            let k = match shape {
                "circle" | "ellipse" => ShapeKind::Ellipse,
                "triangle" => ShapeKind::Triangle,
                "righttriangle" | "right_triangle" => ShapeKind::RightTriangle,
                "diamond" => ShapeKind::Diamond,
                "pentagon" => ShapeKind::Pentagon,
                "hexagon" => ShapeKind::Hexagon,
                "octagon" => ShapeKind::Octagon,
                "cross" | "plus" => ShapeKind::Cross,
                "callout" => ShapeKind::Callout,
                "arrow" => ShapeKind::Arrow,
                "star" => ShapeKind::Star,
                "heart" => ShapeKind::Heart,
                _ => ShapeKind::Rectangle,
            };
            super::insert::shape(s, &json!({"kind": format!("{k:?}").to_lowercase()}))?;
            Ok(json!({"converted": true, "shape": shape}))
        })
        .params(r#"{"shape"?: "rectangle|circle|triangle|diamond|arrow|star|hexagon|octagon|pentagon|cross|callout"}"#),
        CommandSpec::new("draw.inkToMath", "Ink to Math", "Draw › Convert", |s, v| {
            let latex = p::str(v, "latex").unwrap_or("E = mc^2");
            super::insert::equation(s, &json!({"latex": latex}))?;
            Ok(json!({"converted": true, "latex": latex}))
        })
        .params(r#"{"latex"?: string}"#),
        CommandSpec::new("draw.replay", "Ink Replay", "Draw › Replay", |s, v| {
            let speed = p::f32(v, "speed").unwrap_or(1.0).clamp(0.25, 4.0);
            s.status = format!("Draw: Ink Replay (speed {speed}x)");
            Ok(json!({"replaying": true, "speed": speed}))
        })
        .params(r#"{"speed"?: number}"#)
        .pure(),
    ]
}
