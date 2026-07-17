//! P127 Intimacy Gradient — does each building's real front door (placed
//! by `p221_natural_doors_and_windows`) actually open into the cell
//! `p127_intimacy_gradient` itself marked as the most public (its lowest
//! `depth`)?
//!
//! From Alexander, *A Pattern Language*, Pattern 127:
//! > Lay out the spaces of a building so that they create a sequence
//! > which begins with the entrance and the most public parts of the
//! > building, then leads into the slightly more private areas, and
//! > finally to the most private domains.
//!
//! # Why this is a real check, not a tautology
//! Both operators independently compute "which way is the public realm"
//! from the same shared fact (`street-smarts-patterns::orientation::
//! nearest_public_realm_point`) -- but `p127_intimacy_gradient` uses it to
//! pick a continuous depth AXIS through the footprint's centroid, while
//! `p221_natural_doors_and_windows` uses it to pick a specific WALL EDGE
//! (its own `choose_door_wall`, weighing both facing-angle and edge
//! length). On an irregular or rotated footprint those two independent
//! picks can disagree: a door can land on a wall that P127's axis
//! actually placed in the DEEPEST band, not the shallowest one. That
//! divergence is exactly what this opinion looks for -- it never re-derives
//! either operator's own geometry, it only checks whether their two
//! independent answers agree on the ground.
//!
//! # What this checks
//! For every building with a multi-cell gradient (`interior_cells.len() >
//! 1`, so there's an actual public/private distinction to make) and at
//! least one ground-realm door (`Opening::kind == Door`, `on_hole ==
//! false` -- courtyard-ring doors don't exist in this pipeline, see
//! `p221_natural_doors_and_windows`), find the interior cell whose
//! centroid sits nearest the door's physical location, and check whether
//! that cell's `depth` is the building's own minimum. `value` is the
//! door-count-weighted fraction that qualify.
//!
//! Nearest-cell-by-centroid is a proxy for true point-in-polygon
//! containment (see `p106_positive_outdoor_space`'s own module doc for why
//! this crate keeps its own minimal local math rather than reaching into
//! `street-smarts-patterns`) -- reasonable for `p127`'s own band/bay
//! shapes, which are wide relative to a door's width, not exact.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::{Building, Neighborhood, OpeningKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P127IntimacyGradient;

/// Physical location of an opening: `t` along the edge from `ring[i]` to
/// `ring[i+1]` (wrapping), same convention `p221_natural_doors_and_windows`
/// writes and `p159_light_on_two_sides` already reads elsewhere in this
/// crate.
fn opening_point(ring: &[LngLat], ring_index: usize, t: f64) -> Option<LngLat> {
    let n = ring.len();
    if n < 2 {
        return None;
    }
    let a = ring[ring_index % n];
    let b = ring[(ring_index + 1) % n];
    Some(LngLat::new(
        a.lng + t * (b.lng - a.lng),
        a.lat + t * (b.lat - a.lat),
    ))
}

/// The interior cell whose centroid sits nearest `pt`, and whether its
/// depth equals the building's own minimum (within floating-point slop).
fn nearest_cell_is_most_public(b: &Building, pt: &LngLat) -> Option<bool> {
    if b.interior_cells.is_empty() {
        return None;
    }
    let min_depth = b
        .interior_cells
        .iter()
        .map(|c| c.depth)
        .fold(f64::INFINITY, f64::min);
    let nearest = b
        .interior_cells
        .iter()
        .min_by(|a, c| {
            let da = haversine_m(pt, &a.polygon.centroid());
            let dc = haversine_m(pt, &c.polygon.centroid());
            da.partial_cmp(&dc).unwrap_or(std::cmp::Ordering::Equal)
        })?;
    Some((nearest.depth - min_depth).abs() < 1e-6)
}

impl Opinion for P127IntimacyGradient {
    fn name(&self) -> &'static str {
        "p127_intimacy_gradient"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p127".into(),
            display: "Alexander et al., A Pattern Language, Pattern 127 (Intimacy Gradient)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl127/apl127.htm".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let candidates: Vec<&Building> = n
            .buildings
            .iter()
            .filter(|b| {
                b.interior_cells.len() > 1
                    && b.openings
                        .iter()
                        .any(|o| o.kind == OpeningKind::Door && !o.on_hole)
            })
            .collect();

        if candidates.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building has both a multi-cell intimacy gradient and a placed front \
                         door yet -- run p127_intimacy_gradient then p221_natural_doors_and_windows."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut n_doors = 0;
        let mut n_aligned = 0;
        let mut misaligned: Vec<String> = Vec::new();

        for b in &candidates {
            for door in b
                .openings
                .iter()
                .filter(|o| o.kind == OpeningKind::Door && !o.on_hole)
            {
                let Some(pt) = opening_point(&b.polygon.outer, door.ring_index, door.t) else {
                    continue;
                };
                let Some(qualifies) = nearest_cell_is_most_public(b, &pt) else {
                    continue;
                };
                n_doors += 1;
                if qualifies {
                    n_aligned += 1;
                } else {
                    misaligned.push(b.id.clone());
                }
            }
        }

        if n_doors == 0 {
            return OpinionOutput::NoView {
                reason: "Buildings have gradient and door data, but no door landed on a cell we \
                         could compare -- degenerate geometry."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let value = n_aligned as f64 / n_doors as f64;
        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_buildings_evaluated".into(), candidates.len().to_string());
        details.insert("n_doors_evaluated".into(), n_doors.to_string());
        details.insert("n_doors_aligned".into(), n_aligned.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} front door(s) across {} building(s): {:.0}% open into the cell their own \
                 building's intimacy gradient marked most public.",
                n_doors,
                candidates.len(),
                value * 100.0
            ),
            sub_scores: BTreeMap::new(),
            details,
            caveats: vec![
                "Nearest-cell-by-centroid is a proxy for true point-in-polygon containment, not an \
                 exact test -- reasonable for p127's own band/bay shapes, which are wide relative \
                 to a door's width."
                    .into(),
                "p127_intimacy_gradient and p221_natural_doors_and_windows both derive their \
                 orientation from the same nearest_public_realm_point fact, but pick DIFFERENT \
                 things with it (a continuous depth axis vs. a specific wall edge) -- this opinion \
                 is the only place in the pipeline that checks whether those two independent picks \
                 actually agree on the ground."
                    .into(),
                "Only ground-realm doors are checked (on_hole == false) -- this pipeline never \
                 places a door on a courtyard hole ring, so there's nothing on_hole == true to \
                 evaluate.".into(),
                "A single-cell building (no real gradient to speak of) is excluded from both the \
                 numerator and denominator, not scored as a failure.".into(),
            ],
            contributing_features: misaligned,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{InteriorCell, NeighborhoodMeta, Opening};

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.001, 0.001],
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
                label: "P127 opinion fixture".into(),
            },
        }
    }

    /// Three 10m bands along x in [0,30], y in [0,10] -- band 0 (x in
    /// [0,10]) is depth 0.0 (most public), band 2 (x in [20,30]) is depth
    /// 1.0 (deepest).
    fn three_band_building(id: &str) -> Building {
        let m = 1.0 / 111_320.0;
        let band = |x0: f64, x1: f64, depth: f64, cell_id: &str| InteriorCell {
            id: cell_id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x0 * m, 0.0),
                LngLat::new(x1 * m, 0.0),
                LngLat::new(x1 * m, 10.0 * m),
                LngLat::new(x0 * m, 10.0 * m),
                LngLat::new(x0 * m, 0.0),
            ]),
            depth,
            is_common: false,
            kind: "room".into(),
            connects_to: vec![],
            floor: 0,
        };
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(30.0 * m, 0.0),
                LngLat::new(30.0 * m, 10.0 * m),
                LngLat::new(0.0, 10.0 * m),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(7.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![],
            interior_cells: vec![
                band(0.0, 10.0, 0.0, "c0"),
                band(10.0, 20.0, 0.5, "c1"),
                band(20.0, 30.0, 1.0, "c2"),
            ],
        }
    }

    fn door(ring_index: usize, t: f64) -> Opening {
        Opening {
            kind: OpeningKind::Door,
            ring_index,
            on_hole: false,
            t,
            width_m: 1.0,
            sill_height_m: 0.0,
            head_height_m: 2.1,
            floor: 0,
        }
    }

    #[test]
    fn no_data_is_no_view() {
        let n = nbhd(vec![]);
        let out = P127IntimacyGradient.evaluate(&n);
        assert!(matches!(out, OpinionOutput::NoView { .. }));
    }

    #[test]
    fn door_on_the_public_wall_qualifies() {
        // Edge 0 runs (0,0) -> (30,0), the y=0 wall -- entirely inside
        // band c0 (x in [0,10]) near t=0.15 (x ~= 4.5).
        let mut b = three_band_building("B1");
        b.openings = vec![door(0, 0.15)];
        let n = nbhd(vec![b]);
        let out = P127IntimacyGradient.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn door_on_the_deep_wall_does_not_qualify() {
        // Same edge 0, but at t=0.85 (x ~= 25.5) -- lands in band c2, the
        // DEEPEST band, not the most public one.
        let mut b = three_band_building("B1");
        b.openings = vec![door(0, 0.85)];
        let n = nbhd(vec![b]);
        let out = P127IntimacyGradient.evaluate(&n);
        match out {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value < 1e-9, "got {value}");
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn mixed_doors_produce_a_fraction() {
        let mut good = three_band_building("GOOD");
        good.openings = vec![door(0, 0.15)];
        let mut bad = three_band_building("BAD");
        bad.openings = vec![door(0, 0.85)];
        let n = nbhd(vec![good, bad]);
        let out = P127IntimacyGradient.evaluate(&n);
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 0.5).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn single_cell_building_is_excluded_not_penalized() {
        let mut b = three_band_building("SOLO");
        b.interior_cells = vec![b.interior_cells[0].clone()];
        b.openings = vec![door(0, 0.5)];
        let n = nbhd(vec![b]);
        let out = P127IntimacyGradient.evaluate(&n);
        assert!(matches!(out, OpinionOutput::NoView { .. }));
    }
}
