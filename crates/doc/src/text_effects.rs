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

/// The text effects of a run. Empty (the default) draws the text plainly.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TextEffects {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<TextOutline>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<TextShadow>,
}

impl TextEffects {
    /// `self` with the effects `patch` sets. An effect `patch` leaves unset keeps `self`'s.
    pub fn merged(self, patch: &TextEffects) -> TextEffects {
        TextEffects { outline: patch.outline.or(self.outline), shadow: patch.shadow.or(self.shadow) }
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
        let base = TextEffects { outline: None, shadow: Some(TextShadow { on: true, color: RED, dx: 2.0, dy: 2.0 }) };
        let patch = TextEffects { outline: Some(TextOutline { on: true, color: Rgb::BLACK, width: 1.0 }), shadow: None };
        let merged = base.merged(&patch);
        assert_eq!(merged.outline, patch.outline);
        assert_eq!(merged.shadow, base.shadow);
    }

    #[test]
    fn explicit_off_overrides_an_inherited_effect() {
        let base = TextEffects { outline: Some(TextOutline { on: true, color: RED, width: 1.0 }), shadow: None };
        let off = TextEffects { outline: Some(TextOutline { on: false, color: RED, width: 1.0 }), shadow: None };
        assert_eq!(base.merged(&off).active_outline(), None);
    }

    #[test]
    fn clamp_bounds_width_and_offsets_and_removes_nan() {
        let wild = TextEffects {
            outline: Some(TextOutline { on: true, color: RED, width: 1.0e9 }),
            shadow: Some(TextShadow { on: true, color: RED, dx: f32::NAN, dy: -1.0e9 }),
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
