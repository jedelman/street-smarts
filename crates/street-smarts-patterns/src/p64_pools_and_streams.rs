//! P64 Pools and Streams -- the site's rain kept above ground, in ponds with
//! a marsh round them and bald cypress in the shallows, and streams between
//! ponds that are close enough.
//!
//! From Alexander, *A Pattern Language*, Pattern 64, via
//! patternlanguage.cc/Patterns/Pools-and-Streams-(64):
//! > **Problem:** We came from the water; our bodies are largely water; and
//! > water plays a fundamental role in our psychology. ...
//! > **Solution:** Preserve natural pools and streams and allow them to run
//! > through the city; ...
//! The solution goes on to ask for paths and footbridges along the streams,
//! and, where it can be done, rainwater collected in open channels above
//! ground. Pattern 25, Access to Water, asks for a belt of common land
//! along the water's edge: here, the marsh.
//!
//! # What this generator does
//!
//! The site has no natural pond to preserve (it was a mall and its parking
//! lots), so this makes the kind a city makes now for its rain, a
//! stormwater wetland, on the open ground P172 let grow as meadow:
//! 1. Samples the site on a grid and measures, at each point, how far it is
//!    to the nearest building, street, square or common, activity marker,
//!    transit line or the site's edge: how big a pond could sit there.
//! 2. Picks up to `max_ponds` of the roomiest points, far enough apart that
//!    their marshes don't touch.
//! 3. At each, a pond (`GroundCover` kind "pond") with an irregular,
//!    seeded outline, and a marsh ring round it (kind "marsh", the pond as
//!    its hole), `marsh_width` of the pond's radius wide.
//! 4. Bald cypress (`Tree` form "cypress") along the pond's edge, in the
//!    shallows and on the bank, and a stand in the open water.
//! 5. A boardwalk (kind "boardwalk") out to each pond a street is near
//!    enough to: from the nearest street, straight across the marsh to the
//!    shallows (when that line stays clear of buildings and squares), then an
//!    overlook curving along inside the pond's edge to a small platform.
//!    The pattern asks for paths and footbridges along the water; this is
//!    the shape the Bald Cypress Trail at First Landing State Park takes
//!    ("boardwalk scenic swamp overviews," in the park's trail guide).
//!    Cypress keep clear of the decks.
//! 6. Streams (kind "stream") along the fewest links that join every pond
//!    (a minimum spanning tree, so the water runs as one chain), each link
//!    within `stream_reach_m` and clear of buildings and squares. A stream
//!    may cross a street: a culvert or a
//!    footbridge, which isn't modeled.
//!
//! Runs after P172 (the meadow is the site's extent) and before P171, which
//! keeps its trees out of the water and the marsh and clear of the cypress.
//!
//! # Precondition errors
//!
//! `Err` if there's no site ring, or no point on the site has room for even
//! the smallest pond (then the plan has no water, and says so).
//!
//! # Caveats
//!
//! - No grading, depth, inflow or outflow, soils or water table: the ponds
//!   are where there's room, not where water would run to. The site's real
//!   levels aren't in the data.
//! - The pattern gives no sizes; every figure here (radii, spacing, the
//!   cypress's height) is ours, plausible, and labeled as such.
//! - Bald cypress is the tree of southeast Virginia's swamps (the Great
//!   Dismal Swamp); which other plants grow in the marsh is the walk's,
//!   not this operator's.

use crate::p172_garden_growing_wild::site_ring;
use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{self, Pt2};
use crate::prng::Prng;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{LngLat, Polygon, PolygonPart};
use street_smarts_core::nir::{GroundCover, Neighborhood, Tree};
use street_smarts_core::opinion::SourceCitation;

/// `GroundCover.kind`s this operator adds, and the tree form it plants.
pub const POND: &str = "pond";
pub const MARSH: &str = "marsh";
pub const STREAM: &str = "stream";
pub const CYPRESS: &str = "cypress";
pub const BOARDWALK: &str = "boardwalk";

/// Grid step for the room-for-a-pond survey.
const GRID_M: f64 = 4.0;
/// Clearance kept from a building's walls, a square's edge, the site's edge.
const EDGE_CLEARANCE_M: f64 = 3.0;
/// Clearance beyond a street's half width.
const STREET_CLEARANCE_M: f64 = 3.0;
/// Gap between two ponds' marshes.
const POND_GAP_M: f64 = 8.0;
/// The smallest pond worth making, by radius.
const MIN_RADIUS_M: f64 = 6.0;
/// Width of a stream channel.
const STREAM_WIDTH_M: f64 = 3.0;
/// Width of a boardwalk, and how far from a street its approach may start.
const BOARDWALK_WIDTH_M: f64 = 2.4;
const BOARDWALK_REACH_M: f64 = 90.0;
/// The overlook runs this far into the pond (as a fraction of its radius) and this far round it (radians).
const OVERLOOK_AT: f64 = 0.86;
const OVERLOOK_SWEEP: f64 = 1.1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P64Params {
    pub max_ponds: f64,
    pub max_radius_m: f64,
    /// The marsh's outer edge, as a multiple of the pond's radius.
    pub marsh_width: f64,
    pub cypress_spacing_m: f64,
    pub stream_reach_m: f64,
}

impl Parameters for P64Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float("max_ponds", "Most ponds to make (ours).", 0.0, 12.0, 8.0),
            ParamSpec::float("max_radius_m", "Largest pond radius (ours).", 10.0, 80.0, 55.0),
            ParamSpec::float("marsh_width", "The marsh's outer edge as a multiple of the pond's radius (ours).", 1.1, 2.2, 1.6),
            ParamSpec::float("cypress_spacing_m", "Spacing of bald cypress along a pond's edge (ours).", 3.0, 15.0, 6.5),
            ParamSpec::float("stream_reach_m", "Ponds closer than this, edge to edge, are joined by a stream (ours).", 0.0, 300.0, 220.0),
        ]
    }

    fn defaults() -> Self {
        Self { max_ponds: 8.0, max_radius_m: 55.0, marsh_width: 1.6, cypress_spacing_m: 6.5, stream_reach_m: 220.0 }
    }

    fn as_vector(&self) -> Vec<f64> {
        vec![self.max_ponds, self.max_radius_m, self.marsh_width, self.cypress_spacing_m, self.stream_reach_m]
    }

    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        let mut slots = [&mut p.max_ponds, &mut p.max_radius_m, &mut p.marsh_width, &mut p.cypress_spacing_m, &mut p.stream_reach_m];
        for (i, slot) in slots.iter_mut().enumerate() {
            if let (Some(s), Some(x)) = (schema.get(i), v.get(i)) {
                **slot = s.clamp(*x);
            }
        }
        p
    }
}

/// Distance from `p` to the nearest edge of `ring`.
fn ring_distance(p: Pt2, ring: &[Pt2]) -> f64 {
    let n = ring.len();
    (0..n).map(|i| planar::point_segment_distance(p, ring[i], ring[(i + 1) % n])).fold(f64::INFINITY, f64::min)
}

/// What a pond has to keep clear of, in local metres.
struct Obstacles {
    site: Vec<Pt2>,
    /// Buildings, platforms, squares and commons: (ring, bbox lo, bbox hi).
    areas: Vec<(Vec<Pt2>, Pt2, Pt2)>,
    /// Streets and transit lines: (polyline, half width).
    lines: Vec<(Vec<Pt2>, f64)>,
    markers: Vec<Pt2>,
}

impl Obstacles {
    /// How far a pond's edge could reach from `p` (0 if `p` itself is taken).
    fn room(&self, p: Pt2) -> f64 {
        if !planar::point_in_polygon(p, &self.site) {
            return 0.0;
        }
        let mut r = ring_distance(p, &self.site) - EDGE_CLEARANCE_M;
        for (ring, lo, hi) in &self.areas {
            // Cheap reject: a box farther than the room we already have can't lower it.
            let dx = (lo.x - p.x).max(p.x - hi.x).max(0.0);
            let dy = (lo.y - p.y).max(p.y - hi.y).max(0.0);
            if dx.hypot(dy) - EDGE_CLEARANCE_M > r {
                continue;
            }
            if planar::point_in_polygon(p, ring) {
                return 0.0;
            }
            r = r.min(ring_distance(p, ring) - EDGE_CLEARANCE_M);
        }
        for (line, half) in &self.lines {
            let d = line.windows(2).map(|w| planar::point_segment_distance(p, w[0], w[1])).fold(f64::INFINITY, f64::min);
            r = r.min(d - half - STREET_CLEARANCE_M);
        }
        for m in &self.markers {
            r = r.min(m.dist(p) - EDGE_CLEARANCE_M);
        }
        r.max(0.0)
    }
    /// Does the segment a-b stay out of every building, square and common?
    fn clear_line(&self, a: Pt2, b: Pt2) -> bool {
        let steps = (a.dist(b) / 2.0).ceil().max(1.0) as usize;
        (0..=steps).all(|k| {
            let t = k as f64 / steps as f64;
            let p = Pt2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            planar::point_in_polygon(p, &self.site) && !self.areas.iter().any(|(ring, _, _)| planar::point_in_polygon(p, ring))
        })
    }
}

/// An irregular closed outline round `c`: radius `r` times a seeded sum of
/// low harmonics, stretched along a seeded axis. Counter-clockwise.
fn outline(c: Pt2, r: f64, rng: &mut Prng) -> Vec<Pt2> {
    let n = 56;
    let (a2, a3, a5) = (rng.range(0.08, 0.16), rng.range(0.04, 0.1), rng.range(0.0, 0.04));
    let (p2, p3, p5) = (rng.range(0.0, 6.28), rng.range(0.0, 6.28), rng.range(0.0, 6.28));
    let stretch = rng.range(1.0, 1.35);
    let axis = rng.range(0.0, std::f64::consts::PI);
    let (ca, sa) = (axis.cos(), axis.sin());
    (0..n)
        .map(|i| {
            let t = i as f64 / n as f64 * std::f64::consts::TAU;
            let k = 1.0 + a2 * (2.0 * t + p2).sin() + a3 * (3.0 * t + p3).sin() + a5 * (5.0 * t + p5).sin();
            let (x, y) = (t.cos() * r * k * stretch.sqrt(), t.sin() * r * k / stretch.sqrt());
            Pt2::new(c.x + x * ca - y * sa, c.y + x * sa + y * ca)
        })
        .collect()
}

/// The same outline scaled about `c` by `f` (the marsh's outer edge).
fn scaled(ring: &[Pt2], c: Pt2, f: f64) -> Vec<Pt2> {
    ring.iter().map(|p| Pt2::new(c.x + (p.x - c.x) * f, c.y + (p.y - c.y) * f)).collect()
}

/// A gently meandering channel from a to b, as a closed outline `w` wide.
fn channel(a: Pt2, b: Pt2, w: f64, rng: &mut Prng) -> Vec<Pt2> {
    let len = a.dist(b);
    let (ux, uy) = ((b.x - a.x) / len, (b.y - a.y) / len);
    let (nx, ny) = (-uy, ux);
    let n = ((len / 6.0).ceil() as usize).max(4);
    let amp = (len * 0.06).min(9.0);
    let ph = rng.range(0.0, 6.28);
    let mid: Vec<Pt2> = (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let s = amp * (t * std::f64::consts::PI).sin() * (t * 2.0 * std::f64::consts::PI + ph).sin();
            Pt2::new(a.x + ux * len * t + nx * s, a.y + uy * len * t + ny * s)
        })
        .collect();
    let left = mid.iter().map(|p| Pt2::new(p.x + nx * w / 2.0, p.y + ny * w / 2.0));
    let right: Vec<Pt2> = mid.iter().map(|p| Pt2::new(p.x - nx * w / 2.0, p.y - ny * w / 2.0)).collect();
    left.chain(right.into_iter().rev()).collect()
}

pub struct P64PoolsAndStreams;

impl PatternOperator for P64PoolsAndStreams {
    type Params = P64Params;

    fn name(&self) -> &'static str {
        "p64_pools_and_streams"
    }

    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p64".into(),
            display: "Alexander et al., A Pattern Language, Pattern 64 (Pools and Streams)".into(),
            url: Some("https://www.patternlanguage.cc/Patterns/Pools-and-Streams-(64)".into()),
        }
    }

    fn description(&self) -> &'static str {
        "Keep the rain above ground: ponds where the open ground has room, a marsh round each, bald cypress in the shallows, streams between close ponds."
    }

    fn apply(&self, nbhd: &Neighborhood, _parcel_id: &str, params: &Self::Params, seed: u64) -> Result<Subdivision, String> {
        let ring = site_ring(nbhd).ok_or_else(|| "p64_pools_and_streams: no site ring (needs P172's meadow or a closed jurisdictional boundary).".to_string())?;
        let origin = planar::average_centroid(&ring);
        let local = |r: &Vec<LngLat>| planar::ring_to_local(r, &origin);
        let with_box = |r: Vec<Pt2>| {
            let (lo, hi) = planar::bbox(&r);
            (r, lo, hi)
        };
        let obstacles = Obstacles {
            site: planar::ring_to_local(&ring, &origin),
            areas: nbhd
                .buildings
                .iter()
                .flat_map(|b| b.polygon.parts_view())
                .map(|part| with_box(local(&part.outer)))
                .chain(nbhd.open_space.iter().filter(|o| o.polygon.area_m2() > 0.0).map(|o| with_box(local(&o.polygon.outer))))
                .chain(nbhd.transit.iter().flat_map(|l| &l.stations).map(|s| with_box(local(&s.platform.outer))))
                .collect(),
            lines: nbhd
                .streets
                .iter()
                .map(|s| (s.centerline.iter().map(|p| planar::lnglat_to_local(p, &origin)).collect(), s.row_width_m.unwrap_or(6.0) / 2.0))
                .chain(nbhd.transit.iter().map(|l| (l.centerline.iter().map(|p| planar::lnglat_to_local(p, &origin)).collect(), 3.0)))
                .collect(),
            markers: nbhd.activity_nodes.iter().map(|a| planar::lnglat_to_local(&a.location, &origin)).collect(),
        };

        // 1. Room for a pond at every grid point.
        let (lo, hi) = planar::bbox(&obstacles.site);
        let mut spots: Vec<(Pt2, f64)> = Vec::new();
        let mut y = lo.y;
        while y <= hi.y {
            let mut x = lo.x;
            while x <= hi.x {
                let p = Pt2::new(x, y);
                let room = obstacles.room(p);
                if room > 0.0 {
                    spots.push((p, room));
                }
                x += GRID_M;
            }
            y += GRID_M;
        }
        spots.sort_by(|a, b| b.1.total_cmp(&a.1));

        // 2. The roomiest points, far enough apart. Each pond's outline is drawn at unit size
        // first, so its marsh's farthest reach (irregularity and stretch included) fits the room.
        let mut rng = Prng::new(seed ^ 0x64_64);
        let origin0 = Pt2::new(0.0, 0.0);
        let mut ponds: Vec<(Pt2, f64)> = Vec::new(); // (centre, pond radius)
        let mut units: Vec<(Vec<Pt2>, f64)> = Vec::new(); // (unit outline, its farthest reach)
        for &(p, room) in &spots {
            if ponds.len() as f64 >= params.max_ponds.round() {
                break;
            }
            let unit = outline(origin0, 1.0, &mut rng);
            let reach = unit.iter().map(|q| q.dist(origin0)).fold(0.0, f64::max);
            let r = (room / (params.marsh_width * reach)).min(params.max_radius_m);
            if r < MIN_RADIUS_M {
                continue;
            }
            if ponds.iter().zip(&units).all(|(&(q, rq), (_, reach_q))| p.dist(q) > (r * reach + rq * reach_q) * params.marsh_width + POND_GAP_M) {
                ponds.push((p, r));
                units.push((unit, reach));
            }
        }
        if ponds.is_empty() {
            return Err("p64_pools_and_streams: no open ground on the site has room for a pond.".into());
        }

        // 5 (first, so the cypress can keep clear of it). A boardwalk out to each pond a
        // street is near enough to: an approach across the marsh, then an overlook in the shallows.
        let n_streets = nbhd.streets.len();
        let mut walks: Vec<Option<(Vec<Pt2>, Vec<Pt2>)>> = Vec::new(); // (approach, overlook centreline)
        for &(c, r) in &ponds {
            // The nearest point on any street's edge.
            let mut best: Option<(Pt2, f64)> = None;
            for (line, half) in obstacles.lines.iter().take(n_streets) {
                for w in line.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    let len2 = dx * dx + dy * dy;
                    let t = if len2 > 0.0 { (((c.x - a.x) * dx + (c.y - a.y) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
                    let q = Pt2::new(a.x + dx * t, a.y + dy * t);
                    let d = q.dist(c).max(1e-6);
                    // Step off the carriageway, toward the pond.
                    let edge = Pt2::new(q.x + (c.x - q.x) / d * (half + 0.5), q.y + (c.y - q.y) / d * (half + 0.5));
                    if best.map_or(true, |(_, bd)| d < bd) {
                        best = Some((edge, d));
                    }
                }
            }
            let walk = best.and_then(|(start, _)| {
                let d = start.dist(c).max(1e-6);
                let (ux, uy) = ((start.x - c.x) / d, (start.y - c.y) / d);
                let end = Pt2::new(c.x + ux * r * OVERLOOK_AT, c.y + uy * r * OVERLOOK_AT);
                if start.dist(end) > BOARDWALK_REACH_M || d <= r * OVERLOOK_AT || !obstacles.clear_line(start, end) {
                    return None;
                }
                let a0 = uy.atan2(ux);
                let dir = if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
                let arc: Vec<Pt2> = (0..=16)
                    .map(|i| {
                        let a = a0 + dir * OVERLOOK_SWEEP * i as f64 / 16.0;
                        Pt2::new(c.x + a.cos() * r * OVERLOOK_AT, c.y + a.sin() * r * OVERLOOK_AT)
                    })
                    .collect();
                Some((vec![start, end], arc))
            });
            walks.push(walk);
        }
        let near_walk = |p: Pt2| {
            walks.iter().flatten().any(|(approach, arc)| {
                approach.windows(2).chain(arc.windows(2)).any(|w| planar::point_segment_distance(p, w[0], w[1]) < BOARDWALK_WIDTH_M / 2.0 + 1.6)
            })
        };

        // 3-4. Each pond, its marsh, and its cypress.
        let mut cover: Vec<GroundCover> = Vec::new();
        let mut trees: Vec<Tree> = Vec::new();
        let mut steps = Vec::new();
        let mut planted: Vec<Pt2> = Vec::new();
        let plant = |p: Pt2, rng: &mut Prng, trees: &mut Vec<Tree>, planted: &mut Vec<Pt2>| {
            if planted.iter().any(|q| q.dist(p) < 3.2) || near_walk(p) {
                return false;
            }
            planted.push(p);
            trees.push(Tree {
                id: format!("p64_cypress_{:04}", trees.len()),
                location: planar::local_to_lnglat(p, &origin),
                height_m: rng.range(13.0, 19.0),
                crown_radius_m: rng.range(2.6, 3.8),
                form: CYPRESS.into(),
            });
            true
        };
        let mut pond_area = 0.0;
        let mut marsh_area = 0.0;
        for (k, &(c, r)) in ponds.iter().enumerate() {
            let water: Vec<Pt2> = units[k].0.iter().map(|q| Pt2::new(c.x + q.x * r, c.y + q.y * r)).collect();
            let marsh = scaled(&water, c, params.marsh_width);
            pond_area += planar::area(&water).abs();
            marsh_area += planar::area(&marsh).abs() - planar::area(&water).abs();
            let to_ring = |pts: &[Pt2]| planar::local_to_ring(pts, &origin);
            cover.push(GroundCover { id: format!("p64_pond_{k}"), polygon: Polygon::from_ring(to_ring(&water)), kind: POND.into() });
            let (outer, hole) = (to_ring(&marsh), to_ring(&water));
            cover.push(GroundCover {
                id: format!("p64_marsh_{k}"),
                polygon: Polygon { outer: outer.clone(), holes: vec![hole.clone()], parts: vec![PolygonPart { outer, holes: vec![hole] }] },
                kind: MARSH.into(),
            });
            // Along the edge: the shallows (0.8-0.95 of the way out) and the bank (1.05-1.2).
            let before = trees.len();
            let perimeter: f64 = (0..water.len()).map(|i| water[i].dist(water[(i + 1) % water.len()])).sum();
            let n = (perimeter / params.cypress_spacing_m).floor() as usize;
            for i in 0..n {
                if rng.next_f64() < 0.25 {
                    continue; // gaps, so the edge reads as stands, not a fence
                }
                let w = water[(i * water.len()) / n.max(1)];
                let f = if rng.next_f64() < 0.6 { rng.range(0.8, 0.95) } else { rng.range(1.05, 1.2) };
                let p = Pt2::new(c.x + (w.x - c.x) * f + rng.range(-1.0, 1.0), c.y + (w.y - c.y) * f + rng.range(-1.0, 1.0));
                plant(p, &mut rng, &mut trees, &mut planted);
            }
            // A stand out in the open water, toward one side.
            let side = rng.range(0.0, std::f64::consts::TAU);
            let at = Pt2::new(c.x + side.cos() * r * 0.45, c.y + side.sin() * r * 0.45);
            for _ in 0..((r / 6.0).round() as usize).clamp(3, 9) {
                let p = Pt2::new(at.x + rng.range(-r * 0.22, r * 0.22), at.y + rng.range(-r * 0.22, r * 0.22));
                plant(p, &mut rng, &mut trees, &mut planted);
            }
            steps.push(format!(
                "p64_pond_{k}: a pond about {:.0} m across where the open ground has {:.0} m of room, a marsh to {:.2}x its edge, {} bald cypress.",
                2.0 * r,
                spots.iter().find(|s| s.0 == c).map(|s| s.1).unwrap_or(r),
                params.marsh_width,
                trees.len() - before
            ));
        }

        // 5, drawn: each boardwalk's approach, its overlook (a strip between two arcs round the
        // pond's centre) and a platform at the overlook's end.
        let half = BOARDWALK_WIDTH_M / 2.0;
        let mut n_walks = 0;
        for (k, walk) in walks.iter().enumerate() {
            let Some((approach, arc)) = walk else { continue };
            let (c, r) = ponds[k];
            let to_ring = |pts: &[Pt2]| planar::local_to_ring(pts, &origin);
            let deck = |id: String, pts: Vec<Pt2>| GroundCover { id, polygon: Polygon::from_ring(to_ring(&pts)), kind: BOARDWALK.into() };
            // The approach runs a little past the overlook's start so the two meet.
            let (s0, e0) = (approach[0], approach[1]);
            let l = s0.dist(e0).max(1e-6);
            let e1 = Pt2::new(e0.x + (e0.x - s0.x) / l * half, e0.y + (e0.y - s0.y) / l * half);
            cover.push(deck(format!("p64_boardwalk_{k}_approach"), planar::rect_corridor(s0, e1, half)));
            let (ro, ri) = (r * OVERLOOK_AT + half, r * OVERLOOK_AT - half);
            let ang = |p: &Pt2| (p.y - c.y).atan2(p.x - c.x);
            let outer = arc.iter().map(|p| { let a = ang(p); Pt2::new(c.x + a.cos() * ro, c.y + a.sin() * ro) });
            let inner: Vec<Pt2> = arc.iter().map(|p| { let a = ang(p); Pt2::new(c.x + a.cos() * ri, c.y + a.sin() * ri) }).collect();
            cover.push(deck(format!("p64_boardwalk_{k}_overlook"), outer.chain(inner.into_iter().rev()).collect()));
            let tip = *arc.last().unwrap();
            cover.push(deck(
                format!("p64_boardwalk_{k}_platform"),
                (0..8).map(|i| { let a = i as f64 / 8.0 * std::f64::consts::TAU; Pt2::new(tip.x + a.cos() * 3.0, tip.y + a.sin() * 3.0) }).collect(),
            ));
            n_walks += 1;
            steps.push(format!(
                "p64_boardwalk_{k}: a boardwalk {:.0} m from the nearest street across the marsh, then an overlook {:.0} m along the shallows to a platform.",
                l,
                r * OVERLOOK_AT * OVERLOOK_SWEEP
            ));
        }

        // 6. Streams along the fewest links that join every pond (a minimum spanning tree, so
        // the water runs as one chain, not a mesh), where the link is short and clear.
        let mut n_streams = 0;
        let centres: Vec<Pt2> = ponds.iter().map(|p| p.0).collect();
        let links: Vec<(usize, usize)> = planar::kruskal_mst(&centres).mst_edges.iter().map(|&(i, j, _)| (i.min(j), i.max(j))).collect();
        for &(i, j) in &links {
            {
                let ((a, ra), (b, rb)) = (ponds[i], ponds[j]);
                let gap = a.dist(b) - (ra + rb) * params.marsh_width;
                if gap > params.stream_reach_m {
                    continue;
                }
                let d = a.dist(b);
                let (ux, uy) = ((b.x - a.x) / d, (b.y - a.y) / d);
                let (s, e) = (Pt2::new(a.x + ux * ra * 0.9, a.y + uy * ra * 0.9), Pt2::new(b.x - ux * rb * 0.9, b.y - uy * rb * 0.9));
                if !obstacles.clear_line(s, e) {
                    continue;
                }
                cover.push(GroundCover {
                    id: format!("p64_stream_{i}_{j}"),
                    polygon: Polygon::from_ring(planar::local_to_ring(&channel(s, e, STREAM_WIDTH_M, &mut rng), &origin)),
                    kind: STREAM.into(),
                });
                n_streams += 1;
                steps.push(format!("p64_stream_{i}_{j}: a stream {:.0} m long joining two ponds.", s.dist(e)));
            }
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!(
                "{} pond{} ({:.0} m² of water) in {:.0} m² of marsh, {} bald cypress, {n_streams} stream{}, {n_walks} boardwalk{}.",
                ponds.len(),
                if ponds.len() == 1 { "" } else { "s" },
                pond_area,
                marsh_area,
                trees.len(),
                if n_streams == 1 { "" } else { "s" },
                if n_walks == 1 { "" } else { "s" }
            ),
            steps,
            caveats: vec![
                "No grading, depth, inflow or outflow: the ponds go where the open ground has room, not where water would run; the site's levels aren't in the data.".into(),
                "The pattern gives no sizes: every radius, spacing and height here is ours.".into(),
                "There was no natural pond to preserve: this is a stormwater wetland made for the rain.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings: vec![],
            new_streets: vec![],
            new_activity_nodes: vec![],
            new_boundaries: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
            new_fields: vec![],
            new_trees: trees,
            new_ground_cover: cover,
            new_transit: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p172_garden_growing_wild::{P172GardenGrowingWild, P172Params};
    use crate::subdivision::apply_subdivision;
    use street_smarts_core::nir::{Boundary, BoundaryKind, Building, NeighborhoodMeta};

    const M: f64 = 1.0 / 111_320.0;

    fn ring(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<LngLat> {
        vec![LngLat::new(x0 * M, y0 * M), LngLat::new(x1 * M, y0 * M), LngLat::new(x1 * M, y1 * M), LngLat::new(x0 * M, y1 * M), LngLat::new(x0 * M, y0 * M)]
    }

    /// A 300 m site with one building in a corner; P172's meadow on it.
    fn site() -> Neighborhood {
        let mut building: Building = serde_json::from_value(serde_json::json!({
            "id": "B1", "polygon": {"outer": [], "holes": []}, "height_m": 10.0
        }))
        .unwrap();
        building.polygon = Polygon::from_ring(ring(20.0, 20.0, 80.0, 80.0));
        let n = Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 300.0 * M, 300.0 * M],
            parcels: vec![],
            buildings: vec![building],
            streets: vec![],
            open_space: vec![],
            boundaries: vec![Boundary { id: "site_perimeter".into(), centerline: ring(0.0, 0.0, 300.0, 300.0), kind: BoundaryKind::Jurisdictional }],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta { source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(), layer_provenance: Default::default(), label: "P64 unit fixture".into() },
            pattern_fields: vec![],
            trees: vec![],
            ground_cover: vec![],
            transit: vec![],
        };
        let meadow = P172GardenGrowingWild.apply(&n, "*", &P172Params::defaults(), 1).unwrap();
        apply_subdivision(&n, &meadow)
    }

    #[test]
    fn makes_ponds_marsh_and_cypress_clear_of_the_building() {
        let n = site();
        let sub = P64PoolsAndStreams.apply(&n, "*", &P64Params::defaults(), 7).unwrap();
        let ponds: Vec<_> = sub.new_ground_cover.iter().filter(|g| g.kind == POND).collect();
        let marshes: Vec<_> = sub.new_ground_cover.iter().filter(|g| g.kind == MARSH).collect();
        assert!(!ponds.is_empty() && ponds.len() == marshes.len(), "{} ponds, {} marshes", ponds.len(), marshes.len());
        assert!(sub.new_trees.iter().all(|t| t.form == CYPRESS) && sub.new_trees.len() >= 6);
        let b = |x: f64, y: f64| (20.0..=80.0).contains(&x) && (20.0..=80.0).contains(&y);
        for g in &sub.new_ground_cover {
            for p in &g.polygon.outer {
                assert!(!b(p.lng / M, p.lat / M), "{} reaches into the building", g.id);
                assert!((0.0..=300.0).contains(&(p.lng / M)) && (0.0..=300.0).contains(&(p.lat / M)), "{} leaves the site", g.id);
            }
        }
        // Every marsh has its pond as a hole.
        for m in &marshes {
            assert_eq!(m.polygon.holes.len(), 1);
        }
    }

    #[test]
    fn lays_a_boardwalk_from_a_street() {
        let mut n = site();
        n.streets = vec![street_smarts_core::nir::Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(0.0, 150.0 * M), LngLat::new(300.0 * M, 150.0 * M)],
            classification: Some("local".into()),
            row_width_m: Some(6.0),
            surface: None,
        }];
        let sub = P64PoolsAndStreams.apply(&n, "*", &P64Params::defaults(), 7).unwrap();
        let decks: Vec<_> = sub.new_ground_cover.iter().filter(|g| g.kind == BOARDWALK).collect();
        assert!(!decks.is_empty(), "no boardwalk in {:?}", sub.trace.steps);
        let b = |x: f64, y: f64| (20.0..=80.0).contains(&x) && (20.0..=80.0).contains(&y);
        for g in &decks {
            assert!(g.polygon.outer.iter().all(|p| !b(p.lng / M, p.lat / M)), "{} reaches into the building", g.id);
        }
    }

    #[test]
    fn needs_room() {
        let mut n = site();
        // Fill the site with one building: no room anywhere.
        n.buildings[0].polygon = Polygon::from_ring(ring(2.0, 2.0, 298.0, 298.0));
        assert!(P64PoolsAndStreams.apply(&n, "*", &P64Params::defaults(), 7).is_err());
    }

    #[test]
    fn is_deterministic() {
        let n = site();
        let a = P64PoolsAndStreams.apply(&n, "*", &P64Params::defaults(), 7).unwrap();
        let b = P64PoolsAndStreams.apply(&n, "*", &P64Params::defaults(), 7).unwrap();
        assert_eq!(serde_json::to_string(&a.new_ground_cover).unwrap(), serde_json::to_string(&b.new_ground_cover).unwrap());
    }
}
