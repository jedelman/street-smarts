//! P163 Outdoor Room — an outdoor space needs real enclosure on several
//! sides to read as a "room," not just be left open to spill away.
//!
//! From Alexander, *A Pattern Language*, Pattern 163, via
//! patternlanguage.cc/Patterns/Outdoor-Room-(163):
//! > **Problem:** A garden is the place for lying in the grass, swinging,
//! > croquet, growing flowers, throwing a ball for the dog. But there is
//! > another way of being outdoors: and its needs are not met by the
//! > garden at all.
//! > **Solution:** Build a place outdoors which has so much enclosure
//! > round it, that it takes on the feeling of a room, even though it is
//! > open to the sky. To do this, define it at the corners with columns,
//! > perhaps roof it partially with a trellis or a sliding canvas roof,
//! > and create 'walls' around it, with fences, sitting walls, screens,
//! > hedges, or the exterior walls of the building itself.
//!
//! # Two real, checkable proxies -- distinct from P106 Positive Outdoor
//! # Space, which only checks shape convexity and resolved-use, never
//! # whether anything real actually stands around the space
//!
//! - `enclosure_fraction`: for each qualifying open-space polygon (Plaza,
//!   Park, or Common -- the deliberately-occupied kinds, not Vacant/
//!   Sponge/Parking/Other/Undecided), the fraction of its own outer-ring
//!   vertices that have a real building within `enclosure_threshold_m`
//!   (default 15m). A space ringed by buildings on most sides reads as a
//!   real outdoor room; one with no nearby buildings at all is just open
//!   land.
//! - `circularity`: `4*pi*area / perimeter^2` -- 1.0 for a circle, small
//!   for a long thin sliver. A crude room-shape proxy (Alexander's "room"
//!   sense implies compact, not elongated), not a substitute for real
//!   enclosure.
//!
//! `value = 0.5 * enclosure_fraction + 0.5 * circularity`, area-weighted
//! across all qualifying open space.
//!
//! Cannot detect fences, sitting walls, screens, hedges, columns, or
//! trellis roofs -- Alexander's own listed enclosure elements -- none of
//! which exist in this schema. Only real buildings count as "walls."

use std::collections::BTreeMap;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P163OutdoorRoom;

const ENCLOSURE_THRESHOLD_M: f64 = 15.0;

impl Opinion for P163OutdoorRoom {
    fn name(&self) -> &'static str {
        "p163_outdoor_room"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p163".into(),
            display: "Alexander et al., A Pattern Language, Pattern 163 (Outdoor Room)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Outdoor-Room-(163)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let candidates: Vec<_> = n
            .open_space
            .iter()
            .filter(|o| {
                matches!(o.kind, OpenSpaceKind::Plaza | OpenSpaceKind::Park | OpenSpaceKind::Common)
                    && o.polygon.area_m2() > 0.0
            })
            .collect();
        if candidates.is_empty() {
            return OpinionOutput::NoView {
                reason: "No Plaza/Park/Common open space in this neighborhood yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        if n.buildings.is_empty() {
            return OpinionOutput::NoView {
                reason: "No buildings to check real enclosure against yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let building_verts: Vec<_> = n.buildings.iter().flat_map(|b| b.polygon.outer.iter().copied()).collect();

        let mut enclosure_scores: Vec<(f64, f64)> = Vec::new();
        let mut circularity_scores: Vec<(f64, f64)> = Vec::new();
        let mut weak: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for o in &candidates {
            let ring = &o.polygon.outer;
            if ring.len() < 3 {
                continue;
            }
            let n_enclosed = ring
                .iter()
                .filter(|v| building_verts.iter().any(|bv| haversine_m(v, bv) <= ENCLOSURE_THRESHOLD_M))
                .count();
            let enclosure = n_enclosed as f64 / ring.len() as f64;
            let area = o.polygon.area_m2();
            let perimeter = o.polygon.perimeter_m();
            let circularity = if perimeter > 0.0 {
                (4.0 * std::f64::consts::PI * area / (perimeter * perimeter)).min(1.0)
            } else {
                0.0
            };
            enclosure_scores.push((enclosure, area));
            circularity_scores.push((circularity, area));
            details.insert(format!("{}.enclosure_fraction", o.id), format!("{enclosure:.2}"));
            details.insert(format!("{}.circularity", o.id), format!("{circularity:.2}"));
            if enclosure < 0.5 {
                weak.push(o.id.clone());
            }
        }

        if enclosure_scores.is_empty() {
            return OpinionOutput::NoView {
                reason: "No qualifying open space had real ring geometry to check.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let total_area: f64 = enclosure_scores.iter().map(|(_, a)| a).sum::<f64>().max(1e-9);
        let mean_enclosure = enclosure_scores.iter().map(|(s, a)| s * a).sum::<f64>() / total_area;
        let mean_circularity = circularity_scores.iter().map(|(s, a)| s * a).sum::<f64>() / total_area;
        let value = 0.5 * mean_enclosure + 0.5 * mean_circularity;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("enclosure_fraction".into(), mean_enclosure);
        sub_scores.insert("circularity".into(), mean_circularity);
        details.insert("n_candidates".into(), enclosure_scores.len().to_string());
        details.insert("enclosure_threshold_m".into(), format!("{ENCLOSURE_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} Plaza/Park/Common space(s); area-weighted mean enclosure {:.0}%, mean circularity {:.2}.",
                enclosure_scores.len(), mean_enclosure * 100.0, mean_circularity
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot detect fences, sitting walls, screens, hedges, columns, or trellis roofs \
                 -- Alexander's own listed enclosure elements -- none of which exist in this \
                 schema. Only real buildings count toward enclosure.".into(),
                "Enclosure is checked per-vertex against the nearest building vertex within 15m \
                 -- a coarse proxy, not a real line-of-sight or facade-facing check.".into(),
                "Circularity rewards compact shapes generally; a well-enclosed rectangular room \
                 scores well below 1.0 even though it's a real 'room' by Alexander's own sense.".into(),
            ],
            contributing_features: weak,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, OpenSpace};

    fn nbhd(buildings: Vec<Building>, open_space: Vec<OpenSpace>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space,
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P163 unit fixture".into(),
            },
        }
    }

    fn square_at(x_m: f64, y_m: f64, side: f64) -> Vec<LngLat> {
        let m = 1.0 / 111_320.0;
        vec![
            LngLat::new(x_m * m, y_m * m),
            LngLat::new((x_m + side) * m, y_m * m),
            LngLat::new((x_m + side) * m, (y_m + side) * m),
            LngLat::new(x_m * m, (y_m + side) * m),
            LngLat::new(x_m * m, y_m * m),
        ]
    }

    fn building_at(id: &str, x_m: f64, y_m: f64, side: f64) -> Building {
        Building {
            id: id.into(), polygon: Polygon::from_ring(square_at(x_m, y_m, side)),
            height_m: Some(6.0), typology: Some("p107_solid_v01".into()), year_built: None,
            parcel_id: None, floors: Some(2), openings: vec![], interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        }
    }

    fn plaza(id: &str, x_m: f64, y_m: f64, side: f64) -> OpenSpace {
        OpenSpace { id: id.into(), polygon: Polygon::from_ring(square_at(x_m, y_m, side)), kind: OpenSpaceKind::Plaza }
    }

    #[test]
    fn no_qualifying_open_space_is_no_view() {
        assert!(matches!(P163OutdoorRoom.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_plaza_ringed_by_buildings_scores_well_on_enclosure() {
        // A 20x20 plaza surrounded closely on all 4 sides by buildings.
        let n = nbhd(
            vec![
                building_at("N", 0.0, 25.0, 20.0),
                building_at("S", 0.0, -25.0, 20.0),
                building_at("E", 25.0, 0.0, 20.0),
                building_at("W", -25.0, 0.0, 20.0),
            ],
            vec![plaza("P1", 0.0, 0.0, 20.0)],
        );
        let out = P163OutdoorRoom.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!(sub_scores["enclosure_fraction"] > 0.5, "got {:?}", sub_scores);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn an_isolated_plaza_with_no_nearby_buildings_is_flagged() {
        let n = nbhd(vec![building_at("B1", 5000.0, 5000.0, 10.0)], vec![plaza("P1", 0.0, 0.0, 20.0)]);
        let out = P163OutdoorRoom.evaluate(&n);
        match out {
            OpinionOutput::Value { contributing_features, .. } => {
                assert!(contributing_features.contains(&"P1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
