//! P36 Degrees of Publicness — a neighborhood needs roughly equal parts
//! quiet, in-between, and busy homes, not one uniform exposure level.
//!
//! From Alexander, *A Pattern Language*, Pattern 36, via
//! patternlanguage.cc/Patterns/Degrees-of-Publicness-(36):
//! > **Solution:** Make a clear distinction between three kinds of
//! > homes -- those on quiet backwaters, those on busy streets, and those
//! > that are more or less in between... Give every neighborhood about
//! > equal numbers of these three kinds of homes.
//!
//! # Real classification, real (likely gapped) vocabulary
//!
//! Every building is bucketed by its nearest real `Street`'s
//! `StreetClassification` (`street-smarts-core::components`): `Arterial`
//! maps to "busy," `Local` to "in-between," `Pedestrian`/`Alley` to
//! "quiet." `path_network`/`p61_small_public_squares` -- the only two
//! real origin operators for this field -- only ever produce `Local` and
//! `Pedestrian` (confirmed by grep, not assumed; see
//! `components.rs`'s own doc comment) -- no operator ever produces
//! `Arterial` or `Alley`. A real site can therefore never show a "busy"
//! bucket today; `value` will reflect that honestly, not a detector bug.
//!
//! `value` = smallest bucket's building count, relative to a perfectly
//! equal three-way split (`1.0` if every bucket has >= 1/3 of buildings,
//! falling toward `0.0` as any bucket empties out).

use std::collections::BTreeMap;
use street_smarts_core::components::StreetClassification;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P36DegreesOfPublicness;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
enum Bucket {
    Quiet,
    InBetween,
    Busy,
}

impl Opinion for P36DegreesOfPublicness {
    fn name(&self) -> &'static str {
        "p36_degrees_of_publicness"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p36".into(),
            display: "Alexander et al., A Pattern Language, Pattern 36 (Degrees of Publicness)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Degrees-of-Publicness-(36)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.buildings.is_empty() || n.streets.is_empty() {
            return OpinionOutput::NoView {
                reason: "Needs both real buildings and real streets to bucket by proximity.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut counts: BTreeMap<&str, usize> = BTreeMap::from([("quiet", 0), ("in_between", 0), ("busy", 0)]);
        for b in &n.buildings {
            let bc = b.polygon.centroid();
            let nearest = n.streets.iter()
                .filter_map(|s| s.centerline.first().map(|p| (s, haversine_m(&bc, p))))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            let Some((street, _)) = nearest else { continue };
            let bucket = match street.classification.as_deref().and_then(StreetClassification::from_label) {
                Some(StreetClassification::Arterial) => Bucket::Busy,
                Some(StreetClassification::Local) => Bucket::InBetween,
                Some(StreetClassification::Pedestrian) | Some(StreetClassification::Alley) => Bucket::Quiet,
                None => continue,
            };
            let key = match bucket { Bucket::Quiet => "quiet", Bucket::InBetween => "in_between", Bucket::Busy => "busy" };
            *counts.get_mut(key).unwrap() += 1;
        }

        let total: usize = counts.values().sum();
        if total == 0 {
            return OpinionOutput::NoView {
                reason: "No building's nearest street has a real, recognized classification.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let equal_share = total as f64 / 3.0;
        let min_count = *counts.values().min().unwrap() as f64;
        let value = (min_count / equal_share).clamp(0.0, 1.0);

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("smallest_bucket_ratio".into(), value);

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        for (k, v) in &counts {
            details.insert(format!("n_{k}"), v.to_string());
        }
        details.insert("n_total".into(), total.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s): {} quiet, {} in-between, {} busy (equal-thirds target: {:.1} each).",
                total, counts["quiet"], counts["in_between"], counts["busy"], equal_share
            ),
            sub_scores,
            details,
            caveats: vec![
                "No current operator produces StreetClassification::Arterial or Alley -- see this \
                 module's own doc comment. The 'busy' bucket is structurally empty on any real \
                 pipeline output today, not merely underrepresented.".into(),
                "Buckets by nearest-street classification only, not Alexander's richer 'twisting \
                 paths / physically secluded' vs 'exposed to passers-by' qualitative distinction.".into(),
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
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Street};

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.02, 0.02],
            parcels: vec![], buildings, streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P36 unit fixture".into(),
            },
        }
    }

    fn building_near(id: &str, x_m: f64) -> (Building, Street) {
        let m = 1.0 / 111_320.0;
        let b = Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![LngLat::new(x_m * m, 0.0), LngLat::new((x_m + 5.0) * m, 0.0), LngLat::new((x_m + 5.0) * m, 5.0 * m), LngLat::new(x_m * m, 5.0 * m), LngLat::new(x_m * m, 0.0)]),
            height_m: Some(6.0), typology: Some("p107_solid_v01".into()), year_built: None, parcel_id: None, floors: Some(2), openings: vec![], interior_cells: vec![],
        };
        let s = Street { id: format!("S_{id}"), centerline: vec![LngLat::new(x_m * m, -5.0 * m), LngLat::new((x_m + 10.0) * m, -5.0 * m)], classification: None, row_width_m: Some(4.0) };
        (b, s)
    }

    #[test]
    fn no_buildings_or_streets_is_no_view() {
        assert!(matches!(P36DegreesOfPublicness.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn only_local_and_pedestrian_leaves_busy_bucket_empty() {
        let (b1, mut s1) = building_near("B1", 0.0);
        s1.classification = Some("local".into());
        let (b2, mut s2) = building_near("B2", 100.0);
        s2.classification = Some("pedestrian".into());
        let n = nbhd(vec![b1, b2], vec![s1, s2]);
        let out = P36DegreesOfPublicness.evaluate(&n);
        match out {
            OpinionOutput::Value { details, sub_scores, .. } => {
                assert_eq!(details["n_busy"], "0");
                assert!((sub_scores["smallest_bucket_ratio"] - 0.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn equal_thirds_scores_full_credit() {
        let (b1, mut s1) = building_near("B1", 0.0);
        s1.classification = Some("pedestrian".into());
        let (b2, mut s2) = building_near("B2", 100.0);
        s2.classification = Some("local".into());
        let (b3, mut s3) = building_near("B3", 200.0);
        s3.classification = Some("arterial".into());
        let n = nbhd(vec![b1, b2, b3], vec![s1, s2, s3]);
        let out = P36DegreesOfPublicness.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
