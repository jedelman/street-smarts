//! P114 Hierarchy of Open Space — every outdoor space should have a
//! smaller space that forms its "back," and itself open onto something
//! larger, not sit alone at one uniform scale.
//!
//! From Alexander, *A Pattern Language*, Pattern 114, via
//! patternlanguage.cc/Patterns/Hierarchy-of-Open-Space-(114):
//! > **Problem:** Outdoors, people always try to find a spot where they
//! > can have their backs protected, looking out toward some larger
//! > opening, beyond the space immediately in front of them.
//! > **Solution:** Whatever space you are shaping... make at least one
//! > smaller space, which looks into it and forms a natural back for it.
//! > ...place it, and its openings, so that it looks into at least one
//! > larger space.
//!
//! # An honest proxy, not a sightline check
//!
//! This schema has no visibility/enclosure geometry -- "looks into" can't
//! be checked directly (same honest limitation `p106_positive_outdoor_space`
//! names for "real enclosure"). What IS real and checkable: whether a
//! genuine SIZE gradient exists locally, not just a uniform scale
//! everywhere. For each open-space feature, this opinion looks for another
//! open-space feature within `nearby_threshold_m` (default 100m) that is
//! meaningfully larger (area ratio >= `larger_ratio`, default 1.5x) --
//! a real, checkable proxy for "opens toward something larger nearby,"
//! even though it can't confirm the two actually face each other.
//!
//! `value` = area-weighted fraction of open-space features that have such
//! a larger neighbor nearby.

use std::collections::BTreeMap;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P114HierarchyOfOpenSpace;

const NEARBY_THRESHOLD_M: f64 = 100.0;
const LARGER_RATIO: f64 = 1.5;

impl Opinion for P114HierarchyOfOpenSpace {
    fn name(&self) -> &'static str {
        "p114_hierarchy_of_open_space"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p114".into(),
            display: "Alexander et al., A Pattern Language, Pattern 114 (Hierarchy of Open Space)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Hierarchy-of-Open-Space-(114)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.open_space.len() < 2 {
            return OpinionOutput::NoView {
                reason: format!(
                    "{} open-space feature(s) -- a hierarchy needs at least two to compare.",
                    n.open_space.len()
                ),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let features: Vec<(String, f64, street_smarts_core::geometry::LngLat)> = n.open_space.iter()
            .filter_map(|o| {
                let area = o.polygon.area_m2();
                if area <= 0.0 {
                    return None;
                }
                Some((o.id.clone(), area, o.polygon.centroid()))
            })
            .collect();

        if features.len() < 2 {
            return OpinionOutput::NoView {
                reason: "Fewer than two open-space features had positive real area to compare.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let total_area: f64 = features.iter().map(|(_, a, _)| a).sum();
        let mut satisfied_area = 0.0;
        let mut without_larger_neighbor: Vec<String> = Vec::new();

        for (id, area, centroid) in &features {
            let has_larger_nearby = features.iter().any(|(other_id, other_area, other_centroid)| {
                other_id != id
                    && *other_area >= area * LARGER_RATIO
                    && haversine_m(centroid, other_centroid) <= NEARBY_THRESHOLD_M
            });
            if has_larger_nearby {
                satisfied_area += area;
            } else {
                without_larger_neighbor.push(id.clone());
            }
        }

        let value = satisfied_area / total_area;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("has_larger_neighbor_fraction".into(), value);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_open_space".into(), features.len().to_string());
        details.insert("n_without_larger_neighbor".into(), without_larger_neighbor.len().to_string());
        details.insert("nearby_threshold_m".into(), format!("{NEARBY_THRESHOLD_M:.0}"));
        details.insert("larger_ratio".into(), format!("{LARGER_RATIO:.1}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} open-space feature(s); {:.0}% (by area) have a meaningfully larger open-space \
                 neighbor within {:.0}m.",
                features.len(), value * 100.0, NEARBY_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "Proximity-and-size-ratio only -- cannot check Alexander's literal 'looks into' \
                 (orientation, sightline, or whether anything actually blocks the view). Two \
                 spaces of the right size at the right distance score full credit here even if a \
                 building sits directly between them.".into(),
                "Doesn't check the 'forms a natural back' half of the pattern (that the SMALLER \
                 space itself has some enclosure) -- only that a larger one exists nearby. See \
                 p106_positive_outdoor_space for the enclosure-shape check this opinion \
                 deliberately doesn't duplicate.".into(),
                "The largest open-space feature in the whole neighborhood can never satisfy this \
                 check by definition (nothing is larger than it) -- a single dominant park will \
                 always show up in without_larger_neighbor, honestly, not as an opinion bug.".into(),
            ],
            contributing_features: without_larger_neighbor,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, OpenSpace, OpenSpaceKind};

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
                label: "P114 unit fixture".into(),
            },
        }
    }

    fn square_at(id: &str, cx_m: f64, cy_m: f64, side_m: f64, kind: OpenSpaceKind) -> OpenSpace {
        let m = 1.0 / 111_320.0;
        let half = side_m / 2.0;
        OpenSpace {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new((cx_m - half) * m, (cy_m - half) * m),
                LngLat::new((cx_m + half) * m, (cy_m - half) * m),
                LngLat::new((cx_m + half) * m, (cy_m + half) * m),
                LngLat::new((cx_m - half) * m, (cy_m + half) * m),
                LngLat::new((cx_m - half) * m, (cy_m - half) * m),
            ]),
            kind,
        }
    }

    #[test]
    fn fewer_than_two_features_is_no_view() {
        let n = nbhd(vec![square_at("A", 0.0, 0.0, 20.0, OpenSpaceKind::Plaza)]);
        assert!(matches!(P114HierarchyOfOpenSpace.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn small_space_near_a_much_larger_one_is_satisfied() {
        let n = nbhd(vec![
            square_at("SMALL", 0.0, 0.0, 20.0, OpenSpaceKind::Common),
            square_at("BIG", 50.0, 0.0, 60.0, OpenSpaceKind::Park),
        ]);
        let out = P114HierarchyOfOpenSpace.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                // BIG has nothing larger than it, SMALL does -- exactly
                // half the feature count, but area-weighted so the small
                // one (400 m²) is a small share of total area (400+3600).
                assert!(value > 0.05 && value < 0.15, "got {value}");
                assert!(contributing_features.contains(&"BIG".to_string()));
                assert!(!contributing_features.contains(&"SMALL".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn two_similarly_sized_spaces_satisfy_neither() {
        let n = nbhd(vec![
            square_at("A", 0.0, 0.0, 20.0, OpenSpaceKind::Plaza),
            square_at("B", 30.0, 0.0, 20.0, OpenSpaceKind::Plaza),
        ]);
        let out = P114HierarchyOfOpenSpace.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6, "same-size spaces should satisfy neither, got {value}");
                assert_eq!(contributing_features.len(), 2);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_larger_space_far_away_does_not_count() {
        let n = nbhd(vec![
            square_at("SMALL", 0.0, 0.0, 20.0, OpenSpaceKind::Common),
            square_at("FAR_BIG", 5000.0, 0.0, 60.0, OpenSpaceKind::Park),
        ]);
        let out = P114HierarchyOfOpenSpace.evaluate(&n);
        match out {
            OpinionOutput::Value { contributing_features, .. } => {
                assert!(contributing_features.contains(&"SMALL".to_string()), "a larger neighbor 5km away shouldn't count as nearby");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
