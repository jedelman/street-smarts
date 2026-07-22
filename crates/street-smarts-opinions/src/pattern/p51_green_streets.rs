//! P51 Green Streets — local, no-through-traffic streets should be grass
//! with paving stones set into it for wheels, not solid asphalt.
//!
//! From Alexander, *A Pattern Language*, Pattern 51 (p. 266), via
//! patternlanguage.cc/Patterns/Green-Streets-(51):
//! > **Problem:** There is too much hot hard asphalt in the world. A local
//! > road, which only gives access to buildings, needs a few stones for
//! > the wheels of the cars; nothing more. Most of it can still be green.
//! > **Solution:** On local roads, closed to through traffic, plant grass
//! > all over the road and set occasional paving stones into the grass to
//! > form a surface for the wheels of those cars that need access to the
//! > street. Make no distinction between street and sidewalk.
//!
//! # A real, checkable proxy -- now generated, not just modeled
//!
//! `Street.surface` (added alongside this opinion) is a real, free-form
//! field, same convention as `Parcel.use_category`/`Building.typology`:
//! `path_network.rs` and `p61_small_public_squares.rs` -- the only two
//! generators that emit Local/Pedestrian-classified streets today -- now
//! tag every street they produce `"grass_pavers"`. This opinion checks,
//! for every Local- or Pedestrian-classified street (Alexander's own
//! qualifier: "local roads, closed to through traffic" -- arterial
//! streets are correctly NOT expected to be grass), whether
//! `surface == Some("grass_pavers")`. `value` = fraction that clear this.
//! Cannot check "make no distinction between street and sidewalk" at all
//! -- no sidewalk concept exists separately from the street itself in this
//! schema.

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P51GreenStreets;

fn is_local_scope(classification: &Option<String>) -> bool {
    matches!(classification.as_deref(), Some("local") | Some("pedestrian"))
}

impl Opinion for P51GreenStreets {
    fn name(&self) -> &'static str {
        "p51_green_streets"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p51".into(),
            display: "Alexander et al., A Pattern Language, Pattern 51 (Green Streets)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Green-Streets-(51)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let local_streets: Vec<_> = n.streets.iter().filter(|s| is_local_scope(&s.classification)).collect();
        if local_streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "No Local- or Pedestrian-classified street in this neighborhood.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut green = 0usize;
        let mut not_green: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for s in &local_streets {
            let ok = s.surface.as_deref() == Some("grass_pavers");
            details.insert(format!("{}.surface", s.id), s.surface.clone().unwrap_or_else(|| "unset".into()));
            if ok {
                green += 1;
            } else {
                not_green.push(s.id.clone());
            }
        }

        let value = green as f64 / local_streets.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("grass_surface_fraction".into(), value);
        details.insert("n_local_streets".into(), local_streets.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} Local/Pedestrian street(s) checked; {:.0}% are tagged \"grass_pavers\".",
                local_streets.len(), value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Surface is a single free-form tag (\"grass_pavers\"), not real grass/paving-stone \
                 geometry or material rendering -- render.py doesn't draw a different surface for it \
                 yet.".into(),
                "Cannot check 'make no distinction between street and sidewalk' -- no sidewalk \
                 concept exists separately from the street itself in this schema.".into(),
                "Arterial-classified streets are correctly excluded from this check (Alexander's own \
                 text is scoped to 'local roads, closed to through traffic'), not penalized for \
                 staying asphalt.".into(),
            ],
            contributing_features: not_green,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::LngLat;
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn nbhd(streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings: vec![], streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P51 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn street(id: &str, classification: &str, surface: Option<&str>) -> Street {
        let m = 1.0 / 111_320.0;
        Street {
            id: id.into(),
            centerline: vec![LngLat::new(0.0, 0.0), LngLat::new(100.0 * m, 0.0)],
            classification: Some(classification.into()),
            row_width_m: Some(5.5),
            surface: surface.map(|s| s.to_string()),
        }
    }

    #[test]
    fn no_local_streets_is_no_view() {
        assert!(matches!(P51GreenStreets.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
        let n = nbhd(vec![street("S1", "arterial", None)]);
        assert!(matches!(P51GreenStreets.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_grass_pavers_local_street_scores_full() {
        let n = nbhd(vec![street("S1", "local", Some("grass_pavers"))]);
        match P51GreenStreets.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn an_asphalt_local_street_is_flagged() {
        let n = nbhd(vec![street("S1", "pedestrian", Some("asphalt"))]);
        match P51GreenStreets.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6, "got {value}");
                assert!(contributing_features.contains(&"S1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn an_arterial_street_does_not_count_against_the_score() {
        let n = nbhd(vec![
            street("S1", "local", Some("grass_pavers")),
            street("S2", "arterial", None),
        ]);
        match P51GreenStreets.evaluate(&n) {
            OpinionOutput::Value { value, details, .. } => {
                assert!((value - 1.0).abs() < 1e-6, "arterial street should be excluded, not penalized, got {value}");
                assert_eq!(details.get("n_local_streets"), Some(&"1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
