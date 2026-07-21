//! P195 Staircase Volume — a stair needs a real, adequately sized volume
//! of its own, not a leftover sliver.
//!
//! From Alexander, *A Pattern Language*, Pattern 195 (p. 900), via
//! patternlanguage.cc/Patterns/Staircase-Volume-(195):
//! > **Problem:** Lay people often make mistakes about the volume which a
//! > staircase needs and therefore make their plans unbuildable.
//! > **Solution:** Make a two story volume to contain the stairs... The
//! > stair may be 2 feet wide (for a very steep stair) or 5 feet wide for
//! > a generous shallow stair.
//!
//! # A real, checkable proxy -- width only
//!
//! `p133_staircase_as_a_stage` carves a real `InteriorCell { kind: "stair"
//! }` out of each multi-story building's common area, with a real
//! `stair_width_m` parameter (range 0.61-1.52m, default 1.2m -- Alexander's
//! own literal 2-5 foot figure, see its own module doc). This opinion
//! checks that real stair cell's own local
//! bounding-box short side against Alexander's literal 2-5 foot
//! (0.61-1.52m) width range. Cannot check the "two story volume" claim at
//! all -- `InteriorCell` has no height/vertical-extent field, and (per
//! `InteriorCell.floor`'s own doc comment) there is no modeled way to
//! reach an upper floor yet, so a real two-story check isn't possible.

use std::collections::BTreeMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P195StaircaseVolume;

const MIN_WIDTH_M: f64 = 0.61; // 2 feet
const MAX_WIDTH_M: f64 = 1.52; // 5 feet

fn bbox_short_side_m(ring: &[LngLat]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let lat0 = ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64;
    let mlat = lat0.to_radians().cos();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in ring {
        let x = p.lng * mlat * 111_320.0;
        let y = p.lat * 110_540.0;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    (max_x - min_x).min(max_y - min_y)
}

impl Opinion for P195StaircaseVolume {
    fn name(&self) -> &'static str {
        "p195_staircase_volume"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p195".into(),
            display: "Alexander et al., A Pattern Language, Pattern 195 (Staircase Volume)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Staircase-Volume-(195)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let stairs: Vec<(&str, f64)> = n.buildings.iter()
            .flat_map(|b| b.interior_cells.iter().filter(|c| c.kind == "stair").map(move |c| (b.id.as_str(), bbox_short_side_m(&c.polygon.outer))))
            .collect();

        if stairs.is_empty() {
            return OpinionOutput::NoView {
                reason: "No stair InteriorCell in this neighborhood -- run P133 Staircase as a Stage first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut in_range = 0usize;
        let mut out_of_range: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for (bid, width) in &stairs {
            let ok = *width >= MIN_WIDTH_M && *width <= MAX_WIDTH_M;
            details.insert(format!("{bid}.stair_width_m"), format!("{width:.2}"));
            if ok {
                in_range += 1;
            } else {
                out_of_range.push(bid.to_string());
            }
        }

        let value = in_range as f64 / stairs.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("width_in_range_fraction".into(), value);
        details.insert("n_stairs".into(), stairs.len().to_string());
        details.insert("min_width_m".into(), format!("{MIN_WIDTH_M:.2}"));
        details.insert("max_width_m".into(), format!("{MAX_WIDTH_M:.2}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} stair cell(s) checked; {:.0}% fall within Alexander's literal 2-5ft \
                 ({MIN_WIDTH_M:.2}-{MAX_WIDTH_M:.2}m) width range.",
                stairs.len(), value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Cannot check the real text's 'two story volume' claim at all -- InteriorCell has no \
                 height/vertical-extent field, and there is no modeled way to reach an upper floor \
                 yet (see InteriorCell.floor's own doc comment).".into(),
                "Width is a bounding-box short-side proxy on the carved stair cell, not a measured \
                 stair-run width.".into(),
            ],
            contributing_features: out_of_range,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{Building, InteriorCell, NeighborhoodMeta};

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P195 unit fixture".into(),
            },
        }
    }

    fn rect_ring(w: f64, h: f64) -> Vec<LngLat> {
        let m = 1.0 / 111_320.0;
        vec![
            LngLat::new(0.0, 0.0), LngLat::new(w * m, 0.0),
            LngLat::new(w * m, h * m), LngLat::new(0.0, h * m), LngLat::new(0.0, 0.0),
        ]
    }

    fn building_with_stair(id: &str, stair_width: f64) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(rect_ring(20.0, 20.0)),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None, floors: Some(2),
            openings: vec![],
            interior_cells: vec![InteriorCell {
                id: format!("{id}_common_stair"),
                polygon: Polygon::from_ring(rect_ring(stair_width, 4.0)),
                depth: 0.5, is_common: false, kind: "stair".into(), connects_to: vec![], floor: 0,
            }],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    #[test]
    fn no_stairs_is_no_view() {
        assert!(matches!(P195StaircaseVolume.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_1_2m_stair_within_the_generators_default_scores_full() {
        let n = nbhd(vec![building_with_stair("B1", 1.2)]);
        match P195StaircaseVolume.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_too_narrow_stair_is_flagged() {
        let n = nbhd(vec![building_with_stair("B1", 0.3)]);
        match P195StaircaseVolume.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-6, "got {value}");
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
