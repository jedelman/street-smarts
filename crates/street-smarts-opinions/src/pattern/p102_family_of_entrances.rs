//! P102 Family of Entrances — a complex's entrances should read as one
//! related group (clustered, visible together, similar in kind), not
//! scattered doorways that happen to share a site.
//!
//! From Alexander, *A Pattern Language*, Pattern 102 (p. 499), via
//! patternlanguage.cc/Patterns/Family-of-Entrances-(102):
//! > **Problem:** When a person arrives in a complex... there is a good
//! > chance they will experience confusion unless the whole collection is
//! > laid out before them...
//! > **Solution:** Lay out the entrances to form a family: (1) They form a
//! > group, are visible together, and each visible from all the others.
//! > (2) They are all broadly similar... marked by a similar kind of
//! > doorway.
//!
//! # Two real, checkable proxies
//!
//! - `clustering`: fraction of main entrance doors (one per building,
//!   ground floor, outer wall) that have at least one OTHER entrance
//!   within `cluster_threshold_m` (default 40m) -- a proximity proxy for
//!   "visible together," not a real sightline check.
//! - `similarity`: fraction of doors whose width is within
//!   `similarity_tolerance` (default 25%) of the neighborhood-wide mean
//!   door width -- a real, checkable proxy for "broadly similar... kind of
//!   doorway" (this schema has no door material/style data, only width and
//!   head height; width is the one dimension every door actually varies
//!   on in practice).
//!
//! `value = 0.5 * clustering + 0.5 * similarity`.

use std::collections::BTreeMap;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::{Neighborhood, OpeningKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P102FamilyOfEntrances;

const CLUSTER_THRESHOLD_M: f64 = 40.0;
const SIMILARITY_TOLERANCE: f64 = 0.25;

impl Opinion for P102FamilyOfEntrances {
    fn name(&self) -> &'static str {
        "p102_family_of_entrances"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p102".into(),
            display: "Alexander et al., A Pattern Language, Pattern 102 (Family of Entrances)"
                .into(),
            url: Some("https://patternlanguage.cc/Patterns/Family-of-Entrances-(102)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        // One entrance point per building: the real-world position of its
        // ground-floor, outer-wall Door, projected along the wall edge it
        // sits on.
        let entrances: Vec<(String, street_smarts_core::geometry::LngLat, f64)> = n
            .buildings
            .iter()
            .filter_map(|b| {
                let door = b
                    .openings
                    .iter()
                    .find(|o| o.kind == OpeningKind::Door && !o.on_hole && o.floor == 0)?;
                let ring = &b.polygon.outer;
                if door.ring_index + 1 >= ring.len() {
                    return None;
                }
                let a = ring[door.ring_index];
                let c = ring[door.ring_index + 1];
                let pos = street_smarts_core::geometry::LngLat::new(
                    a.lng + (c.lng - a.lng) * door.t,
                    a.lat + (c.lat - a.lat) * door.t,
                );
                Some((b.id.clone(), pos, door.width_m))
            })
            .collect();

        if entrances.len() < 2 {
            return OpinionOutput::NoView {
                reason: format!(
                    "{} building(s) with a real ground-floor entrance -- a family needs at least two to compare.",
                    entrances.len()
                ),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut isolated: Vec<String> = Vec::new();
        for (id, pos, _) in &entrances {
            let has_nearby = entrances.iter().any(|(other_id, other_pos, _)| {
                other_id != id && haversine_m(pos, other_pos) <= CLUSTER_THRESHOLD_M
            });
            if !has_nearby {
                isolated.push(id.clone());
            }
        }
        let clustering = 1.0 - (isolated.len() as f64 / entrances.len() as f64);

        let mean_width = entrances.iter().map(|(_, _, w)| w).sum::<f64>() / entrances.len() as f64;
        let mut dissimilar: Vec<String> = Vec::new();
        for (id, _, w) in &entrances {
            if (w - mean_width).abs() / mean_width > SIMILARITY_TOLERANCE {
                dissimilar.push(id.clone());
            }
        }
        let similarity = 1.0 - (dissimilar.len() as f64 / entrances.len() as f64);

        let value = 0.5 * clustering + 0.5 * similarity;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("clustering".into(), clustering);
        sub_scores.insert("similarity".into(), similarity);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_entrances".into(), entrances.len().to_string());
        details.insert("n_isolated".into(), isolated.len().to_string());
        details.insert("n_dissimilar_width".into(), dissimilar.len().to_string());
        details.insert("mean_door_width_m".into(), format!("{mean_width:.2}"));

        let mut contributing: Vec<String> = isolated.clone();
        for id in &dissimilar {
            if !contributing.contains(id) {
                contributing.push(id.clone());
            }
        }

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building entrance(s); {:.0}% cluster within {:.0}m of another entrance, {:.0}% \
                 have a door width within {:.0}% of the {:.2}m mean.",
                entrances.len(), clustering * 100.0, CLUSTER_THRESHOLD_M,
                similarity * 100.0, SIMILARITY_TOLERANCE * 100.0, mean_width
            ),
            sub_scores,
            details,
            caveats: vec![
                "clustering is straight-line proximity, not Alexander's literal 'visible together' \
                 (a real sightline/visibility check) -- two entrances close together but with a \
                 building between them score full credit here.".into(),
                "similarity only compares door WIDTH -- this schema has no door material, color, \
                 or style data, so 'a similar kind of doorway' (Alexander's own phrase) can only be \
                 checked on the one dimension that actually varies here.".into(),
                "One entrance per building (the first ground-floor outer door found) -- doesn't \
                 account for buildings with multiple real public entrances.".into(),
            ],
            contributing_features: contributing,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Opening};

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.02, 0.02],
            parcels: vec![],
            buildings,
            streets: vec![],
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P102 unit fixture".into(),
            },
        }
    }

    fn building_at(id: &str, x_m: f64, y_m: f64, door_width_m: f64) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x_m * m, y_m * m),
                LngLat::new((x_m + 10.0) * m, y_m * m),
                LngLat::new((x_m + 10.0) * m, (y_m + 10.0) * m),
                LngLat::new(x_m * m, (y_m + 10.0) * m),
                LngLat::new(x_m * m, y_m * m),
            ]),
            height_m: Some(6.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            openings: vec![Opening {
                kind: OpeningKind::Door,
                ring_index: 0,
                on_hole: false,
                t: 0.5,
                width_m: door_width_m,
                sill_height_m: 0.0,
                head_height_m: 2.1,
                floor: 0,
            }],
            interior_cells: vec![],
            wall_thickness_m: None,
        }
    }

    #[test]
    fn fewer_than_two_entrances_is_no_view() {
        let n = nbhd(vec![building_at("B1", 0.0, 0.0, 1.0)]);
        assert!(matches!(
            P102FamilyOfEntrances.evaluate(&n),
            OpinionOutput::NoView { .. }
        ));
    }

    #[test]
    fn clustered_similar_entrances_score_high() {
        let n = nbhd(vec![
            building_at("B1", 0.0, 0.0, 1.0),
            building_at("B2", 20.0, 0.0, 1.0),
            building_at("B3", 40.0, 0.0, 1.1),
        ]);
        let out = P102FamilyOfEntrances.evaluate(&n);
        match out {
            OpinionOutput::Value {
                value, sub_scores, ..
            } => {
                assert!(
                    (sub_scores["clustering"] - 1.0).abs() < 1e-6,
                    "got {}",
                    sub_scores["clustering"]
                );
                assert!(
                    sub_scores["similarity"] > 0.9,
                    "got {}",
                    sub_scores["similarity"]
                );
                assert!(value > 0.9, "got {value}");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_far_flung_entrance_is_isolated() {
        let n = nbhd(vec![
            building_at("B1", 0.0, 0.0, 1.0),
            building_at("B2", 20.0, 0.0, 1.0),
            building_at("FAR", 5000.0, 0.0, 1.0),
        ]);
        let out = P102FamilyOfEntrances.evaluate(&n);
        match out {
            OpinionOutput::Value {
                contributing_features,
                sub_scores,
                ..
            } => {
                assert!(contributing_features.contains(&"FAR".to_string()));
                assert!((sub_scores["clustering"] - 2.0 / 3.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_wildly_different_door_width_is_dissimilar() {
        let n = nbhd(vec![
            building_at("B1", 0.0, 0.0, 1.0),
            building_at("B2", 20.0, 0.0, 1.0),
            building_at("WIDE", 40.0, 0.0, 5.0),
        ]);
        let out = P102FamilyOfEntrances.evaluate(&n);
        match out {
            OpinionOutput::Value {
                contributing_features,
                ..
            } => {
                assert!(contributing_features.contains(&"WIDE".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
