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
//!
//! # v0.2: a real, textually-grounded south tie-break, closing P128's gap
//!
//! Alexander's own text cites Indoor Sunlight (128) directly from Pattern
//! 129's own "smaller patterns" list (verified in
//! `data/apl-pattern-graph.json`: 129's `cites` includes 128) -- the common
//! area isn't just wherever the plan's arithmetic center happens to fall,
//! it should also get real sun. This operator still picks by distance to
//! the center of gravity FIRST -- that's Alexander's own literal
//! instruction and this doesn't change it. But for a solid building whose
//! depth axis (`p127_intimacy_gradient`'s own cardinal-snapped axis) runs
//! north-south, several bands can sit at nearly the SAME distance from the
//! center of gravity (their only difference is which side of it they're
//! on) -- previously an arbitrary tie-break (first cell found at the exact
//! minimum, order-dependent). Now, among cells within `TIE_TOLERANCE_M` of
//! the true minimum distance, the SOUTHERNMOST one wins -- a real,
//! measurable preference (smaller latitude), not a fabricated one, and it
//! only ever activates when Alexander's own primary rule (nearest the
//! center of gravity) already left more than one real candidate on the
//! table.
//!
//! **Honestly scoped.** This cannot help a building whose common cell has
//! no real candidate anywhere near a south wall at all (e.g. a narrow
//! building whose depth axis runs north-south and whose single
//! center-of-gravity band sits landlocked between two others, touching
//! neither the north nor south wall) -- there's no tie to break there, and
//! inventing one would mean abandoning Alexander's own center-of-gravity
//! rule, not refining it. Measured on the real fixture: see this crate's
//! own `check_detector_impact.rs` output for `p128_indoor_sunlight`'s
//! current real number.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::{Building, Neighborhood};
use street_smarts_core::opinion::SourceCitation;

/// P128 Indoor Sunlight tie-break margin (see this file's own "v0.2"
/// module doc): cells within this many real metres of the true minimum
/// distance-to-center-of-gravity count as tied. Sized to roughly half
/// `p127_intimacy_gradient`'s own `band_depth_m` default (5.0m) -- close
/// enough that two adjacent bands genuinely read as "basically the same
/// distance from the middle," not an arbitrary number. Not exposed as its
/// own `P129Params` field (yet) -- no caller has asked to tune it, and
/// `P129Params` staying empty keeps this operator's "no free variable"
/// framing honest for the primary rule it implements.
const TIE_TOLERANCE_M: f64 = 2.5;

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
            let distances: Vec<f64> = updated
                .interior_cells
                .iter()
                .map(|cell| haversine_m(&gravity, &cell.polygon.centroid()))
                .collect();
            let min_dist = distances.iter().cloned().fold(f64::INFINITY, f64::min);

            // P128 Indoor Sunlight (see this file's own "v0.2" module doc):
            // among cells within TIE_TOLERANCE_M of the true minimum
            // distance -- i.e. genuinely tied by Alexander's own
            // center-of-gravity rule, not just close -- prefer the
            // southernmost (smallest latitude). A real, measurable
            // tie-break, not a fabricated bias: it never overrides a clear
            // single winner.
            let best_idx = updated
                .interior_cells
                .iter()
                .enumerate()
                .filter(|(i, _)| distances[*i] <= min_dist + TIE_TOLERANCE_M)
                .min_by(|(_, a), (_, b)| {
                    a.polygon.centroid().lat.partial_cmp(&b.polygon.centroid().lat).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
                .unwrap_or(0);

            for (i, cell) in updated.interior_cells.iter_mut().enumerate() {
                cell.is_common = i == best_idx;
            }
            steps.push(format!(
                "{}: common cell = {} ({:.1}m from the plan's center of gravity, of {} cell(s)).",
                b.id, updated.interior_cells[best_idx].id, distances[best_idx], updated.interior_cells.len()
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

    /// Building outer ring: x in [0,10], y in [0,20] -> centroid (5,10).
    fn ns_building(id: &str, cells: Vec<InteriorCell>) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(10.0 * m, 0.0),
                LngLat::new(10.0 * m, 20.0 * m), LngLat::new(0.0, 20.0 * m),
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

    /// P128 Indoor Sunlight tie-break: two cells at EXACTLY the same
    /// distance from the center of gravity (10m north-south building,
    /// centroid at y=10) -- one 1.5m south of it, one 1.5m north. Alexander's
    /// own primary rule (nearest the center of gravity) can't distinguish
    /// them; the real tie-break should prefer the southern one.
    #[test]
    fn exact_tie_prefers_the_southern_cell() {
        let m = 1.0 / 111_320.0;
        let b = ns_building("B1", vec![
            cell("south", 5.0 * m, 8.5 * m, 0.0),
            cell("north", 5.0 * m, 11.5 * m, 1.0),
        ]);
        let n = nbhd(vec![b]);
        let sub = P129CommonAreasAtTheHeart.apply(&n, "*", &P129Params::defaults(), 1).expect("should run");
        let cells = &sub.new_buildings[0].interior_cells;
        let common = cells.iter().find(|c| c.is_common).expect("one cell should be common");
        assert_eq!(common.id, "south", "an exact tie should prefer the southern cell");
    }

    /// A cell that's slightly FARTHER from the center of gravity, but still
    /// within TIE_TOLERANCE_M, should still win over a slightly-closer
    /// northern cell -- the tie-break's whole point is that "roughly as
    /// close" counts, not just an exact tie.
    #[test]
    fn near_tie_within_tolerance_prefers_the_southern_cell() {
        let m = 1.0 / 111_320.0;
        // north: 1.0m from gravity (y=9.0, i.e. north of the y=10 centroid
        // -- wait, smaller y is south, so y=9.0 is SOUTH of centroid by 1.0m).
        // Build this explicitly south/north to avoid confusing myself:
        // south cell at y=8.0 (2.0m south, farther), north cell at y=11.0
        // (1.0m north, closer). Difference in distance (1.0m) is well within
        // TIE_TOLERANCE_M (2.5m), so the farther-but-southern cell should win.
        let b = ns_building("B1", vec![
            cell("north_closer", 5.0 * m, 11.0 * m, 0.0),
            cell("south_farther", 5.0 * m, 8.0 * m, 1.0),
        ]);
        let n = nbhd(vec![b]);
        let sub = P129CommonAreasAtTheHeart.apply(&n, "*", &P129Params::defaults(), 1).expect("should run");
        let cells = &sub.new_buildings[0].interior_cells;
        let common = cells.iter().find(|c| c.is_common).expect("one cell should be common");
        assert_eq!(common.id, "south_farther", "a near-tie within tolerance should still prefer the southern cell");
    }

    /// A cell far outside TIE_TOLERANCE_M must NOT be overridden just for
    /// being southern -- Alexander's own primary center-of-gravity rule
    /// still wins when the two aren't genuinely tied.
    #[test]
    fn clear_winner_outside_tolerance_is_not_overridden() {
        let m = 1.0 / 111_320.0;
        let b = ns_building("B1", vec![
            cell("clear_winner", 5.0 * m, 10.0 * m, 0.0), // exactly at gravity
            cell("far_south", 5.0 * m, 0.5 * m, 1.0), // 9.5m south -- way outside tolerance
        ]);
        let n = nbhd(vec![b]);
        let sub = P129CommonAreasAtTheHeart.apply(&n, "*", &P129Params::defaults(), 1).expect("should run");
        let cells = &sub.new_buildings[0].interior_cells;
        let common = cells.iter().find(|c| c.is_common).expect("one cell should be common");
        assert_eq!(common.id, "clear_winner", "a cell far outside tolerance shouldn't be overridden just for being southern");
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
