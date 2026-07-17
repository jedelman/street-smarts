//! P128 Indoor Sunlight — does the building's own common area (the room
//! `p129_common_areas_at_the_heart` flagged as the shared heart) actually
//! get a real, south-facing window, or does its light come from whichever
//! wall happened to face the street?
//!
//! From Alexander, *A Pattern Language*, Pattern 128 (Indoor Sunlight):
//! sunlight matters most for the rooms people actually spend their active
//! daylight hours in, and a building whose plan doesn't put those rooms on
//! its sunny (south, in the northern hemisphere -- this pipeline's fixture
//! data is all Norfolk/NYC/Seattle, all northern-hemisphere) side leaves
//! them permanently in shadow regardless of how many windows it has
//! elsewhere.
//!
//! **Citation note:** `patternlanguage.com`'s sample/direct pages for
//! Pattern 128 returned 404 this session (`/apl/aplsample/apl128/apl128.htm`
//! and the newer `/apl/direct-128.htm` both dead -- the same "Full Hypertext
//! Available to Members Only" wall `p221_natural_doors_and_windows`'s own
//! module doc already hit for its own sub-references). The mechanism above
//! is this codebase's best-effort paraphrase of the pattern's well-known
//! substance, not a verified block quote -- unlike `p127`/`p129`/`p131`'s
//! citations, which WERE checked against a live source page. Treat the
//! `SourceCitation` url below as "where this should be verified," not
//! "where this was verified."
//!
//! # Why this is a real check, not a tautology
//! Nothing upstream in this pipeline ever reasons about compass direction.
//! `p127_intimacy_gradient`'s depth axis points toward the nearest street
//! or open space (the PUBLIC realm), which has no necessary relationship to
//! true south -- a building can face its entrance dead north and still be
//! geometrically correct by every upstream pattern's own criteria. This
//! opinion is the only place in the chorus that ever asks "which way is
//! south," and it asks the question Alexander actually cares about: does
//! the room that matters most (the common area, P129's own proxy for "where
//! people spend their time together") get real south light, not just
//! whichever window the entrance orientation happened to produce.
//!
//! # What this checks
//! For every building with a `p129`-flagged common-area cell and real
//! window data, find the window bay(s) nearest that cell (same
//! nearest-centroid proxy `p127_intimacy_gradient`'s own opinion uses), and
//! test whether ANY of them sits on a wall edge whose outward normal points
//! predominantly south (south component of the normal exceeds its east/west
//! component -- a loose 90-degree cone centered on due south, not a precise
//! solar-path calculation this pipeline has no season/latitude model to
//! support). `value` is the building-count-weighted fraction that qualify.

use std::collections::BTreeMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Building, Neighborhood, OpeningKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P128IndoorSunlight;

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

/// Outward normal of wall edge `ring_index` (the edge `ring[i] -> ring[i+1]`),
/// as a raw (dlng, dlat) direction -- not unit length, sign/dominance only.
/// Same "which perpendicular points away from the centroid" technique
/// `p127_intimacy_gradient::depth_axis`'s own longest-edge fallback uses,
/// done directly in degree space since only the SIGN of the south
/// component matters here, not a metric distance -- this crate stays
/// decoupled from `street-smarts-patterns`'s local-projection math (see
/// `p106_positive_outdoor_space`'s own module doc for the same principle).
fn outward_normal(ring: &[LngLat], ring_index: usize, centroid: &LngLat) -> Option<(f64, f64)> {
    let n = ring.len();
    if n < 2 {
        return None;
    }
    let a = ring[ring_index % n];
    let b = ring[(ring_index + 1) % n];
    let (edx, edy) = (b.lng - a.lng, b.lat - a.lat);
    let (nx, ny) = (edy, -edx);
    let mid = LngLat::new((a.lng + b.lng) / 2.0, (a.lat + b.lat) / 2.0);
    let (tox, toy) = (mid.lng - centroid.lng, mid.lat - centroid.lat);
    if nx * tox + ny * toy >= 0.0 {
        Some((nx, ny))
    } else {
        Some((-nx, -ny))
    }
}

fn is_south_facing(outward: (f64, f64)) -> bool {
    outward.1 < 0.0 && outward.1.abs() > outward.0.abs()
}

impl Opinion for P128IndoorSunlight {
    fn name(&self) -> &'static str {
        "p128_indoor_sunlight"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_128".into(),
            display: "Alexander et al., A Pattern Language, Pattern 128 (Indoor Sunlight) -- \
                      paraphrased; patternlanguage.com's sample pages for this pattern 404'd \
                      this session, see this opinion's own module doc."
                .into(),
            url: Some("https://www.patternlanguage.com/apl/direct-128.htm".into()),
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
                b.interior_cells.iter().any(|c| c.is_common)
                    && b.openings.iter().any(|o| o.kind == OpeningKind::Window && !o.on_hole)
            })
            .collect();

        if candidates.is_empty() {
            return OpinionOutput::NoView {
                reason: "No building has both a marked common area and street/yard-facing window \
                         data yet -- run p129_common_areas_at_the_heart then \
                         p221_natural_doors_and_windows."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut n_qualify = 0;
        let mut shaded: Vec<String> = Vec::new();

        for b in &candidates {
            let centroid = b.polygon.centroid();
            if !b.interior_cells.iter().any(|c| c.is_common) {
                continue;
            }

            let mut has_south_window = false;
            for o in b.openings.iter().filter(|o| o.kind == OpeningKind::Window && !o.on_hole) {
                let Some(pt) = opening_point(&b.polygon.outer, o.ring_index, o.t) else {
                    continue;
                };
                // Only consider windows whose nearest interior cell IS the
                // common area -- a south window on some other room doesn't
                // count, same "which cell does this opening belong to"
                // proxy p127's own opinion uses.
                let nearest_is_common = b
                    .interior_cells
                    .iter()
                    .min_by(|x, y| {
                        let dx = haversine_deg(&pt, &x.polygon.centroid());
                        let dy = haversine_deg(&pt, &y.polygon.centroid());
                        dx.partial_cmp(&dy).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|c| c.is_common)
                    .unwrap_or(false);
                if !nearest_is_common {
                    continue;
                }
                if let Some(normal) = outward_normal(&b.polygon.outer, o.ring_index, &centroid) {
                    if is_south_facing(normal) {
                        has_south_window = true;
                        break;
                    }
                }
            }
            if has_south_window {
                n_qualify += 1;
            } else {
                shaded.push(b.id.clone());
            }
        }

        let value = n_qualify as f64 / candidates.len() as f64;
        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_buildings_evaluated".into(), candidates.len().to_string());
        details.insert("n_south_lit".into(), n_qualify.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} of {} building(s) with a marked common area give it a real south-facing \
                 window.",
                n_qualify,
                candidates.len()
            ),
            sub_scores: BTreeMap::new(),
            details,
            caveats: vec![
                "\"South-facing\" is a loose 90-degree cone (the wall's outward normal has a \
                 bigger south component than east/west), not a solar-path calculation -- this \
                 pipeline has no season, latitude-angle, or shading model.".into(),
                "This pipeline never reasons about compass direction anywhere else -- the depth \
                 axis p127_intimacy_gradient uses points toward the public realm, which has no \
                 necessary relationship to true south. A building can be geometrically correct by \
                 every other pattern's own criteria and still fail this one.".into(),
                "Pattern 128's primary-source text could not be reverified this session \
                 (patternlanguage.com 404s for this pattern) -- see this opinion's own module doc."
                    .into(),
            ],
            contributing_features: shaded,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

/// Plain Euclidean distance in raw lng/lat degrees -- fine for a NEAREST
/// comparison (which of several candidates is closest), never used as a
/// real distance. Named distinctly from `haversine_m` so nobody mistakes
/// this for a metric answer.
fn haversine_deg(a: &LngLat, b: &LngLat) -> f64 {
    let dx = a.lng - b.lng;
    let dy = a.lat - b.lat;
    (dx * dx + dy * dy).sqrt()
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
                label: "P128 opinion fixture".into(),
            },
        }
    }

    /// A 20x10 rectangle: (0,0)-(20,0)-(20,10)-(0,10). Edge 0 is the SOUTH
    /// wall (y=0, outward normal points toward -y = south). Edge 2 is the
    /// NORTH wall (y=10, outward normal points toward +y = north).
    fn rect_building(id: &str) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(20.0 * m, 0.0),
                LngLat::new(20.0 * m, 10.0 * m),
                LngLat::new(0.0, 10.0 * m),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(7.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![],
            interior_cells: vec![InteriorCell {
                id: format!("{id}_cell_0"),
                polygon: Polygon::from_ring(vec![
                    LngLat::new(0.0, 0.0),
                    LngLat::new(20.0 * m, 0.0),
                    LngLat::new(20.0 * m, 10.0 * m),
                    LngLat::new(0.0, 10.0 * m),
                    LngLat::new(0.0, 0.0),
                ]),
                depth: 0.0,
                is_common: true,
                kind: "room".into(),
                connects_to: vec![],
                floor: 0,
            }],
        }
    }

    fn win(ring_index: usize, t: f64) -> Opening {
        Opening {
            kind: OpeningKind::Window,
            ring_index,
            on_hole: false,
            t,
            width_m: 1.5,
            sill_height_m: 1.0,
            head_height_m: 2.0,
            floor: 0,
        }
    }

    #[test]
    fn no_data_is_no_view() {
        let n = nbhd(vec![]);
        assert!(matches!(P128IndoorSunlight.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn south_window_on_the_common_room_qualifies() {
        let mut b = rect_building("B1");
        b.openings = vec![win(0, 0.5)]; // edge 0 = south wall
        let n = nbhd(vec![b]);
        match P128IndoorSunlight.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn north_only_window_does_not_qualify() {
        let mut b = rect_building("B1");
        b.openings = vec![win(2, 0.5)]; // edge 2 = north wall
        let n = nbhd(vec![b]);
        match P128IndoorSunlight.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!(value < 1e-9, "got {value}");
                assert!(contributing_features.contains(&"B1".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
