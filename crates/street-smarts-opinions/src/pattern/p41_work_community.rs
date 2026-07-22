//! P41 Work Community — workplaces should cluster into real building
//! groups, each with its own courtyard, arranged around a shared common
//! space.
//!
//! From Alexander, *A Pattern Language*, Pattern 41 (p. 222), via
//! patternlanguage.cc/Patterns/Work-Community-(41):
//! > **Problem:** If you spend eight hours of your day at work, and
//! > eight hours at home, there is no reason why your workplace should
//! > be any less of a community than your home.
//! > **Solution:** Establish work communities comprising 10-20 smaller
//! > workplace clusters, each with their own courtyards arranged around
//! > a central common square or courtyard. This central space should
//! > feature shops and lunch counters to foster community interaction.
//!
//! # A real, checkable proxy -- and a real, honest limit
//!
//! This pipeline never tags anything as a "workplace" specifically --
//! `use_category` has no employment-type value any generator produces.
//! What IS real and checkable: `p108_connected_buildings.rs`'s real
//! party-wall clusters (buildings whose `parcel_id` starts with
//! `p108_merged_`, the generator's own real naming convention -- see
//! that opinion's own doc) are the structural analog of a "workplace
//! cluster," and courtyard buildings (`polygon.holes` non-empty) are the
//! real "own courtyard" claim. This checks whether real merged-cluster,
//! real-courtyard buildings sit near a real P61 public square
//! (`OpenSpaceKind::Plaza`) within `adjacency_threshold_m` (default
//! 30m) -- the real "arranged around a central common space" claim.
//! Cannot confirm anything is actually a WORKPLACE (same class of honest
//! limit `p32_shopping_street.rs` already documents for "shop"), nor the
//! literal 10-20-cluster count (no per-building pad-count data survives
//! into the plain `Neighborhood` schema).
//!
//! `value` = fraction of merged-cluster courtyard buildings with a real
//! square nearby. `NoView` if no merged-cluster courtyard building
//! exists at all.

use std::collections::BTreeMap;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P41WorkCommunity;

const ADJACENCY_THRESHOLD_M: f64 = 30.0;

impl Opinion for P41WorkCommunity {
    fn name(&self) -> &'static str {
        "p41_work_community"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p41".into(),
            display: "Alexander et al., A Pattern Language, Pattern 41 (Work Community)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Work-Community-(41)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let clustered_courtyards: Vec<_> = n.buildings.iter()
            .filter(|b| b.parcel_id.as_deref().map(|id| id.starts_with("p108_merged_")).unwrap_or(false))
            .filter(|b| !b.polygon.holes.is_empty())
            .collect();
        if clustered_courtyards.is_empty() {
            return OpinionOutput::NoView {
                reason: "No real P108-merged courtyard buildings in this neighborhood yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        let squares: Vec<_> = n.open_space.iter().filter(|o| o.kind == OpenSpaceKind::Plaza).collect();
        if squares.is_empty() {
            return OpinionOutput::NoView {
                reason: "No real public squares (OpenSpaceKind::Plaza) to check work-community centering against yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let n_near_square = clustered_courtyards.iter()
            .filter(|b| squares.iter().any(|sq| haversine_m(&b.polygon.centroid(), &sq.polygon.centroid()) <= ADJACENCY_THRESHOLD_M))
            .count();
        let value = n_near_square as f64 / clustered_courtyards.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("near_common_square_fraction".into(), value);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_merged_courtyard_buildings".into(), clustered_courtyards.len().to_string());
        details.insert("n_near_square".into(), n_near_square.to_string());
        details.insert("adjacency_threshold_m".into(), format!("{ADJACENCY_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} real party-wall-clustered courtyard building(s); {}/{} ({:.0}%) sit within \
                 {:.0}m of a real public square.",
                clustered_courtyards.len(), n_near_square, clustered_courtyards.len(), value * 100.0, ADJACENCY_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot confirm anything is actually a WORKPLACE -- no operator in this pipeline \
                 tags employment use. Checks the real structural analog (a P108 party-wall cluster \
                 with a real courtyard) near a real common square instead.".into(),
                "Cannot check the literal '10-20 workplace clusters' count -- per-building source-pad \
                 counts don't survive into the plain Neighborhood schema (see \
                 p108_connected_buildings.rs's own entity_provenance for where that data actually \
                 lives, in the ledger, not here).".into(),
                "Cannot check 'shops and lunch counters' at the central square -- no program/use \
                 data exists on OpenSpace at all.".into(),
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
    use street_smarts_core::geometry::{LngLat, Polygon, PolygonPart};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, OpenSpace};

    fn nbhd(buildings: Vec<Building>, open_space: Vec<OpenSpace>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space,
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P41 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn ring(cx: f64, cy: f64, half: f64) -> Vec<LngLat> {
        let m = 1.0 / 111_320.0;
        vec![
            LngLat::new((cx - half) * m, (cy - half) * m), LngLat::new((cx + half) * m, (cy - half) * m),
            LngLat::new((cx + half) * m, (cy + half) * m), LngLat::new((cx - half) * m, (cy + half) * m),
        ]
    }

    fn clustered_courtyard_building(id: &str, cx: f64, cy: f64) -> Building {
        let outer = ring(cx, cy, 30.0);
        let inner = ring(cx, cy, 10.0);
        Building {
            id: format!("{id}_building"),
            polygon: Polygon::from_parts(vec![PolygonPart { outer, holes: vec![inner] }]),
            height_m: Some(9.0), typology: None, year_built: None,
            parcel_id: Some(format!("p108_merged_{id}")),
            floors: None, openings: vec![], interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    fn square(id: &str, cx: f64, cy: f64) -> OpenSpace {
        OpenSpace { id: id.into(), polygon: Polygon::from_ring(ring(cx, cy, 5.0)), kind: OpenSpaceKind::Plaza }
    }

    #[test]
    fn no_clustered_courtyard_buildings_is_no_view() {
        assert!(matches!(P41WorkCommunity.evaluate(&nbhd(vec![], vec![square("SQ1", 0.0, 0.0)])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn no_squares_is_no_view() {
        let n = nbhd(vec![clustered_courtyard_building("1", 0.0, 0.0)], vec![]);
        assert!(matches!(P41WorkCommunity.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_cluster_near_a_square_scores_full() {
        let n = nbhd(vec![clustered_courtyard_building("1", 0.0, 0.0)], vec![square("SQ1", 20.0, 0.0)]);
        let out = P41WorkCommunity.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_cluster_far_from_any_square_scores_zero() {
        let n = nbhd(vec![clustered_courtyard_building("1", 0.0, 0.0)], vec![square("SQ1", 500.0, 500.0)]);
        let out = P41WorkCommunity.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 0.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
