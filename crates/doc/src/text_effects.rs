//! Text effects on a run (Word's Text Effects): an outline stroke and a shadow, drawn with the
//! run's glyphs, a gradient fill, and a warp (Word's Transform). Lengths are points.
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

/// Word's Transform presets: the DrawingML preset text warps (ST_TextShapeType). They are named
/// as DrawingML names them, so a DOCX mapping is name for name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WarpPreset {
    TextArchUp,
    TextArchDown,
    TextCircle,
    TextChevron,
    TextChevronInverted,
    TextWave1,
    TextDoubleWave1,
    TextInflate,
    TextDeflate,
    TextSlantUp,
    TextSlantDown,
}

impl WarpPreset {
    /// Every preset this model bends text with, in gallery order.
    pub const ALL: [WarpPreset; 11] = [
        WarpPreset::TextArchUp,
        WarpPreset::TextArchDown,
        WarpPreset::TextCircle,
        WarpPreset::TextChevron,
        WarpPreset::TextChevronInverted,
        WarpPreset::TextWave1,
        WarpPreset::TextDoubleWave1,
        WarpPreset::TextInflate,
        WarpPreset::TextDeflate,
        WarpPreset::TextSlantUp,
        WarpPreset::TextSlantDown,
    ];

    /// The DrawingML name, as a file spells it.
    pub fn name(self) -> &'static str {
        match self {
            WarpPreset::TextArchUp => "textArchUp",
            WarpPreset::TextArchDown => "textArchDown",
            WarpPreset::TextCircle => "textCircle",
            WarpPreset::TextChevron => "textChevron",
            WarpPreset::TextChevronInverted => "textChevronInverted",
            WarpPreset::TextWave1 => "textWave1",
            WarpPreset::TextDoubleWave1 => "textDoubleWave1",
            WarpPreset::TextInflate => "textInflate",
            WarpPreset::TextDeflate => "textDeflate",
            WarpPreset::TextSlantUp => "textSlantUp",
            WarpPreset::TextSlantDown => "textSlantDown",
        }
    }

    /// The preset with this DrawingML name. ASCII case is ignored.
    pub fn parse(name: &str) -> Option<WarpPreset> {
        WarpPreset::ALL.into_iter().find(|p| p.name().eq_ignore_ascii_case(name))
    }
}

/// How far an arch rises, as a share of the box height at full amount. The middle sits half of it
/// above the box centre and the ends half of it below.
const ARCH_RISE: f32 = 0.5;
/// The circle's rise: a deeper bend than an arch.
const CIRCLE_RISE: f32 = 0.9;
/// A chevron's rise, as a share of the box height at full amount.
const CHEVRON_RISE: f32 = 0.5;
/// A slant's rise from one end to the other, as a share of the box height at full amount.
const SLANT_RISE: f32 = 0.5;
/// A wave's amplitude, as a share of the box height at full amount.
const WAVE_AMPLITUDE: f32 = 0.25;
/// The most an inflate or deflate scales the glyphs at the centre of the run, at full amount.
const INFLATE_SHARE: f32 = 0.5;

/// A warp that bends the text along a preset. `amount` 0 leaves the text flat and 1 is the
/// preset's full bend.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextWarp {
    pub on: bool,
    pub preset: WarpPreset,
    /// 0 to 1 (see `TextEffects::clamped`).
    pub amount: f32,
}

/// The box a run's glyphs are warped within: their bounds, in the same units as the outlines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarpBox {
    pub x0: f32,
    pub y0: f32,
    pub w: f32,
    pub h: f32,
}

impl TextWarp {
    /// Where the point `(x, y)` of a run's outline goes. Outlines are y down, so a smaller y is up.
    /// The point stays where it is when the warp is off or flat, when the box has no area, and when
    /// any input is not finite, so a bad value can never throw a glyph off the page.
    pub fn point(&self, b: WarpBox, x: f32, y: f32) -> (f32, f32) {
        let amount = finite(self.amount, 0.0, 1.0);
        let area = b.w.is_finite() && b.h.is_finite() && b.w > 0.0 && b.h > 0.0;
        if !self.on || amount <= 0.0 || !area || !x.is_finite() || !y.is_finite() || !b.x0.is_finite() || !b.y0.is_finite() {
            return (x, y);
        }
        let cx = b.x0 + b.w / 2.0;
        let cy = b.y0 + b.h / 2.0;
        let half = b.w / 2.0;
        // Across the box: `u` runs 0 to 1 from left to right, `v` runs -1 to 1.
        let u = ((x - b.x0) / b.w).clamp(0.0, 1.0);
        let v = 2.0 * u - 1.0;
        let tau = std::f32::consts::TAU;
        let (nx, ny) = match self.preset {
            WarpPreset::TextArchUp => arch(x, y, cx, half, ARCH_RISE * amount * b.h, false),
            WarpPreset::TextArchDown => arch(x, y, cx, half, ARCH_RISE * amount * b.h, true),
            WarpPreset::TextCircle => arch(x, y, cx, half, CIRCLE_RISE * amount * b.h, false),
            WarpPreset::TextChevron | WarpPreset::TextChevronInverted => {
                // The middle moves by half the rise one way and the ends by half the other way,
                // so the chevron is centred on the box like an arch is.
                let d = CHEVRON_RISE * amount * b.h;
                let shift = d / 2.0 - d * (1.0 - v.abs());
                if self.preset == WarpPreset::TextChevron { (x, y + shift) } else { (x, y - shift) }
            }
            WarpPreset::TextWave1 => (x, y + WAVE_AMPLITUDE * amount * b.h * (tau * u).sin()),
            WarpPreset::TextDoubleWave1 => (x, y + WAVE_AMPLITUDE * amount * b.h * (2.0 * tau * u).sin()),
            WarpPreset::TextInflate | WarpPreset::TextDeflate => {
                // The centre of the run scales about the box centre line; the ends do not move.
                let share = INFLATE_SHARE * amount * (1.0 - v * v);
                let f = if self.preset == WarpPreset::TextInflate { 1.0 + share } else { 1.0 - share };
                (x, cy + (y - cy) * f)
            }
            // Each end moves half of the rise, so the run rises by the whole of it from end to end.
            WarpPreset::TextSlantUp => (x, y - SLANT_RISE * amount * b.h * v / 2.0),
            WarpPreset::TextSlantDown => (x, y + SLANT_RISE * amount * b.h * v / 2.0),
        };
        if nx.is_finite() && ny.is_finite() { (nx, ny) } else { (x, y) }
    }
}

/// An arch: the baseline bends into a parabola, `rise` higher at the middle than at the ends, so the
/// middle sits half of it above the centre line and the ends half of it below. A point moves only up
/// or down, so no glyph folds over another however narrow the run is; the glyphs keep their upright
/// angle rather than turning along the curve. `down` mirrors the arch below the centre line.
fn arch(x: f32, y: f32, cx: f32, half: f32, rise: f32, down: bool) -> (f32, f32) {
    // At most a quarter of the width, so the baseline never leans past 45 degrees.
    let rise = rise.min(half / 2.0);
    if rise.is_nan() || rise <= 1e-6 {
        return (x, y);
    }
    let v = ((x - cx) / half).clamp(-1.0, 1.0);
    let d = rise * (v * v - 0.5);
    (x, if down { y - d } else { y + d })
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
    /// A preset warp that bends the text (Word's Transform).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warp: Option<TextWarp>,
}

impl TextEffects {
    /// `self` with the effects `patch` sets. An effect `patch` leaves unset keeps `self`'s.
    pub fn merged(self, patch: &TextEffects) -> TextEffects {
        TextEffects {
            outline: patch.outline.or(self.outline),
            shadow: patch.shadow.or(self.shadow),
            fill: patch.fill.or(self.fill),
            warp: patch.warp.or(self.warp),
        }
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
            warp: self.warp.map(|w| TextWarp { amount: finite(w.amount, 0.0, 1.0), ..w }),
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

    /// The warp to bend the text with, if one is on and its amount is above zero.
    pub fn active_warp(&self) -> Option<TextWarp> {
        self.warp.filter(|w| w.on && w.amount > 0.0)
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

    /// A box 200 wide and 40 tall, with its centre at (100, 20).
    const BOX: WarpBox = WarpBox { x0: 0.0, y0: 0.0, w: 200.0, h: 40.0 };

    fn warp(preset: WarpPreset, amount: f32) -> TextWarp {
        TextWarp { on: true, preset, amount }
    }

    #[test]
    fn patch_keeps_the_effect_it_does_not_set() {
        let base = TextEffects { shadow: Some(TextShadow { on: true, color: RED, dx: 2.0, dy: 2.0 }), ..Default::default() };
        let patch = TextEffects { outline: Some(TextOutline { on: true, color: Rgb::BLACK, width: 1.0 }), ..Default::default() };
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
        let base = TextEffects { outline: Some(TextOutline { on: true, color: RED, width: 1.0 }), ..Default::default() };
        let off = TextEffects { outline: Some(TextOutline { on: false, color: RED, width: 1.0 }), ..Default::default() };
        assert_eq!(base.merged(&off).active_outline(), None);
    }

    #[test]
    fn clamp_bounds_width_and_offsets_and_removes_nan() {
        let wild = TextEffects {
            outline: Some(TextOutline { on: true, color: RED, width: 1.0e9 }),
            shadow: Some(TextShadow { on: true, color: RED, dx: f32::NAN, dy: -1.0e9 }),
            ..Default::default()
        }
        .clamped();
        assert_eq!(wild.outline.map(|o| o.width), Some(MAX_OUTLINE_WIDTH));
        let s = wild.shadow.expect("shadow");
        assert_eq!((s.dx, s.dy), (0.0, -MAX_SHADOW_OFFSET));
    }

    #[test]
    fn default_draws_nothing() {
        let e = TextEffects::default();
        assert!(e.active_outline().is_none() && e.active_shadow().is_none() && e.active_warp().is_none());
    }

    #[test]
    fn warp_names_are_drawingml_names_and_round_trip() {
        for p in WarpPreset::ALL {
            assert_eq!(WarpPreset::parse(p.name()), Some(p));
            assert_eq!(serde_json::to_string(&p).expect("json"), format!("\"{}\"", p.name()));
        }
        assert_eq!(WarpPreset::parse("TEXTARCHUP"), Some(WarpPreset::TextArchUp), "case is ignored");
        assert_eq!(WarpPreset::parse("textPlain"), None, "plain text is not a preset here: switch the warp off");
        assert_eq!(WarpPreset::parse(""), None);
    }

    #[test]
    fn warp_amount_is_clamped_and_a_flat_or_off_warp_is_not_active() {
        let on = |amount: f32| TextEffects { warp: Some(warp(WarpPreset::TextWave1, amount)), ..Default::default() }.clamped();
        assert_eq!(on(7.0).active_warp().map(|w| w.amount), Some(1.0));
        assert_eq!(on(-3.0).active_warp(), None, "a negative amount is flat");
        assert_eq!(on(f32::NAN).active_warp(), None, "a NaN amount is flat");
        let off = TextEffects { warp: Some(TextWarp { on: false, ..warp(WarpPreset::TextArchUp, 1.0) }), ..Default::default() };
        assert!(off.active_warp().is_none());
    }

    #[test]
    fn a_flat_warp_or_a_degenerate_box_leaves_points_alone() {
        let p = (37.5, 12.25);
        assert_eq!(warp(WarpPreset::TextArchUp, 0.0).point(BOX, p.0, p.1), p);
        let flat = WarpBox { w: 0.0, ..BOX };
        assert_eq!(warp(WarpPreset::TextCircle, 1.0).point(flat, p.0, p.1), p, "no width, no bend");
        let (nx, ny) = warp(WarpPreset::TextCircle, 1.0).point(BOX, f32::NAN, p.1);
        assert!(nx.is_nan() && ny == p.1, "a NaN coordinate is passed through, not bent");
    }

    #[test]
    fn arch_up_lifts_the_middle_and_drops_the_ends() {
        let w = warp(WarpPreset::TextArchUp, 1.0);
        let (mx, my) = w.point(BOX, 100.0, 20.0);
        let (ex, ey) = w.point(BOX, 0.0, 20.0);
        assert!((mx - 100.0).abs() < 1e-3, "the middle stays in place across");
        assert!(my < ey, "the middle is higher (smaller y) than the end: {my} vs {ey}");
        assert!((my - 10.0).abs() < 1e-3 && (ey - 30.0).abs() < 1e-3, "rise is half the box height each way: {my}, {ey}");
        assert!(ex.abs() < 1e-3, "the end stays on the centre line's span");
    }

    #[test]
    fn arch_down_mirrors_arch_up() {
        let up = warp(WarpPreset::TextArchUp, 1.0);
        let down = warp(WarpPreset::TextArchDown, 1.0);
        for x in [0.0, 50.0, 100.0, 150.0, 200.0] {
            let (_, yu) = up.point(BOX, x, 20.0);
            let (_, yd) = down.point(BOX, x, 20.0);
            assert!((yu - (40.0 - yd)).abs() < 1e-3, "mirror about the centre line at x={x}: {yu} vs {yd}");
        }
    }

    #[test]
    fn chevrons_point_the_middle_the_other_way() {
        let (_, up) = warp(WarpPreset::TextChevron, 1.0).point(BOX, 100.0, 20.0);
        let (_, down) = warp(WarpPreset::TextChevronInverted, 1.0).point(BOX, 100.0, 20.0);
        assert!(up < 20.0 && down > 20.0, "chevron raises the middle, inverted lowers it: {up}, {down}");
    }

    #[test]
    fn every_preset_moves_points_no_further_than_the_box_allows() {
        for p in WarpPreset::ALL {
            let w = warp(p, 1.0);
            for i in 0..=20 {
                for j in 0..=4 {
                    let (x, y) = (i as f32 * 10.0, j as f32 * 10.0);
                    let (nx, ny) = w.point(BOX, x, y);
                    assert!(nx.is_finite() && ny.is_finite(), "{p:?} at {x},{y}");
                    assert!((ny - y).abs() <= BOX.h * 1.5, "{p:?} moves {x},{y} by {} which is more than the box allows", (ny - y).abs());
                }
            }
        }
    }

    #[test]
    fn an_arch_never_folds_a_narrow_run() {
        // A tall, narrow box: each column keeps its order, so no point passes another, and x is kept.
        let narrow = WarpBox { x0: 0.0, y0: 0.0, w: 4.0, h: 40.0 };
        for preset in [WarpPreset::TextArchUp, WarpPreset::TextArchDown, WarpPreset::TextCircle, WarpPreset::TextChevron] {
            let w = warp(preset, 1.0);
            for i in 0..=4 {
                let x = i as f32;
                let ys: Vec<f32> = (0..=40).map(|j| w.point(narrow, x, j as f32).1).collect();
                assert!(ys.windows(2).all(|p| p[0] < p[1]), "{preset:?} folds the column at x={x}: {ys:?}");
                assert!((0..=40).all(|j| w.point(narrow, x, j as f32).0 == x), "{preset:?} keeps x");
            }
        }
    }

    #[test]
    fn a_tiny_amount_barely_moves_the_text() {
        let w = warp(WarpPreset::TextArchUp, 1.0e-6);
        for x in [0.0, 50.0, 100.0, 200.0] {
            for y in [0.0, 20.0, 40.0] {
                let (_, ny) = w.point(BOX, x, y);
                assert!((ny - y).abs() < 1e-3, "amount 1e-6 moves ({x}, {y}) to {ny}");
            }
        }
    }

    #[test]
    fn a_finite_point_always_maps_to_a_finite_point() {
        let inputs = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, 1.0e30, -1.0e30];
        let boxes = [BOX, WarpBox { x0: f32::NAN, ..BOX }, WarpBox { w: f32::INFINITY, ..BOX }, WarpBox { h: 1.0e-30, ..BOX }];
        for p in WarpPreset::ALL {
            for &b in &boxes {
                for &x in &inputs {
                    for &y in &inputs {
                        let (nx, ny) = warp(p, 1.0).point(b, x, y);
                        if x.is_finite() && y.is_finite() {
                            assert!(nx.is_finite() && ny.is_finite(), "{p:?} gave {nx},{ny} from {x},{y}");
                        }
                    }
                }
            }
        }
    }
}
