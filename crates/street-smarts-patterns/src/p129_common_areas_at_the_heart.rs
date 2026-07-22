//! P129 Common Areas at the Heart — identify which of a building's
//! `InteriorCell`s (from `p127_intimacy_gradient`) is the common area: the
//! one nearest the whole footprint's center of gravity.
//!
//! From Alexander, *A Pattern Language*, Pattern 129 (verified against the
//! primary text -- see README's Reference section for the link):
//! > No social group -- whether a family, a work group, or a school
//! > group -- can survive without constant informal contact among its
//! > members... Create a single common area for every social group.
//! > Locate it at the center of gravity of all the spaces the group
//! > occupies, and in such a way that the paths which go in and out of the
//! > building lie tangent to it.
//!
//! Runs after `p127_intimacy_gradient` in the pipeline, with
//! `p130_entrance_room` now sandwiched between them (see p130's own module
//! doc for why: it never changes cell geometry or count, so nothing about
//! this operator's own center-of-gravity computation depends on whether
//! the entrance cell has been relabeled yet). No reordering justification
//! needed for 127 -> 129 itself -- see p127's own module doc for the full
//! sourced sequence this is part of.
//!
//! # The "tangent, not through the middle" half of the pattern
//! Alexander's text is explicit that the common area must sit ON the path
//! between entrance and private rooms (not a dead end) but NOT be cut
//! through its own middle by that path (too exposed to feel like a place
//! to stay). This operator does not need a separate geometric check for
//! that half of the rule: `p127_intimacy_gradient`'s band-chain / ring-bay
//! layout only ever connects a cell to its immediate neighbors at its
//! borders, never diagonally through a cell's own interior -- so "tangent,
//! not through the middle" is satisfied BY CONSTRUCTION of the partition
//! itself, not by anything this operator checks.
//!
//! # What this operator deliberately does NOT do
//! No use, ever -- same discipline as P127. "Common area" here means
//! nothing more than "the cell nearest the plan's center of gravity." It
//! is not labeled a living room, a lobby, or anything else.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::{Building, Neighborhood};
use street_smarts_core::opinion::SourceCitation;

/// No tunable parameters yet -- "nearest the center of gravity" has no
/// free variable to expose. `NoParams`-shaped by hand (rather than
/// `crate::NoParams`) so this operator still shows up cleanly in the
/// registry/UI parameter listing like every other operator, not as a
/// special case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P129Params;

impl Parameters for P129Params {
    fn schema() -> Vec<ParamSpec> { vec![] }
    fn defaults() -> Self { Self }
    fn as_vector(&self) -> Vec<f64> { vec![] }
    fn from_vector(_v: &[f64]) -> Self { Self }
}

pub struct P129CommonAreasAtTheHeart;

impl PatternOperator for P129CommonAreasAtTheHeart {
    type Params = P129Params;

    fn name(&self) -> &'static str { "p129_common_areas_at_the_heart" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p129".into(),
            display: "Alexander et al., A Pattern Language, Pattern 129 (Common Areas at the Heart)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl129/apl129.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Identify which interior cell sits nearest the building's center of gravity -- the common area."
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
            return Err("p129_common_areas_at_the_heart only supports parcel_id \"*\" -- it runs on every building in one pass.".into());
        }
        let candidates: Vec<&Building> = nbhd.buildings.iter().filter(|b| !b.interior_cells.is_empty()).collect();
        if candidates.is_empty() {
            return Err("p129_common_areas_at_the_heart: no building has interior cells -- run p127_intimacy_gradient first.".into());
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();

        for b in &candidates {
            let mut updated = (*b).clone();
            let gravity = b.polygon.centroid();
            let mut best_idx = 0;
            let mut best_dist = f64::INFINITY;
            for (i, cell) in updated.interior_cells.iter().enumerate() {
                let d = haversine_m(&gravity, &cell.polygon.centroid());
                if d < best_dist {
                    best_dist = d;
                    best_idx = i;
                }
            }
            for (i, cell) in updated.interior_cells.iter_mut().enumerate() {
                cell.is_common = i == best_idx;
            }
            steps.push(format!(
                "{}: common cell = {} ({:.1}m from the plan's center of gravity, of {} cell(s)).",
                b.id, updated.interior_cells[best_idx].id, best_dist, updated.interior_cells.len()
            ));
            new_buildings.push(updated);
            replaced.push(b.id.clone());
        }

        let trace = SubdivisionTrace {
            operator_name: "p129_common_areas_at_the_heart".into(),
            operator_source: self.source(),
            headline: format!("Identified the common area in {} building(s).", new_buildings.len()),
            steps,
            caveats: vec![
                "\"Common area\" means only \"nearest the plan's center of gravity\" -- no use is \
                 assumed or implied. This is not a claim about what activity the cell will host."
                    .into(),
                "The 'tangent, not through the middle' half of Alexander's rule is satisfied by \
                 construction of p127_intimacy_gradient's band-chain / ring-bay partition (cells \
                 only ever border their immediate neighbors), not by any check this operator makes \
                 itself -- see this operator's own module doc.".into(),
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
            new_fields: vec![],
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
                label: "P129 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn cell(id: &str, cx: f64, cy: f64, depth: f64) -> InteriorCell {
        let m = 1.0 / 111_320.0;
        let s = 2.0 * m;
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(cx - s, cy - s), LngLat::new(cx + s, cy - s),
                LngLat::new(cx + s, cy + s), LngLat::new(cx - s, cy + s),
                LngLat::new(cx - s, cy - s),
            ]),
            depth,
            is_common: false,
            kind: "room".into(),
            connects_to: vec![],
            floor: 0,
        }
    }

    #[test]
    fn middle_cell_of_a_three_band_chain_is_common() {
        // Whole footprint spans x in [0,30] (three 10-wide bands centered
        // at x=5,15,25) -- footprint centroid is near x=15, so the MIDDLE
        // band should be picked as common, not either end.
        let m = 1.0 / 111_320.0;
        let b = Building {
            id: "B1".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(30.0 * m, 0.0),
                LngLat::new(30.0 * m, 10.0 * m), LngLat::new(0.0, 10.0 * m),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(7.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![],
            interior_cells: vec![
                cell("c0", 5.0 * m, 5.0 * m, 0.0),
                cell("c1", 15.0 * m, 5.0 * m, 0.5),
                cell("c2", 25.0 * m, 5.0 * m, 1.0),
            ],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], };
        let n = nbhd(vec![b]);
        let sub = P129CommonAreasAtTheHeart.apply(&n, "*", &P129Params::defaults(), 1).expect("should run");
        let cells = &sub.new_buildings[0].interior_cells;
        assert!(cells[1].is_common, "middle band should be common");
        assert!(!cells[0].is_common && !cells[2].is_common, "exactly one cell should be common");
    }

    #[test]
    fn no_interior_cells_anywhere_is_an_error() {
        let b = Building {
            id: "B1".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(0.001, 0.0),
                LngLat::new(0.001, 0.001), LngLat::new(0.0, 0.001),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(7.0),
            typology: None,
            year_built: None,
            parcel_id: None,
            floors: None,
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], };
        let n = nbhd(vec![b]);
        assert!(P129CommonAreasAtTheHeart.apply(&n, "*", &P129Params::defaults(), 1).is_err());
    }
}
