//! P160 Building Edge — treat a building's perimeter as a real zone with
//! volume, not a flat line -- "deep enough to contain seats, bookshelves,
//! bay windows."
//!
//! From Alexander, *A Pattern Language*, Pattern 160 (p. 752), via
//! patternlanguage.cc/Patterns/Building-Edge-(160):
//! > **Problem:** A building is most often thought of as something which
//! > turns inward.
//! > **Solution:** Treat the edge of the building as a 'thing', a
//! > 'place', a zone with volume to it. Crenelate the edge of buildings
//! > with places that invite people to stop.
//!
//! # A real generator, keyed to real entrance data that already exists
//!
//! `p221_natural_doors_and_windows` already places every real door
//! `Opening` (`ring_index` + `t` position along that wall edge) --
//! Alexander's own "places that invite people to stop" reads naturally as
//! a real bay/niche flanking the building's own real entrance, not an
//! arbitrary point on an arbitrary wall. This operator places one real
//! `WallNiche` immediately beside each real door, on the SAME wall edge.
//!
//! **Runs after `p221_natural_doors_and_windows`**: needs real door
//! `Opening`s to flank. Doesn't touch `Building.wall_thickness_m` (P197's
//! own uniform scalar) -- `extra_depth_m` is additive local bulge, not a
//! replacement.
//!
//! Placement: immediately to the right of the door's own bay (toward
//! increasing `t`); if that would run off the end of the wall edge, tries
//! the left side instead; skips a door with no room on either side (a
//! real, short wall edge) rather than fabricating an out-of-bounds niche.
//! `niche_width_m`/`extra_depth_m` are plausible real bay/window-seat
//! figures -- Alexander's own text gives no precise dimension.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::ring_to_local;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Building, Neighborhood, OpeningKind, WallNiche};
use street_smarts_core::opinion::SourceCitation;

pub struct P160BuildingEdge;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P160Params {
    /// Real width of the niche placed beside each real door, in meters.
    pub niche_width_m: f64,
    /// A real gap between the door's own bay and the niche, in meters --
    /// keeps the niche from visually merging into the doorway itself.
    pub margin_m: f64,
    /// Extra depth beyond `Building.wall_thickness_m` at the niche's own
    /// span -- the real bay/seat bump-out.
    pub extra_depth_m: f64,
}

impl Parameters for P160Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float("niche_width_m", "Real width of the niche placed beside each real door.", 0.6, 2.5, 1.2).with_unit("m"),
            ParamSpec::float("margin_m", "Real gap between the door's own bay and the niche.", 0.0, 1.0, 0.3).with_unit("m"),
            ParamSpec::float("extra_depth_m", "Extra depth beyond the building's own uniform wall_thickness_m at the niche's span.", 0.2, 1.0, 0.5).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self { niche_width_m: 1.2, margin_m: 0.3, extra_depth_m: 0.5 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.niche_width_m, self.margin_m, self.extra_depth_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.niche_width_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.margin_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.extra_depth_m = s.clamp(*x); }
        p
    }
}

/// Real length in meters of outer-ring edge `ring_index`, local-projected
/// around the building's own centroid -- same technique
/// `p197_thick_walls`'s `min_bbox_dimension_m` uses.
fn edge_len_m(b: &Building, ring_index: usize) -> f64 {
    let bc = b.polygon.centroid();
    let origin = LngLat::new(bc.lng, bc.lat);
    let local = ring_to_local(&b.polygon.outer, &origin);
    let n = local.len();
    if n < 2 || ring_index >= n {
        return 0.0;
    }
    local[ring_index].dist(local[(ring_index + 1) % n])
}

impl PatternOperator for P160BuildingEdge {
    type Params = P160Params;

    fn name(&self) -> &'static str {
        "p160_building_edge"
    }
    fn description(&self) -> &'static str {
        "Places a real WallNiche flanking each real door opening, on the same wall edge."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p160".into(),
            display: "Alexander et al., A Pattern Language, Pattern 160 (Building Edge)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Building-Edge-(160)".into()),
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
            return Err("p160_building_edge only supports parcel_id \"*\" -- it re-scores every building in one pass.".into());
        }
        let any_doors = nbhd.buildings.iter().any(|b| b.openings.iter().any(|o| o.kind == OpeningKind::Door));
        if !any_doors {
            return Err("p160_building_edge: no building has a real door opening yet -- run p221_natural_doors_and_windows first.".into());
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced_building_ids: Vec<String> = Vec::new();
        let mut n_niched_buildings = 0usize;
        let mut n_niches = 0usize;
        let mut n_skipped_no_room = 0usize;

        for b in &nbhd.buildings {
            let doors: Vec<_> = b.openings.iter().filter(|o| o.kind == OpeningKind::Door).collect();
            if doors.is_empty() {
                continue;
            }
            let mut nb = b.clone();
            let mut placed_any = false;

            for door in &doors {
                let len = edge_len_m(b, door.ring_index);
                if len <= 0.0 {
                    continue;
                }
                let width_frac = (params.niche_width_m / len).min(0.4);
                let margin_frac = params.margin_m / len;
                let door_half_frac = (door.width_m / 2.0) / len;

                let right_start = door.t + door_half_frac + margin_frac;
                let right_end = right_start + width_frac;
                let left_end = door.t - door_half_frac - margin_frac;
                let left_start = left_end - width_frac;

                let (t_start, t_end) = if right_end <= 1.0 {
                    (right_start, right_end)
                } else if left_start >= 0.0 {
                    (left_start, left_end)
                } else {
                    n_skipped_no_room += 1;
                    continue;
                };

                nb.wall_niches.push(WallNiche {
                    ring_index: door.ring_index,
                    on_hole: door.on_hole,
                    t_start,
                    t_end,
                    extra_depth_m: params.extra_depth_m,
                });
                n_niches += 1;
                placed_any = true;
            }

            if placed_any {
                n_niched_buildings += 1;
                new_buildings.push(nb);
                replaced_building_ids.push(b.id.clone());
            }
        }

        if new_buildings.is_empty() {
            return Err("p160_building_edge: every real door's own wall edge was too short to fit a niche on either side.".into());
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!("Placed {n_niches} real wall niche(s) flanking a real door on {n_niched_buildings} building(s)."),
            steps: vec![format!(
                "{n_niched_buildings} building(s) got at least one real {:.1}m-wide niche beside a real door; {n_skipped_no_room} door(s) had no room on either side of their own wall edge and were skipped.",
                params.niche_width_m
            )],
            caveats: vec![
                "Only places niches beside real DOORS, not windows or other wall spans -- \
                 Alexander's own 'crenelate the edge' claim is broader than entrance-adjacent \
                 bays alone; this is a real, scoped subset, not the full pattern.".into(),
                "niche_width_m/extra_depth_m are plausible real bay/seat figures, not Alexander's \
                 own literal dimensions -- his text gives none.".into(),
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
    use street_smarts_core::nir::{NeighborhoodMeta, Opening, Street};

    fn m() -> f64 { 1.0 / 111_320.0 }

    fn building_with_door(id: &str, side_m: f64, door_t: f64, door_width_m: f64) -> Building {
        let mm = m();
        let s = (side_m / 2.0) * mm;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-s, -s), LngLat::new(s, -s), LngLat::new(s, s), LngLat::new(-s, s), LngLat::new(-s, -s),
            ]),
            height_m: Some(9.0), typology: Some("p107_solid_v01".into()),
            year_built: None, parcel_id: None, floors: Some(3),
            openings: vec![Opening {
                kind: OpeningKind::Door, ring_index: 0, on_hole: false,
                t: door_t, width_m: door_width_m, sill_height_m: 0.0, head_height_m: 2.1, floor: 0,
            }],
            interior_cells: vec![], wall_thickness_m: Some(0.3), roof: None,
            roof_segments: vec![], canopies: vec![], wall_niches: vec![],
        }
    }

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![] as Vec<Street>, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P160 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    #[test]
    fn no_doors_anywhere_is_an_error() {
        let b = Building {
            id: "B1".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-0.0001, -0.0001), LngLat::new(0.0001, -0.0001),
                LngLat::new(0.0001, 0.0001), LngLat::new(-0.0001, 0.0001), LngLat::new(-0.0001, -0.0001),
            ]),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None, floors: Some(1),
            openings: vec![], interior_cells: vec![], wall_thickness_m: None, roof: None,
            roof_segments: vec![], canopies: vec![], wall_niches: vec![],
        };
        let n = nbhd(vec![b]);
        assert!(P160BuildingEdge.apply(&n, "*", &P160Params::defaults(), 0).is_err());
    }

    #[test]
    fn a_centered_door_gets_a_niche_placed_to_its_right() {
        let n = nbhd(vec![building_with_door("B1", 20.0, 0.5, 1.0)]);
        let sub = P160BuildingEdge.apply(&n, "*", &P160Params::defaults(), 0).unwrap();
        assert_eq!(sub.new_buildings.len(), 1);
        let niches = &sub.new_buildings[0].wall_niches;
        assert_eq!(niches.len(), 1);
        assert_eq!(niches[0].ring_index, 0);
        assert!(niches[0].t_start > 0.5, "niche should sit to the right of the centered door, got t_start={}", niches[0].t_start);
        assert!(niches[0].t_end <= 1.0);
    }

    #[test]
    fn a_door_near_the_right_end_gets_a_niche_placed_to_its_left() {
        let n = nbhd(vec![building_with_door("B1", 20.0, 0.92, 1.0)]);
        let sub = P160BuildingEdge.apply(&n, "*", &P160Params::defaults(), 0).unwrap();
        let niches = &sub.new_buildings[0].wall_niches;
        assert_eq!(niches.len(), 1);
        assert!(niches[0].t_end < 0.92, "niche should sit to the left of the door near the wall's end, got t_end={}", niches[0].t_end);
        assert!(niches[0].t_start >= 0.0);
    }

    #[test]
    fn params_roundtrip() {
        let p = P160Params { niche_width_m: 1.5, margin_m: 0.4, extra_depth_m: 0.6 };
        let v = p.as_vector();
        let back = P160Params::from_vector(&v);
        assert_eq!(back.niche_width_m, 1.5);
        assert_eq!(back.extra_depth_m, 0.6);
    }
}
