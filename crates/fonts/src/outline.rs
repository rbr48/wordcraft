//! Glyph outline geometry that is not specific to one font: a point map applied to a path. Word's
//! warp presets bend glyphs this way, in both the screen and the PDF renderers.

use kurbo::{BezPath, PathEl, Point};

/// Most pieces one straight edge is split into, so a bad piece length cannot make the work unbounded.
const MAX_PIECES: usize = 256;

/// `path` with every point moved by `map`, so a map that is not linear bends the outline as a whole.
/// Curves are flattened to within `tolerance`, and straight edges are split into pieces no longer
/// than `piece` (both in the path's units) before they are moved; otherwise a long straight edge
/// would stay a chord between its moved ends. A piece length of zero or less splits nothing.
pub fn map_outline(path: &BezPath, tolerance: f64, piece: f64, map: impl Fn(f64, f64) -> (f64, f64)) -> BezPath {
    let at = |p: Point| {
        let (x, y) = map(p.x, p.y);
        Point::new(x, y)
    };
    let mut out = BezPath::new();
    let (mut cur, mut start) = (Point::ZERO, Point::ZERO);
    kurbo::flatten(path.elements().iter().copied(), tolerance, |el| match el {
        PathEl::MoveTo(p) => {
            out.move_to(at(p));
            cur = p;
            start = p;
        }
        PathEl::LineTo(p) => {
            push_edge(&mut out, &at, cur, p, piece);
            cur = p;
        }
        PathEl::ClosePath => {
            push_edge(&mut out, &at, cur, start, piece);
            out.close_path();
            cur = start;
        }
        // Flattening leaves only the elements above.
        PathEl::QuadTo(..) | PathEl::CurveTo(..) => {}
    });
    out
}

/// The straight edge from `from` to `to`, split into pieces no longer than `piece`, each moved by `at`.
fn push_edge(out: &mut BezPath, at: &impl Fn(Point) -> Point, from: Point, to: Point, piece: f64) {
    let len = (to - from).hypot();
    let pieces = if piece > 0.0 && len.is_finite() { ((len / piece).ceil() as usize).clamp(1, MAX_PIECES) } else { 1 };
    for i in 1..=pieces {
        let t = i as f64 / pieces as f64;
        out.line_to(at(Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    fn curve() -> BezPath {
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        p.curve_to((10.0, -20.0), (30.0, 20.0), (40.0, 0.0));
        p.line_to((40.0, 10.0));
        p.close_path();
        p
    }

    #[test]
    fn identity_keeps_the_outline_and_flattens_the_curves() {
        let p = curve();
        let q = map_outline(&p, 0.01, 1.0e9, |x, y| (x, y));
        assert!(q.elements().iter().all(|e| matches!(e, PathEl::MoveTo(_) | PathEl::LineTo(_) | PathEl::ClosePath)));
        let (a, b) = (p.bounding_box(), q.bounding_box());
        assert!((a.x0 - b.x0).abs() < 0.05 && (a.y0 - b.y0).abs() < 0.05 && (a.x1 - b.x1).abs() < 0.05 && (a.y1 - b.y1).abs() < 0.05);
    }

    #[test]
    fn the_map_moves_every_point() {
        let base = curve().bounding_box();
        let b = map_outline(&curve(), 0.01, 1.0e9, |x, y| (x + 5.0, y - 2.0)).bounding_box();
        assert!((b.x0 - (base.x0 + 5.0)).abs() < 0.05 && (b.y0 - (base.y0 - 2.0)).abs() < 0.05, "{b:?} from {base:?}");
    }

    #[test]
    fn a_straight_edge_bends_through_its_middle() {
        // The edge from (0, 0) to (200, 0) is split every 10 units, so the bend at x = 100 shows.
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        p.line_to((200.0, 0.0));
        let q = map_outline(&p, 0.01, 10.0, |x, y| (x, (x - 100.0) * (x - 100.0) / 100.0 + y));
        let bent = q.elements().iter().any(|e| matches!(e, PathEl::LineTo(pt) if (pt.x - 100.0).abs() < 1e-9 && pt.y.abs() < 1e-9));
        assert!(bent, "the middle of the edge stays on the straight line: {:?}", q.elements());
    }

    #[test]
    fn piece_counts_are_capped() {
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        p.line_to((1.0e6, 0.0));
        let q = map_outline(&p, 0.01, 1.0e-9, |x, y| (x, y));
        assert!(q.elements().len() <= MAX_PIECES + 1, "{} elements", q.elements().len());
    }
}
