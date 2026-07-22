//! P108 Connected Buildings — did pads that should have merged into one
//! continuous party-wall footprint actually merge, or are there
//! still-separate pads sitting close enough that a construction joint
//! (not a real gap) is all that's between them?
//!
//! From Alexander, *A Pattern Language*, Pattern 108:
//! > Encourage row houses... wherever buildings stand close together...
//! > and let common walls between buildings become the rule, rather than
//! > the exception.
//!
//! # What this checks
//! For every pair of `p95_building_pad`/`p95_pad_with_building` parcels,
//! estimate whether their footprints are close enough to be touching --
//! centroid distance less than the sum of each pad's own characteristic
//! radius (`sqrt(area / pi)`), a cheap proxy for "these two circles would
//! overlap," not an exact polygon-boundary distance. Two DISTINCT pad
//! parcels that read as touching by this proxy are exactly what
//! `p108_connected_buildings` is supposed to have already merged into one
//! parcel -- their continued separate existence is the failure signal,
//! not their touching.

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;
use street_smarts_core::Scope;

pub struct P108ConnectedBuildings;

impl Opinion for P108ConnectedBuildings {
    fn name(&self) -> &'static str {
        "p108_connected_buildings"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p108".into(),
            display: "Alexander et al., A Pattern Language, Pattern 108 (Connected Buildings)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl108/apl108.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let pads: Vec<_> = n.select(&Scope::BUILDING_PAD).collect();
        if pads.len() < 2 {
            return OpinionOutput::NoView {
                reason: "Fewer than two building pads exist -- nothing for \
                         p108_connected_buildings to have merged yet."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        // Approximate centroid + characteristic radius per pad.
        let projected: Vec<(String, (f64, f64), f64)> = pads
            .iter()
            .map(|p| {
                let c = p.polygon.centroid();
                let area = p.polygon.area_m2();
                let mlat = (c.lat.to_radians()).cos();
                let x = c.lng * mlat * 111_320.0;
                let y = c.lat * 110_540.0;
                let radius = (area / std::f64::consts::PI).max(0.0).sqrt();
                (p.id.clone(), (x, y), radius)
            })
            .collect();

        let mut unmerged_pairs: Vec<String> = Vec::new();
        for i in 0..projected.len() {
            for j in (i + 1)..projected.len() {
                let (id_a, pos_a, r_a) = &projected[i];
                let (id_b, pos_b, r_b) = &projected[j];
                let dx = pos_a.0 - pos_b.0;
                let dy = pos_a.1 - pos_b.1;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < r_a + r_b {
                    unmerged_pairs.push(format!("{id_a}~{id_b}"));
                }
            }
        }

        let n_pairs_checked = projected.len() * (projected.len().saturating_sub(1)) / 2;
        let value = if n_pairs_checked == 0 {
            1.0
        } else {
            1.0 - (unmerged_pairs.len() as f64 / n_pairs_checked as f64)
        };

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_pads".into(), pads.len().to_string());
        details.insert("n_pairs_checked".into(), n_pairs_checked.to_string());
        details.insert("n_apparently_unmerged_touching_pairs".into(), unmerged_pairs.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} of {} pad pair(s) read as still-separate despite apparently touching footprints.",
                unmerged_pairs.len(),
                n_pairs_checked
            ),
            sub_scores: BTreeMap::new(),
            details,
            caveats: vec![
                "\"Touching\" is estimated from centroid distance vs. combined characteristic \
                 radius (sqrt(area/pi) per pad) -- a cheap circle-overlap proxy, not an exact \
                 polygon boundary distance. Two elongated, non-circular pads can read as \
                 touching by this proxy while their real footprints have a genuine gap, or vice \
                 versa."
                    .into(),
                "A real reserved gap (a street, square, or common land between two pads) is \
                 legitimate and should NOT merge -- this check can't distinguish that case from \
                 a genuine construction-joint-only gap p108 should have closed; it only flags \
                 pairs close enough to be worth a second, geometric look."
                    .into(),
            ],
            contributing_features: unmerged_pairs,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, Parcel};

    fn nbhd(parcels: Vec<Parcel>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels,
            buildings: vec![],
            streets: vec![],
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P108 opinion fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    /// A square pad `side_m` on a side, centered at `(center_lng, center_lat)`.
    fn pad(id: &str, center_lng_m: f64, center_lat_m: f64, side_m: f64) -> Parcel {
        let to_lng = |m: f64| m / 111_320.0;
        let to_lat = |m: f64| m / 110_540.0;
        let half = side_m / 2.0;
        Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(to_lng(center_lng_m - half), to_lat(center_lat_m - half)),
                LngLat::new(to_lng(center_lng_m + half), to_lat(center_lat_m - half)),
                LngLat::new(to_lng(center_lng_m + half), to_lat(center_lat_m + half)),
                LngLat::new(to_lng(center_lng_m - half), to_lat(center_lat_m + half)),
                LngLat::new(to_lng(center_lng_m - half), to_lat(center_lat_m - half)),
            ]),
            area_acres: 0.0,
            use_category: Some("p95_building_pad".into()),
            ownership: None,
            is_eda: false,
            spec: None,
            density_tier: None,
            target_stories: None,
        }
    }

    #[test]
    fn fewer_than_two_pads_is_no_view() {
        let n = nbhd(vec![pad("a", 0.0, 0.0, 10.0)]);
        assert!(matches!(P108ConnectedBuildings.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn well_separated_pads_score_full() {
        let n = nbhd(vec![pad("a", 0.0, 0.0, 10.0), pad("b", 200.0, 0.0, 10.0)]);
        match P108ConnectedBuildings.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 1.0).abs() < 1e-9, "got {value}");
                assert!(contributing_features.is_empty());
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn touching_unmerged_pads_are_flagged() {
        // Two 10m-side pads with centers only 8m apart -- well within
        // combined characteristic radius, so this reads as unmerged.
        let n = nbhd(vec![pad("a", 0.0, 0.0, 10.0), pad("b", 8.0, 0.0, 10.0)]);
        match P108ConnectedBuildings.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value.abs() < 1e-9, "got {value}");
                assert_eq!(contributing_features, vec!["a~b".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
