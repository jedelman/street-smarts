//! P131 The Flow Through Rooms — wire the actual room-to-room connectivity
//! `p127_intimacy_gradient`'s cells left empty, and (for solid buildings,
//! where the depth-band chain doesn't already close on itself) attempt a
//! real loop by adding one parallel passage cell -- capped at Pattern
//! 132's own cited length threshold.
//!
//! From Alexander, *A Pattern Language*, Pattern 131 (verified against the
//! primary text -- see README's Reference section for the link):
//! > The movement between rooms is as important as the rooms themselves;
//! > and its arrangement has as much effect on social interaction in the
//! > rooms, as the interiors of the rooms... As far as possible, avoid the
//! > use of corridors and passages. Instead, use public rooms and common
//! > rooms as rooms for movement and for gathering... place the common
//! > rooms to form a chain, or loop, so that it becomes possible to walk
//! > from room to room -- and so that private rooms open directly off
//! > these public rooms... Even better, is the case where there is a
//! > loop... A building where there is a chain of rooms in sequence also
//! > works like this, if there is a passage in parallel with the chain of
//! > rooms.
//!
//! Runs immediately after `p129_common_areas_at_the_heart` (127 < 129 <
//! 131, no reordering needed -- see `p127_intimacy_gradient`'s module doc
//! for the full sourced sequence).
//!
//! # Solid vs. courtyard, again
//! - **Courtyard buildings** get a real loop for free: the ring `p127`
//!   already sliced is closed by construction, so bay k just connects to
//!   bay (k+1) mod n, wrapping around. No passage cell needed.
//! - **Solid buildings** get a plain chain (band i -- band i+1) always,
//!   which Alexander's own text validates as legitimate on its own, not a
//!   degraded fallback. Closing it into a full loop needs an artificial
//!   passage alongside the chain -- attempted only when BOTH hold:
//!   - the chain's total depth-span is within the **verified, cited**
//!     Pattern 132 threshold (see `PASSAGE_MAX_LENGTH_M` below) -- a real
//!     number from the text, not a guess, so it's a constant here, not a
//!     tunable parameter;
//!   - the footprint is wide enough (`min_width_for_passage_m`, a real
//!     placeholder parameter, unlike the length cap) to carve off a strip
//!     without leaving the main bands too thin to be real rooms.
//!
//!   If either check fails, the plain chain is left as-is -- not replaced
//!   with a bare, dead corridor Pattern 132's own cited evidence (Spivack,
//!   *Hospital and Community Psychiatry*, 1967) says people find
//!   unnerving past that length.
//!
//! # Pattern 132's rule, folded in here rather than a separate operator
//! Pattern 132 (Short Passages) has no independent geometric decision
//! beyond the length cap and "treat it as a room" (furnished, lit) --
//! there's nothing left for a standalone P132 operator to generate that
//! isn't already covered by giving the passage cell the same `kind:
//! "room"`-adjacent treatment (`kind: "passage"`, still a real
//! `InteriorCell` with its own footprint) everything else here gets. Same
//! precedent as `p221_natural_doors_and_windows` folding in P192's
//! orientation heuristic without a separate P192 operator.
//!
//! # What this operator deliberately does NOT do
//! No use, ever -- same discipline as P127/P129. Connectivity is a graph
//! over cell ids; nothing here assumes what activity crosses a doorway.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{centroid, clip_half_plane, lnglat_to_local, local_to_ring, ring_to_local, Pt2};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::components::BuildingTypology;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::{Building, InteriorCell, Neighborhood};
use street_smarts_core::opinion::SourceCitation;

/// Pattern 132's own cited threshold (~50 feet), converted to metres. A
/// real number from the primary text (Spivack, *Hospital and Community
/// Psychiatry* 18(1), 1967, cited by Alexander) -- fixed, not a tunable
/// parameter, unlike the placeholders below.
const PASSAGE_MAX_LENGTH_M: f64 = 15.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P131Params {
    /// Width of a loop-closing passage cell. A placeholder in the same
    /// category as P221's door_width_m -- plausible, not sourced.
    pub passage_width_m: f64,
    /// Minimum average cross-width (footprint area / chain depth-span) a
    /// solid building needs before a parallel passage is even attempted --
    /// below this, carving off a strip would leave the main bands too
    /// thin to be real rooms.
    pub min_width_for_passage_m: f64,
}

impl Parameters for P131Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "passage_width_m",
                "Width of a loop-closing passage cell.",
                1.0, 3.0, 1.5,
            ).with_unit("m"),
            ParamSpec::float(
                "min_width_for_passage_m",
                "Minimum average cross-width before a parallel passage is attempted.",
                4.0, 20.0, 8.0,
            ).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self { passage_width_m: 1.5, min_width_for_passage_m: 8.0 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.passage_width_m, self.min_width_for_passage_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.passage_width_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.min_width_for_passage_m = s.clamp(*x); }
        p
    }
}

pub struct P131TheFlowThroughRooms;

impl PatternOperator for P131TheFlowThroughRooms {
    type Params = P131Params;

    fn name(&self) -> &'static str { "p131_the_flow_through_rooms" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p131".into(),
            display: "Alexander et al., A Pattern Language, Pattern 131 (The Flow Through Rooms)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl131/apl131.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Connect interior cells into a chain or loop -- courtyard buildings close for free, solid buildings get a passage when short and wide enough, per Pattern 132's cited length threshold."
    }

    /// `parcel_id` must be `"*"` -- targets every building with interior
    /// cells in one pass.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        _seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p131_the_flow_through_rooms only supports parcel_id \"*\" -- it runs on every building in one pass.".into());
        }
        let candidates: Vec<&Building> = nbhd.buildings.iter().filter(|b| !b.interior_cells.is_empty()).collect();
        if candidates.is_empty() {
            return Err("p131_the_flow_through_rooms: no building has interior cells -- run p127_intimacy_gradient first.".into());
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut n_courtyard_loops = 0;
        let mut n_solid_chains = 0;
        let mut n_closed_loops = 0;
        let mut n_single_cell = 0;

        for b in &candidates {
            let mut updated = (*b).clone();
            let is_courtyard = BuildingTypology::label_is_courtyard(b.typology.as_deref());

            if updated.interior_cells.len() < 2 {
                n_single_cell += 1;
                new_buildings.push(updated);
                replaced.push(b.id.clone());
                continue;
            }

            if is_courtyard {
                n_courtyard_loops += 1;
                let n = updated.interior_cells.len();
                let ids: Vec<String> = updated.interior_cells.iter().map(|c| c.id.clone()).collect();
                for i in 0..n {
                    let next = ids[(i + 1) % n].clone();
                    let prev = ids[(i + n - 1) % n].clone();
                    let cell = &mut updated.interior_cells[i];
                    cell.connects_to = vec![prev, next];
                }
                steps.push(format!("{}: {} bays connected in a closed loop (free, ring-shaped).", b.id, n));
            } else {
                n_solid_chains += 1;
                // Defensive: connect strictly by depth order, not Vec
                // position, in case that ever diverges from how p127 built it.
                let mut order: Vec<usize> = (0..updated.interior_cells.len()).collect();
                order.sort_by(|&a, &c| {
                    updated.interior_cells[a].depth.partial_cmp(&updated.interior_cells[c].depth).unwrap()
                });
                let ids: Vec<String> = order.iter().map(|&i| updated.interior_cells[i].id.clone()).collect();
                for w in order.windows(2) {
                    let (a, b2) = (w[0], w[1]);
                    let (id_a, id_b2) = (updated.interior_cells[a].id.clone(), updated.interior_cells[b2].id.clone());
                    updated.interior_cells[a].connects_to.push(id_b2);
                    updated.interior_cells[b2].connects_to.push(id_a);
                }

                let closed = try_close_loop(b, &mut updated, &ids, params);
                if closed {
                    n_closed_loops += 1;
                }
                steps.push(format!(
                    "{}: {} bands connected in a chain{}.",
                    b.id, updated.interior_cells.len() - if closed { 1 } else { 0 },
                    if closed { ", closed into a loop with one passage cell" } else { "" }
                ));
            }

            new_buildings.push(updated);
            replaced.push(b.id.clone());
        }

        let trace = SubdivisionTrace {
            operator_name: "p131_the_flow_through_rooms".into(),
            operator_source: self.source(),
            headline: format!(
                "Connected {} building(s): {} courtyard loop(s), {} solid chain(s) ({} closed into a full loop), {} single-cell (nothing to connect).",
                new_buildings.len(), n_courtyard_loops, n_solid_chains, n_closed_loops, n_single_cell
            ),
            steps,
            caveats: vec![
                "A solid building's loop-closing passage span is approximated from the first and \
                 last cell's centroid distance, and its cross-width from footprint area / that \
                 span -- proxies, not a re-derivation of p127_intimacy_gradient's own depth axis. \
                 Reasonable for a threshold check, not exact.".into(),
                "Only ONE passage is ever added, running the full length of the chain. A long, \
                 non-straight chain (an L-shaped or wrapped footprint) isn't handled -- the passage \
                 assumes the chain's overall direction is roughly a straight line.".into(),
                "No use, ever -- connectivity is a graph over cell ids. Nothing here assumes what \
                 activity crosses a doorway.".into(),
            ],
            seed: _seed,
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
            replaced_building_ids: replaced,
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
        })
    }
}

/// Attempt to close a solid building's chain into a loop by adding one
/// parallel passage cell, connected only to the first and last band.
/// Returns true if a passage was added. Mutates `updated.interior_cells`
/// in place (pushes the new passage cell, and appends its id to the first
/// and last band's `connects_to`).
fn try_close_loop(
    original: &Building,
    updated: &mut Building,
    ordered_ids: &[String],
    params: &P131Params,
) -> bool {
    let first = updated.interior_cells.iter().find(|c| &c.id == &ordered_ids[0]).unwrap().clone();
    let last = updated.interior_cells.iter().find(|c| &c.id == ordered_ids.last().unwrap()).unwrap().clone();

    let origin = original.polygon.centroid();
    let first_c = first.polygon.centroid();
    let last_c = last.polygon.centroid();
    let span_m = haversine_m(&first_c, &last_c);
    if span_m < 1e-6 || span_m > PASSAGE_MAX_LENGTH_M {
        return false;
    }

    let footprint_area_m2 = original.polygon.area_m2();
    let avg_width_m = footprint_area_m2 / span_m;
    if avg_width_m < params.min_width_for_passage_m {
        return false;
    }

    let outer_local = ring_to_local(&original.polygon.outer, &origin);
    if outer_local.len() < 3 {
        return false;
    }
    let first_local = lnglat_to_local(&first_c, &origin);
    let last_local = lnglat_to_local(&last_c, &origin);
    let axis_raw = last_local.sub(first_local);
    let axis_len = axis_raw.len();
    if axis_len < 1e-6 {
        return false;
    }
    let axis = Pt2::new(axis_raw.x / axis_len, axis_raw.y / axis_len);
    let perp = Pt2::new(-axis.y, axis.x);
    let c = centroid(&outer_local);

    // Full depth range (project all outer vertices onto axis) and full
    // cross-width range (project onto perp) -- the passage runs the whole
    // depth range, hugging one edge of the cross-width range.
    let (mut s_min, mut s_max) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut w_min, mut w_max) = (f64::INFINITY, f64::NEG_INFINITY);
    for &p in &outer_local {
        let d = p.sub(c);
        let s = d.dot(axis);
        let w = d.dot(perp);
        if s < s_min { s_min = s; }
        if s > s_max { s_max = s; }
        if w < w_min { w_min = w; }
        if w > w_max { w_max = w; }
    }
    if w_max - w_min < params.passage_width_m * 1.5 {
        return false; // not enough cross-width to carve a real strip
    }

    let w_hi = w_max;
    let w_lo = w_max - params.passage_width_m;
    let p_s_min = Pt2::new(c.x + s_min * axis.x, c.y + s_min * axis.y);
    let p_s_max = Pt2::new(c.x + s_max * axis.x, c.y + s_max * axis.y);
    let p_w_hi = Pt2::new(c.x + w_hi * perp.x, c.y + w_hi * perp.y);
    let p_w_lo = Pt2::new(c.x + w_lo * perp.x, c.y + w_lo * perp.y);

    // Clip the whole footprint down to the strip: s in [s_min,s_max] (a
    // no-op bound-wise, but clip_half_plane needs both edges) and w in
    // [w_lo, w_hi]. Same derivation as p127_intimacy_gradient::solid_bands:
    // "keep <= hi" pairs the HIGH threshold point with +perp; "keep >= lo"
    // pairs the LOW threshold point with -perp.
    let mut strip = clip_half_plane(&outer_local, p_s_max, Pt2::new(p_s_max.x + perp.x, p_s_max.y + perp.y));
    strip = clip_half_plane(&strip, p_s_min, Pt2::new(p_s_min.x - perp.x, p_s_min.y - perp.y));
    strip = clip_half_plane(&strip, p_w_lo, Pt2::new(p_w_lo.x + axis.x, p_w_lo.y + axis.y));
    strip = clip_half_plane(&strip, p_w_hi, Pt2::new(p_w_hi.x - axis.x, p_w_hi.y - axis.y));
    if strip.len() < 3 {
        return false;
    }

    let passage_id = format!("{}_passage", original.id);
    let passage = InteriorCell {
        id: passage_id.clone(),
        polygon: street_smarts_core::geometry::Polygon::from_ring(local_to_ring(&strip, &origin)),
        depth: 0.5, // spans the whole gradient -- not itself part of the public/private sequence
        is_common: false,
        kind: "passage".into(),
        connects_to: vec![first.id.clone(), last.id.clone()],
        floor: 0,
    };
    updated.interior_cells.push(passage);
    for c in updated.interior_cells.iter_mut() {
        if c.id == first.id || c.id == last.id {
            c.connects_to.push(passage_id.clone());
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{InteriorCell, NeighborhoodMeta};

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
                label: "P131 unit fixture".into(),
            },
        }
    }

    fn band(id: &str, x0: f64, x1: f64, depth: f64) -> InteriorCell {
        let m = 1.0 / 111_320.0;
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x0 * m, 0.0), LngLat::new(x1 * m, 0.0),
                LngLat::new(x1 * m, 20.0 * m), LngLat::new(x0 * m, 20.0 * m),
                LngLat::new(x0 * m, 0.0),
            ]),
            depth,
            is_common: false,
            kind: "room".into(),
            connects_to: vec![],
            floor: 0,
        }
    }

    fn solid_building(id: &str, width_m: f64, cells: Vec<InteriorCell>) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(width_m * m, 0.0),
                LngLat::new(width_m * m, 20.0 * m), LngLat::new(0.0, 20.0 * m),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(7.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![],
            interior_cells: cells,
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    #[test]
    fn short_wide_building_gets_a_closed_loop() {
        // 12m deep total (well under the 15m cap), 20m wide -- should close.
        let cells = vec![band("c0", 0.0, 4.0, 0.0), band("c1", 4.0, 8.0, 0.5), band("c2", 8.0, 12.0, 1.0)];
        let b = solid_building("B1", 12.0, cells);
        let n = nbhd(vec![b]);
        let sub = P131TheFlowThroughRooms.apply(&n, "*", &P131Params::defaults(), 1).expect("should run");
        let result = &sub.new_buildings[0];
        assert_eq!(result.interior_cells.len(), 4, "should have 3 bands + 1 passage");
        let passage = result.interior_cells.iter().find(|c| c.kind == "passage").expect("passage should exist");
        assert_eq!(passage.connects_to.len(), 2);
        let c0 = result.interior_cells.iter().find(|c| c.id == "c0").unwrap();
        assert!(c0.connects_to.contains(&passage.id), "first band should connect to the passage");
        let c2 = result.interior_cells.iter().find(|c| c.id == "c2").unwrap();
        assert!(c2.connects_to.contains(&passage.id), "last band should connect to the passage");
        assert!(c0.connects_to.contains(&"c1".to_string()), "chain adjacency should still be there too");
    }

    #[test]
    fn deep_building_stays_a_plain_chain_no_passage() {
        // 60m deep total -- well over the 15m cap, should NOT close.
        let cells = vec![band("c0", 0.0, 20.0, 0.0), band("c1", 20.0, 40.0, 0.5), band("c2", 40.0, 60.0, 1.0)];
        let b = solid_building("B1", 60.0, cells);
        let n = nbhd(vec![b]);
        let sub = P131TheFlowThroughRooms.apply(&n, "*", &P131Params::defaults(), 1).expect("should run");
        let result = &sub.new_buildings[0];
        assert_eq!(result.interior_cells.len(), 3, "no passage should be added");
        assert!(result.interior_cells.iter().all(|c| c.kind != "passage"));
        let c0 = result.interior_cells.iter().find(|c| c.id == "c0").unwrap();
        assert_eq!(c0.connects_to, vec!["c1".to_string()], "still a plain chain");
    }

    #[test]
    fn courtyard_bays_connect_in_a_wrapped_loop() {
        let m = 1.0 / 111_320.0;
        let mk = |id: &str| InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(1.0 * m, 0.0),
                LngLat::new(1.0 * m, 1.0 * m), LngLat::new(0.0, 1.0 * m),
                LngLat::new(0.0, 0.0),
            ]),
            depth: 0.0,
            is_common: false,
            kind: "room".into(),
            connects_to: vec![],
            floor: 0,
        };
        let b = Building {
            id: "CY1".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(40.0 * m, 0.0),
                LngLat::new(40.0 * m, 40.0 * m), LngLat::new(0.0, 40.0 * m),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(7.0),
            typology: Some("p107_courtyard_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![],
            interior_cells: vec![mk("bay_0"), mk("bay_1"), mk("bay_2"), mk("bay_3")],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], };
        let n = nbhd(vec![b]);
        let sub = P131TheFlowThroughRooms.apply(&n, "*", &P131Params::defaults(), 1).expect("should run");
        let cells = &sub.new_buildings[0].interior_cells;
        assert_eq!(cells[0].connects_to.len(), 2, "each bay should connect to exactly two neighbors in a closed loop");
        assert!(cells[0].connects_to.contains(&"bay_1".to_string()));
        assert!(cells[0].connects_to.contains(&"bay_3".to_string()), "should wrap around to the last bay");
    }

    #[test]
    fn no_interior_cells_anywhere_is_an_error() {
        let n = nbhd(vec![]);
        assert!(P131TheFlowThroughRooms.apply(&n, "*", &P131Params::defaults(), 1).is_err());
    }
}
