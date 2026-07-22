//! P106 Positive Outdoor Space — is each open space a real "room" outdoors,
//! or just whatever's left over between other things?
//!
//! From Alexander, *A Pattern Language*, Pattern 106:
//! > Outdoor spaces which are merely "left over" between buildings will,
//! > in general, not be used... Make all the outdoor spaces which surround
//! > and lie between your buildings positive. Give each one some degree of
//! > enclosure... until it becomes an entity with a positive quality and
//! > does not spill out formlessly into the other space around it.
//!
//! Two independent, honest checks, not one blended guess:
//!
//! - `mean_convexity`: for each open-space polygon, real area / convex-hull
//!   area. Alexander's own diagrams for this pattern contrast a definite,
//!   roughly-convex "room" shape against an amorphous one that spills out
//!   around whatever's nearby -- convexity is a real, checkable proxy for
//!   that distinction, even though it can't see actual enclosure (whether
//!   buildings/trees/walls really surround the space -- see caveats).
//! - `resolved`: 1 minus the area-fraction of open space explicitly tagged
//!   `OpenSpaceKind::Undecided` (P61's leftover/capped-off land -- see
//!   `p61_small_public_squares.rs`). Land nobody has decided the use of
//!   cannot be "positive" in Alexander's sense no matter how it's shaped;
//!   this is the literal, structural half of the pattern this opinion can
//!   check without guessing.
//!
//! `value = 0.5 * mean_convexity + 0.5 * resolved`, area-weighted across all
//! open space in the neighborhood.

use std::collections::BTreeMap;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P106PositiveOutdoorSpace;

/// Minimal local-meter geometry for a convexity ratio. Opinions and
/// patterns are deliberately decoupled crates (see the geometric family's
/// own local math in `strong_centers.rs`/`levels_of_scale.rs`) so this
/// doesn't reach for `street-smarts-patterns::planar`'s convex hull --
/// carries its own, scoped to exactly what a convexity ratio needs.
mod hull {
    use street_smarts_core::geometry::LngLat;

    #[derive(Clone, Copy)]
    pub struct Pt {
        pub x: f64,
        pub y: f64,
    }

    fn project(ring: &[LngLat]) -> Vec<Pt> {
        if ring.is_empty() {
            return vec![];
        }
        let lat0 = ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64;
        let lng0 = ring.iter().map(|p| p.lng).sum::<f64>() / ring.len() as f64;
        let mlat = (lat0 * std::f64::consts::PI / 180.0).cos();
        let mut pts: Vec<Pt> = ring
            .iter()
            .map(|p| Pt {
                x: (p.lng - lng0) * mlat * 111_320.0,
                y: (p.lat - lat0) * 110_540.0,
            })
            .collect();
        if pts.len() >= 2 && (pts.first().map(|a| (a.x, a.y)) == pts.last().map(|a| (a.x, a.y))) {
            pts.pop();
        }
        pts
    }

    fn area(poly: &[Pt]) -> f64 {
        if poly.len() < 3 {
            return 0.0;
        }
        let n = poly.len();
        let mut s = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            s += poly[i].x * poly[j].y - poly[j].x * poly[i].y;
        }
        (s * 0.5).abs()
    }

    /// Graham scan convex hull.
    fn convex_hull(points: &[Pt]) -> Vec<Pt> {
        if points.len() < 3 {
            return points.to_vec();
        }
        let mut pts = points.to_vec();
        pts.sort_by(|a, b| {
            a.x.partial_cmp(&b.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
        });
        let cross = |a: Pt, b: Pt, c: Pt| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        let mut lower: Vec<Pt> = Vec::new();
        for &p in &pts {
            while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
                lower.pop();
            }
            lower.push(p);
        }
        let mut upper: Vec<Pt> = Vec::new();
        for &p in pts.iter().rev() {
            while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
                upper.pop();
            }
            upper.push(p);
        }
        lower.pop();
        upper.pop();
        lower.extend(upper);
        lower
    }

    /// Real area / convex-hull area for a WGS84 ring. `None` if there isn't
    /// enough real geometry (degenerate ring, or a zero-area hull) to say
    /// anything.
    pub fn convexity_ratio(ring: &[LngLat]) -> Option<f64> {
        let local = project(ring);
        if local.len() < 3 {
            return None;
        }
        let real_area = area(&local);
        let hull_area = area(&convex_hull(&local));
        if hull_area <= 0.0 {
            return None;
        }
        Some((real_area / hull_area).clamp(0.0, 1.0))
    }
}

impl Opinion for P106PositiveOutdoorSpace {
    fn name(&self) -> &'static str {
        "p106_positive_outdoor_space"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p106".into(),
            display: "Alexander et al., A Pattern Language, Pattern 106 (Positive Outdoor Space)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl106/apl106.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.open_space.is_empty() {
            return OpinionOutput::NoView {
                reason: "No open space in this neighborhood -- nothing to judge as positive or negative.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut total_area = 0.0;
        let mut convexity_weighted_sum = 0.0;
        let mut undecided_area = 0.0;
        let mut low_convexity: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for o in &n.open_space {
            let piece_area = o.polygon.area_m2();
            if piece_area <= 0.0 {
                continue;
            }
            total_area += piece_area;
            if o.kind == OpenSpaceKind::Undecided {
                undecided_area += piece_area;
            }
            if let Some(ratio) = hull::convexity_ratio(&o.polygon.outer) {
                convexity_weighted_sum += ratio * piece_area;
                details.insert(format!("{}.convexity", o.id), format!("{:.2}", ratio));
                if ratio < 0.6 {
                    low_convexity.push(o.id.clone());
                }
            }
        }

        if total_area <= 0.0 || convexity_weighted_sum <= 0.0 {
            return OpinionOutput::NoView {
                reason: "Open space present but none had enough real geometry (non-degenerate \
                         polygon, positive area) to score."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mean_convexity = convexity_weighted_sum / total_area;
        let undecided_fraction = undecided_area / total_area;
        let resolved = 1.0 - undecided_fraction;
        let value = 0.5 * mean_convexity + 0.5 * resolved;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("mean_convexity".into(), mean_convexity);
        sub_scores.insert("resolved".into(), resolved);

        details.insert("total_open_space_m2".into(), format!("{:.0}", total_area));
        details.insert("undecided_area_m2".into(), format!("{:.0}", undecided_area));
        details.insert("undecided_fraction".into(), format!("{:.2}", undecided_fraction));
        details.insert("n_open_space".into(), n.open_space.len().to_string());
        details.insert("n_low_convexity_lt_0_6".into(), low_convexity.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} open space feature(s), {:.0}m² total. Area-weighted mean convexity {:.2}; \
                 {:.0}% ({:.0}m²) explicitly Undecided.",
                n.open_space.len(),
                total_area,
                mean_convexity,
                undecided_fraction * 100.0,
                undecided_area
            ),
            sub_scores,
            details,
            caveats: vec![
                "Convexity (real area / convex-hull area) is a shape proxy for Alexander's \
                 'positive, room-like' quality. It cannot see actual enclosure -- whether \
                 buildings, trees, hedges, or walls genuinely surround the space. A perfectly \
                 convex field sitting alone with nothing around it scores well here even though \
                 Alexander would call it just as negative as a shapeless leftover.".into(),
                "The 'resolved' sub-score only checks for the OpenSpaceKind::Undecided marker \
                 P61 emits. It has no way to detect leftover or undesigned space that was never \
                 flagged as such -- hand-authored fixtures, or output from an operator that \
                 doesn't emit Undecided, reads as fully 'resolved' by default even if it's just \
                 as much a leftover in reality.".into(),
                "Scores each open-space feature independently. Doesn't see whether several \
                 adjacent small spaces should actually read as one larger positive space, or a \
                 single oddly-shaped one should be several.".into(),
            ],
            contributing_features: low_convexity,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, OpenSpace};

    fn nbhd(open_space: Vec<OpenSpace>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.001, 0.001],
            parcels: vec![],
            buildings: vec![],
            streets: vec![],
            open_space,
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P106 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn square_ring(side_deg: f64) -> Vec<LngLat> {
        vec![
            LngLat::new(0.0, 0.0),
            LngLat::new(side_deg, 0.0),
            LngLat::new(side_deg, side_deg),
            LngLat::new(0.0, side_deg),
            LngLat::new(0.0, 0.0),
        ]
    }

    #[test]
    fn no_open_space_is_no_view() {
        let n = nbhd(vec![]);
        let out = P106PositiveOutdoorSpace.evaluate(&n);
        assert!(matches!(out, OpinionOutput::NoView { .. }));
    }

    #[test]
    fn convex_plaza_with_no_undecided_land_scores_high() {
        let n = nbhd(vec![OpenSpace {
            id: "PLAZA_1".into(),
            polygon: Polygon::from_ring(square_ring(0.0005)),
            kind: OpenSpaceKind::Plaza,
        }]);
        let out = P106PositiveOutdoorSpace.evaluate(&n);
        match out {
            OpinionOutput::Value { value, sub_scores, .. } => {
                assert!(value > 0.95, "a perfect square with nothing Undecided should score near 1.0, got {value}");
                assert!((sub_scores["mean_convexity"] - 1.0).abs() < 1e-6);
                assert!((sub_scores["resolved"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn undecided_land_drags_the_resolved_subscore_down() {
        let n = nbhd(vec![
            OpenSpace { id: "PLAZA_1".into(), polygon: Polygon::from_ring(square_ring(0.0005)), kind: OpenSpaceKind::Plaza },
            OpenSpace { id: "UNDECIDED_1".into(), polygon: Polygon::from_ring(square_ring(0.0005)), kind: OpenSpaceKind::Undecided },
        ]);
        let out = P106PositiveOutdoorSpace.evaluate(&n);
        match out {
            OpinionOutput::Value { value, sub_scores, .. } => {
                // Both squares are equally convex, so mean_convexity stays
                // ~1.0 -- only `resolved` should move, to ~0.5 (half the
                // area is Undecided).
                assert!((sub_scores["mean_convexity"] - 1.0).abs() < 1e-6);
                assert!((sub_scores["resolved"] - 0.5).abs() < 0.01, "resolved should be ~0.5, got {}", sub_scores["resolved"]);
                assert!(value < 0.8, "half-undecided land should pull the composite score down, got {value}");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_spindly_l_shape_scores_lower_convexity_than_a_square() {
        // An L-shape: a 10x10 square with a 5x5 notch bitten out of one
        // corner -- real area 75. The notch's inner corner (0,10) isn't a
        // vertex of this shape, so the convex hull of the 6 actual vertices
        // is the pentagon (0,0)-(10,0)-(10,10)-(5,10)-(0,5), area 87.5 (it
        // cuts the corner diagonally rather than tracing the full missing
        // square) -> convexity 75/87.5 ~= 0.857, not the naive 0.75 a full
        // 10x10 bounding-box hull would give.
        let m = 1.0 / 111_320.0;
        let l_shape = vec![
            LngLat::new(0.0, 0.0),
            LngLat::new(10.0 * m, 0.0),
            LngLat::new(10.0 * m, 10.0 * m),
            LngLat::new(5.0 * m, 10.0 * m),
            LngLat::new(5.0 * m, 5.0 * m),
            LngLat::new(0.0, 5.0 * m),
            LngLat::new(0.0, 0.0),
        ];
        let n = nbhd(vec![OpenSpace {
            id: "PLAZA_L".into(),
            polygon: Polygon::from_ring(l_shape),
            kind: OpenSpaceKind::Plaza,
        }]);
        let out = P106PositiveOutdoorSpace.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                let c = sub_scores["mean_convexity"];
                assert!(c < 1.0, "notched shape should score below a perfect square's 1.0, got {c}");
                assert!((c - 0.857).abs() < 0.01, "L-shape convexity should be ~0.857, got {c}");
                assert!(!contributing_features.contains(&"PLAZA_L".to_string()), "0.857 is above the 0.6 low-convexity threshold, should not be flagged");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_plus_shape_is_flagged_as_low_convexity() {
        // A plus/cross shape -- arms of half-width 1, extending to 5 in each
        // direction. Real area 36 (two 2x10 arms overlapping in a 2x2
        // center: 20+20-4). Convex hull is the octagon connecting the 8 arm
        // tips, area 68. Convexity 36/68 ~= 0.53, below the 0.6 threshold --
        // this is the shape Alexander's "spills out formlessly" language
        // actually describes, unlike the merely-notched L-shape above.
        let m = 1.0 / 111_320.0;
        let plus = vec![
            LngLat::new(1.0 * m, 5.0 * m), LngLat::new(1.0 * m, 1.0 * m),
            LngLat::new(5.0 * m, 1.0 * m), LngLat::new(5.0 * m, -m),
            LngLat::new(1.0 * m, -m), LngLat::new(1.0 * m, -5.0 * m),
            LngLat::new(-m, -5.0 * m), LngLat::new(-m, -m),
            LngLat::new(-5.0 * m, -m), LngLat::new(-5.0 * m, 1.0 * m),
            LngLat::new(-m, 1.0 * m), LngLat::new(-m, 5.0 * m),
            LngLat::new(1.0 * m, 5.0 * m),
        ];
        let n = nbhd(vec![OpenSpace {
            id: "PLAZA_PLUS".into(),
            polygon: Polygon::from_ring(plus),
            kind: OpenSpaceKind::Plaza,
        }]);
        let out = P106PositiveOutdoorSpace.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, contributing_features, .. } => {
                let c = sub_scores["mean_convexity"];
                assert!((c - 0.529).abs() < 0.01, "plus-shape convexity should be ~0.529, got {c}");
                assert!(contributing_features.contains(&"PLAZA_PLUS".to_string()), "a convexity below 0.6 should be flagged in contributing_features");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
