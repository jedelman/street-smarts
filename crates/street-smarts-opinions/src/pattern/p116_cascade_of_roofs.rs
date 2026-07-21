//! P116 Cascade of Roofs — roofs should step down toward wing ends,
//! following the social hierarchy of the spaces below.
//!
//! From Alexander, *A Pattern Language*, Pattern 116 (p. 565), via
//! patternlanguage.cc/Patterns/Cascade-of-Roofs-(116):
//! > **Problem:** Few buildings will be structurally and socially
//! > intact, unless the floors step down toward the ends of wings, and
//! > unless the roof, accordingly, forms a cascade.
//! > **Solution:** Designers should envision the entire building as a
//! > roof system, positioning the largest and highest roofs over the
//! > most significant areas. Lesser roofs should cascade from these
//! > primary structures.
//!
//! # A real check, now that the schema supports it -- no generator yet
//!
//! `Building.roof_segments` (a `Vec<RoofSegment>`, each a real sub-polygon
//! of the footprint with its own `RoofForm`) now exists specifically for
//! this pattern. `p117_sheltering_roof` still only ever assigns the
//! whole-building `roof` field (one segment, never `roof_segments`) -- so
//! on every real fixture this pipeline ships today, `roof_segments` is
//! empty everywhere, and this opinion still returns `NoView`. What
//! changed: the reason is now "no generator populates this yet", not "the
//! schema can't represent this at all" -- and the check below is REAL,
//! not a placeholder, verified against synthetic fixtures in this file's
//! own tests. Once a real P116 generator exists (needs a per-wing
//! footprint partition keyed to `p127_intimacy_gradient`'s cell graph,
//! still not built), this opinion starts scoring real buildings with no
//! further changes needed here.
//!
//! For each building with 2+ real roof segments: a real cascade means the
//! ridge heights actually step down, not stay uniform (uniform ridges
//! across multiple "segments" isn't a cascade, just one roof cut into
//! pieces). `value` = fraction of segmented buildings whose ridge heights
//! aren't all equal. A building with 0 or 1 segments can't exhibit a
//! cascade by definition -- excluded from the denominator entirely (not
//! penalized for lacking wings to begin with; that's `p107_wings_of_
//! light`'s own claim, not this pattern's).

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P116CascadeOfRoofs;

/// Real float-noise floor for "these ridge heights are actually the
/// same" -- not an Alexander figure, just enough to not call two ridges
/// separated by a millimeter of floating-point drift a "cascade".
const SAME_RIDGE_TOLERANCE_M: f64 = 0.01;

impl Opinion for P116CascadeOfRoofs {
    fn name(&self) -> &'static str {
        "p116_cascade_of_roofs"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p116".into(),
            display: "Alexander et al., A Pattern Language, Pattern 116 (Cascade of Roofs)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Cascade-of-Roofs-(116)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let segmented: Vec<_> = n.buildings.iter().filter(|b| b.roof_segments.len() >= 2).collect();
        if segmented.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building carries 2+ real roof_segments yet -- Building.roof_segments \
                         exists in the schema, but no generator populates it (p117_sheltering_roof \
                         only ever assigns the single whole-building `roof` field). A building with \
                         0 or 1 segments can't exhibit a real cascade. See this opinion's own module \
                         doc for the real generator prerequisite."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut n_cascading = 0usize;
        let mut flat_stepped: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &segmented {
            let ridges: Vec<f64> = b.roof_segments.iter().map(|s| s.form.ridge_height_m).collect();
            let min = ridges.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = ridges.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let cascades = (max - min) > SAME_RIDGE_TOLERANCE_M;
            details.insert(format!("{}.n_segments", b.id), ridges.len().to_string());
            details.insert(format!("{}.ridge_height_range_m", b.id), format!("{:.2}", max - min));
            if cascades {
                n_cascading += 1;
            } else {
                flat_stepped.push(b.id.clone());
            }
        }

        let value = n_cascading as f64 / segmented.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("cascading_fraction".into(), value);
        details.insert("n_segmented_buildings".into(), segmented.len().to_string());
        details.insert("same_ridge_tolerance_m".into(), format!("{SAME_RIDGE_TOLERANCE_M:.3}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s) with 2+ real roof segments; {} ({:.0}%) have ridge heights that \
                 actually step down rather than staying uniform.",
                segmented.len(), n_cascading, value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Only checks buildings that already have 2+ real roof_segments -- no generator \
                 produces these on any real fixture this pipeline ships today, so this opinion \
                 returns NoView in practice until one exists.".into(),
                "Doesn't check that segment placement follows the actual 'social hierarchy of the \
                 spaces below' (which cell is more significant) -- only that ridge heights differ \
                 across segments, a real but partial proxy for a real cascade.".into(),
            ],
            contributing_features: flat_stepped,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, RoofForm, RoofSegment, RoofShape};

    fn m() -> f64 { 111_320.0 }

    fn square_ring(half_side: f64) -> Vec<LngLat> {
        let s = half_side / m();
        vec![
            LngLat::new(-s, -s), LngLat::new(s, -s),
            LngLat::new(s, s), LngLat::new(-s, s), LngLat::new(-s, -s),
        ]
    }

    fn roof(ridge_height_m: f64) -> RoofForm {
        RoofForm { shape: RoofShape::Shed, ridge_height_m, eave_height_m: 6.0, slope_azimuth_deg: 0.0, occupiable: false }
    }

    fn segment(ridge_height_m: f64) -> RoofSegment {
        RoofSegment { footprint: Polygon::from_ring(square_ring(5.0)), form: roof(ridge_height_m) }
    }

    fn building(id: &str, roof_segments: Vec<RoofSegment>) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(square_ring(10.0)),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None, floors: None,
            openings: vec![], interior_cells: vec![], wall_thickness_m: None, roof: None,
            roof_segments, canopies: vec![], wall_niches: vec![],
        }
    }

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P116 unit fixture".into(),
            },
        }
    }

    #[test]
    fn no_segmented_buildings_is_no_view() {
        let n = nbhd(vec![building("B1", vec![])]);
        assert!(matches!(P116CascadeOfRoofs.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_single_segment_is_excluded_not_penalized() {
        let n = nbhd(vec![building("B1", vec![segment(10.0)])]);
        assert!(matches!(P116CascadeOfRoofs.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn segments_with_a_real_ridge_step_down_score_full() {
        let n = nbhd(vec![building("B1", vec![segment(11.0), segment(9.0), segment(8.0)])]);
        match P116CascadeOfRoofs.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn uniform_ridge_segments_are_flagged_not_cascading() {
        let n = nbhd(vec![building("FLAT", vec![segment(10.0), segment(10.0)])]);
        match P116CascadeOfRoofs.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-9, "got {value}");
                assert_eq!(contributing_features, vec!["FLAT".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
