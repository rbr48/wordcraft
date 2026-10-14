//! Text effects on a run (Word's Text Effects): an outline stroke and a shadow, drawn with the
//! run's glyphs. Lengths are points.
//!
//! Each effect has an explicit `on` flag, so a run can switch an inherited effect off, as Word
//! does with "No Outline". Merging is per effect: setting the outline keeps the shadow already on
//! the run.

use serde::{Deserialize, Serialize};

use crate::props::Rgb;

/// Largest outline width, points.
pub const MAX_OUTLINE_WIDTH: f32 = 10.0;
/// Largest shadow offset from the text on each axis, points.
pub const MAX_SHADOW_OFFSET: f32 = 50.0;

/// A stroke drawn around each glyph, over its fill.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOutline {
    pub on: bool,
    pub color: Rgb,
    /// Stroke width, points.
    pub width: f32,
}

/// A copy of the glyphs drawn behind the text, offset from it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextShadow {
    pub on: bool,
    pub color: Rgb,
    /// Offset from the text, points (positive is right and down).
    pub dx: f32,
    pub dy: f32,
}

/// Most gradient stops a text fill keeps. A DOCX gradient can have more; the extra ones are dropped.
pub const MAX_GRADIENT_STOPS: usize = 8;

/// One colour along a gradient.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradientStop {
    /// Position along the gradient, 0 to 1.
    pub pos: f32,
    pub color: Rgb,
}

/// A linear gradient that paints the text instead of the run's colour. Only `stops[..count]` are
/// used, in order of position.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextGradient {
    pub on: bool,
    pub stops: [GradientStop; MAX_GRADIENT_STOPS],
    pub count: u8,
    /// Direction in degrees, DrawingML's convention: 0 runs left to right, 90 top to bottom.
    pub angle: f32,
}

impl TextGradient {
    /// An off gradient: it overrides an inherited gradient, so the text shows the run's colour.
    pub fn off() -> TextGradient {
        TextGradient { on: false, stops: [GradientStop { pos: 0.0, color: Rgb::BLACK }; MAX_GRADIENT_STOPS], count: 0, angle: 0.0 }
    }

    /// The stops in use, ordered by position.
    pub fn used_stops(&self) -> &[GradientStop] {
        &self.stops[..(self.count as usize).min(MAX_GRADIENT_STOPS)]
    }

    /// The same gradient with its stops and angle made finite and in range.
    fn clamped(self) -> TextGradient {
        let count = (self.count as usize).min(MAX_GRADIENT_STOPS);
        let mut stops = self.stops;
        for st in stops.iter_mut() {
            st.pos = finite(st.pos, 0.0, 1.0);
        }
        stops[..count].sort_by(|a, b| a.pos.total_cmp(&b.pos));
        let angle = if self.angle.is_finite() { self.angle.rem_euclid(360.0) } else { 0.0 };
        TextGradient { on: self.on, stops, count: count as u8, angle }
    }
}

/// The text effects of a run. Empty (the default) draws the text plainly.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TextEffects {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<TextOutline>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<TextShadow>,
    /// A gradient that paints the text (Word's Text Fill, gradient).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<TextGradient>,
}

impl TextEffects {
    /// `self` with the effects `patch` sets. An effect `patch` leaves unset keeps `self`'s.
    pub fn merged(self, patch: &TextEffects) -> TextEffects {
        TextEffects { outline: patch.outline.or(self.outline), shadow: patch.shadow.or(self.shadow), fill: patch.fill.or(self.fill) }
    }

    /// The same effects with every number finite and in range. Hostile input (a file, a command,
    /// a control message) cannot make a stroke or an offset that is out of bounds or NaN.
    pub fn clamped(self) -> TextEffects {
        TextEffects {
            outline: self.outline.map(|o| TextOutline { width: finite(o.width, 0.0, MAX_OUTLINE_WIDTH), ..o }),
            shadow: self.shadow.map(|s| TextShadow {
                dx: finite(s.dx, -MAX_SHADOW_OFFSET, MAX_SHADOW_OFFSET),
                dy: finite(s.dy, -MAX_SHADOW_OFFSET, MAX_SHADOW_OFFSET),
                ..s
            }),
            fill: self.fill.map(TextGradient::clamped),
        }
    }

    /// The outline to draw, if one is on.
    pub fn active_outline(&self) -> Option<TextOutline> {
        self.outline.filter(|o| o.on && o.width > 0.0)
    }

    /// The shadow to draw, if one is on.
    pub fn active_shadow(&self) -> Option<TextShadow> {
        self.shadow.filter(|s| s.on)
    }

    /// The gradient to paint the text with, if one is on and has two or more stops.
    pub fn active_fill(&self) -> Option<TextGradient> {
        self.fill.filter(|g| g.on && g.count >= 2)
    }
}

/// `v` clamped to `lo..=hi`, or 0 when `v` is not finite.
fn finite(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgb = Rgb(255, 0, 0);

    #[test]
    fn patch_keeps_the_effect_it_does_not_set() {
        let base = TextEffects { outline: None, shadow: Some(TextShadow { on: true, color: RED, dx: 2.0, dy: 2.0 }), fill: None };
        let patch = TextEffects { outline: Some(TextOutline { on: true, color: Rgb::BLACK, width: 1.0 }), shadow: None, fill: None };
        let merged = base.merged(&patch);
        assert_eq!(merged.outline, patch.outline);
        assert_eq!(merged.shadow, base.shadow);
    }

    #[test]
    fn gradient_needs_two_stops_and_is_clamped_ordered_and_wrapped() {
        let st = |pos: f32, c: Rgb| GradientStop { pos, color: c };
        let mut stops = [st(0.0, Rgb::BLACK); MAX_GRADIENT_STOPS];
        stops[0] = st(1.5, Rgb(0, 0, 255));
        stops[1] = st(-0.5, Rgb(255, 0, 0));
        let fx = TextEffects { fill: Some(TextGradient { on: true, stops, count: 2, angle: -90.0 }), ..Default::default() }.clamped();
        let g = fx.active_fill().expect("two stops are enough");
        assert_eq!(g.used_stops().iter().map(|s| s.pos).collect::<Vec<_>>(), [0.0, 1.0]);
        assert_eq!(g.used_stops()[0].color, Rgb(255, 0, 0), "stops are ordered by position");
        assert_eq!(g.angle, 270.0);
        let one = TextEffects { fill: Some(TextGradient { on: true, stops, count: 1, angle: 0.0 }), ..Default::default() };
        assert!(one.active_fill().is_none(), "one stop is not a gradient");
        assert!(TextEffects { fill: Some(TextGradient::off()), ..Default::default() }.active_fill().is_none());
        // A count above the limit is cut, and a non-finite angle becomes 0.
        let wide = TextGradient { on: true, stops, count: 200, angle: f32::NAN }.clamped();
        assert_eq!((wide.count as usize, wide.angle), (MAX_GRADIENT_STOPS, 0.0));
    }

    #[test]
    fn explicit_off_overrides_an_inherited_effect() {
        let base = TextEffects { outline: Some(TextOutline { on: true, color: RED, width: 1.0 }), shadow: None, fill: None };
        let off = TextEffects { outline: Some(TextOutline { on: false, color: RED, width: 1.0 }), shadow: None, fill: None };
        assert_eq!(base.merged(&off).active_outline(), None);
    }

    #[test]
    fn clamp_bounds_width_and_offsets_and_removes_nan() {
        let wild = TextEffects {
            outline: Some(TextOutline { on: true, color: RED, width: 1.0e9 }),
            shadow: Some(TextShadow { on: true, color: RED, dx: f32::NAN, dy: -1.0e9 }),
            fill: None,
        }
        .clamped();
        assert_eq!(wild.outline.map(|o| o.width), Some(MAX_OUTLINE_WIDTH));
        let s = wild.shadow.expect("shadow");
        assert_eq!((s.dx, s.dy), (0.0, -MAX_SHADOW_OFFSET));
    }

    #[test]
    fn default_draws_nothing() {
        let e = TextEffects::default();
        assert!(e.active_outline().is_none() && e.active_shadow().is_none());
    }
}
