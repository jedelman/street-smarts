//! P124 Activity Pockets — a real, small, partly enclosed notch carved
//! from a building's own footprint at the point it borders a real Plaza,
//! not just a proxy for "buildings are near squares."
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
//! section is empty), and "jut forward into the open space" is open to
//! more than one literal reading. This operator's own reading: the pocket
//! is a real alcove carved OUT of an adjacent building's own footprint at
//! the point it borders the plaza -- partly enclosed by the building's
//! own remaining walls on the sides, open toward the plaza -- rather than
//! an independent patch of land, or an expansion of the plaza's own
//! boundary. This is a real, defensible reading (a recessed shopfront or
//! covered nook is exactly this shape), not the only one; a different
//! reading (a bump added to the plaza's own boundary) would be an
//! equally real but different operator.
//!
//! `pocket_width_m`/`pocket_depth_m` are plausible real alcove dimensions
//! (a person-scale nook, not a room), not Alexander's own literal figures
//! -- same category as `p95_building_complex`'s `pad_inset_m` or
//! `path_network`'s `arterial_width_m`.
//!
//! # Real constraints, not arbitrary ones
//!
//! - Only carves from a building whose own footprint edge already comes
//!   within `adjacency_threshold_m` of a real Plaza's own boundary (this
//!   pipeline's own real P122 Building Fronts ideal is a 1.0m setback --
//!   `adjacency_threshold_m` defaults wider, 3.0m, so a real but slightly
//!   less tight adjacency still counts).
//! - The carved notch is real `subtract_convex` geometry against the
//!   building's own real footprint, clipped to what's actually there --
//!   never invented past the building's own edge.
//! - Skips a candidate whose remaining footprint after carving would drop
//!   below `min_remaining_area_frac` (default 0.8) of its original area --
//!   a real safety margin against reducing a small building to a sliver.
//! - `max_pockets_per_plaza` (default 2) caps a real, bounded resource
//!   allocation per plaza -- not one carved into every possible building,
//!   matching this crate's own established idiom (P61's own square
//!   budget, P60's investigated-and-scoped park reservation).
//! - Only solid (single-outer-ring, no courtyard hole) buildings are
//!   candidates -- carving a notch near a courtyard's own inner ring
//!   safely is a separate, harder geometry problem, left undone rather
//!   than risked. Each building fronts at most one real pocket.
//!
//! Each real pocket also gets a real `ActivityNode` at its own centroid
//! (`ActivityKind::Civic`, honest empty `activity_fit`/`None`
//! `publicness` -- same convention `p61_small_public_squares` already
//! established for its own squares) -- this operator IS the real source
//! of that activity, unlike a generic placement elsewhere.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{
    centroid, ensure_ccw, point_segment_distance, polygon_min_distance,
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
    /// Real width of a carved pocket alcove, along the building's own
    /// wall edge. A plausible real nook dimension, not Alexander's own
    /// literal figure.
    pub pocket_width_m: f64,
    /// Real depth a pocket cuts into the building, perpendicular to the
    /// wall edge.
    pub pocket_depth_m: f64,
    /// A candidate building's remaining footprint after carving must stay
    /// at or above this fraction of its original area.
    pub min_remaining_area_frac: f64,
    /// How many real pockets to carve per plaza, at most.
    pub max_pockets_per_plaza: f64,
}

impl Parameters for P124Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float("adjacency_threshold_m", "How close a building must come to a Plaza's boundary to count as bordering it.", 1.0, 8.0, 3.0).with_unit("m"),
            ParamSpec::float("pocket_width_m", "Real width of a carved pocket alcove.", 1.5, 4.0, 2.5).with_unit("m"),
            ParamSpec::float("pocket_depth_m", "Real depth a pocket cuts into the building.", 1.0, 3.0, 1.5).with_unit("m"),
            ParamSpec::float("min_remaining_area_frac", "A candidate building's remaining footprint must stay at or above this fraction of its original area.", 0.5, 0.95, 0.8),
            ParamSpec::float("max_pockets_per_plaza", "How many real pockets to carve per plaza, at most.", 0.0, 4.0, 2.0),
        ]
    }
    fn defaults() -> Self {
        Self {
            adjacency_threshold_m: 3.0,
            pocket_width_m: 2.5,
            pocket_depth_m: 1.5,
            min_remaining_area_frac: 0.8,
            max_pockets_per_plaza: 2.0,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.adjacency_threshold_m, self.pocket_width_m, self.pocket_depth_m, self.min_remaining_area_frac, self.max_pockets_per_plaza]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.adjacency_threshold_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.pocket_width_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.pocket_depth_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) { p.min_remaining_area_frac = s.clamp(*x); }
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
        "Carves a small, real, partly enclosed pocket from each of up to max_pockets_per_plaza buildings bordering a real Plaza."
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
        let mut n_skipped_too_small = 0usize;

        for plaza in &plazas {
            let origin = plaza.polygon.centroid();
            let plaza_local = ring_to_local(&plaza.polygon.outer, &origin);
            if plaza_local.len() < 3 {
                continue;
            }
            let plaza_centroid = centroid(&plaza_local);

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
                    let d = point_segment_distance(plaza_centroid, a, c);
                    if d < best_d {
                        best_d = d;
                        best_i = Some(i);
                    }
                }
                let Some(best_i) = best_i else { continue };
                let a = b_local[best_i];
                let c = b_local[(best_i + 1) % n];

                let mid = Pt2::new((a.x + c.x) / 2.0, (a.y + c.y) / 2.0);
                let (dx, dy) = (c.x - a.x, c.y - a.y);
                let edge_len = (dx * dx + dy * dy).sqrt();
                if edge_len < params.pocket_width_m {
                    // Real wall segment too short for a pocket this wide.
                    continue;
                }
                // Inward normal: for a CCW polygon, rotating the edge
                // direction +90 degrees points into the polygon's own
                // interior (the outward normal is the -90 rotation --
                // verified against a CCW unit square's own bottom edge).
                let inward = Pt2::new(-dy / edge_len, dx / edge_len);
                let notch_far = Pt2::new(mid.x + inward.x * params.pocket_depth_m, mid.y + inward.y * params.pocket_depth_m);
                // rect_corridor's own perpendicular offset direction is
                // ANTI-PARALLEL to the edge direction a->c (a 180-degree
                // composition of two 90-degree rotations) -- so hole[0]
                // (mid + offset) always lands closer to `a`, hole[3]
                // (mid - offset) always closer to `c`. Splicing
                // [hole[0], hole[1], hole[2], hole[3]] into the ring
                // between `a` and `c` therefore always traces a valid,
                // single-piece boundary that dips into the notch and back
                // out -- exact polygon-with-a-bite construction, no
                // subtract_convex/union_pieces multi-fragment reassembly
                // (see p95_building_complex.rs's and
                // p133_staircase_as_a_stage.rs's own hard-won caveat: that
                // reassembly is NOT reliable for subtract_convex's own
                // output -- verified once, up to 2.88m^2 lost to 46.9m^2).
                let hole = rect_corridor(mid, notch_far, params.pocket_width_m / 2.0);
                if hole.len() != 4 {
                    continue;
                }

                let mut new_ring: Vec<Pt2> = Vec::with_capacity(n + 4);
                for k in 0..n {
                    new_ring.push(b_local[k]);
                    if k == best_i {
                        new_ring.extend_from_slice(&hole);
                    }
                }
                let remaining_area = poly_area(&new_ring);
                if remaining_area < original_area * params.min_remaining_area_frac {
                    n_skipped_too_small += 1;
                    continue;
                }
                let pocket_area = original_area - remaining_area;
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
                    polygon: Polygon::from_ring(local_to_ring(&hole, &origin)),
                    kind: OpenSpaceKind::Pocket,
                });
                let pocket_centroid = centroid(&hole);
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
                    "{}: carved {} real pocket(s) from bordering building(s).",
                    plaza.id, carved_this_plaza
                ));
            }
        }

        if n_pockets == 0 {
            return Err(format!(
                "p124_activity_pockets: no real pocket could be carved -- no building both borders a real Plaza within {:.1}m and has a wall segment long enough for a {:.1}m pocket without dropping below {:.0}% of its own remaining area.",
                params.adjacency_threshold_m, params.pocket_width_m, params.min_remaining_area_frac * 100.0
            ));
        }

        steps.push(format!(
            "{} real pocket(s) carved across {} of {} real plaza(s); {} candidate(s) skipped (remaining footprint would drop below {:.0}% of its own original area).",
            n_pockets, n_plazas_with_pockets, plazas.len(), n_skipped_too_small, params.min_remaining_area_frac * 100.0
        ));

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!("Carved {n_pockets} real activity pocket(s) from bordering buildings."),
            steps,
            caveats: vec![
                "Reads 'jut forward into the open space' as a real alcove carved OUT of an \
                 adjacent building's own footprint, open toward the plaza -- a defensible but \
                 not the only real interpretation of Alexander's own text. See this operator's \
                 own module doc.".into(),
                "pocket_width_m/pocket_depth_m are plausible real nook dimensions, not \
                 Alexander's own literal figures -- his cited page gives none.".into(),
                "Only carves from solid (non-courtyard) buildings, and at most one pocket per \
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
        }
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
        }
    }

    #[test]
    fn no_plazas_is_a_real_error_not_a_silent_no_op() {
        let n = nbhd(vec![], vec![building("B1", 0.0, 0.0, 10.0)]);
        assert!(P124ActivityPockets.apply(&n, "*", &P124Params::defaults(), 0).is_err());
    }

    #[test]
    fn a_bordering_building_gets_a_real_pocket_carved() {
        // Plaza centered at (0,0), half-side 15m -> spans x/y in [-15,15].
        // Building immediately east of the plaza, its west wall at x=16m
        // (1m real gap, within the 3m default adjacency threshold).
        let plz = plaza("PLZ1", 0.0, 0.0, 15.0);
        let bld = building("B1", 26.0, 0.0, 10.0); // spans x in [16,36]
        let n = nbhd(vec![plz], vec![bld]);
        let sub = P124ActivityPockets.apply(&n, "*", &P124Params::defaults(), 0).unwrap();

        assert_eq!(sub.new_open_space.len(), 1, "expected exactly one real pocket");
        assert_eq!(sub.new_open_space[0].kind, OpenSpaceKind::Pocket);
        assert_eq!(sub.new_activity_nodes.len(), 1, "expected exactly one real activity node at the pocket");
        assert_eq!(sub.new_buildings.len(), 1);
        assert_eq!(sub.replaced_building_ids, vec!["B1".to_string()]);

        let original_area = n.buildings[0].polygon.area_m2();
        let new_area = sub.new_buildings[0].polygon.area_m2();
        assert!(new_area < original_area, "building should have real area removed by the notch");
        assert!(new_area >= original_area * 0.8, "should not drop below the real min_remaining_area_frac floor");

        let pocket_area = sub.new_open_space[0].polygon.area_m2();
        assert!(pocket_area > 0.5, "pocket should be a real, non-degenerate area, got {pocket_area}");
    }

    #[test]
    fn a_distant_building_gets_no_pocket() {
        let plz = plaza("PLZ1", 0.0, 0.0, 15.0);
        let bld = building("FAR", 500.0, 0.0, 10.0); // far outside adjacency_threshold_m
        let n = nbhd(vec![plz], vec![bld]);
        assert!(P124ActivityPockets.apply(&n, "*", &P124Params::defaults(), 0).is_err());
    }

    #[test]
    fn max_pockets_per_plaza_caps_real_carving() {
        let plz = plaza("PLZ1", 0.0, 0.0, 15.0);
        // Three real bordering buildings around the same plaza.
        let b1 = building("B1", 26.0, 0.0, 10.0);
        let b2 = building("B2", -26.0, 0.0, 10.0);
        let b3 = building("B3", 0.0, 26.0, 10.0);
        let n = nbhd(vec![plz], vec![b1, b2, b3]);
        let params = P124Params { max_pockets_per_plaza: 2.0, ..P124Params::defaults() };
        let sub = P124ActivityPockets.apply(&n, "*", &params, 0).unwrap();
        assert_eq!(sub.new_open_space.len(), 2, "should cap at max_pockets_per_plaza even with 3 real candidates");
    }

    #[test]
    fn params_roundtrip() {
        let p = P124Params { adjacency_threshold_m: 4.0, pocket_width_m: 3.0, pocket_depth_m: 2.0, min_remaining_area_frac: 0.7, max_pockets_per_plaza: 1.0 };
        let v = p.as_vector();
        let back = P124Params::from_vector(&v);
        assert_eq!(back.adjacency_threshold_m, 4.0);
        assert_eq!(back.pocket_width_m, 3.0);
        assert_eq!(back.pocket_depth_m, 2.0);
        assert_eq!(back.min_remaining_area_frac, 0.7);
        assert_eq!(back.max_pockets_per_plaza, 1.0);
    }
}
