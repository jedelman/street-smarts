//! P22 Nine Per Cent Parking — parking should never consume more than a
//! small fraction of a site's land.
//!
//! From Alexander, *A Pattern Language*, Pattern 22, via
//! patternlanguage.cc/Patterns/Nine-Per-Cent-Parking-(22):
//! > **Problem:** When the area devoted to parking is too great, it
//! > destroys the land.
//! > **Solution:** Do not allow more than 9 percent of the land in any
//! > given area to be used for parking.
//!
//! # A schema gap worth naming, not hiding
//!
//! `OpenSpaceKind::Parking` exists in the NIR schema but no current
//! operator ever produces it (confirmed by grep across
//! `street-smarts-patterns/src`, not assumed). A 0% result from this
//! opinion today confirms the ABSENCE of over-parking, not confirmation
//! that appropriate parking has been zoned and stayed under the cap --
//! there simply isn't any parking modeled yet either way.
//!
//! `value` = `1.0` if parking area / total parcel area <= 9%, degrading
//! linearly to `0.0` at 18% (double the cap) and beyond.

use std::collections::BTreeMap;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P22NineParPercentParking;

const MAX_FRACTION: f64 = 0.09;

impl Opinion for P22NineParPercentParking {
    fn name(&self) -> &'static str {
        "p22_nine_per_cent_parking"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p22".into(),
            display: "Alexander et al., A Pattern Language, Pattern 22 (Nine Per Cent Parking)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Nine-Per-Cent-Parking-(22)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let total_area: f64 = n.parcels.iter().map(|p| p.polygon.area_m2()).sum();
        if total_area <= 0.0 {
            return OpinionOutput::NoView {
                reason: "No parcels with positive real area to measure total site land against.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        let parking_area: f64 = n.open_space.iter()
            .filter(|o| o.kind == OpenSpaceKind::Parking)
            .map(|o| o.polygon.area_m2())
            .sum();
        let fraction = parking_area / total_area;
        let value = (1.0 - (fraction - MAX_FRACTION).max(0.0) / MAX_FRACTION).clamp(0.0, 1.0);

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("under_cap".into(), value);
        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("parking_fraction".into(), format!("{fraction:.4}"));
        details.insert("parking_area_m2".into(), format!("{parking_area:.0}"));
        details.insert("total_parcel_area_m2".into(), format!("{total_area:.0}"));
        details.insert("max_fraction".into(), format!("{MAX_FRACTION:.2}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "Parking occupies {:.1}% of total parcel area (cap: {:.0}%); {:.0}m² of {:.0}m² total.",
                fraction * 100.0, MAX_FRACTION * 100.0, parking_area, total_area
            ),
            sub_scores,
            details,
            caveats: vec![
                "No current operator produces OpenSpaceKind::Parking -- a 0% result confirms no \
                 over-parking exists, not that parking has been appropriately zoned and stayed \
                 under the cap.".into(),
                "Checks total site parking against the whole site's area, not Alexander's own \
                 finer 'no zone larger than 10 acres' subdivision requirement.".into(),
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
    use street_smarts_core::nir::{NeighborhoodMeta, OpenSpace, Parcel};

    fn nbhd(parcels: Vec<Parcel>, open_space: Vec<OpenSpace>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels, buildings: vec![], streets: vec![], open_space,
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P22 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn square_ring(side_m: f64) -> Vec<LngLat> {
        let m = 1.0 / 111_320.0;
        vec![LngLat::new(0.0, 0.0), LngLat::new(side_m * m, 0.0), LngLat::new(side_m * m, side_m * m), LngLat::new(0.0, side_m * m), LngLat::new(0.0, 0.0)]
    }

    fn parcel(id: &str, side_m: f64) -> Parcel {
        Parcel {
            id: id.into(), polygon: Polygon::from_ring(square_ring(side_m)), area_acres: 1.0,
            use_category: None, ownership: None, is_eda: false, spec: None, density_tier: None, target_stories: None,
        }
    }

    #[test]
    fn no_parcels_is_no_view() {
        assert!(matches!(P22NineParPercentParking.evaluate(&nbhd(vec![], vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn no_parking_scores_full_credit() {
        let n = nbhd(vec![parcel("P1", 100.0)], vec![]);
        let out = P22NineParPercentParking.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn parking_at_exactly_9_percent_scores_full_credit() {
        // 100m x 100m parcel = 10,000 m²; 9% = 900 m² -> 30m x 30m parking lot.
        let parking = OpenSpace { id: "LOT1".into(), polygon: Polygon::from_ring(square_ring(30.0)), kind: OpenSpaceKind::Parking };
        let n = nbhd(vec![parcel("P1", 100.0)], vec![parking]);
        let out = P22NineParPercentParking.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!(value > 0.98, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn parking_at_18_percent_scores_zero() {
        let parking = OpenSpace { id: "LOT1".into(), polygon: Polygon::from_ring(square_ring(42.5)), kind: OpenSpaceKind::Parking };
        let n = nbhd(vec![parcel("P1", 100.0)], vec![parking]);
        let out = P22NineParPercentParking.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!(value < 0.05, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
