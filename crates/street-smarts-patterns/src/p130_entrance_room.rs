//! P130 Entrance Room — mark the cell `p127_intimacy_gradient` built at
//! depth 0.0 (the public-facing band/bay, guaranteed unique by that
//! operator's own construction -- see the module doc there) as a real
//! `kind: "entrance"` cell, not just another undifferentiated `"room"`.
//!
//! From Alexander, *A Pattern Language*, Pattern 130 (Entrance Room):
//! a building's entrance should open into a real, modest room of its own
//! -- a place to arrive, not a hallway you pass through and not the
//! private heart of the building you're dropped straight into.
//!
//! **Citation note:** `patternlanguage.com`'s sample/direct pages for
//! Pattern 130 returned 404 this session (both `/apl/aplsample/apl130/
//! apl130.htm` and the newer `/apl/direct-130.htm`) -- the same "Full
//! Hypertext Available to Members Only" wall `p221_natural_doors_and_
//! windows`'s own module doc already hit for its own sub-references, and
//! `p128_indoor_sunlight`'s opinion hit for Pattern 128. The description
//! above is this codebase's best-effort paraphrase of the pattern's
//! well-known substance, not a verified block quote.
//!
//! Runs immediately after `p127_intimacy_gradient`, before
//! `p129_common_areas_at_the_heart` -- Alexander's own cited sequence
//! (127 -> 128 -> 129 -> 130 -> 131...) actually places 130 AFTER 129, but
//! this operator never changes cell geometry or count (see "What this
//! operator deliberately does NOT do" below), so nothing about P129's own
//! center-of-gravity computation depends on whether the entrance cell has
//! been relabeled yet. Running it here, right next to the operator whose
//! output it reads (`depth == 0.0`), keeps the two next to each other in
//! the pipeline instead of splitting a tightly-coupled pair across P129.
//!
//! # What this operator deliberately does NOT do
//! **No resizing.** Alexander's text calls for a "modest" room, distinct
//! in character from the rooms beyond it -- which would mean giving it its
//! own real dimension, not just a label. This operator does not do that:
//! `p127_intimacy_gradient`'s bands/bays are uniformly sized by
//! construction (its own module doc already flags this: "not derived from
//! any real room program"), and reshaping just the entrance cell without
//! either a verified target dimension (blocked by the same 404 above) or
//! risking a fragile ad hoc geometry cut was judged worse than being
//! honest about the gap. What this DOES give the rest of the pipeline: a
//! real `kind` distinction a renderer or opinion can key off of, where
//! before every cell was indistinguishable `"room"` regardless of role.
//! Sizing the entrance for real is future work (either a dedicated
//! `entrance_depth_m` parameter threaded through `p127`'s own band/bay
//! construction, or a boundary-aware clip here once there's a verified
//! target number to clip to).

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::{Building, Neighborhood};
use street_smarts_core::opinion::SourceCitation;

/// No tunable parameters -- "the cell at depth 0.0" has no free variable
/// to expose. Same hand-shaped `NoParams` convention as `P129Params`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P130Params;

impl Parameters for P130Params {
    fn schema() -> Vec<ParamSpec> {
        vec![]
    }
    fn defaults() -> Self {
        Self
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![]
    }
    fn from_vector(_v: &[f64]) -> Self {
        Self
    }
}

pub struct P130EntranceRoom;

impl PatternOperator for P130EntranceRoom {
    type Params = P130Params;

    fn name(&self) -> &'static str {
        "p130_entrance_room"
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_130".into(),
            display: "Alexander et al., A Pattern Language, Pattern 130 (Entrance Room) -- \
                      paraphrased; patternlanguage.com's sample pages for this pattern 404'd \
                      this session, see this operator's own module doc."
                .into(),
            url: Some("https://www.patternlanguage.com/apl/direct-130.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Tag the cell p127_intimacy_gradient placed at depth 0.0 as a real entrance room, not an undifferentiated band/bay."
    }

    /// `parcel_id` must be `"*"` -- targets every building in one pass,
    /// same convention as `p127_intimacy_gradient`/`p129_common_areas_at_the_heart`.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        _seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p130_entrance_room only supports parcel_id \"*\" -- it runs on every building in one pass.".into());
        }
        let candidates: Vec<&Building> = nbhd.buildings.iter().filter(|b| !b.interior_cells.is_empty()).collect();
        if candidates.is_empty() {
            return Err("p130_entrance_room: no building has interior cells -- run p127_intimacy_gradient first.".into());
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut n_tagged = 0;
        let mut n_ambiguous = 0;

        for b in &candidates {
            let mut updated = (*b).clone();
            // Guaranteed unique by p127_intimacy_gradient's own
            // construction (solid: band 0's depth is exactly 0.0 by the
            // `k as f64 / (total - 1) as f64` formula; courtyard: only
            // bay k=0 has angular_dist 0). A tie here means a single-cell
            // building (already depth 0.0 with nothing to distinguish it
            // from) -- tag it anyway, since it IS the entrance by
            // definition even if there's no gradient to speak of.
            let mut tagged_any = false;
            for cell in updated.interior_cells.iter_mut() {
                if cell.depth.abs() < 1e-9 {
                    cell.kind = "entrance".into();
                    tagged_any = true;
                }
            }
            let n_at_zero = updated.interior_cells.iter().filter(|c| c.depth.abs() < 1e-9).count();
            if n_at_zero > 1 {
                n_ambiguous += 1;
            }
            if tagged_any {
                n_tagged += 1;
                steps.push(format!("{}: tagged {} cell(s) at depth 0.0 as entrance.", b.id, n_at_zero));
            }
            new_buildings.push(updated);
            replaced.push(b.id.clone());
        }

        let trace = SubdivisionTrace {
            operator_name: "p130_entrance_room".into(),
            operator_source: self.source(),
            headline: format!("Tagged the entrance cell in {} of {} building(s).", n_tagged, candidates.len()),
            steps,
            caveats: vec![
                "Label only -- no geometry change. The entrance cell is still sized by \
                 p127_intimacy_gradient's uniform band_depth_m/bay spacing, not a real entrance-room \
                 dimension. See this operator's own module doc for why."
                    .into(),
                format!(
                    "{n_ambiguous} building(s) had more than one cell at exactly depth 0.0 (all tagged) -- \
                     shouldn't happen for p127's own solid-band/courtyard-bay construction, flagged here \
                     rather than silently picking one."
                ),
                "Pattern 130's primary-source text could not be reverified this session \
                 (patternlanguage.com 404s for this pattern) -- see this operator's own module doc."
                    .into(),
            ],
            seed: _seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings,
            new_streets: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids: replaced,
            trace,
        })
    }
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
                label: "P130 unit fixture".into(),
            },
        }
    }

    fn cell(id: &str, depth: f64) -> InteriorCell {
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(0.0001, 0.0),
                LngLat::new(0.0001, 0.0001),
                LngLat::new(0.0, 0.0001),
                LngLat::new(0.0, 0.0),
            ]),
            depth,
            is_common: false,
            kind: "room".into(),
            connects_to: vec![],
            floor: 0,
        }
    }

    fn building(id: &str, cells: Vec<InteriorCell>) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(0.001, 0.0),
                LngLat::new(0.001, 0.001),
                LngLat::new(0.0, 0.001),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(7.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![],
            interior_cells: cells,
        }
    }

    #[test]
    fn no_interior_cells_is_an_error() {
        let n = nbhd(vec![building("B1", vec![])]);
        assert!(P130EntranceRoom.apply(&n, "*", &P130Params::defaults(), 1).is_err());
    }

    #[test]
    fn tags_exactly_the_zero_depth_cell() {
        let b = building("B1", vec![cell("c0", 0.0), cell("c1", 0.5), cell("c2", 1.0)]);
        let n = nbhd(vec![b]);
        let sub = P130EntranceRoom.apply(&n, "*", &P130Params::defaults(), 1).expect("should tag");
        let cells = &sub.new_buildings[0].interior_cells;
        assert_eq!(cells[0].kind, "entrance");
        assert_eq!(cells[1].kind, "room");
        assert_eq!(cells[2].kind, "room");
    }

    #[test]
    fn single_cell_building_still_gets_tagged() {
        let b = building("SOLO", vec![cell("c0", 0.0)]);
        let n = nbhd(vec![b]);
        let sub = P130EntranceRoom.apply(&n, "*", &P130Params::defaults(), 1).expect("should tag");
        assert_eq!(sub.new_buildings[0].interior_cells[0].kind, "entrance");
    }

    #[test]
    fn near_zero_but_not_exact_is_not_tagged() {
        // A band/bay whose depth rounds close to zero but isn't the exact
        // 0.0 p127 always assigns its first cell -- shouldn't happen in
        // practice, but this operator's exact-match discipline (not a
        // fuzzy threshold) means it correctly leaves such a cell alone.
        let b = building("B1", vec![cell("c0", 0.0), cell("c1", 0.02)]);
        let n = nbhd(vec![b]);
        let sub = P130EntranceRoom.apply(&n, "*", &P130Params::defaults(), 1).expect("should tag");
        let cells = &sub.new_buildings[0].interior_cells;
        assert_eq!(cells[0].kind, "entrance");
        assert_eq!(cells[1].kind, "room");
    }
}
