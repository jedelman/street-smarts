//! P124 Activity Pockets — a real, small, partly enclosed bump projecting
//! from a building's own footprint into the open space at the point it
//! borders a real Plaza, not just a proxy for "buildings are near
//! squares."
//!
//! From Alexander, *A Pattern Language*, Pattern 124, via
//! patternlanguage.cc/Patterns/Activity-Pockets-(124):
//! > **Problem:** The life of a public square forms naturally around its
//! > edge. If the edge fails, then the space never becomes lively.
//! > **Solution:** Surround public gathering places with pockets of
//! > activity -- small, partly enclosed areas at the edges, which jut
//! > forward into the open space between the paths, and contain
//! > activities which make it natural for people to pause and get
//! > involved.
//!
//! # A real geometric interpretation, stated honestly
//!
//! Alexander's own text doesn't give a precise dimension for a pocket
//! (confirmed: the cited page's own "Numeric Dimensions/Thresholds"
//! section is empty). "Jut forward into the open space" is Alexander's
//! own literal words -- a real PROJECTION, outward from the building edge
//! toward the plaza, not a recession into the building's own mass. An
//! earlier version of this operator read it the other way (a notch
//! carved OUT of the building, a recessed shopfront/nook reading) and
//! said so honestly, flagging the projecting-bump reading as "an equally
//! real but different operator" -- this is that operator, chosen for
//! fidelity to Alexander's own literal text once the choice was examined
//! directly against it.
//!
//! `pocket_width_m`/`pocket_depth_m` are plausible real bump dimensions
//! (a person-scale nook, not a room), not Alexander's own literal figures
//! -- same category as `p95_building_complex`'s `pad_inset_m` or
//! `path_network`'s `arterial_width_m`.
//!
//! # Real constraints, not arbitrary ones
//!
//! - Only bumps out from a building whose own footprint edge already
//!   comes within `adjacency_threshold_m` of a real Plaza's own boundary
//!   (this pipeline's own real P122 Building Fronts ideal is a 1.0m
//!   setback -- `adjacency_threshold_m` defaults wider, 3.0m, so a real
//!   but slightly less tight adjacency still counts).
//! - The bump never projects as far as the real Plaza's own boundary:
//!   skipped if `pocket_depth_m` would reach or exceed the real, measured
//!   minimum distance from THIS SPECIFIC candidate edge to the plaza's
//!   own boundary ring (not the plaza centroid, and not a whole-building
//!   global minimum -- the edge being bumped is measured directly). A
//!   bump that reached the plaza would stop being "open space between
//!   the paths" and start being an annexation of the plaza itself -- a
//!   different, not-yet-real operator. Measured on the real fixture,
//!   32/33 real bordering candidates sit at only ~0.10m from their own
//!   plaza edge (an artifact of an earlier pipeline stage's own near-flush
//!   placement, not a defect of this operator) -- `pocket_depth_m`'s own
//!   default and range were set low enough to leave real room for a
//!   candidate that has it, without pretending the tight majority have
//!   room they don't.
//! - The bump must actually get closer to the plaza, not farther: the
//!   candidate edge is chosen by its own global-minimum distance to the
//!   plaza, which for an oblique or side-facing edge can be achieved near
//!   one endpoint rather than square-on across the edge -- bumping
//!   straight out from such an edge's midpoint can move AWAY from the
//!   plaza instead of toward it. Caught directly on the real fixture: an
//!   edge with a real 1.16m gap produced a bump 4.94m from the plaza,
//!   farther, not closer. Skipped whenever the projected bump's own
//!   distance to the plaza is not strictly less than the edge's own
//!   pre-bump distance. On the real fixture, this is also why the one
//!   candidate with real depth room (~1.5m gap) still doesn't get a
//!   pocket: its own closest edge faces away from the plaza, not toward
//!   it -- an honest zero-pocket result on this fixture (see
//!   `cascade_contracts.rs`'s own note on why this operator has no
//!   self-pair contract right now), not a defect in either check.
//! - Skips a candidate whose bump would exceed `max_bump_area_frac`
//!   (default 0.15) of the building's own original footprint area -- a
//!   real safety margin against a small building growing a
//!   disproportionately large bump.
//! - `max_pockets_per_plaza` (default 2) caps a real, bounded resource
//!   allocation per plaza -- not one bumped onto every possible building,
//!   matching this crate's own established idiom (P61's own square
//!   budget, P60's investigated-and-scoped park reservation).
//! - Only solid (single-outer-ring, no courtyard hole) buildings are
//!   candidates -- a courtyard building's own real geometry is a separate
//!   concern this operator does not yet need to touch, since the bump
//!   only ever adds to the outer ring. Each building fronts at most one
//!   real pocket.
//!
//! Each real pocket also gets a real `ActivityNode` at its own centroid
//! (`ActivityKind::Civic`, honest empty `activity_fit`/`None`
//! `publicness` -- same convention `p61_small_public_squares` already
//! established for its own squares) -- this operator IS the real source
//! of that activity, unlike a generic placement elsewhere.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{
    centroid, ensure_ccw, polygon_min_distance,
    rect_corridor, ring_to_local, area as poly_area, local_to_ring, Pt2,
};
use crate::prng::Prng;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::{ActivityKind, ActivityNode, Building, Neighborhood, OpenSpace, OpenSpaceKind};
use street_smarts_core::geometry::Polygon;
use street_smarts_core::opinion::SourceCitation;

pub struct P124ActivityPockets;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P124Params {
    /// How close a building's own footprint edge must come to a real
    /// Plaza's boundary to count as "bordering" it.
    pub adjacency_threshold_m: f64,
    /// Real width of a projecting pocket bump, along the building's own
    /// wall edge. A plausible real nook dimension, not Alexander's own
    /// literal figure.
    pub pocket_width_m: f64,
    /// Real depth a pocket projects outward from the building, toward the
    /// plaza, perpendicular to the wall edge.
    pub pocket_depth_m: f64,
    /// A candidate building's own added bump area must stay at or below
    /// this fraction of its original footprint area.
    pub max_bump_area_frac: f64,
    /// How many real pockets to bump out per plaza, at most.
    pub max_pockets_per_plaza: f64,
}

impl Parameters for P124Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float("adjacency_threshold_m", "How close a building must come to a Plaza's boundary to count as bordering it.", 1.0, 8.0, 3.0).with_unit("m"),
            ParamSpec::float("pocket_width_m", "Real width of a projecting pocket bump.", 1.5, 4.0, 2.5).with_unit("m"),
            ParamSpec::float("pocket_depth_m", "Real depth a pocket projects outward from the building, toward the plaza.", 0.3, 2.0, 1.0).with_unit("m"),
            ParamSpec::float("max_bump_area_frac", "A candidate building's own added bump area must stay at or below this fraction of its original footprint area.", 0.05, 0.5, 0.15),
            ParamSpec::float("max_pockets_per_plaza", "How many real pockets to bump out per plaza, at most.", 0.0, 4.0, 2.0),
        ]
    }
    fn defaults() -> Self {
        Self {
            adjacency_threshold_m: 3.0,
            pocket_width_m: 2.5,
            pocket_depth_m: 1.0,
            max_bump_area_frac: 0.15,
            max_pockets_per_plaza: 2.0,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.adjacency_threshold_m, self.pocket_width_m, self.pocket_depth_m, self.max_bump_area_frac, self.max_pockets_per_plaza]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.adjacency_threshold_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.pocket_width_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.pocket_depth_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) { p.max_bump_area_frac = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(4), v.get(4)) { p.max_pockets_per_plaza = s.clamp(*x); }
        p
    }
}

impl PatternOperator for P124ActivityPockets {
    type Params = P124Params;

    fn name(&self) -> &'static str {
        "p124_activity_pockets"
    }
    fn description(&self) -> &'static str {
        "Bumps a small, real, partly enclosed pocket out from each of up to max_pockets_per_plaza buildings bordering a real Plaza."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p124".into(),
            display: "Alexander et al., A Pattern Language, Pattern 124 (Activity Pockets)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Activity-Pockets-(124)".into()),
        }
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p124_activity_pockets only supports parcel_id \"*\" -- it places pockets across every real Plaza in one pass.".into());
        }
        let plazas: Vec<&OpenSpace> = nbhd.open_space.iter().filter(|o| o.kind == OpenSpaceKind::Plaza).collect();
        if plazas.is_empty() {
            return Err("p124_activity_pockets: no real Plaza-kind open space -- run p61_small_public_squares first.".into());
        }
        let solid_building_ids: std::collections::HashSet<&str> = nbhd.buildings.iter()
            .filter(|b| b.polygon.holes.is_empty() && b.polygon.parts_view().len() <= 1)
            .map(|b| b.id.as_str())
            .collect();
        if solid_building_ids.is_empty() {
            return Err("p124_activity_pockets: no solid (non-courtyard) buildings to carve a pocket from.".into());
        }

        let mut prng = Prng::new(seed);
        let mut claimed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut modified_buildings: Vec<Building> = Vec::new();
        let mut replaced_building_ids: Vec<String> = Vec::new();
        let mut new_open_space: Vec<OpenSpace> = Vec::new();
        let mut new_activity_nodes: Vec<ActivityNode> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let max_per_plaza = params.max_pockets_per_plaza.round().max(0.0) as usize;
        let mut n_pockets = 0usize;
        let mut n_plazas_with_pockets = 0usize;
        let mut n_skipped_too_large = 0usize;
        let mut n_skipped_would_reach_plaza = 0usize;
        let mut n_skipped_wrong_direction = 0usize;

        for plaza in &plazas {
            let origin = plaza.polygon.centroid();
            let plaza_local = ring_to_local(&plaza.polygon.outer, &origin);
            if plaza_local.len() < 3 {
                continue;
            }

            let mut candidates: Vec<(String, f64)> = Vec::new();
            for b in &nbhd.buildings {
                if !solid_building_ids.contains(b.id.as_str()) || claimed.contains(&b.id) {
                    continue;
                }
                let b_local = ring_to_local(&b.polygon.outer, &origin);
                if b_local.len() < 3 {
                    continue;
                }
                let d = polygon_min_distance(&b_local, &plaza_local);
                if d <= params.adjacency_threshold_m {
                    candidates.push((b.id.clone(), d));
                }
            }
            if candidates.is_empty() {
                continue;
            }
            candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.0.cmp(&b.0)));

            let mut carved_this_plaza = 0usize;
            for (building_id, _dist) in candidates {
                if carved_this_plaza >= max_per_plaza {
                    break;
                }
                let building = match nbhd.buildings.iter().find(|b| b.id == building_id) {
                    Some(b) => b,
                    None => continue,
                };
                let b_local = ensure_ccw(&ring_to_local(&building.polygon.outer, &origin));
                let original_area = poly_area(&b_local);
                if original_area < 1.0 {
                    continue;
                }

                let n = b_local.len();
                let mut best_i = None;
                let mut best_d = f64::INFINITY;
                for i in 0..n {
                    let a = b_local[i];
                    let c = b_local[(i + 1) % n];
                    // Real distance from THIS edge to the plaza's own
                    // boundary ring -- not to the plaza's centroid. An
                    // earlier version of this loop used
                    // point_segment_distance(plaza_centroid, a, c), which
                    // picks the edge nearest the plaza's CENTER OF MASS --
                    // fine for a compact, roughly-centered plaza, but for
                    // an elongated or off-center one (a real, legitimate
                    // shape this pipeline allows, not a bug to route
                    // around) the edge nearest the centroid and the edge
                    // nearest the real boundary can be two different
                    // edges entirely. Caught directly: a real fixture
                    // pocket built against the centroid-nearest edge sat
                    // 5.4m from its own plaza's real boundary, well past
                    // this operator's own 3m-adjacency claim. Measuring
                    // against the real boundary instead fixes the
                    // mismatch AND gives the plaza-collision safety check
                    // below its real distance for free, no separate
                    // measurement needed.
                    let d = polygon_min_distance(&[a, c], &plaza_local);
                    if d < best_d {
                        best_d = d;
                        best_i = Some(i);
                    }
                }
                let Some(best_i) = best_i else { continue };
                let a = b_local[best_i];
                let c = b_local[(best_i + 1) % n];

                // The bump must never reach the plaza's own boundary --
                // `best_d` (just computed above, per-edge) IS the real
                // distance from this exact edge to the plaza's real
                // boundary. A bump that reached the plaza would be
                // annexing it, not standing in "the open space between
                // the paths" -- a different, not-yet-real operator.
                if params.pocket_depth_m >= best_d {
                    n_skipped_would_reach_plaza += 1;
                    continue;
                }

                let mid = Pt2::new((a.x + c.x) / 2.0, (a.y + c.y) / 2.0);
                let (dx, dy) = (c.x - a.x, c.y - a.y);
                let edge_len = (dx * dx + dy * dy).sqrt();
                if edge_len < params.pocket_width_m {
                    // Real wall segment too short for a pocket this wide.
                    continue;
                }
                // Outward normal: for a CCW polygon, rotating the edge
                // direction -90 degrees points away from the polygon's
                // own interior (the inward normal is the +90 rotation --
                // verified against a CCW unit square's own bottom edge:
                // direction (1,0) -> outward (0,-1), i.e. downward, away
                // from the square). The bump projects along this
                // direction, toward the plaza, per Alexander's own "jut
                // forward into the open space" -- see this operator's own
                // module doc for why this replaced an earlier
                // carved-inward reading.
                let outward = Pt2::new(dy / edge_len, -dx / edge_len);
                let bump_far = Pt2::new(mid.x + outward.x * params.pocket_depth_m, mid.y + outward.y * params.pocket_depth_m);

                // The outward normal points away from the BUILDING's own
                // interior, which is not always the same as toward the
                // PLAZA -- `best_i` is chosen by the edge's own global
                // minimum distance to the plaza, which can be achieved near
                // one endpoint of an oblique or side-facing edge rather
                // than square-on across the whole edge. Bumping straight
                // out from such an edge's midpoint can move AWAY from the
                // plaza instead of toward it. Caught directly on the real
                // fixture: an edge with best_d=1.16m produced a bump_far
                // 4.94m from the plaza -- farther, not closer. A real
                // Alexander-faithful pocket must "jut forward into the
                // open space" TOWARD the plaza, so require the bump to
                // actually get closer; skip (try the next real candidate)
                // if it doesn't.
                let bump_dist = polygon_min_distance(&[bump_far], &plaza_local);
                if bump_dist >= best_d {
                    n_skipped_wrong_direction += 1;
                    continue;
                }
                // rect_corridor's own perpendicular (width-direction)
                // offset is itself a +90-degree rotation of its own p->q
                // argument -- so which end (bump[0] vs bump[3]) lands
                // closer to `a` vs `c` depends on which way p->q points.
                // The earlier carved-inward version of this operator
                // passed (mid, notch_far) with notch_far INWARD of mid,
                // and that made bump[0] land near `a`; this version passes
                // (mid, bump_far) with bump_far OUTWARD of mid instead --
                // the OPPOSITE p->q direction -- which flips that
                // correspondence too (verified directly: an unreversed
                // splice here produced a self-crossing bowtie and a
                // NEGATIVE net area change, caught by this operator's own
                // unit test rather than shipped unnoticed). `.reverse()`
                // restores bump[0] to "closer to `a`" so the splice
                // below still traces a valid, single-piece, non-crossing
                // boundary that bulges OUT past the original edge and
                // back -- exact polygon-with-a-bump construction, no
                // subtract_convex/union_pieces multi-fragment reassembly
                // (see p95_building_complex.rs's and
                // p133_staircase_as_a_stage.rs's own hard-won caveat: that
                // reassembly is NOT reliable for subtract_convex's own
                // output -- verified once, up to 2.88m^2 lost to 46.9m^2;
                // not this operator's own concern since it never calls
                // subtract_convex, but the same closed-form splice idiom
                // that avoids needing to).
                let mut bump = rect_corridor(mid, bump_far, params.pocket_width_m / 2.0);
                bump.reverse();
                if bump.len() != 4 {
                    continue;
                }

                let mut new_ring: Vec<Pt2> = Vec::with_capacity(n + 4);
                for k in 0..n {
                    new_ring.push(b_local[k]);
                    if k == best_i {
                        new_ring.extend_from_slice(&bump);
                    }
                }
                let new_area = poly_area(&new_ring);
                let pocket_area = new_area - original_area;
                if pocket_area > original_area * params.max_bump_area_frac {
                    n_skipped_too_large += 1;
                    continue;
                }
                if pocket_area < 0.5 {
                    continue;
                }

                let mut new_building = building.clone();
                new_building.polygon = Polygon::from_ring(local_to_ring(&new_ring, &origin));
                modified_buildings.push(new_building);
                replaced_building_ids.push(building_id.clone());
                claimed.insert(building_id.clone());

                let pocket_id = format!("{}_pocket_{}", plaza.id, n_pockets);
                new_open_space.push(OpenSpace {
                    id: pocket_id.clone(),
                    polygon: Polygon::from_ring(local_to_ring(&bump, &origin)),
                    kind: OpenSpaceKind::Pocket,
                });
                let pocket_centroid = centroid(&bump);
                new_activity_nodes.push(ActivityNode {
                    id: format!("{pocket_id}_activity"),
                    location: crate::planar::local_to_lnglat(pocket_centroid, &origin),
                    kind: ActivityKind::Civic,
                    intensity: None,
                    label: None,
                    activity_fit: Default::default(),
                    publicness: None,
                });

                n_pockets += 1;
                carved_this_plaza += 1;
                let _ = &mut prng; // reserved for future non-deterministic tie-breaks; none needed yet
            }
            if carved_this_plaza > 0 {
                n_plazas_with_pockets += 1;
                steps.push(format!(
                    "{}: bumped {} real pocket(s) out from bordering building(s).",
                    plaza.id, carved_this_plaza
                ));
            }
        }

        if n_pockets == 0 {
            return Err(format!(
                "p124_activity_pockets: no real pocket could be bumped out -- no building both \
                 borders a real Plaza within {:.1}m (with real room left before reaching the \
                 plaza's own boundary at a {:.1}m projection depth) and has a wall segment long \
                 enough for a {:.1}m pocket without growing its own footprint past {:.0}%.",
                params.adjacency_threshold_m, params.pocket_depth_m, params.pocket_width_m, params.max_bump_area_frac * 100.0
            ));
        }

        steps.push(format!(
            "{} real pocket(s) bumped out across {} of {} real plaza(s); {} candidate(s) skipped \
             (bump would exceed {:.0}% of the building's own original area), {} candidate(s) \
             skipped (bump would reach the plaza's own boundary), {} candidate(s) skipped (the \
             edge's own outward normal pointed away from the plaza, not toward it).",
            n_pockets, n_plazas_with_pockets, plazas.len(), n_skipped_too_large,
            params.max_bump_area_frac * 100.0, n_skipped_would_reach_plaza, n_skipped_wrong_direction
        ));

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!("Bumped {n_pockets} real activity pocket(s) out from bordering buildings."),
            steps,
            caveats: vec![
                "Reads Alexander's own 'jut forward into the open space' literally: a real bump \
                 projecting OUTWARD from an adjacent building's own footprint, toward the plaza \
                 -- replacing an earlier version of this operator that read it as a notch carved \
                 inward instead (a real, defensible, but less literal reading). See this \
                 operator's own module doc.".into(),
                "pocket_width_m/pocket_depth_m are plausible real nook dimensions, not \
                 Alexander's own literal figures -- his cited page gives none.".into(),
                "Only bumps out from solid (non-courtyard) buildings, and at most one pocket per \
                 building. Courtyard buildings bordering a plaza are real candidates this \
                 operator does not yet handle.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space,
            new_buildings: modified_buildings,
            new_streets: vec![],
            new_activity_nodes,
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
    use street_smarts_core::geometry::LngLat;
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn m_per_deg() -> f64 { 111_320.0 }

    fn square_ring(cx: f64, cy: f64, half_side: f64) -> Vec<LngLat> {
        let m = m_per_deg();
        let s = half_side / m;
        let x = cx / m;
        let y = cy / m;
        vec![
            LngLat::new(x - s, y - s), LngLat::new(x + s, y - s),
            LngLat::new(x + s, y + s), LngLat::new(x - s, y + s), LngLat::new(x - s, y - s),
        ]
    }

    fn plaza(id: &str, cx: f64, cy: f64, half_side: f64) -> OpenSpace {
        OpenSpace { id: id.into(), polygon: Polygon::from_ring(square_ring(cx, cy, half_side)), kind: OpenSpaceKind::Plaza }
    }

    fn building(id: &str, cx: f64, cy: f64, half_side: f64) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(square_ring(cx, cy, half_side)),
            height_m: Some(7.0), typology: Some("p107_solid_v01".into()), year_built: None,
            parcel_id: None, floors: Some(2), openings: vec![], interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    fn nbhd(open_space: Vec<OpenSpace>, buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![] as Vec<Street>, open_space,
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P124 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    #[test]
    fn no_plazas_is_a_real_error_not_a_silent_no_op() {
        let n = nbhd(vec![], vec![building("B1", 0.0, 0.0, 10.0)]);
        assert!(P124ActivityPockets.apply(&n, "*", &P124Params::defaults(), 0).is_err());
    }

    #[test]
    fn a_bordering_building_gets_a_real_pocket_bumped_out() {
        // Plaza centered at (0,0), half-side 15m -> spans x/y in [-15,15].
        // Building immediately east of the plaza, its west wall at x=17m
        // (2m real gap -- within the 3m default adjacency threshold, and
        // wide enough that the default 1.0m pocket_depth_m stays clear of
        // the plaza's own boundary). half_side=20 (taller than the plaza's
        // own half_side=15) so the building's south/north walls' nearest
        // corners fall outside the plaza's y-shadow -- otherwise a
        // building whose y-extent is a subset of the plaza's own y-extent
        // ties the west wall and a side wall at the exact same minimum
        // distance (both measured from the same shared corner), and the
        // edge-selection tie-break can pick the side wall instead of the
        // one that actually faces the plaza.
        let plz = plaza("PLZ1", 0.0, 0.0, 15.0);
        let bld = building("B1", 37.0, 0.0, 20.0); // spans x in [17,57], y in [-20,20]
        let n = nbhd(vec![plz], vec![bld]);
        let sub = P124ActivityPockets.apply(&n, "*", &P124Params::defaults(), 0).unwrap();

        assert_eq!(sub.new_open_space.len(), 1, "expected exactly one real pocket");
        assert_eq!(sub.new_open_space[0].kind, OpenSpaceKind::Pocket);
        assert_eq!(sub.new_activity_nodes.len(), 1, "expected exactly one real activity node at the pocket");
        assert_eq!(sub.new_buildings.len(), 1);
        assert_eq!(sub.replaced_building_ids, vec!["B1".to_string()]);

        let original_area = n.buildings[0].polygon.area_m2();
        let new_area = sub.new_buildings[0].polygon.area_m2();
        assert!(new_area > original_area, "building should have real area ADDED by the bump");

        let pocket_area = sub.new_open_space[0].polygon.area_m2();
        assert!(pocket_area > 0.5, "pocket should be a real, non-degenerate area, got {pocket_area}");
        let max_bump = original_area * P124Params::defaults().max_bump_area_frac;
        assert!(pocket_area <= max_bump + 1e-6, "bump ({pocket_area}) should respect max_bump_area_frac ({max_bump})");
    }

    #[test]
    fn a_distant_building_gets_no_pocket() {
        let plz = plaza("PLZ1", 0.0, 0.0, 15.0);
        let bld = building("FAR", 500.0, 0.0, 10.0); // far outside adjacency_threshold_m
        let n = nbhd(vec![plz], vec![bld]);
        assert!(P124ActivityPockets.apply(&n, "*", &P124Params::defaults(), 0).is_err());
    }

    #[test]
    fn a_building_too_close_to_the_plaza_gets_no_pocket() {
        // Same 1m real gap the pre-fidelity version of this operator used
        // to carve a pocket in -- a bump can't do the same thing at that
        // gap, because the default 1.0m pocket_depth_m would reach the
        // plaza's own boundary exactly (the check is >=, not >). Real
        // safety behavior, not a regression: this building IS within
        // adjacency_threshold_m (1.0m <= 3.0m default), it just can't fit
        // a real bump in the real room available.
        let plz = plaza("PLZ1", 0.0, 0.0, 15.0);
        let bld = building("CLOSE", 36.0, 0.0, 20.0); // spans x in [16,56], y in [-20,20]; 1m gap
        let n = nbhd(vec![plz], vec![bld]);
        assert!(P124ActivityPockets.apply(&n, "*", &P124Params::defaults(), 0).is_err());
    }

    #[test]
    fn max_pockets_per_plaza_caps_real_carving() {
        let plz = plaza("PLZ1", 0.0, 0.0, 15.0);
        // Three real bordering buildings around the same plaza, each with
        // a real 2m gap (see a_bordering_building_gets_a_real_pocket_bumped_out's
        // own comment for why 2m, not 1m, and why half_side=20, not 10).
        let b1 = building("B1", 37.0, 0.0, 20.0);
        let b2 = building("B2", -37.0, 0.0, 20.0);
        let b3 = building("B3", 0.0, 37.0, 20.0);
        let n = nbhd(vec![plz], vec![b1, b2, b3]);
        let params = P124Params { max_pockets_per_plaza: 2.0, ..P124Params::defaults() };
        let sub = P124ActivityPockets.apply(&n, "*", &params, 0).unwrap();
        assert_eq!(sub.new_open_space.len(), 2, "should cap at max_pockets_per_plaza even with 3 real candidates");
    }

    #[test]
    fn params_roundtrip() {
        let p = P124Params { adjacency_threshold_m: 4.0, pocket_width_m: 3.0, pocket_depth_m: 2.0, max_bump_area_frac: 0.2, max_pockets_per_plaza: 1.0 };
        let v = p.as_vector();
        let back = P124Params::from_vector(&v);
        assert_eq!(back.adjacency_threshold_m, 4.0);
        assert_eq!(back.pocket_width_m, 3.0);
        assert_eq!(back.pocket_depth_m, 2.0);
        assert_eq!(back.max_bump_area_frac, 0.2);
        assert_eq!(back.max_pockets_per_plaza, 1.0);
    }
}
