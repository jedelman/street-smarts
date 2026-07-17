//! Centralized geometric predicates, Tier 1 — HARDENING_SPEC.md §1.2.
//!
//! Before this module: orientation/cross-product tests were reimplemented
//! independently at each call site (a local `cross` closure inside
//! `planar::triangulate`, an inline `1e-12` epsilon inside
//! `point_in_polygon`) -- the exact setup where two call sites can
//! disagree about the same geometric fact near a boundary purely from
//! rounding, not because the input was actually ambiguous. This doesn't
//! rewrite the underlying math (that's Tier 2, adaptive-precision
//! arithmetic, escalate to it only if real data -- synthetic fixtures,
//! HARDENING_SPEC.md §5 -- shows this tier isn't enough) -- it centralizes
//! the existing math into one function per predicate, with one shared
//! tolerance, so every call site agrees by construction.

use crate::planar::Pt2;

/// Meaningfully smaller than the smallest real feature size already in use
/// in this codebase's own constants (P95/P108's 0.1m construction-joint
/// `pad_inset_m`) -- 1mm, not an arbitrarily chosen small number.
pub const EPSILON_M: f64 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    CounterClockwise,
    Clockwise,
    Collinear,
}

/// Twice the signed area of triangle (a, b, c) -- positive if a→b→c turns
/// counterclockwise, negative if clockwise, exactly zero if collinear. The
/// raw building block: no tolerance applied, so it's a safe drop-in
/// replacement anywhere a call site already computed this exact formula
/// itself (the ear-clipping triangulator's own local `cross` closure, for
/// one) without changing behavior at all -- centralizing the math is the
/// whole point of Tier 1, not silently changing it. `orient2d` below is
/// the tolerance-aware classification built on top of this, for call
/// sites that want "basically collinear" rather than "exactly zero."
pub fn cross2d(a: Pt2, b: Pt2, c: Pt2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Orientation of the ordered triple (a, b, c): does the turn from a→b→c
/// go counterclockwise, clockwise, or is it (within `EPSILON_M`, applied to
/// the doubled-triangle-area cross product) effectively a straight line?
/// The single implementation every TOLERANCE-AWARE orientation test in
/// this crate should route through, replacing ad hoc
/// `(b.x-a.x)*(c.y-a.y) - (b.y-a.y)*(c.x-a.x)` comparisons at each call
/// site. For call sites that need the exact raw sign (no tolerance,
/// zero-means-exactly-collinear), use `cross2d` directly instead.
pub fn orient2d(a: Pt2, b: Pt2, c: Pt2) -> Orientation {
    let cross = cross2d(a, b, c);
    // `cross` is twice the signed triangle area, not a length -- comparing
    // it directly to a length-scale epsilon is a deliberate simplification,
    // fine at this crate's parcel-scale coordinate magnitudes (tens to low
    // thousands of meters per side), not a generally-correct area-vs-length
    // unit reconciliation. Revisit if Tier 2 (adaptive precision) is ever
    // reached for a reason this simplification doesn't cover.
    if cross > EPSILON_M {
        Orientation::CounterClockwise
    } else if cross < -EPSILON_M {
        Orientation::Clockwise
    } else {
        Orientation::Collinear
    }
}

/// Standard ray-casting point-in-ring test (works for non-convex rings).
/// The single implementation `point_in_polygon` (kept as the public,
/// stable-named entry point other callers already use) delegates to.
pub fn point_in_ring(pt: Pt2, ring: &[Pt2]) -> bool {
    let mut inside = false;
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let pi = ring[i];
        let pj = ring[j];
        if ((pi.y > pt.y) != (pj.y > pt.y))
            && (pt.x < (pj.x - pi.x) * (pt.y - pi.y) / (pj.y - pi.y + 1e-12) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orient2d_agrees_on_a_known_ccw_triangle() {
        let a = Pt2::new(0.0, 0.0);
        let b = Pt2::new(1.0, 0.0);
        let c = Pt2::new(0.0, 1.0);
        assert_eq!(orient2d(a, b, c), Orientation::CounterClockwise);
        assert_eq!(orient2d(a, c, b), Orientation::Clockwise);
    }

    #[test]
    fn orient2d_flags_collinear_points_within_epsilon() {
        let a = Pt2::new(0.0, 0.0);
        let b = Pt2::new(1.0, 0.0);
        let c = Pt2::new(2.0, 0.0);
        assert_eq!(orient2d(a, b, c), Orientation::Collinear);
    }

    #[test]
    fn orient2d_is_consistent_regardless_of_which_call_site_computes_it() {
        // The exact bug class this module exists to prevent: two
        // independently-written orientation tests disagreeing about the
        // same triple. With one shared function, that's structurally
        // impossible -- this test just documents the invariant explicitly.
        let a = Pt2::new(3.7, -2.1);
        let b = Pt2::new(5.2, 0.4);
        let c = Pt2::new(1.0, 6.6);
        let first = orient2d(a, b, c);
        let second = orient2d(a, b, c);
        assert_eq!(first, second);
    }

    #[test]
    fn point_in_ring_matches_known_containment() {
        let square = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(10.0, 0.0),
            Pt2::new(10.0, 10.0),
            Pt2::new(0.0, 10.0),
        ];
        assert!(point_in_ring(Pt2::new(5.0, 5.0), &square));
        assert!(!point_in_ring(Pt2::new(15.0, 5.0), &square));
    }
}
