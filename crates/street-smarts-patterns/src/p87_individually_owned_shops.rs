//! P87 Individually Owned Shops -- many very small shops along the street,
//! each with its own door, instead of a few large ones.
//!
//! From Alexander, *A Pattern Language*, Pattern 87 (p. 432), via
//! patternlanguage.cc/Patterns/Individually-Owned-Shops-(87):
//! > **Problem:** When shops are too large, or controlled by absentee
//! > owners, they become plastic, bland, and abstract.
//! > **Solution:** Do what you can to encourage the development of
//! > individually owned shops. ... Approve new commercial building permits
//! > only if the proposed structure includes many very very small rental
//! > spaces.
//!
//! Its related patterns tie each shop to the sidewalk ("make at least some
//! part of the shop part of the sidewalk") and to P165 Opening to the Street.
//!
//! # What this generator does
//!
//! The spatial half of the pattern, "many very very small rental spaces":
//! every ground-floor outer wall that faces a street or a square is divided
//! into small shop fronts, `shop_width_m` wide (default 5 m), and each one
//! that doesn't already have a door gets its own `OpeningKind::ShopDoor`.
//! - A wall *faces* a street or square when, looking straight out from its
//!   middle, a street's right-of-way or a square (plaza, common, pocket)
//!   comes within `reach_m` (12 m) before another building does: the same
//!   test `tools/vibe-render/render.py`'s `frontage_detail` uses to decide
//!   where the walk builds storefronts.
//! - A door already on the wall (P221, P110, P100) keeps its own shop's
//!   width either side; the rest of the wall is shared out evenly.
//! - A new door goes in the gap between existing ground-floor openings
//!   nearest the middle of its shop front. Where windows leave no gap, the
//!   window nearest the middle becomes the shop's door, glazed and as wide
//!   as the window was (up to 2 m), so the wall stays as open as P165 made it.
//!
//! # Why `ShopDoor`, not `Door`
//!
//! P110 Main Entrance scores a building by how singular its entrance is
//! (`1 / doors`). A shop's door isn't an entrance to the building, so it's
//! its own kind, and P110, P102 and the interior patterns, which read
//! `Door`, don't see it.
//!
//! # Caveats
//!
//! - Ownership, the pattern's first instruction, isn't modeled: the schema
//!   has no owners, leases or licences. This places the many small spaces
//!   the permit instruction asks for, nothing more.
//! - The schema has no ground-floor use, so every ground floor that faces a
//!   street or square is treated as shop frontage. That's a mixed-use main
//!   street assumption, stated here rather than hidden.
//! - "Very very small" has no number in the pattern; 5 m of frontage is a
//!   plausible small-shop figure, not Alexander's.
//! - Doors only. Interior cells aren't split into the shops behind them.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{self, Pt2};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Building, Neighborhood, OpenSpaceKind, Opening, OpeningKind};
use street_smarts_core::opinion::SourceCitation;

/// How far out a wall looks for a street or square, in steps (render.py's FRONTAGE_PROBE_M).
const PROBES_M: [f64; 7] = [1.5, 3.0, 4.5, 6.0, 8.0, 10.0, 12.0];
/// A shop front stops this short of a wall's corner (render.py's SHOP_END_MARGIN_M).
const END_MARGIN_M: f64 = 0.35;
/// Clear wall kept either side of a new door, between it and the next opening.
const DOOR_CLEARANCE_M: f64 = 0.15;
/// The widest window a shop door may take over.
const MAX_TAKEOVER_M: f64 = 2.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P87Params {
    /// Frontage of one small shop (a plausible figure; the pattern gives none).
    pub shop_width_m: f64,
    /// Width of a new shop door.
    pub door_width_m: f64,
    /// Head height of a new shop door.
    pub door_head_height_m: f64,
    /// How far out a wall looks for a street or square.
    pub reach_m: f64,
}

impl Parameters for P87Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float("shop_width_m", "Frontage of one small shop (a plausible figure; the pattern says only 'very very small').", 3.0, 12.0, 5.0),
            ParamSpec::float("door_width_m", "Width of a new shop door.", 0.8, 2.0, 1.0),
            ParamSpec::float("door_head_height_m", "Head height of a new shop door.", 2.0, 2.8, 2.3),
            ParamSpec::float("reach_m", "How far out a wall looks for a street or square.", 3.0, 24.0, 12.0),
        ]
    }
    fn defaults() -> Self {
        Self { shop_width_m: 5.0, door_width_m: 1.0, door_head_height_m: 2.3, reach_m: 12.0 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.shop_width_m, self.door_width_m, self.door_head_height_m, self.reach_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        let mut slots = [&mut p.shop_width_m, &mut p.door_width_m, &mut p.door_head_height_m, &mut p.reach_m];
        for (i, slot) in slots.iter_mut().enumerate() {
            if let (Some(s), Some(x)) = (schema.get(i), v.get(i)) {
                **slot = s.clamp(*x);
            }
        }
        p
    }
}

/// A ring with its bounding box, in local metres.
struct Ring {
    pts: Vec<Pt2>,
    lo: Pt2,
    hi: Pt2,
}

impl Ring {
    fn new(pts: Vec<Pt2>) -> Self {
        let (lo, hi) = planar::bbox(&pts);
        Self { pts, lo, hi }
    }
    fn contains(&self, p: Pt2) -> bool {
        p.x >= self.lo.x && p.x <= self.hi.x && p.y >= self.lo.y && p.y <= self.hi.y && planar::point_in_polygon(p, &self.pts)
    }
}

/// Everything the "does this wall face a street or square" test needs.
struct Surroundings {
    footprints: Vec<(Ring, Vec<Ring>)>,
    squares: Vec<Ring>,
    streets: Vec<(Vec<Pt2>, f64)>,
}

impl Surroundings {
    fn faces_public(&self, mid: Pt2, n: Pt2, reach: f64) -> bool {
        for d in PROBES_M.iter().copied().filter(|d| *d <= reach) {
            let p = Pt2::new(mid.x + n.x * d, mid.y + n.y * d);
            if self.footprints.iter().any(|(outer, holes)| outer.contains(p) && !holes.iter().any(|h| h.contains(p))) {
                return false; // another building (or this one, round a corner) comes first
            }
            if self.squares.iter().any(|s| s.contains(p)) {
                return true;
            }
            let near_street = self.streets.iter().any(|(line, half)| {
                line.windows(2).any(|w| planar::point_segment_distance(p, w[0], w[1]) <= half + 0.5)
            });
            if near_street {
                return true;
            }
        }
        false
    }
}

fn signed_area(ring: &[Pt2]) -> f64 {
    let n = ring.len();
    (0..n).map(|i| ring[i].x * ring[(i + 1) % n].y - ring[(i + 1) % n].x * ring[i].y).sum::<f64>() / 2.0
}

/// The free stretches of [lo, hi] once `taken` (sorted or not) is removed.
fn free_spans(lo: f64, hi: f64, mut taken: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    taken.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut at = lo;
    for (a, b) in taken {
        if a > at {
            out.push((at, a.min(hi)));
        }
        at = at.max(b);
        if at >= hi {
            break;
        }
    }
    if at < hi {
        out.push((at, hi));
    }
    out.into_iter().filter(|(a, b)| b > a).collect()
}

pub struct P87IndividuallyOwnedShops;

impl PatternOperator for P87IndividuallyOwnedShops {
    type Params = P87Params;

    fn name(&self) -> &'static str {
        "p87_individually_owned_shops"
    }

    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p87".into(),
            display: "Alexander et al., A Pattern Language, Pattern 87 (Individually Owned Shops)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Individually-Owned-Shops-(87)".into()),
        }
    }

    fn description(&self) -> &'static str {
        "Divide every ground floor that faces a street or square into very small shop fronts, each with its own door."
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        _parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if nbhd.buildings.is_empty() {
            return Err("p87_individually_owned_shops: no buildings to give shop fronts to.".into());
        }
        let [x0, y0, x1, y1] = nbhd.bbox_wgs84;
        let origin = LngLat::new((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let local = |r: &Vec<LngLat>| planar::ring_to_local(r, &origin);
        let around = Surroundings {
            footprints: nbhd
                .buildings
                .iter()
                .flat_map(|b| b.polygon.parts_view())
                .map(|p| (Ring::new(local(&p.outer)), p.holes.iter().map(|h| Ring::new(local(h))).collect()))
                .collect(),
            squares: nbhd
                .open_space
                .iter()
                .filter(|o| matches!(o.kind, OpenSpaceKind::Plaza | OpenSpaceKind::Common | OpenSpaceKind::Pocket))
                .map(|o| Ring::new(local(&o.polygon.outer)))
                .collect(),
            streets: nbhd
                .streets
                .iter()
                .filter(|s| s.centerline.len() >= 2)
                .map(|s| (s.centerline.iter().map(|p| planar::lnglat_to_local(p, &origin)).collect(), s.row_width_m.unwrap_or(4.0) / 2.0))
                .collect(),
        };

        let sw = params.shop_width_m;
        let dw = params.door_width_m;
        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced_building_ids = Vec::new();
        let mut steps = Vec::new();
        let (mut n_walls, mut n_in_gaps, mut n_taken_over) = (0usize, 0usize, 0usize);

        for b in &nbhd.buildings {
            let ring = local(&b.polygon.outer);
            let n = ring.len();
            if n < 3 {
                continue;
            }
            let ccw = signed_area(&ring) >= 0.0;
            let mut openings = b.openings.clone();
            let mut taken_over: Vec<usize> = Vec::new();
            let mut added: Vec<Opening> = Vec::new();
            for i in 0..n {
                let (a, c) = (ring[i], ring[(i + 1) % n]);
                let len = a.dist(c);
                if len < 2.0 * END_MARGIN_M + sw * 0.6 {
                    continue;
                }
                let d = Pt2::new((c.x - a.x) / len, (c.y - a.y) / len);
                let normal = if ccw { Pt2::new(d.y, -d.x) } else { Pt2::new(-d.y, d.x) };
                let mid = Pt2::new((a.x + c.x) / 2.0, (a.y + c.y) / 2.0);
                if !around.faces_public(mid, normal, params.reach_m) {
                    continue;
                }
                // Ground-floor openings on this wall, as spans along it: (centre, half width, index).
                let mut spans: Vec<(f64, f64, Option<usize>)> = openings
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| !o.on_hole && o.ring_index == i && o.floor == 0)
                    .map(|(k, o)| (o.t * len, o.width_m / 2.0, Some(k)))
                    .collect();
                let door_at: Vec<f64> = openings
                    .iter()
                    .filter(|o| !o.on_hole && o.ring_index == i && o.floor == 0 && matches!(o.kind, OpeningKind::Door | OpeningKind::ShopDoor))
                    .map(|o| o.t * len)
                    .collect();
                let reserved: Vec<(f64, f64)> = door_at.iter().map(|x| (x - sw / 2.0, x + sw / 2.0)).collect();
                let mut wall_added = 0;
                for (s, e) in free_spans(END_MARGIN_M, len - END_MARGIN_M, reserved) {
                    if e - s < sw * 0.6 {
                        continue;
                    }
                    let units = ((e - s) / sw).floor().max(1.0) as usize;
                    let uw = (e - s) / units as f64;
                    for k in 0..units {
                        let (u0, u1) = (s + k as f64 * uw, s + (k + 1) as f64 * uw);
                        let target = (u0 + u1) / 2.0;
                        let clear = |p: f64, spans: &[(f64, f64, Option<usize>)]| {
                            spans.iter().all(|(c, h, _)| (p - c).abs() >= h + dw / 2.0 + DOOR_CLEARANCE_M)
                        };
                        // The gap nearest the middle of this shop front.
                        let room = (uw / 2.0 - dw / 2.0 - 0.3).max(0.0);
                        let steps_n = (room / 0.1).floor() as i32;
                        let gap = (0..=steps_n)
                            .flat_map(|j| [j, -j])
                            .map(|j| target + j as f64 * 0.1)
                            .find(|p| clear(*p, &spans));
                        let (at, width) = match gap {
                            Some(p) => {
                                n_in_gaps += 1;
                                (p, dw)
                            }
                            None => {
                                // No gap: the window nearest the middle becomes the shop's door.
                                let pick = spans
                                    .iter()
                                    .filter(|(c, _, idx)| *c > u0 && *c < u1 && idx.is_some_and(|k| openings[k].kind == OpeningKind::Window))
                                    .min_by(|x, y| (x.0 - target).abs().total_cmp(&(y.0 - target).abs()))
                                    .copied();
                                let Some((c, h, Some(idx))) = pick else { continue };
                                taken_over.push(idx);
                                spans.retain(|sp| sp.2 != Some(idx));
                                n_taken_over += 1;
                                (c, (2.0 * h).clamp(dw, MAX_TAKEOVER_M))
                            }
                        };
                        spans.push((at, width / 2.0, None));
                        added.push(Opening {
                            kind: OpeningKind::ShopDoor,
                            ring_index: i,
                            on_hole: false,
                            t: at / len,
                            width_m: width,
                            sill_height_m: 0.0,
                            head_height_m: params.door_head_height_m,
                            floor: 0,
                        });
                        wall_added += 1;
                    }
                }
                if wall_added > 0 {
                    n_walls += 1;
                }
            }
            if added.is_empty() {
                continue;
            }
            let n_added = added.len();
            let mut kept: Vec<Opening> = openings
                .drain(..)
                .enumerate()
                .filter(|(k, _)| !taken_over.contains(k))
                .map(|(_, o)| o)
                .collect();
            kept.extend(added);
            let mut nb = b.clone();
            nb.openings = kept;
            steps.push(format!("{}: {n_added} shop door(s) on its street-facing walls.", b.id));
            new_buildings.push(nb);
            replaced_building_ids.push(b.id.clone());
        }

        let n_doors = n_in_gaps + n_taken_over;
        steps.insert(
            0,
            format!(
                "{n_doors} shop doors on {n_walls} walls facing a street or square: {n_in_gaps} between existing openings, \
                 {n_taken_over} in place of a window."
            ),
        );
        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!("{n_doors} small shops, each with its own door, on {} buildings.", new_buildings.len()),
            steps,
            caveats: vec![
                "Ownership, the pattern's first instruction, isn't modeled; this places the many very small spaces its permit rule asks for.".into(),
                "The schema has no ground-floor use, so every ground floor facing a street or square is treated as shop frontage.".into(),
                format!("{sw:.1} m of frontage per shop is a plausible small-shop figure, not Alexander's."),
                "Doors only: the interior cells behind them aren't split into shops.".into(),
            ],
            seed,
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
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
            new_fields: vec![],
            new_trees: vec![],
            new_ground_cover: vec![],
            new_transit: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{NeighborhoodMeta, OpenSpace, Street};

    const M: f64 = 1.0 / 111_320.0;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<LngLat> {
        vec![
            LngLat::new(x0 * M, y0 * M),
            LngLat::new(x1 * M, y0 * M),
            LngLat::new(x1 * M, y1 * M),
            LngLat::new(x0 * M, y1 * M),
            LngLat::new(x0 * M, y0 * M),
        ]
    }

    fn door(ring_index: usize, t: f64) -> Opening {
        Opening { kind: OpeningKind::Door, ring_index, on_hole: false, t, width_m: 0.9, sill_height_m: 0.0, head_height_m: 2.1, floor: 0 }
    }
    fn window(ring_index: usize, t: f64, width_m: f64, floor: u32) -> Opening {
        Opening { kind: OpeningKind::Window, ring_index, on_hole: false, t, width_m, sill_height_m: 1.0, head_height_m: 2.8, floor }
    }

    /// A 30 m x 12 m building whose south wall (ring edge 0) faces a street
    /// 6 m away; its north wall faces nothing within reach.
    fn site(openings: Vec<Opening>) -> Neighborhood {
        let mut b: Building = serde_json::from_value(serde_json::json!({"id": "B1", "polygon": {"outer": [], "holes": []}})).unwrap();
        b.polygon = Polygon::from_ring(rect(0.0, 0.0, 30.0, 12.0));
        b.openings = openings;
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [-20.0 * M, -20.0 * M, 50.0 * M, 60.0 * M],
            parcels: vec![],
            buildings: vec![b],
            streets: vec![Street {
                id: "S1".into(),
                centerline: vec![LngLat::new(-20.0 * M, -6.0 * M), LngLat::new(50.0 * M, -6.0 * M)],
                classification: Some("local".into()),
                row_width_m: Some(6.0),
                surface: None,
            }],
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P87 unit fixture".into(),
            },
            pattern_fields: vec![],
            trees: vec![],
            ground_cover: vec![],
            transit: vec![],
        }
    }

    fn shop_doors(sub: &Subdivision) -> Vec<&Opening> {
        sub.new_buildings.iter().flat_map(|b| &b.openings).filter(|o| o.kind == OpeningKind::ShopDoor).collect()
    }

    #[test]
    fn a_street_wall_gets_many_small_shops_and_the_back_wall_none() {
        let sub = P87IndividuallyOwnedShops.apply(&site(vec![door(0, 0.5)]), "*", &P87Params::defaults(), 1).unwrap();
        let doors = shop_doors(&sub);
        // 30 m wall, the building's own door keeps 5 m in the middle: 12.15 m free each side, two shops each.
        assert_eq!(doors.len(), 4, "{:?}", doors.iter().map(|d| d.t).collect::<Vec<_>>());
        assert!(doors.iter().all(|d| d.ring_index == 0), "only the street wall faces anything");
    }

    #[test]
    fn the_building_keeps_exactly_its_own_door() {
        let sub = P87IndividuallyOwnedShops.apply(&site(vec![door(0, 0.5)]), "*", &P87Params::defaults(), 1).unwrap();
        let b = &sub.new_buildings[0];
        assert_eq!(b.openings.iter().filter(|o| o.kind == OpeningKind::Door).count(), 1, "P110 still sees one entrance");
    }

    #[test]
    fn doors_fit_between_windows_or_take_one_over_and_never_overlap() {
        let windows: Vec<Opening> = (0..10).map(|k| window(0, (k as f64 + 0.5) / 10.0, 2.6, 0)).collect();
        let upper = window(0, 0.5, 2.6, 1);
        let mut ops = windows.clone();
        ops.push(upper.clone());
        let sub = P87IndividuallyOwnedShops.apply(&site(ops), "*", &P87Params::defaults(), 1).unwrap();
        let b = &sub.new_buildings[0];
        let ground: Vec<&Opening> = b.openings.iter().filter(|o| o.floor == 0 && o.ring_index == 0).collect();
        assert!(ground.iter().any(|o| o.kind == OpeningKind::ShopDoor));
        // 0.4 m gaps between 2.6 m windows: no room, so doors replace windows, as wide as they were (capped).
        assert!(ground.iter().filter(|o| o.kind == OpeningKind::ShopDoor).all(|d| d.width_m > 1.9));
        for (i, a) in ground.iter().enumerate() {
            for b in &ground[i + 1..] {
                let gap = ((a.t - b.t).abs() * 30.0) - (a.width_m + b.width_m) / 2.0;
                assert!(gap >= -1e-6, "openings overlap: {a:?} {b:?}");
            }
        }
        assert!(b.openings.contains(&upper), "upper floors untouched");
    }

    #[test]
    fn a_wall_facing_a_square_counts_and_one_behind_a_building_doesnt() {
        let mut n = site(vec![]);
        n.streets.clear();
        n.open_space.push(OpenSpace { id: "SQ".into(), polygon: Polygon::from_ring(rect(0.0, 14.0, 30.0, 30.0)), kind: OpenSpaceKind::Plaza });
        let sub = P87IndividuallyOwnedShops.apply(&n, "*", &P87Params::defaults(), 1).unwrap();
        assert!(shop_doors(&sub).iter().all(|d| d.ring_index == 2), "only the north wall faces the square");
        assert!(!shop_doors(&sub).is_empty());
        // Put a building between the wall and the square: nothing faces it now.
        let mut blocker: Building = serde_json::from_value(serde_json::json!({"id": "B2", "polygon": {"outer": [], "holes": []}})).unwrap();
        blocker.polygon = Polygon::from_ring(rect(0.0, 13.0, 30.0, 13.9));
        n.buildings.push(blocker);
        let sub = P87IndividuallyOwnedShops.apply(&n, "*", &P87Params::defaults(), 1).unwrap();
        let b1_doors = sub.new_buildings.iter().filter(|b| b.id == "B1").flat_map(|b| &b.openings).filter(|o| o.kind == OpeningKind::ShopDoor).count();
        assert_eq!(b1_doors, 0, "B1's north wall now looks at B2's back");
        assert!(sub.new_buildings.iter().any(|b| b.id == "B2"), "B2's own north wall faces the square");
    }

    #[test]
    fn running_twice_adds_nothing_the_second_time() {
        let n = site(vec![door(0, 0.5)]);
        let once = crate::subdivision::apply_subdivision(&n, &P87IndividuallyOwnedShops.apply(&n, "*", &P87Params::defaults(), 1).unwrap());
        let again = P87IndividuallyOwnedShops.apply(&once, "*", &P87Params::defaults(), 1).unwrap();
        assert!(shop_doors(&again).is_empty() || again.new_buildings.is_empty());
    }

    #[test]
    fn free_spans_subtracts() {
        assert_eq!(free_spans(0.0, 10.0, vec![(2.0, 4.0), (6.0, 7.0)]), vec![(0.0, 2.0), (4.0, 6.0), (7.0, 10.0)]);
        assert_eq!(free_spans(0.0, 10.0, vec![(-1.0, 11.0)]), vec![]);
    }
}
