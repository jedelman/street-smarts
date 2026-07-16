//! P127 Intimacy Gradient — partition each building's ground floor into a
//! depth-ordered sequence of cells, from the public-facing wall (or, for a
//! courtyard building, the entrance bay) to the deepest point.
//!
//! From Alexander, *A Pattern Language*, Pattern 127 (verified against the
//! primary text -- see README's Reference section for the link, kept as a
//! citation only, never committed as a file):
//! > Unless the spaces in a building are arranged in a sequence which
//! > corresponds to their degrees of privateness, the visits made by
//! > strangers, friends, guests, clients, family, will always be a little
//! > awkward... Lay out the spaces of a building so that they create a
//! > sequence which begins with the entrance and the most public parts of
//! > the building, then leads into the slightly more private areas, and
//! > finally to the most private domains.
//!
//! # Where this sits in the pattern graph, and why it runs where it does
//! Alexander's own transition text, immediately before pattern 127, reads:
//! *"Now, with the paths fixed, we come back to the building: Within the
//! various wings of any one building, work out the fundamental gradients
//! of space, and decide how the movement will connect the spaces in the
//! gradients"* -- followed by his own ordered list: 127, 128 (Indoor
//! Sunlight), 129 (Common Areas at the Heart), 130 (Entrance Room), 131
//! (The Flow Through Rooms), 132 (Short Passages), 133 (Staircase as a
//! Stage)... **This is a sourced sequence, not an inference.** It's also
//! why this operator runs BETWEEN P107 and P221 in the pipeline: canonical
//! numbering (107 < 127 < 129 < 131 < 221) requires no reordering
//! justification here, unlike P29 or P108 elsewhere in this crate.
//!
//! `crate::orientation::nearest_public_realm_point` is shared with
//! `p221_natural_doors_and_windows` (which used to keep a private copy of
//! this exact logic) -- both operators need the same fact ("which
//! direction is the public realm"), and now that this one runs first,
//! duplicating it risked the two silently disagreeing.
//!
//! # What this operator deliberately does NOT do
//! - **No use, ever.** A cell's only properties are its footprint and a
//!   depth coordinate (`0.0` = public wall / entrance bay, `1.0` =
//!   deepest). Nothing is ever labeled a room type. Alexander's own text
//!   here describes a pure spatial relationship (public-to-private
//!   sequence), not a program -- baking in "this cell is a bedroom" would
//!   be an assumption this pipeline has no way to justify. Testing whether
//!   a generated gradient actually supports plausible use is a job for a
//!   future activity-simulation opinion, not this operator.
//! - **No connectivity.** Alexander's own text attributes "decide how
//!   movement will connect the spaces" to pattern 131, not 127. This
//!   operator produces the gradient only; `connects_to` is left empty for
//!   `p131_the_flow_through_rooms` to fill in.
//! - **Ground floor only.** There's no modeled way to reach an upper floor
//!   (no staircase / vertical-circulation pattern implemented), so
//!   partitioning one would be fiction. `InteriorCell::floor` is always 0.
//! - **Solid and courtyard buildings use genuinely different geometry, on
//!   purpose.** A solid building's depth axis is a straight line away from
//!   the public wall, sliced into parallel bands. A courtyard building's
//!   ring already IS a loop -- it's sliced into bays around the ring by
//!   angle from the footprint's own centroid, with depth measured as
//!   angular distance from the entrance bay. Trying to force one algorithm
//!   onto both shapes would be less faithful to either, not more uniform.

use crate::orientation::nearest_public_realm_point;
use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{centroid, clip_half_plane, lnglat_to_local, local_to_ring, ring_to_local, Pt2};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::{Building, InteriorCell, Neighborhood};
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P127Params {
    /// Target depth of one band (solid buildings) or arc-length spacing of
    /// one bay (courtyard buildings). Same category of placeholder as
    /// P221's `room_width_m` -- plausible, not derived from a real room
    /// program.
    pub band_depth_m: f64,
    /// Below this total depth-span (solid) or ring perimeter (courtyard),
    /// don't bother slicing -- the whole footprint is one cell, no
    /// meaningful gradient to draw.
    pub min_span_m: f64,
}

impl Parameters for P127Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "band_depth_m",
                "Target band depth (solid) or bay spacing (courtyard).",
                3.0, 10.0, 5.0,
            ).with_unit("m"),
            ParamSpec::float(
                "min_span_m",
                "Below this span, the whole footprint stays a single cell.",
                2.0, 15.0, 4.0,
            ).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self { band_depth_m: 5.0, min_span_m: 4.0 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.band_depth_m, self.min_span_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.band_depth_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.min_span_m = s.clamp(*x); }
        p
    }
}

pub struct P127IntimacyGradient;

impl PatternOperator for P127IntimacyGradient {
    type Params = P127Params;

    fn name(&self) -> &'static str { "p127_intimacy_gradient" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p127".into(),
            display: "Alexander et al., A Pattern Language, Pattern 127 (Intimacy Gradient)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl127/apl127.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Partition each building's ground floor into a depth-ordered sequence of cells, from the public wall (or entrance bay) to the deepest point."
    }

    /// `parcel_id` must be `"*"` -- targets every building in one pass,
    /// same convention as `p221_natural_doors_and_windows`.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        _seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p127_intimacy_gradient only supports parcel_id \"*\" -- it partitions every building in one pass.".into());
        }
        if nbhd.buildings.is_empty() {
            return Err("p127_intimacy_gradient: no buildings found -- run p107_wings_of_light first.".into());
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut n_solid = 0;
        let mut n_courtyard = 0;
        let mut n_single_cell = 0;
        let mut n_skipped = 0;

        for b in &nbhd.buildings {
            if b.polygon.outer.len() < 3 {
                n_skipped += 1;
                continue;
            }
            let origin = b.polygon.centroid();
            let target = nearest_public_realm_point(nbhd, b).map(|t| lnglat_to_local(&t, &origin));

            let is_courtyard = b.typology.as_deref() == Some("p107_courtyard_v01")
                && !b.polygon.holes.is_empty();

            let cells = if is_courtyard {
                let outer_local = ring_to_local(&b.polygon.outer, &origin);
                let inner_local = ring_to_local(&b.polygon.holes[0], &origin);
                if outer_local.len() < 3 || inner_local.len() < 3 {
                    n_skipped += 1;
                    continue;
                }
                n_courtyard += 1;
                courtyard_bays(&b.id, &outer_local, &inner_local, target, params, &origin)
            } else {
                let outer_local = ring_to_local(&b.polygon.outer, &origin);
                if outer_local.len() < 3 {
                    n_skipped += 1;
                    continue;
                }
                n_solid += 1;
                solid_bands(&b.id, &outer_local, target, params, &origin)
            };

            if cells.is_empty() {
                n_skipped += 1;
                continue;
            }
            if cells.len() == 1 {
                n_single_cell += 1;
            }

            let mut updated = b.clone();
            updated.interior_cells = cells;
            new_buildings.push(updated);
            replaced.push(b.id.clone());
        }

        if new_buildings.is_empty() {
            return Err(format!(
                "p127_intimacy_gradient: 0 of {} building(s) could be partitioned (degenerate footprint).",
                nbhd.buildings.len()
            ));
        }

        steps.push(format!(
            "Partitioned {} building(s): {} solid (parallel bands), {} courtyard (ring bays), {} single-cell (too small to slice), {} skipped.",
            new_buildings.len(), n_solid, n_courtyard, n_single_cell, n_skipped
        ));

        let trace = SubdivisionTrace {
            operator_name: "p127_intimacy_gradient".into(),
            operator_source: self.source(),
            headline: format!(
                "Built a public-to-private depth gradient for {} building(s).",
                new_buildings.len()
            ),
            steps,
            caveats: vec![
                "Ground floor only -- no staircase/vertical-circulation pattern is implemented, so \
                 there's no modeled way to reach an upper floor. Every InteriorCell has floor = 0."
                    .into(),
                "band_depth_m (solid bands) / bay spacing (courtyard) are plausible placeholders, \
                 not derived from any real room program -- same category of assumption as P221's \
                 room_width_m and P107's max_wing_width_m.".into(),
                "Depth is a pure form property (position in the public/private sequence), never a \
                 room type. This operator does not and cannot know whether a cell will become a \
                 bedroom, an office, or anything else.".into(),
                "connects_to is left empty here on purpose -- Alexander's own text attributes \
                 'decide how movement will connect the spaces' to Pattern 131, not 127. See \
                 p131_the_flow_through_rooms.".into(),
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

/// The depth axis for a solid building: a unit vector, in local metres,
/// pointing FROM the footprint centroid TOWARD the public realm. Falls
/// back to the outward normal of the longest wall segment (deterministic,
/// not random) when there's no street or open space anywhere in the
/// neighborhood to orient against -- same fallback category
/// `p221_natural_doors_and_windows::choose_door_wall` already uses for its
/// own door-wall pick, though independently computed since P127 needs an
/// axis, not a specific edge.
fn depth_axis(outer_local: &[Pt2], target: Option<Pt2>) -> Pt2 {
    let c = centroid(outer_local);
    if let Some(t) = target {
        let d = t.sub(c);
        let len = d.len();
        if len > 1e-6 {
            return Pt2::new(d.x / len, d.y / len);
        }
    }
    // Fallback: outward normal of the longest edge.
    let n = outer_local.len();
    let mut best: Option<(f64, Pt2)> = None;
    for i in 0..n {
        let a = outer_local[i];
        let b = outer_local[(i + 1) % n];
        let len = a.dist(b);
        let mid = Pt2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let edge = b.sub(a);
        let mut normal = Pt2::new(edge.y, -edge.x);
        if normal.dot(mid.sub(c)) < 0.0 {
            normal = Pt2::new(-normal.x, -normal.y);
        }
        let nlen = normal.len();
        if nlen > 1e-9 && best.map(|(bl, _)| len > bl).unwrap_or(true) {
            best = Some((len, Pt2::new(normal.x / nlen, normal.y / nlen)));
        }
    }
    best.map(|(_, n)| n).unwrap_or(Pt2::new(1.0, 0.0))
}

/// Slice a solid building's footprint into parallel depth bands via two
/// `clip_half_plane` cuts per band -- reuses this crate's existing
/// half-plane clip (already handles a single-line cut on any simple
/// polygon, convex or not) rather than writing new low-level geometry.
fn solid_bands(
    building_id: &str,
    outer_local: &[Pt2],
    target: Option<Pt2>,
    params: &P127Params,
    origin: &street_smarts_core::geometry::LngLat,
) -> Vec<InteriorCell> {
    let u = depth_axis(outer_local, target);
    let perp = Pt2::new(-u.y, u.x);
    let c = centroid(outer_local);
    let s = |p: Pt2| -> f64 { p.sub(c).dot(u) };

    let (mut s_min, mut s_max) = (f64::INFINITY, f64::NEG_INFINITY);
    for &p in outer_local {
        let v = s(p);
        if v < s_min { s_min = v; }
        if v > s_max { s_max = v; }
    }
    let span = s_max - s_min;
    if span < params.min_span_m || span <= 0.0 {
        return vec![InteriorCell {
            id: format!("{building_id}_cell_0"),
            polygon: street_smarts_core::geometry::Polygon::from_ring(local_to_ring(outer_local, origin)),
            depth: 0.0,
            is_common: false,
            kind: "room".into(),
            connects_to: vec![],
            floor: 0,
        }];
    }

    let n_bands = ((span / params.band_depth_m).round().max(1.0)) as usize;
    let band_width = span / n_bands as f64;
    let mut cells = Vec::with_capacity(n_bands);
    for k in 0..n_bands {
        let s_hi = s_max - k as f64 * band_width;
        let s_lo = s_max - (k as f64 + 1.0) * band_width;
        let p0_hi = Pt2::new(c.x + s_hi * u.x, c.y + s_hi * u.y);
        let p0_lo = Pt2::new(c.x + s_lo * u.x, c.y + s_lo * u.y);
        let mut poly = clip_half_plane(outer_local, p0_hi, Pt2::new(p0_hi.x + perp.x, p0_hi.y + perp.y));
        poly = clip_half_plane(&poly, p0_lo, Pt2::new(p0_lo.x - perp.x, p0_lo.y - perp.y));
        if poly.len() < 3 {
            continue;
        }
        let depth = if n_bands > 1 { k as f64 / (n_bands - 1) as f64 } else { 0.0 };
        cells.push(InteriorCell {
            id: format!("{building_id}_cell_{k}"),
            polygon: street_smarts_core::geometry::Polygon::from_ring(local_to_ring(&poly, origin)),
            depth,
            is_common: false,
            kind: "room".into(),
            connects_to: vec![],
            floor: 0,
        });
    }
    // Re-index ids/depths in case a degenerate band was dropped, so depth
    // stays a clean 0..1 sequence with no gaps.
    let total = cells.len();
    for (i, cell) in cells.iter_mut().enumerate() {
        cell.id = format!("{building_id}_cell_{i}");
        cell.depth = if total > 1 { i as f64 / (total - 1) as f64 } else { 0.0 };
    }
    cells
}

/// Total length of `ring`'s own closed boundary.
fn ring_perimeter(ring: &[Pt2]) -> f64 {
    let n = ring.len();
    (0..n).map(|i| ring[i].dist(ring[(i + 1) % n])).sum()
}

/// Closest point to `p` on the segment `a`-`b`.
fn closest_point_on_segment(a: Pt2, b: Pt2, p: Pt2) -> Pt2 {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let len2 = abx * abx + aby * aby;
    if len2 < 1e-12 {
        return a;
    }
    let t = (((p.x - a.x) * abx + (p.y - a.y) * aby) / len2).clamp(0.0, 1.0);
    Pt2::new(a.x + t * abx, a.y + t * aby)
}

/// Closest point to `pt` lying anywhere on `ring`'s boundary (its edges,
/// not just its vertices) and that point's cumulative arc-length from
/// `ring[0]` walking the ring's own stored vertex order. Well-defined for
/// ANY simple polygon, convex or not -- unlike casting a ray from a single
/// interior point (this function's previous approach), which needs that
/// point to see the whole ring (`star-shaped`), an assumption P108's
/// merges can break: unioning several individually-convex courtyard
/// buildings can bend their combined footprint AND courtyard hole into a
/// non-convex shape no single interior point sees all of, which collapsed
/// nearly every angular ray onto the same nearby edge instead of spreading
/// them around the loop.
fn nearest_point_on_ring(ring: &[Pt2], pt: Pt2) -> (Pt2, f64) {
    let n = ring.len();
    let mut best: Option<(f64, Pt2, f64)> = None; // (dist, point, arc-length so far)
    let mut acc = 0.0;
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        let proj = closest_point_on_segment(a, b, pt);
        let d = proj.dist(pt);
        if best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
            best = Some((d, proj, acc + proj.dist(a)));
        }
        acc += a.dist(b);
    }
    best.map(|(_, p, s)| (p, s)).unwrap_or((ring[0], 0.0))
}

/// Point at cumulative arc-length `s` (wrapped to the ring's own perimeter)
/// walking `ring`'s edges in stored vertex order from `ring[0]`.
fn point_at_arclength(ring: &[Pt2], s: f64) -> Pt2 {
    let perimeter = ring_perimeter(ring);
    if perimeter < 1e-9 {
        return ring[0];
    }
    let mut s = s % perimeter;
    if s < 0.0 {
        s += perimeter;
    }
    let n = ring.len();
    let mut acc = 0.0;
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        let seg_len = a.dist(b);
        if acc + seg_len >= s || i == n - 1 {
            let t = if seg_len > 1e-9 { (s - acc) / seg_len } else { 0.0 };
            return Pt2::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y));
        }
        acc += seg_len;
    }
    ring[n - 1]
}

/// Slice a courtyard building's ring into bays around the loop -- the ring
/// already IS a loop, so unlike the solid case there's no artificial
/// passage to construct; consecutive bays wrap around for free
/// (`p131_the_flow_through_rooms` wires the actual `connects_to` edges,
/// this only builds the cells).
fn courtyard_bays(
    building_id: &str,
    outer_local: &[Pt2],
    inner_local: &[Pt2],
    target: Option<Pt2>,
    params: &P127Params,
    origin: &street_smarts_core::geometry::LngLat,
) -> Vec<InteriorCell> {
    let perimeter = ring_perimeter(outer_local);
    if perimeter < params.min_span_m {
        return vec![];
    }
    let n_bays = ((perimeter / params.band_depth_m).round().max(3.0)) as usize;

    // Walk the OUTER ring by arc length, starting near the entrance target
    // (or an arbitrary vertex if there's no public realm to orient
    // against). Each outer sample's inner-wall partner is just the closest
    // point on the courtyard boundary to it -- both operations are plain
    // nearest-point-on-polygon queries, valid for any simple ring, so this
    // needs no interior "kernel" point and can't collapse the way ray
    // casting did.
    let entrance_target = target.unwrap_or(outer_local[0]);
    let (_, start_s) = nearest_point_on_ring(outer_local, entrance_target);

    let mut boundary_pts: Vec<(Pt2, Pt2)> = Vec::with_capacity(n_bays + 1);
    for k in 0..=n_bays {
        let s = start_s + (k as f64 / n_bays as f64) * perimeter;
        let outer_pt = point_at_arclength(outer_local, s);
        let (inner_pt, _) = nearest_point_on_ring(inner_local, outer_pt);
        boundary_pts.push((outer_pt, inner_pt));
    }

    let half = n_bays / 2;
    let mut cells = Vec::with_capacity(n_bays);
    for k in 0..n_bays {
        let (o0, i0) = boundary_pts[k];
        let (o1, i1) = boundary_pts[k + 1];
        let quad = vec![o0, o1, i1, i0];
        let angular_dist = k.min(n_bays - k);
        let depth = if half > 0 { angular_dist as f64 / half as f64 } else { 0.0 };
        cells.push(InteriorCell {
            id: format!("{building_id}_bay_{k}"),
            polygon: street_smarts_core::geometry::Polygon::from_ring(local_to_ring(&quad, origin)),
            depth: depth.min(1.0),
            is_common: false,
            kind: "room".into(),
            connects_to: vec![],
            floor: 0,
        });
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon, PolygonPart};
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.001, 0.001],
            parcels: vec![],
            buildings,
            streets,
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P127 unit fixture".into(),
            },
        }
    }

    fn square_ring(side_m: f64) -> Vec<LngLat> {
        let m = 1.0 / 111_320.0;
        let s = side_m * m;
        vec![
            LngLat::new(0.0, 0.0),
            LngLat::new(s, 0.0),
            LngLat::new(s, s),
            LngLat::new(0.0, s),
            LngLat::new(0.0, 0.0),
        ]
    }

    fn solid_building(id: &str, side_m: f64) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(square_ring(side_m)),
            height_m: Some(7.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            openings: vec![],
            interior_cells: vec![],
        }
    }

    #[test]
    fn solid_building_gets_depth_ordered_bands() {
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(-0.0005, 0.0), LngLat::new(-0.0005, 0.0002)],
            classification: Some("local".into()),
            row_width_m: Some(8.0),
        };
        let n = nbhd(vec![solid_building("B1", 30.0)], vec![street]);
        let sub = P127IntimacyGradient
            .apply(&n, "*", &P127Params::defaults(), 1)
            .expect("should partition");
        let b = &sub.new_buildings[0];
        assert!(b.interior_cells.len() >= 2, "a 30m-deep building should slice into multiple bands, got {}", b.interior_cells.len());
        let depths: Vec<f64> = b.interior_cells.iter().map(|c| c.depth).collect();
        let mut sorted = depths.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(depths, sorted, "cells should already come out in increasing depth order");
        assert!((depths[0] - 0.0).abs() < 1e-9);
        assert!((depths[depths.len() - 1] - 1.0).abs() < 1e-9);
        assert!(b.interior_cells.iter().all(|c| c.connects_to.is_empty()), "P127 shouldn't set connectivity");
    }

    #[test]
    fn small_building_stays_a_single_cell() {
        let n = nbhd(vec![solid_building("B1", 2.0)], vec![]);
        let sub = P127IntimacyGradient
            .apply(&n, "*", &P127Params::defaults(), 1)
            .expect("should partition");
        assert_eq!(sub.new_buildings[0].interior_cells.len(), 1);
    }

    #[test]
    fn courtyard_building_gets_a_ring_of_bays() {
        let outer = square_ring(40.0);
        let m = 1.0 / 111_320.0;
        let inner: Vec<LngLat> = {
            let s = 15.0 * m;
            let off = 12.5 * m;
            vec![
                LngLat::new(off, off), LngLat::new(off + s, off),
                LngLat::new(off + s, off + s), LngLat::new(off, off + s),
                LngLat::new(off, off),
            ]
        };
        let b = Building {
            id: "CY1".into(),
            polygon: Polygon::from_parts(vec![PolygonPart { outer, holes: vec![inner] }]),
            height_m: Some(7.0),
            typology: Some("p107_courtyard_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            openings: vec![],
            interior_cells: vec![],
        };
        let n = nbhd(vec![b], vec![]);
        let sub = P127IntimacyGradient
            .apply(&n, "*", &P127Params::defaults(), 1)
            .expect("should partition");
        let cells = &sub.new_buildings[0].interior_cells;
        assert!(cells.len() >= 3, "a ring should produce at least a few bays, got {}", cells.len());
        assert!(cells.iter().any(|c| c.depth < 1e-9), "one bay should be the entrance bay at depth 0");
        assert!(cells.iter().any(|c| c.depth > 0.9), "one bay should be near the opposite point, depth close to 1");
    }

    #[test]
    fn no_buildings_is_an_error_not_a_silent_no_op() {
        let n = nbhd(vec![], vec![]);
        assert!(P127IntimacyGradient.apply(&n, "*", &P127Params::defaults(), 1).is_err());
    }
}
