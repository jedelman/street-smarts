//! P61 Small Public Squares — are the generated plazas actually small, or
//! did the generator produce a handful of oversized "squares" that read as
//! parking lots?
//!
//! From Alexander, *A Pattern Language*, Pattern 61:
//! > ...too large, they look and feel deserted... Make a public square
//! > much smaller than you would at first imagine... this applies to
//! > squares which are lively and successful... 45 to 60 feet across
//! > [roughly 14-18m] is about the maximum.
//!
//! # What this checks
//! Every `OpenSpaceKind::Plaza` entity in the neighborhood, scored against
//! Alexander's own explicit upper bound (18m across, read here as an
//! effective "diameter" derived from area assuming a roughly square/
//! circular footprint -- a real Voronoi-clipped plaza isn't a perfect
//! square, so this is a proxy, not an exact measurement, and says so).
//! `value` is the area-weighted fraction of plaza area that falls under
//! the threshold -- a single oversized plaza dominating total plaza area
//! should weigh more than one small plaza among many, matching how
//! Alexander frames the failure mode ("a large, mostly empty square")
//! as a per-place problem, not just a per-count one.

use std::collections::BTreeMap;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

/// Alexander's own cited upper bound, 45-60ft, taken at the wide end (~18m)
/// converted to an area assuming a square footprint, as the "too large"
/// threshold for the effective-diameter proxy below.
const MAX_DIAMETER_M: f64 = 18.0;

pub struct P61SmallPublicSquares;

impl Opinion for P61SmallPublicSquares {
    fn name(&self) -> &'static str {
        "p61_small_public_squares"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p61".into(),
            display: "Alexander et al., A Pattern Language, Pattern 61 (Small Public Squares)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl61/apl61.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let plazas: Vec<_> = n.open_space.iter().filter(|o| o.kind == OpenSpaceKind::Plaza).collect();
        if plazas.is_empty() {
            return OpinionOutput::NoView {
                reason: "No Plaza open-space entities found -- run p61_small_public_squares first."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut total_area = 0.0;
        let mut small_area = 0.0;
        let mut oversized: Vec<String> = Vec::new();

        for p in &plazas {
            let area = p.polygon.area_m2();
            // Effective diameter of a square with this area -- sqrt(area),
            // not a real measurement of the (usually irregular) footprint.
            let effective_diameter_m = area.max(0.0).sqrt();
            total_area += area;
            if effective_diameter_m <= MAX_DIAMETER_M {
                small_area += area;
            } else {
                oversized.push(p.id.clone());
            }
        }

        let value = if total_area > 0.0 { small_area / total_area } else { 0.0 };
        let count_fraction = (plazas.len() - oversized.len()) as f64 / plazas.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("area_weighted_fraction".into(), value);
        sub_scores.insert("count_fraction".into(), count_fraction);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_plazas".into(), plazas.len().to_string());
        details.insert("n_oversized".into(), oversized.len().to_string());
        details.insert("total_plaza_area_m2".into(), format!("{:.0}", total_area));
        details.insert("max_diameter_m".into(), format!("{:.0}", MAX_DIAMETER_M));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} of {} plaza(s) by area stay under Alexander's ~{:.0}m diameter bound ({} oversized by count).",
                plazas.len() - oversized.len(),
                plazas.len(),
                MAX_DIAMETER_M,
                oversized.len()
            ),
            sub_scores,
            details,
            caveats: vec![
                "\"Effective diameter\" is sqrt(area), a proxy for an irregular Voronoi-clipped \
                 footprint, not a real measured span -- an elongated plaza with the same area as \
                 a compact one scores identically here even if it reads very differently in \
                 practice."
                    .into(),
                "Doesn't check what's AROUND the plaza (enclosure, activity, seating) -- only \
                 whether the open-space geometry itself stays within Alexander's own explicit \
                 size bound."
                    .into(),
            ],
            contributing_features: oversized,
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
            bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
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
                label: "P61 opinion fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    /// A roughly-square plaza `side_m` meters on a side, near the equator
    /// where 1 degree of lng/lat is ~111,320m / ~110,540m.
    fn square_plaza(id: &str, side_m: f64) -> OpenSpace {
        let d_lng = side_m / 111_320.0;
        let d_lat = side_m / 110_540.0;
        OpenSpace {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(d_lng, 0.0),
                LngLat::new(d_lng, d_lat),
                LngLat::new(0.0, d_lat),
                LngLat::new(0.0, 0.0),
            ]),
            kind: OpenSpaceKind::Plaza,
        }
    }

    #[test]
    fn no_plazas_is_no_view() {
        assert!(matches!(P61SmallPublicSquares.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn small_plaza_scores_full() {
        let n = nbhd(vec![square_plaza("sq0", 12.0)]);
        match P61SmallPublicSquares.evaluate(&n) {
            OpinionOutput::Value { value, details, .. } => {
                assert!((value - 1.0).abs() < 1e-6, "got {value}");
                assert_eq!(details["n_oversized"], "0");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn oversized_plaza_is_flagged_and_scores_zero() {
        let n = nbhd(vec![square_plaza("huge", 40.0)]);
        match P61SmallPublicSquares.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value.abs() < 1e-6, "got {value}");
                assert_eq!(contributing_features, vec!["huge".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn mixed_plazas_are_area_weighted() {
        // One small (12m, area ~144m²) and one large (40m, area ~1600m²) --
        // area-weighted fraction should be dominated by the large one, so
        // much lower than the simple 1-of-2 count fraction would suggest.
        let n = nbhd(vec![square_plaza("small", 12.0), square_plaza("huge", 40.0)]);
        match P61SmallPublicSquares.evaluate(&n) {
            OpinionOutput::Value { value, sub_scores, .. } => {
                assert!(value < sub_scores["count_fraction"], "area-weighted ({value}) should be pulled down below count fraction ({})", sub_scores["count_fraction"]);
                assert!((sub_scores["count_fraction"] - 0.5).abs() < 1e-9);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
