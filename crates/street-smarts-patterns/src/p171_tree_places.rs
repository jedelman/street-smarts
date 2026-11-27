//! P171 Tree Places -- trees planted to make places: avenues, squares,
//! groves, and single spreading trees.
//!
//! From Alexander, *A Pattern Language*, Pattern 171 (p. 797), via
//! patternlanguage.cc/Patterns/Tree-Places-(171):
//! > **Problem:** When trees are planted or pruned without regard for the
//! > special places they can create, they are as good as dead for the
//! > people who need them.
//! > **Solution:** ...plant them according to their nature, to form
//! > enclosures, avenues, squares, groves, and single spreading trees
//! > toward the middle of open spaces.
//!
//! # What this generator does
//!
//! Plants four of the pattern's forms, in this order (an earlier form keeps
//! its place when a later one would crowd it):
//! 1. **Avenues**: a row each side of every street, `avenue_spacing_m`
//!    apart, set back from the right-of-way edge. One street's trees share
//!    a size, as an avenue's do.
//! 2. **Single spreading trees** ("umbrella"): one broad tree toward the
//!    middle of each `Plaza` big enough to hold it, kept clear of the
//!    square's P126 marker.
//! 3. **Squares**: a ring of trees just inside the edge of every plaza big
//!    enough for one, `square_spacing_m` apart.
//! 4. **Groves**: loose stands on the rest of the site's ground (P172's
//!    meadow, P37's commons), on a jittered grid `grove_spacing_m` apart,
//!    with clearings where a seeded low-frequency field says so
//!    (`grove_cover` is that field's threshold).
//!
//! Every tree stays inside the site's ring (`p172_garden_growing_wild::
//! site_ring`), out of buildings and clear of their walls, off every
//! street's right-of-way, off the activity markers, and far enough from
//! the trees already planted that crowns overlap by no more than a fifth.
//! Groves don't go into plazas: the squares are for people.
//!
//! # Precondition errors
//!
//! `Err` if there's no site ring to plant inside.
//!
//! # Caveats
//!
//! - The pattern gives no spacing, height or crown size (checked against
//!   its page). The defaults here are plausible street-tree and grove
//!   figures, labeled as such, not Alexander's.
//! - The pattern's second half (shape the buildings in response to the
//!   trees) runs the other way here: the buildings exist before the trees
//!   are planted, so trees fit the buildings.
//! - No species, no roots or utilities, no "enclosures" (trees closing a
//!   space on their own): the four forms above only.

use crate::p172_garden_growing_wild::site_ring;
use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{self, Pt2};
use crate::prng::Prng;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind, Tree};
use street_smarts_core::opinion::SourceCitation;

/// Trunks stay this far from a wall, so a crown leans over a building
/// rather than growing out of it.
const WALL_CLEARANCE_M: f64 = 2.0;
/// Extra clearance beyond a street's right-of-way half width.
const STREET_CLEARANCE_M: f64 = 1.0;
/// Clearance from an activity marker (a fountain, a gateway, a P126 middle).
const MARKER_CLEARANCE_M: f64 = 3.0;
/// Avenue trees stand this far beyond the right-of-way edge.
const AVENUE_SETBACK_M: f64 = 1.5;
/// Square rings stand this far inside the square's edge.
const SQUARE_INSET_M: f64 = 2.5;
/// Two crowns may overlap by at most this fraction of their summed radii.
const CROWN_OVERLAP: f64 = 0.2;
/// A plaza needs a short side this long for a ring of trees...
const MIN_SQUARE_SIDE_M: f64 = 12.0;
/// ...and this long for one spreading tree.
const MIN_UMBRELLA_SIDE_M: f64 = 9.0;
/// The grove field's lattice: clearings and stands are about this wide.
const GROVE_FIELD_CELL_M: f64 = 45.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P171Params {
    pub avenue_spacing_m: f64,
    pub square_spacing_m: f64,
    pub grove_spacing_m: f64,
    /// Threshold of the grove field in [0, 1]: 0 plants no groves, 1 plants
    /// groves on all the open ground.
    pub grove_cover: f64,
    /// The smallest trunk-to-trunk distance between any two trees.
    pub min_gap_m: f64,
}

impl Parameters for P171Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float("avenue_spacing_m", "Distance between trees in an avenue row (a plausible street-tree figure, not Alexander's).", 4.0, 20.0, 8.0),
            ParamSpec::float("square_spacing_m", "Distance between trees in a square's ring.", 4.0, 20.0, 7.0),
            ParamSpec::float("grove_spacing_m", "Grid spacing of grove trees before jitter.", 5.0, 30.0, 8.5),
            ParamSpec::float("grove_cover", "Threshold of the seeded grove field: 0 = no groves, 1 = groves on all open ground.", 0.0, 1.0, 0.8),
            ParamSpec::float("min_gap_m", "Smallest trunk-to-trunk distance between any two trees.", 2.0, 12.0, 4.5),
        ]
    }

    fn defaults() -> Self {
        Self { avenue_spacing_m: 8.0, square_spacing_m: 7.0, grove_spacing_m: 8.5, grove_cover: 0.8, min_gap_m: 4.5 }
    }

    fn as_vector(&self) -> Vec<f64> {
        vec![self.avenue_spacing_m, self.square_spacing_m, self.grove_spacing_m, self.grove_cover, self.min_gap_m]
    }

    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        let mut slots = [
            &mut p.avenue_spacing_m,
            &mut p.square_spacing_m,
            &mut p.grove_spacing_m,
            &mut p.grove_cover,
            &mut p.min_gap_m,
        ];
        for (i, slot) in slots.iter_mut().enumerate() {
            if let (Some(s), Some(x)) = (schema.get(i), v.get(i)) {
                **slot = s.clamp(*x);
            }
        }
        p
    }
}

/// A ring with its holes, in local metres.
struct Shape {
    outer: Vec<Pt2>,
    holes: Vec<Vec<Pt2>>,
    lo: Pt2,
    hi: Pt2,
}

impl Shape {
    fn new(outer: Vec<Pt2>, holes: Vec<Vec<Pt2>>) -> Self {
        let (lo, hi) = planar::bbox(&outer);
        Self { outer, holes, lo, hi }
    }
    fn contains(&self, p: Pt2) -> bool {
        p.x >= self.lo.x
            && p.x <= self.hi.x
            && p.y >= self.lo.y
            && p.y <= self.hi.y
            && planar::point_in_polygon(p, &self.outer)
            && !self.holes.iter().any(|h| planar::point_in_polygon(p, h))
    }
    /// Distance from `p` to the nearest edge (outer or hole).
    fn edge_distance(&self, p: Pt2) -> f64 {
        std::iter::once(&self.outer).chain(self.holes.iter()).map(|r| ring_distance(p, r)).fold(f64::INFINITY, f64::min)
    }
    fn near(&self, p: Pt2, margin: f64) -> bool {
        p.x >= self.lo.x - margin && p.x <= self.hi.x + margin && p.y >= self.lo.y - margin && p.y <= self.hi.y + margin
    }
    fn short_side(&self) -> f64 {
        (self.hi.x - self.lo.x).min(self.hi.y - self.lo.y)
    }
}

fn ring_distance(p: Pt2, ring: &[Pt2]) -> f64 {
    let n = ring.len();
    (0..n).map(|i| planar::point_segment_distance(p, ring[i], ring[(i + 1) % n])).fold(f64::INFINITY, f64::min)
}

/// Area-weighted centroid (the vertex average drifts toward dense edges).
fn area_centroid(poly: &[Pt2]) -> Pt2 {
    let n = poly.len();
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let w = p.x * q.y - q.x * p.y;
        a += w;
        cx += (p.x + q.x) * w;
        cy += (p.y + q.y) * w;
    }
    if a.abs() < 1e-9 {
        return planar::centroid(poly);
    }
    Pt2::new(cx / (3.0 * a), cy / (3.0 * a))
}

/// Points every `spacing` metres along a polyline, starting half a spacing
/// in, each with the polyline's unit direction there.
fn stations(line: &[Pt2], spacing: f64, closed: bool) -> Vec<(Pt2, Pt2)> {
    let mut pts: Vec<Pt2> = line.to_vec();
    if closed && !pts.is_empty() {
        pts.push(pts[0]);
    }
    let mut out = Vec::new();
    let mut next = spacing / 2.0;
    let mut walked = 0.0;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.dist(b);
        if len < 1e-9 {
            continue;
        }
        let dir = Pt2::new((b.x - a.x) / len, (b.y - a.y) / len);
        while next <= walked + len {
            let t = next - walked;
            out.push((Pt2::new(a.x + dir.x * t, a.y + dir.y * t), dir));
            next += spacing;
        }
        walked += len;
    }
    out
}

/// A seeded value-noise field in [0, 1], bilinear with smoothstep, on a
/// `GROVE_FIELD_CELL_M` lattice.
fn grove_field(seed: u64, p: Pt2) -> f64 {
    let lattice = |ix: i64, iy: i64| -> f64 {
        let mut z = seed ^ (ix as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ (iy as u64).wrapping_mul(0xC2B2AE3D27D4EB4F);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    };
    let (fx, fy) = (p.x / GROVE_FIELD_CELL_M, p.y / GROVE_FIELD_CELL_M);
    let (ix, iy) = (fx.floor() as i64, fy.floor() as i64);
    let s = |t: f64| t * t * (3.0 - 2.0 * t);
    let (tx, ty) = (s(fx - ix as f64), s(fy - iy as f64));
    let top = lattice(ix, iy) * (1.0 - tx) + lattice(ix + 1, iy) * tx;
    let bot = lattice(ix, iy + 1) * (1.0 - tx) + lattice(ix + 1, iy + 1) * tx;
    top * (1.0 - ty) + bot * ty
}

/// Everything a candidate tree has to stay clear of.
struct Site {
    ring: Shape,
    buildings: Vec<Shape>,
    streets: Vec<(Vec<Pt2>, f64)>,
    markers: Vec<Pt2>,
    squares: Vec<(String, Shape)>,
    /// P64's ponds, marshes, streams and boardwalks: no tree P171 plants stands in them.
    water: Vec<Shape>,
}

impl Site {
    fn clear(&self, p: Pt2) -> bool {
        if !self.ring.contains(p) {
            return false;
        }
        for b in &self.buildings {
            if b.near(p, WALL_CLEARANCE_M) && (b.contains(p) || b.edge_distance(p) < WALL_CLEARANCE_M) {
                return false;
            }
        }
        for (line, half) in &self.streets {
            let d = line.windows(2).map(|w| planar::point_segment_distance(p, w[0], w[1])).fold(f64::INFINITY, f64::min);
            if d < half + STREET_CLEARANCE_M {
                return false;
            }
        }
        !self.markers.iter().any(|m| m.dist(p) < MARKER_CLEARANCE_M) && !self.water.iter().any(|w| w.contains(p))
    }
    fn in_square(&self, p: Pt2) -> bool {
        self.squares.iter().any(|(_, s)| s.contains(p))
    }
}

/// Planted trees with a coarse grid for the crowding test.
struct Planting {
    trees: Vec<(Pt2, f64, f64, &'static str)>,
    grid: HashMap<(i64, i64), Vec<usize>>,
    min_gap: f64,
}

const GRID_CELL_M: f64 = 6.0;

impl Planting {
    fn key(p: Pt2) -> (i64, i64) {
        ((p.x / GRID_CELL_M).floor() as i64, (p.y / GRID_CELL_M).floor() as i64)
    }
    fn crowded(&self, p: Pt2, crown: f64) -> bool {
        let (kx, ky) = Self::key(p);
        for dx in -2..=2 {
            for dy in -2..=2 {
                if let Some(ids) = self.grid.get(&(kx + dx, ky + dy)) {
                    for &i in ids {
                        let (q, _, r, _) = self.trees[i];
                        let need = self.min_gap.max((1.0 - CROWN_OVERLAP) * (crown + r));
                        if p.dist(q) < need {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
    fn plant(&mut self, p: Pt2, height: f64, crown: f64, form: &'static str) -> bool {
        if self.crowded(p, crown) {
            return false;
        }
        self.grid.entry(Self::key(p)).or_default().push(self.trees.len());
        self.trees.push((p, height, crown, form));
        true
    }
}

pub struct P171TreePlaces;

impl PatternOperator for P171TreePlaces {
    type Params = P171Params;

    fn name(&self) -> &'static str {
        "p171_tree_places"
    }

    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p171".into(),
            display: "Alexander et al., A Pattern Language, Pattern 171 (Tree Places)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Tree-Places-(171)".into()),
        }
    }

    fn description(&self) -> &'static str {
        "Plant avenues along the streets, a ring in each square, a spreading tree toward each square's middle, and groves on the open ground."
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        _parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        let ring = site_ring(nbhd).ok_or_else(|| {
            "p171_tree_places: no site ring to plant inside (needs a closed jurisdictional boundary or P172's meadow).".to_string()
        })?;
        let origin = planar::average_centroid(&ring);
        let local = |r: &Vec<LngLat>| planar::ring_to_local(r, &origin);
        let site = Site {
            ring: Shape::new(planar::ring_to_local(&ring, &origin), vec![]),
            buildings: nbhd
                .buildings
                .iter()
                .flat_map(|b| b.polygon.parts_view())
                .map(|part| Shape::new(local(&part.outer), part.holes.iter().map(local).collect()))
                // P34's platforms: no trunk on a platform.
                .chain(nbhd.transit.iter().flat_map(|l| &l.stations).map(|s| Shape::new(local(&s.platform.outer), vec![])))
                .collect(),
            // Streets, then P34's track (kept 3 m clear of its centre). Avenues are planted
            // along `nbhd.streets` zipped with this, so the track never gets an avenue.
            streets: nbhd
                .streets
                .iter()
                .map(|s| {
                    let line = s.centerline.iter().map(|p| planar::lnglat_to_local(p, &origin)).collect();
                    (line, s.row_width_m.unwrap_or(6.0) / 2.0)
                })
                .chain(nbhd.transit.iter().map(|l| (l.centerline.iter().map(|p| planar::lnglat_to_local(p, &origin)).collect(), 2.0)))
                .collect(),
            markers: nbhd.activity_nodes.iter().map(|a| planar::lnglat_to_local(&a.location, &origin)).collect(),
            squares: nbhd
                .open_space
                .iter()
                .filter(|o| matches!(o.kind, OpenSpaceKind::Plaza | OpenSpaceKind::Pocket) && o.polygon.area_m2() > 0.0)
                .map(|o| (o.id.clone(), Shape::new(local(&o.polygon.outer), o.polygon.holes.iter().map(local).collect())))
                .collect(),
            // The marsh's outer ring covers its pond too, so the holes don't matter here.
            water: nbhd
                .ground_cover
                .iter()
                .filter(|g| matches!(g.kind.as_str(), "pond" | "marsh" | "stream" | "boardwalk"))
                .map(|g| Shape::new(local(&g.polygon.outer), vec![]))
                .collect(),
        };

        let mut rng = Prng::new(seed);
        let mut planting = Planting { trees: Vec::new(), grid: HashMap::new(), min_gap: params.min_gap_m };
        // Trees already planted (P64's cypress) crowd new ones too; they're dropped from the output below.
        for t in &nbhd.trees {
            planting.plant(planar::lnglat_to_local(&t.location, &origin), t.height_m, t.crown_radius_m, "existing");
        }
        let n_existing = planting.trees.len();
        let mut steps = Vec::new();

        // 1. Avenues: both sides of every street, one size per street.
        let before = planting.trees.len();
        for (street, (line, half)) in nbhd.streets.iter().zip(&site.streets) {
            let (h, r) = (rng.range(9.0, 12.5), rng.range(3.0, 3.9));
            let offset = half + AVENUE_SETBACK_M;
            let mut n = 0;
            for (p, dir) in stations(line, params.avenue_spacing_m, false) {
                for side in [1.0, -1.0] {
                    let q = Pt2::new(p.x - dir.y * offset * side, p.y + dir.x * offset * side);
                    let k = rng.range(0.94, 1.06);
                    if site.clear(q) && planting.plant(q, h * k, r * k, "avenue") {
                        n += 1;
                    }
                }
            }
            if n > 0 {
                steps.push(format!("{}: an avenue of {n} trees, {:.1} m apart, {:.1} m beyond its right-of-way.", street.id, params.avenue_spacing_m, AVENUE_SETBACK_M));
            }
        }
        let n_avenue = planting.trees.len() - before;

        // 2. One spreading tree toward the middle of each square that can hold one.
        let before = planting.trees.len();
        for (id, sq) in &site.squares {
            if sq.short_side() < MIN_UMBRELLA_SIDE_M {
                continue;
            }
            let c = area_centroid(&sq.outer);
            let crown = rng.range(4.8, 6.2);
            let height = rng.range(9.0, 12.0);
            // The middle itself, else stepped away from the nearest marker
            // (P126 puts one roughly there).
            let nearest = site.markers.iter().copied().min_by(|a, b| a.dist(c).total_cmp(&b.dist(c)));
            let away = nearest
                .map(|m| {
                    let d = c.dist(m).max(1e-6);
                    Pt2::new((c.x - m.x) / d, (c.y - m.y) / d)
                })
                .unwrap_or(Pt2::new(1.0, 0.0));
            let fits = |q: Pt2| sq.contains(q) && sq.edge_distance(q) >= 4.0 && site.clear(q);
            let spot = [0.0, 3.0, 5.0, 7.0]
                .iter()
                .map(|s| Pt2::new(c.x + away.x * s, c.y + away.y * s))
                .find(|&q| fits(q));
            if let Some(q) = spot {
                if planting.plant(q, height, crown, "umbrella") {
                    steps.push(format!("{id}: one spreading tree toward the middle."));
                }
            }
        }
        let n_umbrella = planting.trees.len() - before;

        // 3. A ring just inside each square's edge.
        let before = planting.trees.len();
        for (id, sq) in &site.squares {
            if sq.short_side() < MIN_SQUARE_SIDE_M {
                continue;
            }
            let ring = planar::ensure_ccw(&sq.outer);
            let (h, r) = (rng.range(7.0, 10.0), rng.range(2.6, 3.4));
            let mut n = 0;
            for (p, dir) in stations(&ring, params.square_spacing_m, true) {
                // Counter-clockwise ring: inward is to the left of travel.
                let q = Pt2::new(p.x - dir.y * SQUARE_INSET_M, p.y + dir.x * SQUARE_INSET_M);
                let k = rng.range(0.94, 1.06);
                if sq.contains(q) && sq.edge_distance(q) >= SQUARE_INSET_M - 0.3 && site.clear(q) && planting.plant(q, h * k, r * k, "square") {
                    n += 1;
                }
            }
            if n > 0 {
                steps.push(format!("{id}: a ring of {n} trees {SQUARE_INSET_M:.1} m inside its edge."));
            }
        }
        let n_square = planting.trees.len() - before;

        // 4. Groves on the rest of the ground, with clearings.
        let before = planting.trees.len();
        let field_seed = seed ^ 0x171_171;
        let s = params.grove_spacing_m;
        let jitter = 0.35 * s;
        let (lo, hi) = (site.ring.lo, site.ring.hi);
        let (nx, ny) = (((hi.x - lo.x) / s).ceil() as i64, ((hi.y - lo.y) / s).ceil() as i64);
        let mut n_candidates = 0;
        for j in 0..=ny {
            for i in 0..=nx {
                let base = Pt2::new(lo.x + i as f64 * s + if j % 2 == 1 { s / 2.0 } else { 0.0 }, lo.y + j as f64 * s);
                let q = Pt2::new(base.x + rng.range(-jitter, jitter), base.y + rng.range(-jitter, jitter));
                let (h, r) = (rng.range(6.0, 14.0), rng.range(2.4, 4.4));
                if !site.clear(q) || site.in_square(q) {
                    continue;
                }
                n_candidates += 1;
                if grove_field(field_seed, q) > params.grove_cover {
                    continue;
                }
                planting.plant(q, h, r, "grove");
            }
        }
        let n_grove = planting.trees.len() - before;
        steps.push(format!(
            "groves: {n_grove} trees on {n_candidates} open grid points ({s:.1} m, jittered), clearings where the field passes {:.2}.",
            params.grove_cover
        ));

        let new_trees: Vec<Tree> = planting
            .trees
            .iter()
            .skip(n_existing)
            .enumerate()
            .map(|(i, &(p, height_m, crown_radius_m, form))| Tree {
                id: format!("p171_{form}_{i:04}"),
                location: planar::local_to_lnglat(p, &origin),
                height_m,
                crown_radius_m,
                form: form.into(),
            })
            .collect();

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!(
                "{} trees: {n_avenue} in avenues, {n_umbrella} spreading trees, {n_square} around squares, {n_grove} in groves.",
                new_trees.len()
            ),
            steps,
            caveats: vec![
                "The pattern gives no spacing, height or crown size; these are plausible street-tree and grove figures, not Alexander's.".into(),
                "Trees fit buildings that already exist; the pattern's 'shape the buildings to the trees' half isn't modeled.".into(),
                "No species, roots or utilities, and no enclosures made of trees alone.".into(),
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
            new_trees,
            new_ground_cover: vec![],
            new_transit: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p172_garden_growing_wild::{P172GardenGrowingWild, P172Params};
    use crate::subdivision::apply_subdivision;
    use street_smarts_core::geometry::{haversine_m, Polygon};
    use street_smarts_core::nir::{Boundary, BoundaryKind, Building, NeighborhoodMeta, OpenSpace, Street};

    const M: f64 = 1.0 / 111_320.0;

    fn ring(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<LngLat> {
        vec![
            LngLat::new(x0 * M, y0 * M),
            LngLat::new(x1 * M, y0 * M),
            LngLat::new(x1 * M, y1 * M),
            LngLat::new(x0 * M, y1 * M),
            LngLat::new(x0 * M, y0 * M),
        ]
    }

    /// A 200 m site with one building, one street across it and one square.
    fn site() -> Neighborhood {
        let mut building: Building = serde_json::from_value(serde_json::json!({
            "id": "B1", "polygon": {"outer": [], "holes": []}, "height_m": 10.0
        }))
        .unwrap();
        building.polygon = Polygon::from_ring(ring(20.0, 20.0, 60.0, 60.0));
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 200.0 * M, 200.0 * M],
            parcels: vec![],
            buildings: vec![building],
            streets: vec![Street {
                id: "S1".into(),
                centerline: vec![LngLat::new(0.0, 100.0 * M), LngLat::new(200.0 * M, 100.0 * M)],
                classification: Some("local".into()),
                row_width_m: Some(6.0),
                surface: Some("grass_pavers".into()),
            }],
            open_space: vec![OpenSpace { id: "SQ1".into(), polygon: Polygon::from_ring(ring(120.0, 20.0, 170.0, 70.0)), kind: OpenSpaceKind::Plaza }],
            boundaries: vec![Boundary { id: "site_perimeter".into(), centerline: ring(0.0, 0.0, 200.0, 200.0), kind: BoundaryKind::Jurisdictional }],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P171 unit fixture".into(),
            },
            pattern_fields: vec![],
            trees: vec![],
            ground_cover: vec![],
            transit: vec![],
        }
    }

    fn planted(n: &Neighborhood) -> Vec<Tree> {
        P171TreePlaces.apply(n, "*", &P171Params::defaults(), 7).unwrap().new_trees
    }

    #[test]
    fn plants_every_form() {
        let trees = planted(&site());
        for form in ["avenue", "umbrella", "square", "grove"] {
            assert!(trees.iter().any(|t| t.form == form), "no {form} trees in {} planted", trees.len());
        }
    }

    #[test]
    fn keeps_out_of_buildings_streets_and_the_site_edge() {
        let n = site();
        for t in planted(&n) {
            let (x, y) = (t.location.lng / M, t.location.lat / M);
            assert!((0.0..=200.0).contains(&x) && (0.0..=200.0).contains(&y), "{} outside the site", t.id);
            // 2 m wall clearance, less a little for the fixture's degrees
            // not being exactly the operator's projected metres.
            assert!(!((18.2..=61.8).contains(&x) && (18.2..=61.8).contains(&y)), "{} at ({x:.2}, {y:.2}) in or against the building", t.id);
            assert!((y - 100.0).abs() >= 3.0 + STREET_CLEARANCE_M - 0.01, "{} on the street", t.id);
        }
    }

    #[test]
    fn no_two_trees_crowd_each_other() {
        let trees = planted(&site());
        let gap = P171Params::defaults().min_gap_m;
        for (i, a) in trees.iter().enumerate() {
            for b in &trees[i + 1..] {
                assert!(haversine_m(&a.location, &b.location) >= gap - 0.05, "{} and {} too close", a.id, b.id);
            }
        }
    }

    #[test]
    fn groves_stay_out_of_squares() {
        for t in planted(&site()).iter().filter(|t| t.form == "grove") {
            let (x, y) = (t.location.lng / M, t.location.lat / M);
            assert!(!((120.0..=170.0).contains(&x) && (20.0..=70.0).contains(&y)), "{} grove tree in the square", t.id);
        }
    }

    #[test]
    fn grove_cover_zero_plants_no_groves() {
        let params = P171Params { grove_cover: 0.0, ..P171Params::defaults() };
        let trees = P171TreePlaces.apply(&site(), "*", &params, 7).unwrap().new_trees;
        assert!(trees.iter().all(|t| t.form != "grove"));
    }

    #[test]
    fn same_seed_same_trees_and_meadow_ring_agrees() {
        let n = site();
        assert_eq!(planted(&n), planted(&n));
        let meadow = P172GardenGrowingWild.apply(&n, "*", &P172Params::defaults(), 7).unwrap();
        let with_meadow = apply_subdivision(&n, &meadow);
        assert_eq!(planted(&with_meadow), planted(&n));
        let sub = P171TreePlaces.apply(&with_meadow, "*", &P171Params::defaults(), 7).unwrap();
        assert_eq!(apply_subdivision(&with_meadow, &sub).trees.len(), sub.new_trees.len());
    }

    #[test]
    fn no_site_ring_is_an_error() {
        let mut n = site();
        n.boundaries.clear();
        assert!(P171TreePlaces.apply(&n, "*", &P171Params::defaults(), 7).is_err());
    }
}
