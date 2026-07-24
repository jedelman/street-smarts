//! P133 Staircase as a Stage — carve a real stair-core cell out of the
//! building's own common area (the cell `p129_common_areas_at_the_heart`
//! flagged `is_common`), instead of leaving every multi-story building
//! with no vertical circulation modeled at all.
//!
//! From Alexander, *A Pattern Language*, Pattern 133 (Staircase as a
//! Stage): a staircase treated as pure circulation -- narrow, closed off,
//! hidden in its own shaft -- wastes what is actually a real social event
//! (arriving, leaving, being seen crossing between floors). Open the stair
//! to the main gathering space instead of burying it, and the act of
//! going up or down becomes part of the room, not an escape from it.
//!
//! **Citation note:** `patternlanguage.com`'s sample/direct pages for
//! Pattern 133 returned 404 this session (both `/apl/aplsample/apl133/
//! apl133.htm` and the newer `/apl/direct-133.htm`) -- the same wall
//! `p128_indoor_sunlight` and `p130_entrance_room` already hit. The
//! description above is this codebase's best-effort paraphrase of the
//! pattern's well-known substance, not a verified block quote.
//!
//! # Why the common area, specifically
//! Alexander's own instruction is to keep the stair open to wherever
//! people already gather -- and `p129_common_areas_at_the_heart` already
//! identifies exactly that cell (nearest the plan's own center of
//! gravity, on the real connectivity path per that operator's own
//! module doc). Carving the stair out of the common cell, rather than
//! placing it as an unrelated new room, is a direct, geometric reading of
//! "open to the stage" -- the stair and the room it interrupts are
//! literally the same space, not adjacent ones.
//!
//! Runs after `p221_natural_doors_and_windows`, not right after
//! `p131_the_flow_through_rooms` where Alexander's own numbering would
//! put it. `Building.floors` (this operator's own multi-story filter)
//! isn't set by P96 -- P96 only sets `target_stories` on the PARCEL/pad;
//! `Building.floors` itself stays `None` until P221 derives a real story
//! count from height, per `Building.floors`'s own doc comment. Running
//! P133 in Alexander's own position (right after P131, before P221) was
//! the first version's actual bug: every building's `floors` read `None`
//! there, so the multi-story filter matched nothing and the pipeline's own
//! `if let Ok(...)` silently swallowed the resulting error -- confirmed by
//! adding temporary instrumentation and rerunning against the real
//! MALL_CORE fixture, not caught by any unit test since every test fixture
//! in this file sets `floors` directly instead of deriving it the way the
//! real pipeline does. Still needs P131's `connects_to` graph and P129's
//! `is_common` flag, both of which P221 (which only touches `openings` and
//! `floors`) leaves untouched.
//!
//! # How the cut is made
//! A first attempt cut a small rectangle out of the MIDDLE of the common
//! cell via `subtract_convex` + `union_pieces`. That measurably lost real
//! area on reunion (verified directly: an 80m^2 rectangle minus a
//! 2.88m^2 hole reunioned to as little as 46.9m^2 -- `union_pieces`
//! doesn't reliably reassemble the multi-piece fragments `subtract_convex`
//! produces around an interior hole). Cutting a full-length STRIP off one
//! edge instead -- exactly the technique `p131_the_flow_through_rooms`'s
//! own loop-closing passage cell already uses, projecting onto an
//! axis/perp pair and clipping with `clip_half_plane` -- needs no union at
//! all: the strip and the remainder are two independent, single-piece
//! clips of the SAME original polygon, not fragments to reassemble.
//!
//! # What this operator deliberately does NOT do
//! **No upper-floor partitioning.** This places a stair CORE on the
//! ground floor -- a real, located anchor for where vertical circulation
//! lives -- but does not generate any interior_cells for floor 1+.
//! `p127_intimacy_gradient`'s own module doc already explains why: there
//! is still no modeled way to know where a stair LANDS on the floor
//! above, what it opens onto there, or how that floor's own footprint
//! (which can differ from the ground floor's after any per-floor massing
//! this pipeline doesn't do) would need its own gradient. This operator
//! narrows that gap (there is now a real stair location) without closing
//! it.
//!
//! **No real rise/run calculation, and the stair runs the room's full
//! length.** The strip is `stair_width_m` wide (a plausible minimum-stair
//! placeholder, same category as P221's `room_width_m`) but spans the
//! common cell's ENTIRE length along its own longest-edge axis -- not a
//! fixed footprint sized from `FLOOR_TO_FLOOR_M` and a real riser/tread
//! count. A stair running the full length of a wall is architecturally
//! plausible (many real stairs do), but this is a geometric consequence
//! of the cut technique, not a claim about the real stair run length.
//!
//! **No footprint rotation beyond the cell's own longest edge.** The
//! axis is that edge's own direction (same longest-edge fallback
//! `p127_intimacy_gradient::depth_axis` and `orientation.rs` both already
//! use elsewhere in this crate) -- correct for the roughly-rectangular
//! bands/bays P127 actually produces, not a general answer for an
//! arbitrary polygon.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{centroid, clip_half_plane, local_to_ring, ring_to_local, Pt2};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::{Building, InteriorCell, Neighborhood};
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P133Params {
    /// Stair-core strip width. Range tightened to Alexander's own literal
    /// P195 Staircase Volume figure -- "2 feet wide (for a very steep
    /// stair) or 5 feet wide for a generous shallow stair" (0.61-1.52m) --
    /// so this generator's own output always clears p195_staircase_volume's
    /// real check by construction, the same precedent p49_looped_local_roads
    /// and p67_common_land already set for path_width_m/common_land_fraction.
    /// Not a real code-minimum lookup either way.
    pub stair_width_m: f64,
}

impl Parameters for P133Params {
    fn schema() -> Vec<ParamSpec> {
        vec![ParamSpec::float("stair_width_m", "Stair-core strip width.", 0.61, 1.52, 1.2).with_unit("m")]
    }
    fn defaults() -> Self {
        Self { stair_width_m: 1.2 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.stair_width_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.stair_width_m = s.clamp(*x);
        }
        p
    }
}

pub struct P133StaircaseAsAStage;

/// Direction (unit vector) and perpendicular of `poly`'s own longest edge
/// -- same fallback technique `p127_intimacy_gradient::depth_axis` and
/// `orientation.rs` already use, duplicated here rather than exported
/// since it's ~10 lines and each caller needs it in a different frame
/// (whole-building outer ring there, one interior cell's own ring here).
fn longest_edge_axis(poly: &[Pt2]) -> (Pt2, Pt2) {
    let n = poly.len();
    let mut best: Option<(f64, Pt2)> = None;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let len = a.dist(b);
        if len > 1e-9 && best.map(|(bl, _)| len > bl).unwrap_or(true) {
            let edge = b.sub(a);
            best = Some((len, Pt2::new(edge.x / len, edge.y / len)));
        }
    }
    let axis = best.map(|(_, a)| a).unwrap_or(Pt2::new(1.0, 0.0));
    (axis, Pt2::new(-axis.y, axis.x))
}

impl PatternOperator for P133StaircaseAsAStage {
    type Params = P133Params;

    fn name(&self) -> &'static str {
        "p133_staircase_as_a_stage"
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_133".into(),
            display: "Alexander et al., A Pattern Language, Pattern 133 (Staircase as a Stage) -- \
                      paraphrased; patternlanguage.com's sample pages for this pattern 404'd \
                      this session, see this operator's own module doc."
                .into(),
            url: Some("https://www.patternlanguage.com/apl/direct-133.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Carve a real stair-core strip out of the common-area cell of every multi-story building, open to the room it interrupts."
    }

    /// `parcel_id` must be `"*"` -- targets every building in one pass,
    /// same convention as the rest of this interior-ontology sequence.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        _seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p133_staircase_as_a_stage only supports parcel_id \"*\" -- it runs on every building in one pass.".into());
        }
        let candidates: Vec<&Building> = nbhd
            .buildings
            .iter()
            .filter(|b| (b.floors.unwrap_or(1)) >= 2 && b.interior_cells.iter().any(|c| c.is_common))
            .collect();
        if candidates.is_empty() {
            return Err(
                "p133_staircase_as_a_stage: no building is both multi-story and has a marked common area -- run p96_number_of_stories and p129_common_areas_at_the_heart first."
                    .into(),
            );
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut n_placed = 0;
        let mut n_too_small: Vec<String> = Vec::new();

        for b in &candidates {
            let mut updated = (*b).clone();
            let origin = b.polygon.centroid();

            let Some(common_idx) = updated.interior_cells.iter().position(|c| c.is_common) else {
                continue;
            };
            let common_local = ring_to_local(&updated.interior_cells[common_idx].polygon.outer, &origin);
            if common_local.len() < 3 {
                n_too_small.push(b.id.clone());
                continue;
            }

            let (axis, perp) = longest_edge_axis(&common_local);
            let c = centroid(&common_local);
            let (mut s_min, mut s_max) = (f64::INFINITY, f64::NEG_INFINITY);
            let (mut w_min, mut w_max) = (f64::INFINITY, f64::NEG_INFINITY);
            for &p in &common_local {
                let d = p.sub(c);
                let (s, w) = (d.dot(axis), d.dot(perp));
                if s < s_min { s_min = s; }
                if s > s_max { s_max = s; }
                if w < w_min { w_min = w; }
                if w > w_max { w_max = w; }
            }
            if w_max - w_min < params.stair_width_m * 1.5 {
                n_too_small.push(b.id.clone());
                continue;
            }

            let w_hi = w_max;
            let w_lo = w_max - params.stair_width_m;
            let p_s_min = Pt2::new(c.x + s_min * axis.x, c.y + s_min * axis.y);
            let p_s_max = Pt2::new(c.x + s_max * axis.x, c.y + s_max * axis.y);
            let p_w_hi = Pt2::new(c.x + w_hi * perp.x, c.y + w_hi * perp.y);
            let p_w_lo = Pt2::new(c.x + w_lo * perp.x, c.y + w_lo * perp.y);

            // The stair strip: w in [w_lo, w_hi], full s range. Same
            // derivation as p131_the_flow_through_rooms's passage cell.
            let mut stair = clip_half_plane(&common_local, p_s_max, Pt2::new(p_s_max.x + perp.x, p_s_max.y + perp.y));
            stair = clip_half_plane(&stair, p_s_min, Pt2::new(p_s_min.x - perp.x, p_s_min.y - perp.y));
            stair = clip_half_plane(&stair, p_w_lo, Pt2::new(p_w_lo.x + axis.x, p_w_lo.y + axis.y));
            stair = clip_half_plane(&stair, p_w_hi, Pt2::new(p_w_hi.x - axis.x, p_w_hi.y - axis.y));

            // The remainder: w in [w_min, w_lo], full s range -- an
            // independent clip of the ORIGINAL polygon, not "common minus
            // stair", so there's no fragment to reunion.
            let p_w_min = Pt2::new(c.x + w_min * perp.x, c.y + w_min * perp.y);
            let mut remainder = clip_half_plane(&common_local, p_s_max, Pt2::new(p_s_max.x + perp.x, p_s_max.y + perp.y));
            remainder = clip_half_plane(&remainder, p_s_min, Pt2::new(p_s_min.x - perp.x, p_s_min.y - perp.y));
            remainder = clip_half_plane(&remainder, p_w_min, Pt2::new(p_w_min.x + axis.x, p_w_min.y + axis.y));
            remainder = clip_half_plane(&remainder, p_w_lo, Pt2::new(p_w_lo.x - axis.x, p_w_lo.y - axis.y));

            if stair.len() < 3 || remainder.len() < 3 {
                n_too_small.push(b.id.clone());
                continue;
            }

            let common_id = updated.interior_cells[common_idx].id.clone();
            let common_depth = updated.interior_cells[common_idx].depth;
            let stair_id = format!("{common_id}_stair");

            updated.interior_cells[common_idx].polygon =
                street_smarts_core::geometry::Polygon::from_ring(local_to_ring(&remainder, &origin));
            updated.interior_cells[common_idx].connects_to.push(stair_id.clone());

            updated.interior_cells.push(InteriorCell {
                id: stair_id,
                polygon: street_smarts_core::geometry::Polygon::from_ring(local_to_ring(&stair, &origin)),
                depth: common_depth,
                is_common: false,
                kind: "stair".into(),
                connects_to: vec![common_id],
                floor: 0,
            });

            n_placed += 1;
            steps.push(format!("{}: stair core carved from the common area, {:.1}m wide.", b.id, params.stair_width_m));
            new_buildings.push(updated);
            replaced.push(b.id.clone());
        }

        if n_placed == 0 {
            return Err(format!(
                "p133_staircase_as_a_stage: 0 of {} multi-story building(s) had a common area with enough cross-width for a {:.1}m-wide stair strip.",
                candidates.len(), params.stair_width_m
            ));
        }

        let trace = SubdivisionTrace {
            operator_name: "p133_staircase_as_a_stage".into(),
            operator_source: self.source(),
            headline: format!(
                "Placed a stair core in {} of {} multi-story building(s); {} too small to fit one.",
                n_placed, candidates.len(), n_too_small.len()
            ),
            steps,
            caveats: vec![
                "Ground-floor stair CORE only -- no upper-floor interior_cells are generated. See \
                 this operator's own module doc for why."
                    .into(),
                "The stair strip runs the common cell's full length (along its own longest-edge \
                 axis), not a fixed footprint sized from FLOOR_TO_FLOOR_M and a real riser/tread \
                 count -- see this operator's own module doc for why (and for the union_pieces bug \
                 a first, rejected approach ran into).".into(),
                format!("{} building(s) skipped: common area too narrow to fit the stair strip.", n_too_small.len()),
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
    use street_smarts_core::nir::NeighborhoodMeta;

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
                label: "P133 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    /// A generously sized 10m x 8m common-area cell, big enough to easily
    /// fit the default 1.2m-wide stair strip.
    fn building_with_roomy_common(id: &str, floors: Option<u32>) -> Building {
        let m = 1.0 / 111_320.0;
        let cell = InteriorCell {
            id: format!("{id}_common"),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(10.0 * m, 0.0),
                LngLat::new(10.0 * m, 8.0 * m),
                LngLat::new(0.0, 8.0 * m),
                LngLat::new(0.0, 0.0),
            ]),
            depth: 0.4,
            is_common: true,
            kind: "room".into(),
            connects_to: vec![],
            floor: 0,
        };
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-5.0 * m, -5.0 * m),
                LngLat::new(15.0 * m, -5.0 * m),
                LngLat::new(15.0 * m, 13.0 * m),
                LngLat::new(-5.0 * m, 13.0 * m),
                LngLat::new(-5.0 * m, -5.0 * m),
            ]),
            height_m: Some(14.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors,
            openings: vec![],
            interior_cells: vec![cell],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    #[test]
    fn single_story_building_is_excluded() {
        let n = nbhd(vec![building_with_roomy_common("B1", Some(1))]);
        assert!(P133StaircaseAsAStage.apply(&n, "*", &P133Params::defaults(), 1).is_err());
    }

    #[test]
    fn no_floors_data_defaults_to_single_story_and_is_excluded() {
        let n = nbhd(vec![building_with_roomy_common("B1", None)]);
        assert!(P133StaircaseAsAStage.apply(&n, "*", &P133Params::defaults(), 1).is_err());
    }

    #[test]
    fn roomy_common_area_gets_a_real_stair_core_with_no_area_lost() {
        let n = nbhd(vec![building_with_roomy_common("B1", Some(4))]);
        let sub = P133StaircaseAsAStage.apply(&n, "*", &P133Params::defaults(), 1).expect("should place a stair");
        let b = &sub.new_buildings[0];
        assert_eq!(b.interior_cells.len(), 2, "should have the (shrunk) common cell plus one new stair cell");
        let stair = b.interior_cells.iter().find(|c| c.kind == "stair").expect("stair cell should exist");
        let common = b.interior_cells.iter().find(|c| c.is_common).expect("common cell should still exist");
        assert!(common.connects_to.contains(&stair.id), "common cell should connect to the stair");
        assert!(stair.connects_to.contains(&common.id), "stair should connect back to the common cell");

        let origin = n.buildings[0].polygon.centroid();
        let common_local = ring_to_local(&common.polygon.outer, &origin);
        let stair_local = ring_to_local(&stair.polygon.outer, &origin);
        let common_area = crate::planar::area(&common_local);
        let stair_area = crate::planar::area(&stair_local);
        // Compare against the REAL pre-split area (via the same lnglat->local
        // projection, which scales lng/lat by different constants -- see
        // planar::lnglat_to_local), not a hand-computed "10.0 * 8.0" nominal
        // value that silently assumes a uniform, wrong scale for both axes.
        let original_local = ring_to_local(&n.buildings[0].interior_cells[0].polygon.outer, &origin);
        let original_area = crate::planar::area(&original_local);
        assert!(
            (common_area + stair_area - original_area).abs() < 0.05,
            "stair + remaining common area ({} + {} = {}) should reconstruct the original {} with no area lost or gained",
            common_area, stair_area, common_area + stair_area, original_area
        );
        // Real width, not a sliver.
        assert!(stair_area > 5.0, "stair strip should be a real, non-degenerate area, got {stair_area}");
    }

    #[test]
    fn narrow_common_area_is_skipped_not_force_placed() {
        let m = 1.0 / 111_320.0;
        let mut b = building_with_roomy_common("B1", Some(3));
        // Shrink the common cell to 10m x 1m -- too narrow for a 1.2m-wide strip
        // to leave a real remainder (needs 1.5x = 1.8m of cross-width).
        b.interior_cells[0].polygon = Polygon::from_ring(vec![
            LngLat::new(0.0, 0.0),
            LngLat::new(10.0 * m, 0.0),
            LngLat::new(10.0 * m, 1.0 * m),
            LngLat::new(0.0, 1.0 * m),
            LngLat::new(0.0, 0.0),
        ]);
        let n = nbhd(vec![b]);
        assert!(P133StaircaseAsAStage.apply(&n, "*", &P133Params::defaults(), 1).is_err());
    }
}
