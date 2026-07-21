//! P197 Thick Walls — walls should occupy real volume, not read as thin,
//! prefabricated membranes with no depth.
//!
//! From Alexander, *A Pattern Language*, Pattern 197 (p. 908), via
//! patternlanguage.cc/Patterns/Thick-Walls-(197):
//! > **Problem:** Houses with smooth hard walls made of prefabricated
//! > panels, concrete, gypsum, steel, aluminum, or glass always stay
//! > impersonal and dead.
//! > **Solution:** Open your mind to the possibility that the walls of
//! > your building can be thick, can occupy a substantial volume -- even
//! > actual usable space -- and need not be merely thin membranes which
//! > have no depth.
//!
//! # A real gap, now closed -- with an honest remaining limitation
//!
//! `render.py`'s own documented caveat used to apply directly here: a
//! punch just pierces solid mass, since no wall-thickness field existed
//! anywhere in this pipeline's schema. `p197_thick_walls` (the generator,
//! `street-smarts-patterns`) now assigns every real building a real
//! `wall_thickness_m`, so this opinion checks that field for real instead
//! of always returning `NoView`.
//!
//! `value` = fraction of real buildings with `wall_thickness_m` set and at
//! or above `MIN_THICK_M` -- a real threshold clearly above ordinary thin
//! stud-wall framing (~0.1-0.15m), distinguishing a genuinely thick wall
//! from business-as-usual construction. `NoView` only when there are no
//! real buildings to check at all.
//!
//! # Known, honestly-stated limitation
//!
//! This only checks the SCALAR thickness claim. Alexander's own richer
//! claim -- "even actual usable space" (alcoves, window seats, niches
//! carved INTO the wall depth) -- is not modeled anywhere in this
//! pipeline's schema; see the generator's own module doc for why that's a
//! separate, larger, not-yet-built mechanism.

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P197ThickWalls;

/// Clearly above ordinary thin stud-wall framing (~0.1-0.15m) -- a real
/// threshold for "occupies substantial volume," not the generator's own
/// default (0.3m), so this is a non-trivial check rather than one that
/// trivially passes whenever the generator has run at all.
const MIN_THICK_M: f64 = 0.2;

impl Opinion for P197ThickWalls {
    fn name(&self) -> &'static str {
        "p197_thick_walls"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p197".into(),
            display: "Alexander et al., A Pattern Language, Pattern 197 (Thick Walls)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Thick-Walls-(197)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.buildings.is_empty() {
            return OpinionOutput::NoView {
                reason: "No real buildings to check wall thickness against.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let n_thick = n.buildings.iter()
            .filter(|b| b.wall_thickness_m.is_some_and(|t| t >= MIN_THICK_M))
            .count();
        let total = n.buildings.len();
        let value = n_thick as f64 / total as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("thick_wall_fraction".into(), value);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_thick".into(), n_thick.to_string());
        details.insert("n_total".into(), total.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{total} building(s): {n_thick} have a real wall_thickness_m >= {MIN_THICK_M:.2}m (a genuinely thick wall, not thin-membrane framing)."
            ),
            sub_scores,
            details,
            caveats: vec![
                "Checks only the scalar wall_thickness_m claim -- Alexander's richer 'even actual \
                 usable space' claim (alcoves, window seats, niches carved into the wall depth) is \
                 not modeled anywhere in this pipeline's schema. See this opinion's own module doc.".into(),
                "wall_thickness_m is uniform per building, not per-wall-segment -- a building could \
                 in reality have some thin walls and some thick ones; this schema doesn't distinguish \
                 that yet.".into(),
            ],
            contributing_features: vec![],
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, NeighborhoodMeta};

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P197 unit fixture".into(),
            },
        }
    }

    fn building(id: &str, wall_thickness_m: Option<f64>) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(0.0001, 0.0),
                LngLat::new(0.0001, 0.0001), LngLat::new(0.0, 0.0001), LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(7.0), typology: None, year_built: None, parcel_id: None,
            floors: Some(2), openings: vec![], interior_cells: vec![],
            wall_thickness_m,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    #[test]
    fn no_buildings_is_no_view() {
        assert!(matches!(P197ThickWalls.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn buildings_with_no_wall_thickness_set_score_zero() {
        let n = nbhd(vec![building("B1", None), building("B2", None)]);
        match P197ThickWalls.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-9),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn buildings_at_or_above_the_real_threshold_score_full_credit() {
        let n = nbhd(vec![building("B1", Some(0.3)), building("B2", Some(0.2))]);
        match P197ThickWalls.evaluate(&n) {
            OpinionOutput::Value { value, details, .. } => {
                assert!((value - 1.0).abs() < 1e-9);
                assert_eq!(details["n_thick"], "2");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_wall_thinner_than_the_real_threshold_does_not_count() {
        // Below MIN_THICK_M -- real but still thin-membrane-scale, not a
        // genuinely thick wall.
        let n = nbhd(vec![building("B1", Some(0.15)), building("B2", Some(0.3))]);
        match P197ThickWalls.evaluate(&n) {
            OpinionOutput::Value { value, details, .. } => {
                assert!((value - 0.5).abs() < 1e-9);
                assert_eq!(details["n_thick"], "1");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
