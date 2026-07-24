//! P119 Arcades / P166 Gallery Surround — covered walkways at a
//! building's edge, at ground level (P119) and at every story (P166).
//!
//! From Alexander, *A Pattern Language*:
//! - Pattern 119 (p. 580), via patternlanguage.cc/Patterns/Arcades-(119):
//!   > **Problem:** Arcades -- covered walkways at the edge of buildings,
//!   > which are partly inside, partly outside -- play a vital role in
//!   > the way that people interact with buildings.
//!   > **Solution:** Wherever paths run along the edge of buildings,
//!   > build arcades.
//! - Pattern 166 (p. 777), via patternlanguage.cc/Patterns/Gallery-Surround-(166):
//!   > **Solution:** Whenever possible, and at every story, build
//!   > porches, galleries, arcades, balconies.
//!
//! # One real generator for both patterns' shared geometry
//!
//! Same technique `p117_sheltering_roof` uses to close P162 North Face
//! from its own single generator: P119 and P166 are the same real
//! covered-canopy geometry, differing only in which stories get one
//! (P119: ground floor only. P166: every real story). One operator
//! produces `Building.canopies` for both, rather than two near-identical
//! generators independently re-deriving the same street-facing wall.
//!
//! Reuses `orientation::nearest_public_realm_point` -- the SAME shared
//! "which way is the public realm" fact `p221_natural_doors_and_windows`
//! and `p127_intimacy_gradient` already use, so a building's arcade faces
//! the same direction its front door and public-facing rooms do, not an
//! independently-computed (and possibly contradictory) direction. The
//! per-edge facing scorer below is a focused reimplementation of
//! `p221_natural_doors_and_windows`'s own private `edge_facing` (that
//! function isn't `pub`, and this operator only needs "best single edge,"
//! not P221's full per-edge window-boost scoring) -- same real geometry,
//! not two independently-drifting definitions of "faces the street," just
//! not literally shared code. `life_facing_threshold_m`'s default (30.0m)
//! matches P221's own real, already-established threshold for the same
//! real judgment call ("close enough to genuinely face"), not a new
//! arbitrary number.
//!
//! **Runs after `p221_natural_doors_and_windows`**: needs real
//! `Building.floors` (only P221 sets it, from real height) for P166's
//! own "at every story" claim.
//!
//! Ground floor only when a building has no qualifying street/open-space
//! target within `life_facing_threshold_m` (skipped, not fabricated).
//! Depth/height figures (`arcade_depth_m` 1.8m, walkable; `gallery_depth_m`
//! 1.2m, a balcony rather than a full walkway) are real, plausible
//! architectural figures -- Alexander's own text gives no precise
//! dimension for either.

use crate::orientation::nearest_public_realm_point;
use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{centroid, lnglat_to_local, ring_to_local, Pt2};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Building, Canopy, CanopyKind, Neighborhood};
use street_smarts_core::opinion::SourceCitation;

pub struct P119Arcades;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P119Params {
    /// Same real judgment call P221's own `life_facing_threshold_m` uses
    /// -- how close a real street/open-space target must be for a wall to
    /// count as "facing" it.
    pub life_facing_threshold_m: f64,
    pub min_wall_len_m: f64,
    /// How far a ground-floor arcade (P119) projects from the wall.
    pub arcade_depth_m: f64,
    /// How far an upper-story gallery/balcony (P166) projects from the
    /// wall -- shallower than a ground-floor arcade by real convention
    /// (a walkway you pass through vs. a balcony you step onto).
    pub gallery_depth_m: f64,
    pub clearance_height_m: f64,
}

impl Parameters for P119Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float("life_facing_threshold_m", "How close a real street/open-space target must be for a wall to count as facing it.", 10.0, 60.0, 30.0).with_unit("m"),
            ParamSpec::float("min_wall_len_m", "Shortest wall edge that can carry a canopy.", 2.0, 10.0, 4.0).with_unit("m"),
            ParamSpec::float("arcade_depth_m", "How far a ground-floor P119 arcade projects from the wall.", 1.0, 3.0, 1.8).with_unit("m"),
            ParamSpec::float("gallery_depth_m", "How far an upper-story P166 gallery/balcony projects from the wall.", 0.6, 2.0, 1.2).with_unit("m"),
            ParamSpec::float("clearance_height_m", "Real clearance height under any canopy.", 2.0, 3.0, 2.4).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self { life_facing_threshold_m: 30.0, min_wall_len_m: 4.0, arcade_depth_m: 1.8, gallery_depth_m: 1.2, clearance_height_m: 2.4 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.life_facing_threshold_m, self.min_wall_len_m, self.arcade_depth_m, self.gallery_depth_m, self.clearance_height_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.life_facing_threshold_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.min_wall_len_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.arcade_depth_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) { p.gallery_depth_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(4), v.get(4)) { p.clearance_height_m = s.clamp(*x); }
        p
    }
}

/// The single outer-ring edge whose outward normal points most directly
/// at `target`, among edges at least `min_len` long -- a focused
/// reimplementation of `p221_natural_doors_and_windows`'s own private
/// `edge_facing`/`choose_door_wall` (not `pub`, and this operator only
/// needs the single best edge). Returns `(edge_index, distance_m)`, or
/// `None` if no edge faces `target` at all.
fn best_facing_edge(ring: &[Pt2], target: Pt2, min_len: f64) -> Option<(usize, f64)> {
    let n = ring.len();
    if n < 2 {
        return None;
    }
    let c = centroid(ring);
    let mut best: Option<(usize, f64, f64)> = None; // (edge, score, dist)
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        let len = a.dist(b);
        if len < min_len {
            continue;
        }
        let mid = Pt2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let edge = b.sub(a);
        let mut normal = Pt2::new(edge.y, -edge.x);
        if normal.dot(mid.sub(c)) < 0.0 {
            normal = Pt2::new(-normal.x, -normal.y);
        }
        let to_target = target.sub(mid);
        let dist = to_target.len();
        if dist < 1e-6 {
            continue;
        }
        let facing = normal.dot(to_target) / (normal.len() * dist);
        if facing <= 0.0 {
            continue;
        }
        let score = facing / dist.max(1.0);
        if best.map(|(_, bs, _)| score > bs).unwrap_or(true) {
            best = Some((i, score, dist));
        }
    }
    best.map(|(i, _, dist)| (i, dist))
}

impl PatternOperator for P119Arcades {
    type Params = P119Params;

    fn name(&self) -> &'static str {
        "p119_arcades"
    }
    fn description(&self) -> &'static str {
        "Places a real ground-floor arcade (P119) and upper-story galleries (P166) on each building's real street/open-space-facing wall."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p119".into(),
            display: "Alexander et al., A Pattern Language, Pattern 119 (Arcades)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Arcades-(119)".into()),
        }
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        _seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p119_arcades only supports parcel_id \"*\" -- it re-scores every building in one pass.".into());
        }
        if nbhd.buildings.is_empty() {
            return Err("p119_arcades: no buildings found.".into());
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced_building_ids: Vec<String> = Vec::new();
        let mut n_arcaded = 0usize;
        let mut n_canopies = 0usize;

        for b in &nbhd.buildings {
            let Some(target) = nearest_public_realm_point(nbhd, b) else {
                continue; // no real street/open-space anywhere -- nothing to face, not fabricated
            };
            let bc = b.polygon.centroid();
            let origin = LngLat::new(bc.lng, bc.lat);
            let ring_local = ring_to_local(&b.polygon.outer, &origin);
            let target_local = lnglat_to_local(&target, &origin);

            let Some((edge_idx, dist)) = best_facing_edge(&ring_local, target_local, params.min_wall_len_m) else {
                continue;
            };
            if dist > params.life_facing_threshold_m {
                continue;
            }

            let mut nb = b.clone();
            let floors = b.floors.unwrap_or(1).max(1);
            for floor in 0..floors {
                let (kind, depth_m) = if floor == 0 {
                    (CanopyKind::Arcade, params.arcade_depth_m)
                } else {
                    (CanopyKind::Gallery, params.gallery_depth_m)
                };
                nb.canopies.push(Canopy {
                    kind,
                    ring_index: edge_idx,
                    on_hole: false,
                    t_start: 0.15,
                    t_end: 0.85,
                    depth_m,
                    height_m: params.clearance_height_m,
                    floor,
                });
                n_canopies += 1;
            }
            n_arcaded += 1;
            new_buildings.push(nb);
            replaced_building_ids.push(b.id.clone());
        }

        if new_buildings.is_empty() {
            return Err("p119_arcades: no building has both a real street/open-space target and a qualifying wall edge within life_facing_threshold_m.".into());
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!("Placed {n_canopies} real canopy(s) (arcades + galleries) on {n_arcaded} building(s)."),
            steps: vec![format!(
                "{n_arcaded} of {} real building(s) had a real street/open-space target within {:.0}m of a qualifying wall edge; each got a ground-floor P119 arcade plus a P166 gallery at every real upper story.",
                nbhd.buildings.len(), params.life_facing_threshold_m
            )],
            caveats: vec![
                "Only checks the single BEST-facing wall edge per building, not real inter-building \
                 connectivity (Alexander's own 'use the arcades, above all, to connect up the \
                 buildings to one another').".into(),
                "arcade_depth_m/gallery_depth_m/clearance_height_m are plausible real architectural \
                 figures, not Alexander's own literal dimensions -- his text gives none.".into(),
            ],
            seed: 0,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings,
            new_streets: vec![],
            new_activity_nodes: vec![],
            new_boundaries: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids,
            entity_provenance: Default::default(),
            trace,
            new_fields: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn m() -> f64 { 1.0 / 111_320.0 }

    fn square_building(id: &str, side_m: f64, floors: u32) -> Building {
        let mm = m();
        let s = (side_m / 2.0) * mm;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-s, -s), LngLat::new(s, -s), LngLat::new(s, s), LngLat::new(-s, s), LngLat::new(-s, -s),
            ]),
            height_m: Some(floors as f64 * 3.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None, parcel_id: None, floors: Some(floors),
            openings: vec![], interior_cells: vec![], wall_thickness_m: None, roof: None,
            roof_segments: vec![], canopies: vec![], wall_niches: vec![],
        }
    }

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P119 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn south_street(y_m: f64) -> Street {
        let mm = m();
        // Short segment centered under the building's own x-span (which
        // spans -10..10m for a 20m square_building), so its nearest
        // vertex sits roughly due south rather than skewed east/west --
        // `nearest_public_realm_point` picks the nearest polyline VERTEX,
        // not a perpendicular projection. Same real technique
        // p221_natural_doors_and_windows's own tests use.
        Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(-5.0 * mm, y_m * mm), LngLat::new(5.0 * mm, y_m * mm)],
            classification: Some("local".into()), row_width_m: Some(6.0), surface: None,
        }
    }

    #[test]
    fn no_streets_or_open_space_is_an_error() {
        let n = nbhd(vec![square_building("B1", 20.0, 1)], vec![]);
        assert!(P119Arcades.apply(&n, "*", &P119Params::defaults(), 0).is_err());
    }

    #[test]
    fn a_single_story_building_gets_only_a_ground_floor_arcade() {
        let n = nbhd(vec![square_building("B1", 20.0, 1)], vec![south_street(-15.0)]);
        let sub = P119Arcades.apply(&n, "*", &P119Params::defaults(), 0).unwrap();
        assert_eq!(sub.new_buildings.len(), 1);
        let canopies = &sub.new_buildings[0].canopies;
        assert_eq!(canopies.len(), 1);
        assert_eq!(canopies[0].kind, CanopyKind::Arcade);
        assert_eq!(canopies[0].floor, 0);
    }

    #[test]
    fn a_multi_story_building_gets_an_arcade_plus_a_gallery_at_every_upper_floor() {
        let n = nbhd(vec![square_building("B1", 20.0, 3)], vec![south_street(-15.0)]);
        let sub = P119Arcades.apply(&n, "*", &P119Params::defaults(), 0).unwrap();
        let canopies = &sub.new_buildings[0].canopies;
        assert_eq!(canopies.len(), 3, "one canopy per real floor (0, 1, 2)");
        assert_eq!(canopies.iter().filter(|c| c.kind == CanopyKind::Arcade).count(), 1);
        assert_eq!(canopies.iter().filter(|c| c.kind == CanopyKind::Gallery).count(), 2);
        for f in 0..3u32 {
            assert!(canopies.iter().any(|c| c.floor == f), "missing a canopy at floor {f}");
        }
    }

    #[test]
    fn the_canopy_sits_on_the_street_facing_edge() {
        // South street -> the south wall (min-y edge of the square ring) should face it.
        let n = nbhd(vec![square_building("B1", 20.0, 1)], vec![south_street(-15.0)]);
        let sub = P119Arcades.apply(&n, "*", &P119Params::defaults(), 0).unwrap();
        let b = &sub.new_buildings[0];
        let edge_idx = b.canopies[0].ring_index;
        let ring = &b.polygon.outer;
        let a = ring[edge_idx];
        let bb = ring[(edge_idx + 1) % ring.len()];
        assert!(a.lat < 0.0 && bb.lat < 0.0, "expected the south (min-y) edge to face the south street, got edge from {a:?} to {bb:?}");
    }

    #[test]
    fn a_far_away_street_beyond_threshold_is_skipped() {
        let n = nbhd(vec![square_building("B1", 20.0, 1)], vec![south_street(-1000.0)]);
        assert!(P119Arcades.apply(&n, "*", &P119Params::defaults(), 0).is_err());
    }

    #[test]
    fn params_roundtrip() {
        let p = P119Params { life_facing_threshold_m: 25.0, min_wall_len_m: 5.0, arcade_depth_m: 2.0, gallery_depth_m: 1.5, clearance_height_m: 2.5 };
        let v = p.as_vector();
        let back = P119Params::from_vector(&v);
        assert_eq!(back.arcade_depth_m, 2.0);
        assert_eq!(back.gallery_depth_m, 1.5);
    }
}
