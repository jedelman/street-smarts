//! P191 The Shape of Indoor Space — a room should be a rough rectangle,
//! not an arbitrary crystalline shape.
//!
//! From Alexander, *A Pattern Language*, Pattern 191, via
//! patternlanguage.cc/Patterns/The-Shape-of-Indoor-Space-(191):
//! > **Problem:** The perfectly crystalline squares and rectangles of
//! > ultra-modern architecture make no special sense in human or in
//! > structural terms.
//! > **Solution:** With occasional exceptions, make each indoor space...
//! > a rough rectangle, with roughly straight walls, near right angles in
//! > the corners, and a roughly symmetrical vault over each room.
//!
//! # A real, checkable proxy -- axis-aligned, not rotation-invariant
//!
//! For each `InteriorCell`, this opinion computes `rectangularity = real
//! area / axis-aligned-bounding-box area` in local meters. A true
//! rectangle aligned with the projection axes scores close to 1.0; an
//! irregular or crystalline shape scores lower.
//!
//! `value` = fraction of interior cells with `rectangularity >=
//! min_rectangularity` (default 0.75).
//!
//! Real, named limitation: this measure is axis-aligned, so a genuinely
//! rectangular room simply rotated relative to the projection axes would
//! score poorly even though it fully satisfies Alexander's own text. This
//! pipeline's cells are cut from axis-aligned building footprints in
//! practice, but the check itself can't see rotation and would misjudge a
//! rotated-but-valid room as a failure.

use std::collections::BTreeMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P191ShapeOfIndoorSpace;

const MIN_RECTANGULARITY: f64 = 0.75;

fn aabb_area_m2(ring: &[LngLat]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let lat0 = ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64;
    let mlat = (lat0 * std::f64::consts::PI / 180.0).cos();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in ring {
        let x = p.lng * mlat * 111_320.0;
        let y = p.lat * 110_540.0;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    (max_x - min_x) * (max_y - min_y)
}

impl Opinion for P191ShapeOfIndoorSpace {
    fn name(&self) -> &'static str {
        "p191_shape_of_indoor_space"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p191".into(),
            display: "Alexander et al., A Pattern Language, Pattern 191 (The Shape of Indoor Space)".into(),
            url: Some("https://patternlanguage.cc/Patterns/The-Shape-of-Indoor-Space-(191)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let mut rectangularity_scores: Vec<bool> = Vec::new();
        let mut irregular: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &n.buildings {
            for c in &b.interior_cells {
                let aabb = aabb_area_m2(&c.polygon.outer);
                if aabb <= 0.0 {
                    continue;
                }
                let rectangularity = (c.polygon.area_m2() / aabb).min(1.0);
                let ok = rectangularity >= MIN_RECTANGULARITY;
                rectangularity_scores.push(ok);
                let key = format!("{}.{}", b.id, c.id);
                details.insert(format!("{key}.rectangularity"), format!("{rectangularity:.2}"));
                if !ok {
                    irregular.push(key);
                }
            }
        }

        if rectangularity_scores.is_empty() {
            return OpinionOutput::NoView {
                reason: "No InteriorCell with real ring geometry to check yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = rectangularity_scores.iter().filter(|b| **b).count() as f64 / rectangularity_scores.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("rectangularity_fraction".into(), value);
        details.insert("n_interior_cells".into(), rectangularity_scores.len().to_string());
        details.insert("min_rectangularity".into(), format!("{MIN_RECTANGULARITY:.2}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} interior cell(s); {:.0}% have real area/bounding-box rectangularity >= {:.0}%.",
                rectangularity_scores.len(), value * 100.0, MIN_RECTANGULARITY * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Rectangularity is axis-aligned (real area / axis-aligned-bounding-box area) -- a \
                 genuinely rectangular room rotated relative to the projection axes would score \
                 poorly even though it fully satisfies the pattern; this pipeline's cells are cut \
                 from axis-aligned footprints in practice, but the check itself can't see \
                 rotation.".into(),
                "Cannot check 'near right angles in the corners' directly (no per-corner angle \
                 measurement) or 'a roughly symmetrical vault over each room' -- no roof/ceiling \
                 geometry exists in this schema.".into(),
            ],
            contributing_features: irregular,
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
                layer_provenance: Default::default(), label: "P191 unit fixture".into(),
            },
        }
    }

    fn rect_cell(id: &str, w_m: f64, h_m: f64) -> InteriorCell {
        let m = 1.0 / 111_320.0;
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(vec![LngLat::new(0.0, 0.0), LngLat::new(w_m * m, 0.0), LngLat::new(w_m * m, h_m * m), LngLat::new(0.0, h_m * m), LngLat::new(0.0, 0.0)]),
            depth: 0.5, is_common: false, kind: "room".into(), connects_to: vec![], floor: 0,
        }
    }

    fn l_shaped_cell(id: &str) -> InteriorCell {
        let m = 1.0 / 111_320.0;
        // An L-shape: a 10x10 square with a 5x5 notch bitten out of one corner.
        let pts = [(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (5.0, 5.0), (5.0, 10.0), (0.0, 10.0), (0.0, 0.0)];
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(pts.iter().map(|(x, y)| LngLat::new(x * m, y * m)).collect()),
            depth: 0.5, is_common: false, kind: "room".into(), connects_to: vec![], floor: 0,
        }
    }

    fn building_with_cells(id: &str, cells: Vec<InteriorCell>) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![LngLat::new(0.0, 0.0), LngLat::new(20.0 * m, 0.0), LngLat::new(20.0 * m, 20.0 * m), LngLat::new(0.0, 20.0 * m), LngLat::new(0.0, 0.0)]),
            height_m: Some(6.0), typology: Some("p107_solid_v01".into()), year_built: None, parcel_id: None, floors: Some(2), openings: vec![], interior_cells: cells,
            wall_thickness_m: None,
            roof: None,
        }
    }

    #[test]
    fn no_interior_cells_is_no_view() {
        assert!(matches!(P191ShapeOfIndoorSpace.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_real_rectangle_scores_full() {
        let n = nbhd(vec![building_with_cells("B1", vec![rect_cell("R1", 6.0, 4.0)])]);
        let out = P191ShapeOfIndoorSpace.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn an_l_shaped_cell_is_flagged() {
        let n = nbhd(vec![building_with_cells("B1", vec![l_shaped_cell("R1")])]);
        let out = P191ShapeOfIndoorSpace.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value < 1.0, "got {value}");
                assert!(contributing_features.contains(&"B1.R1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
