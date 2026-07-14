//! Planar geometry: WGS84 ↔ local-meters projection, Voronoi, polygon clipping.
//!
//! Subdivision works in a local equirectangular meter frame anchored at the
//! parcel's centroid. All math (Voronoi, half-plane clipping, area) happens
//! in metres; we convert back to lng/lat only when emitting the final NIR.

use street_smarts_core::geometry::{LngLat, Ring};

/// 2D point in local metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt2 {
    pub x: f64,
    pub y: f64,
}

impl Pt2 {
    pub fn new(x: f64, y: f64) -> Self { Self { x, y } }
    pub fn sub(self, o: Pt2) -> Pt2 { Pt2 { x: self.x - o.x, y: self.y - o.y } }
    pub fn len(self) -> f64 { (self.x * self.x + self.y * self.y).sqrt() }
    pub fn dot(self, o: Pt2) -> f64 { self.x * o.x + self.y * o.y }
    pub fn dist(self, o: Pt2) -> f64 { self.sub(o).len() }
}

/// Project lng/lat to local metres using an equirectangular approximation
/// anchored at `origin`.
pub fn lnglat_to_local(p: &LngLat, origin: &LngLat) -> Pt2 {
    let mlat = (origin.lat * std::f64::consts::PI / 180.0).cos();
    Pt2 {
        x: (p.lng - origin.lng) * mlat * 111_320.0,
        y: (p.lat - origin.lat) * 110_540.0,
    }
}

/// Inverse: local-metre offset → lng/lat near `origin`.
pub fn local_to_lnglat(p: Pt2, origin: &LngLat) -> LngLat {
    let mlat = (origin.lat * std::f64::consts::PI / 180.0).cos();
    LngLat {
        lng: origin.lng + p.x / (mlat * 111_320.0),
        lat: origin.lat + p.y / 110_540.0,
    }
}

/// Convert a WGS84 ring to a local-meter polygon. Assumes the ring is
/// already closed (first == last); the closing repeat is dropped.
pub fn ring_to_local(ring: &Ring, origin: &LngLat) -> Vec<Pt2> {
    let mut pts: Vec<Pt2> = ring.iter().map(|p| lnglat_to_local(p, origin)).collect();
    if pts.len() >= 2 && (pts.first() == pts.last()) {
        pts.pop();
    }
    pts
}

/// Convert a local polygon back to a closed WGS84 ring.
pub fn local_to_ring(poly: &[Pt2], origin: &LngLat) -> Ring {
    let mut out: Vec<LngLat> = poly.iter().map(|p| local_to_lnglat(*p, origin)).collect();
    if !out.is_empty() {
        out.push(out[0]);
    }
    out
}

/// Polygon centroid (vertex average; adequate for our purposes).
pub fn centroid(poly: &[Pt2]) -> Pt2 {
    if poly.is_empty() { return Pt2::new(0.0, 0.0); }
    let (sx, sy) = poly.iter().fold((0.0, 0.0), |(a, b), p| (a + p.x, b + p.y));
    let n = poly.len() as f64;
    Pt2::new(sx / n, sy / n)
}

/// Vertex-average centroid of a WGS84 ring, returned as lng/lat.
pub fn average_centroid(ring: &[LngLat]) -> LngLat {
    if ring.is_empty() { return LngLat::new(0.0, 0.0); }
    let mut lng = 0.0;
    let mut lat = 0.0;
    for p in ring {
        lng += p.lng;
        lat += p.lat;
    }
    let n = ring.len() as f64;
    LngLat::new(lng / n, lat / n)
}

/// Shoelace area (unsigned).
pub fn area(poly: &[Pt2]) -> f64 {
    if poly.len() < 3 { return 0.0; }
    let n = poly.len();
    let mut s = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        s += poly[i].x * poly[j].y - poly[j].x * poly[i].y;
    }
    (s * 0.5).abs()
}

/// Polygon bounding box.
pub fn bbox(poly: &[Pt2]) -> (Pt2, Pt2) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in poly {
        if p.x < min_x { min_x = p.x; }
        if p.y < min_y { min_y = p.y; }
        if p.x > max_x { max_x = p.x; }
        if p.y > max_y { max_y = p.y; }
    }
    (Pt2::new(min_x, min_y), Pt2::new(max_x, max_y))
}

/// Standard ray-casting point-in-polygon (works for non-convex).
pub fn point_in_polygon(pt: Pt2, poly: &[Pt2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 { return false; }
    let mut j = n - 1;
    for i in 0..n {
        let pi = poly[i];
        let pj = poly[j];
        if ((pi.y > pt.y) != (pj.y > pt.y))
            && (pt.x < (pj.x - pi.x) * (pt.y - pi.y) / (pj.y - pi.y + 1e-12) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Clip a convex polygon `subject` against the half-plane defined by directed
/// line from `a` to `b`: keeps points to the LEFT of a→b.
///
/// Returns the clipped polygon (may be empty if entirely outside).
pub fn clip_half_plane(subject: &[Pt2], a: Pt2, b: Pt2) -> Vec<Pt2> {
    if subject.is_empty() { return vec![]; }
    let edge_x = b.x - a.x;
    let edge_y = b.y - a.y;
    // Cross product sign tells us which side: > 0 = left.
    let side = |p: Pt2| -> f64 {
        edge_x * (p.y - a.y) - edge_y * (p.x - a.x)
    };
    let intersect = |p: Pt2, q: Pt2| -> Pt2 {
        let s1 = side(p);
        let s2 = side(q);
        let t = s1 / (s1 - s2 + 1e-12);
        Pt2 { x: p.x + t * (q.x - p.x), y: p.y + t * (q.y - p.y) }
    };

    let mut out: Vec<Pt2> = Vec::with_capacity(subject.len() + 2);
    let n = subject.len();
    for i in 0..n {
        let curr = subject[i];
        let prev = subject[(i + n - 1) % n];
        let s_curr = side(curr);
        let s_prev = side(prev);
        let curr_in = s_curr >= 0.0;
        let prev_in = s_prev >= 0.0;
        if curr_in {
            if !prev_in { out.push(intersect(prev, curr)); }
            out.push(curr);
        } else if prev_in {
            out.push(intersect(prev, curr));
        }
    }
    out
}

/// Inset (negative buffer) a convex polygon by `d` metres. Returns empty if
/// the polygon shrinks to nothing.
///
/// Simple inward-normal offset for convex polygons; we use it on Voronoi
/// cells before clipping against the parcel boundary.
pub fn inset_convex(poly: &[Pt2], d: f64) -> Vec<Pt2> {
    if poly.len() < 3 || d <= 0.0 { return poly.to_vec(); }
    let n = poly.len();
    // Orientation check.
    let signed = {
        let mut s = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            s += poly[i].x * poly[j].y - poly[j].x * poly[i].y;
        }
        s
    };
    let ccw = signed > 0.0;

    let mut working = poly.to_vec();
    for i in 0..n {
        let j = (i + 1) % n;
        let p = poly[i];
        let q = poly[j];
        let mut ex = q.x - p.x;
        let mut ey = q.y - p.y;
        let len = (ex * ex + ey * ey).sqrt();
        if len < 1e-9 { continue; }
        ex /= len; ey /= len;
        // Inward normal: for CCW (interior on left of each edge), rotating
        // edge +90° (math convention, y-up) gives (-ey, ex). For CW polygons
        // we want the OUTWARD-of-CW (= inward) which is the opposite.
        let (nx, ny) = if ccw { (-ey, ex) } else { (ey, -ex) };
        // Offset edge inward by `d`.
        let a = Pt2 { x: p.x + nx * d, y: p.y + ny * d };
        let b = Pt2 { x: q.x + nx * d, y: q.y + ny * d };
        // clip_half_plane keeps points to the LEFT of a→b.
        // For CCW polygon, interior is to the left of original edge p→q,
        // and also to the left of the parallel offset edge a→b. So pass a,b.
        // For CW polygon: interior is to the RIGHT of p→q, so flip a,b.
        let (a, b) = if ccw { (a, b) } else { (b, a) };
        working = clip_half_plane(&working, a, b);
        if working.is_empty() { return vec![]; }
    }
    working
}

/// Compute the Voronoi cell of `site` against the other `sites`, clipped to
/// the convex `bound`. Bound is a convex polygon (we pass the parcel's bbox
/// rectangle; downstream code further clips to the actual parcel shape).
///
/// Voronoi cells are intersections of half-planes — for each other site `s`,
/// the cell of `site` consists of points closer to `site` than to `s`.
pub fn voronoi_cell(site: Pt2, others: &[Pt2], bound: &[Pt2]) -> Vec<Pt2> {
    let mut cell = bound.to_vec();
    for &other in others {
        if other == site { continue; }
        // Bisector line: perpendicular to site→other, passing through midpoint.
        // We keep points closer to `site` ⇒ to the LEFT of the bisector
        // when traversed in a specific direction.
        let mid = Pt2::new((site.x + other.x) * 0.5, (site.y + other.y) * 0.5);
        // Direction perpendicular to site→other (rotated +90°).
        let dx = other.x - site.x;
        let dy = other.y - site.y;
        // Half-plane: points p such that (p - mid) · (site - other) > 0
        // are closer to site. The directed line from `a` to `b` along the
        // bisector, with "left = closer to site":
        // We want left side = site side. Rotate (-dy, dx) is +90° from
        // site→other direction.
        let a = Pt2::new(mid.x - dy, mid.y + dx);
        let b = Pt2::new(mid.x + dy, mid.y - dx);
        // Which of `a→b` or `b→a` puts site on the left? Check.
        let side_ab = (b.x - a.x) * (site.y - a.y) - (b.y - a.y) * (site.x - a.x);
        if side_ab >= 0.0 {
            cell = clip_half_plane(&cell, a, b);
        } else {
            cell = clip_half_plane(&cell, b, a);
        }
        if cell.is_empty() { return vec![]; }
    }
    cell
}

/// Compute the convex hull (Graham scan) of a point set. Returns points in
/// CCW order. Used as a defensive fallback when the actual parcel boundary
/// is non-convex and `clip_convex_to_polygon` would produce wrong results.
pub fn convex_hull(points: &[Pt2]) -> Vec<Pt2> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut pts = points.to_vec();
    // Sort by x then y.
    pts.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)));
    // Build lower hull
    let mut hull: Vec<Pt2> = Vec::with_capacity(pts.len());
    for &p in &pts {
        while hull.len() >= 2 {
            let a = hull[hull.len() - 2];
            let b = hull[hull.len() - 1];
            let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            if cross <= 0.0 { hull.pop(); } else { break; }
        }
        hull.push(p);
    }
    // Upper hull
    let lower_len = hull.len() + 1;
    for &p in pts.iter().rev() {
        while hull.len() >= lower_len {
            let a = hull[hull.len() - 2];
            let b = hull[hull.len() - 1];
            let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            if cross <= 0.0 { hull.pop(); } else { break; }
        }
        hull.push(p);
    }
    hull.pop(); // remove duplicate last == first
    hull
}

/// Ensure a polygon is in CCW orientation. Reverses if CW.
pub fn ensure_ccw(poly: &[Pt2]) -> Vec<Pt2> {
    let n = poly.len();
    if n < 3 { return poly.to_vec(); }
    let mut s = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        s += poly[i].x * poly[j].y - poly[j].x * poly[i].y;
    }
    if s > 0.0 { poly.to_vec() } else { poly.iter().rev().copied().collect() }
}

/// Triangulate a simple polygon (no self-intersections) using ear clipping.
/// Returns triangles as triples of vertices.
///
/// Handles arbitrary non-convex polygons. O(N²) in vertex count; fine for
/// neighborhood-scale parcels (~200 vertices).
pub fn triangulate(polygon: &[Pt2]) -> Vec<[Pt2; 3]> {
    if polygon.len() < 3 { return vec![]; }
    let ccw = ensure_ccw(polygon);
    let mut indices: Vec<usize> = (0..ccw.len()).collect();
    let mut triangles: Vec<[Pt2; 3]> = Vec::with_capacity(ccw.len().saturating_sub(2));

    // Cross product (B - A) × (C - A); > 0 if A→B→C is CCW.
    let cross = |a: Pt2, b: Pt2, c: Pt2| -> f64 {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    };
    // Point-in-triangle (strict; vertex points are treated as outside).
    let in_tri = |p: Pt2, a: Pt2, b: Pt2, c: Pt2| -> bool {
        let d1 = cross(p, a, b);
        let d2 = cross(p, b, c);
        let d3 = cross(p, c, a);
        let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(has_neg && has_pos)
    };

    // Guard against infinite loops on degenerate input.
    let max_iters = ccw.len() * ccw.len() + 16;
    let mut iter = 0;

    while indices.len() > 3 {
        iter += 1;
        if iter > max_iters {
            // Degenerate polygon (self-intersection, collinear chain, ...).
            // Emit what we have and stop, rather than spinning.
            break;
        }
        let n = indices.len();
        let mut ear_idx: Option<usize> = None;
        for i in 0..n {
            let prev = indices[(i + n - 1) % n];
            let curr = indices[i];
            let next = indices[(i + 1) % n];
            let a = ccw[prev];
            let b = ccw[curr];
            let c = ccw[next];
            // Convex test: in CCW polygon, an interior (ear) vertex has cross > 0.
            if cross(a, b, c) <= 0.0 { continue; }
            // No other vertex may lie inside the candidate triangle.
            let mut clean = true;
            for &k in &indices {
                if k == prev || k == curr || k == next { continue; }
                if in_tri(ccw[k], a, b, c) {
                    clean = false;
                    break;
                }
            }
            if clean {
                ear_idx = Some(i);
                break;
            }
        }
        match ear_idx {
            Some(i) => {
                let n = indices.len();
                let prev = indices[(i + n - 1) % n];
                let curr = indices[i];
                let next = indices[(i + 1) % n];
                triangles.push([ccw[prev], ccw[curr], ccw[next]]);
                indices.remove(i);
            }
            None => break, // no ear found — defensive bail
        }
    }
    if indices.len() == 3 {
        triangles.push([ccw[indices[0]], ccw[indices[1]], ccw[indices[2]]]);
    }
    triangles
}

/// Clip a convex polygon against each edge of a (possibly non-convex)
/// polygon. WRONG for non-convex polygons — produces the convex-hull
/// intersection. Use `clip_to_polygon` instead.
pub fn clip_convex_to_polygon(convex_cell: &[Pt2], polygon: &[Pt2]) -> Vec<Pt2> {
    if convex_cell.is_empty() || polygon.len() < 3 { return vec![]; }
    let poly = ensure_ccw(polygon);
    let mut working = convex_cell.to_vec();
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        working = clip_half_plane(&working, a, b);
        if working.is_empty() { return vec![]; }
    }
    working
}

/// Clip a convex polygon (e.g. a Voronoi cell) against an arbitrary
/// (possibly non-convex) polygon. Returns ZERO OR MORE polygon pieces.
///
/// Algorithm: triangulate the clip polygon, intersect the convex subject
/// with each triangle, return the non-empty pieces. The pieces are convex
/// (intersections of convex polygons) and disjoint (since the triangulation
/// is a disjoint partition). For most cases the convex subject only touches
/// 1-3 triangles, so we usually get 1-3 small pieces.
///
/// For our use (Voronoi cells inside an arbitrary parcel boundary), the
/// pieces are *not* generally unioned back into one polygon — keeping them
/// separate is fine because each piece is a valid sub-pad.
pub fn clip_to_polygon(convex_subject: &[Pt2], clip_polygon: &[Pt2]) -> Vec<Vec<Pt2>> {
    if convex_subject.len() < 3 || clip_polygon.len() < 3 {
        return vec![];
    }
    let triangles = triangulate(clip_polygon);
    let mut pieces: Vec<Vec<Pt2>> = Vec::new();
    for tri in triangles {
        // Triangle as a 3-vertex CCW polygon.
        let tri_poly = vec![tri[0], tri[1], tri[2]];
        // Intersection of two convex polygons via half-plane clipping.
        let mut working = convex_subject.to_vec();
        for i in 0..3 {
            let a = tri_poly[i];
            let b = tri_poly[(i + 1) % 3];
            working = clip_half_plane(&working, a, b);
            if working.is_empty() { break; }
        }
        if working.len() >= 3 && area(&working) > 0.5 {
            pieces.push(working);
        }
    }
    pieces
}

/// Convenience: clip a convex subject against a clip polygon, return the
/// single largest piece (or empty if nothing fits). Useful when an operator
/// wants one canonical sub-region per cell.
pub fn clip_to_polygon_largest(convex_subject: &[Pt2], clip_polygon: &[Pt2]) -> Vec<Pt2> {
    let pieces = clip_to_polygon(convex_subject, clip_polygon);
    pieces
        .into_iter()
        .max_by(|a, b| area(a).partial_cmp(&area(b)).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or_default()
}

/// Union a set of polygon pieces that jointly tile a region without gaps or
/// overlaps (e.g. the pieces `clip_to_polygon` returns for ONE convex
/// subject, which are convex intersections against different triangles of
/// the same clip-polygon triangulation).
///
/// Algorithm: every edge shared between two adjacent pieces appears twice —
/// once forward in one piece's boundary, once backward in the other's,
/// since both pieces were cut from the same underlying triangulation edge.
/// Cancel those pairs; whatever's left traces the real outer boundary (or
/// boundaries, if the input pieces are genuinely disjoint — e.g. a Voronoi
/// cell split by a real concavity in the clip polygon. That's correct, not
/// a bug: two truly separate pieces of land shouldn't merge into one pad.)
///
/// Points are matched by rounding to 0.1mm — comfortably above f64 roundoff
/// at parcel-scale (metre) coordinates, comfortably below anything that
/// matters geometrically for a building pad.
///
/// Uses `BTreeMap`/`BTreeSet`, not `HashMap`/`HashSet`: an earlier version
/// used hash maps, whose iteration order isn't seeded, so which vertex
/// started a boundary trace could vary run-to-run on identical input (noted
/// as a known gap when it only shifted floating-point noise in a score).
/// Reworking P95 to build around subtracted reserved land leans on this
/// function to re-merge subtraction seams (see `subtract_convex`'s
/// callers), where that same nondeterminism showed up as real, measurable
/// overlap slivers along a seam, not just score noise -- worth the fix now.
pub fn union_pieces(pieces: &[Vec<Pt2>]) -> Vec<Vec<Pt2>> {
    use std::collections::BTreeMap;

    fn key(p: Pt2) -> (i64, i64) {
        ((p.x * 10_000.0).round() as i64, (p.y * 10_000.0).round() as i64)
    }

    let mut point_by_key: BTreeMap<(i64, i64), Pt2> = BTreeMap::new();
    let mut edge_count: BTreeMap<((i64, i64), (i64, i64)), i32> = BTreeMap::new();

    for piece in pieces {
        let n = piece.len();
        if n < 3 { continue; }
        for i in 0..n {
            let a = piece[i];
            let b = piece[(i + 1) % n];
            let (ka, kb) = (key(a), key(b));
            point_by_key.entry(ka).or_insert(a);
            point_by_key.entry(kb).or_insert(b);
            *edge_count.entry((ka, kb)).or_insert(0) += 1;
        }
    }

    // Keep only edges whose reverse doesn't also occur — the real boundary.
    let mut next: BTreeMap<(i64, i64), (i64, i64)> = BTreeMap::new();
    for (&(a, b), _) in edge_count.iter() {
        if !edge_count.contains_key(&(b, a)) {
            next.insert(a, b);
        }
    }

    let mut visited: std::collections::BTreeSet<(i64, i64)> = std::collections::BTreeSet::new();
    let mut loops: Vec<Vec<Pt2>> = Vec::new();

    for (&start, _) in next.iter() {
        if visited.contains(&start) { continue; }
        let mut loop_pts: Vec<Pt2> = Vec::new();
        let mut cur = start;
        loop {
            if visited.contains(&cur) { break; }
            visited.insert(cur);
            loop_pts.push(*point_by_key.get(&cur).expect("key was inserted alongside edge"));
            match next.get(&cur) {
                Some(&n2) => {
                    cur = n2;
                    if cur == start { break; }
                }
                None => break, // malformed boundary (shouldn't happen for a valid tiling); bail this loop
            }
        }
        if loop_pts.len() >= 3 {
            loops.push(simplify_collinear(&loop_pts));
        }
    }
    loops
}

/// Drop vertices that sit (near-)collinear between their neighbors. Cosmetic
/// only — area/point-in-polygon are unaffected either way — but keeps
/// unioned pad boundaries from carrying redundant vertices along merged
/// straight edges.
fn simplify_collinear(poly: &[Pt2]) -> Vec<Pt2> {
    let n = poly.len();
    if n < 4 { return poly.to_vec(); }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let prev = poly[(i + n - 1) % n];
        let curr = poly[i];
        let next = poly[(i + 1) % n];
        let cross = (curr.x - prev.x) * (next.y - prev.y) - (curr.y - prev.y) * (next.x - prev.x);
        // Keep unless essentially collinear (cross ~ 0 relative to edge lengths).
        let scale = prev.dist(curr).max(curr.dist(next)).max(1e-9);
        if cross.abs() / (scale * scale) > 1e-7 {
            out.push(curr);
        }
    }
    if out.len() >= 3 { out } else { poly.to_vec() }
}
/// Scale a polygon toward its centroid by a LINEAR factor (not area ratio --
/// building_shape.rs's `shrink_toward_centroid` takes an area ratio; this
/// takes a direct linear factor, which is what you want when the target is
/// "this bounding dimension should become X metres," as in P61).
/// factor=1 is a no-op; factor<1 shrinks.
pub fn scale_toward_centroid(poly: &[Pt2], factor: f64) -> Vec<Pt2> {
    if poly.is_empty() { return vec![]; }
    let c = centroid(poly);
    poly.iter().map(|p| Pt2 { x: c.x + (p.x - c.x) * factor, y: c.y + (p.y - c.y) * factor }).collect()
}

/// Plain union-find (path compression + union by size), private to this
/// module. Backs `kruskal_mst` below.
struct UnionFind {
    parent: Vec<usize>,
    size: Vec<usize>,
}
impl UnionFind {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect(), size: vec![1; n] }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) -> bool {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb { return false; }
        let (big, small) = if self.size[ra] >= self.size[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        true
    }
}

/// Result of `kruskal_mst`: the spanning-tree edges (the fewest that still
/// connect every point) and every other pairwise edge NOT used by the tree,
/// both in ascending-distance order. Callers that want a few relieving
/// loops beyond the pure tree (PathNetwork's `loop_budget`, P61's square
/// connectors) take the cheapest entries from `remaining_edges`.
pub struct MstResult {
    pub mst_edges: Vec<(usize, usize, f64)>,
    pub remaining_edges: Vec<(usize, usize, f64)>,
}

/// Kruskal's MST over a point set (Euclidean distance in local metres).
/// Shared by any operator that wants "the fewest edges that still connect
/// everything" rather than a full mesh -- the same honest reading of
/// Alexander's P52 (sparse network, not full connectivity) applies equally
/// to P61's job of linking a handful of small squares.
pub fn kruskal_mst(points: &[Pt2]) -> MstResult {
    let n = points.len();
    let mut edges: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            edges.push((i, j, points[i].dist(points[j])));
        }
    }
    edges.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut uf = UnionFind::new(n);
    let mut mst_edges = Vec::new();
    let mut remaining_edges = Vec::new();
    for (i, j, d) in edges {
        if uf.union(i, j) {
            mst_edges.push((i, j, d));
        } else {
            remaining_edges.push((i, j, d));
        }
    }
    MstResult { mst_edges, remaining_edges }
}

/// Subtract a convex `hole` from a (possibly non-convex) `subject` polygon,
/// returning the remaining land as zero or more simple polygon pieces.
///
/// The one general boolean-subtraction primitive this codebase didn't have:
/// `clip_to_polygon` only computes intersection (A ∩ B). Reworking P95 to
/// build around P52/P61's pre-placed paths and squares (rather than
/// carving its own single leftover courtyard) needs real subtraction --
/// "this parcel, MINUS the land already reserved."
///
/// Algorithm: for a convex hole with CCW edges e_1..e_n, `subject \ hole`
/// decomposes exactly into the union of:
///   Piece_1 = subject ∩ outside(e_1)
///   Piece_2 = subject ∩ inside(e_1) ∩ outside(e_2)
///   Piece_i = subject ∩ inside(e_1..e_{i-1}) ∩ outside(e_i)
/// These are mutually exclusive by construction (each requires every prior
/// edge to be "inside" and the current one "outside"), and their union is
/// exactly "outside at least one edge" = outside the hole. Each piece is
/// produced by `clip_half_plane`, which is exact against a single infinite
/// line even when `subject` itself is non-convex -- only intersecting
/// against MULTIPLE edges of a non-convex clip polygon at once is unsafe
/// (that's what `clip_to_polygon`'s triangulation trick works around);
/// sequential single-line clips have no such problem.
///
/// `hole` must be convex (callers in this codebase only ever subtract
/// squares and path corridors, both convex by construction). A non-convex
/// hole would silently subtract its convex hull instead of its real shape
/// -- not guarded against here, same tradeoff `clip_convex_to_polygon`
/// documents for its own convexity assumption.
///
/// Do NOT feed this function's output through `union_pieces` expecting it
/// to re-merge the pieces into fewer, cleaner shapes. `union_pieces`
/// assumes triangulation-style splits, where a shared internal edge is the
/// exact same segment in both neighboring pieces. These pieces instead
/// share boundary along the HOLE's cut lines -- and a real subject vertex
/// landing near one of those lines can subdivide it differently across
/// neighboring pieces, so union_pieces cancels the wrong edges and
/// silently reintroduces part of the hole. Caught this for real in P95's
/// rework: it cost ~300 m² of a "subtracted" square reappearing in a
/// downstream building pad. If fragment count matters to a caller, filter
/// or accept the un-merged pieces; don't union them.
pub fn subtract_convex(subject: &[Pt2], hole: &[Pt2]) -> Vec<Vec<Pt2>> {
    if subject.len() < 3 {
        return vec![];
    }
    if hole.len() < 3 {
        return vec![subject.to_vec()];
    }
    let hole_ccw = ensure_ccw(hole);
    let n = hole_ccw.len();
    let mut pieces: Vec<Vec<Pt2>> = Vec::new();
    let mut remaining = subject.to_vec();
    for i in 0..n {
        if remaining.len() < 3 {
            break;
        }
        let a = hole_ccw[i];
        let b = hole_ccw[(i + 1) % n];
        // Outside edge i (left of b->a, i.e. right of a->b): the piece of
        // `remaining` that's already known to fail this edge's "inside"
        // test -- part of subject \ hole, final.
        let outside_piece = clip_half_plane(&remaining, b, a);
        if outside_piece.len() >= 3 && area(&outside_piece) > 0.5 {
            pieces.push(outside_piece);
        }
        // Narrow `remaining` to "inside edge i" for the next iteration.
        remaining = clip_half_plane(&remaining, a, b);
    }
    pieces
}

/// Buffer a segment `p`-`q` into a rectangle of the given half-width, CCW
/// oriented -- the shape a path/street's right-of-way actually occupies,
/// for use as a `subtract_convex` hole. Degenerates to an empty vec for a
/// zero-length segment (nothing to buffer).
pub fn rect_corridor(p: Pt2, q: Pt2, half_width: f64) -> Vec<Pt2> {
    let dx = q.x - p.x;
    let dy = q.y - p.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 || half_width <= 0.0 {
        return vec![];
    }
    let (nx, ny) = (-dy / len * half_width, dx / len * half_width);
    vec![
        Pt2::new(p.x + nx, p.y + ny),
        Pt2::new(q.x + nx, q.y + ny),
        Pt2::new(q.x - nx, q.y - ny),
        Pt2::new(p.x - nx, p.y - ny),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_two_adjacent_squares_merges_to_one_loop() {
        let left = vec![Pt2::new(0.0, 0.0), Pt2::new(1.0, 0.0), Pt2::new(1.0, 1.0), Pt2::new(0.0, 1.0)];
        // Shares the edge (1,0)-(1,1) with `left`, traversed in reverse per CCW winding.
        let right = vec![Pt2::new(1.0, 0.0), Pt2::new(2.0, 0.0), Pt2::new(2.0, 1.0), Pt2::new(1.0, 1.0)];
        let merged = union_pieces(&[left, right]);
        assert_eq!(merged.len(), 1, "two adjacent squares should merge into one loop");
        assert!((area(&merged[0]) - 2.0).abs() < 1e-6, "merged area should be 2.0, got {}", area(&merged[0]));
    }

    #[test]
    fn union_two_disjoint_squares_stays_two_loops() {
        let a = vec![Pt2::new(0.0, 0.0), Pt2::new(1.0, 0.0), Pt2::new(1.0, 1.0), Pt2::new(0.0, 1.0)];
        let b = vec![Pt2::new(5.0, 0.0), Pt2::new(6.0, 0.0), Pt2::new(6.0, 1.0), Pt2::new(5.0, 1.0)];
        let merged = union_pieces(&[a, b]);
        assert_eq!(merged.len(), 2, "genuinely disjoint pieces should NOT merge");
    }


    #[test]
    fn area_of_unit_square() {
        let sq = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(1.0, 0.0),
            Pt2::new(1.0, 1.0),
            Pt2::new(0.0, 1.0),
        ];
        assert!((area(&sq) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn point_in_square() {
        let sq = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(2.0, 0.0),
            Pt2::new(2.0, 2.0),
            Pt2::new(0.0, 2.0),
        ];
        assert!(point_in_polygon(Pt2::new(1.0, 1.0), &sq));
        assert!(!point_in_polygon(Pt2::new(3.0, 1.0), &sq));
    }

    #[test]
    fn half_plane_clip_square() {
        // CCW square
        let sq = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(2.0, 0.0),
            Pt2::new(2.0, 2.0),
            Pt2::new(0.0, 2.0),
        ];
        // Line from (1,-1) to (1,3) — left side is x<1
        let clipped = clip_half_plane(&sq, Pt2::new(1.0, -1.0), Pt2::new(1.0, 3.0));
        let a = area(&clipped);
        assert!((a - 2.0).abs() < 1e-6, "clipped area = {a}, expected 2.0");
    }

    #[test]
    fn voronoi_two_sites() {
        let bound = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(10.0, 0.0),
            Pt2::new(10.0, 10.0),
            Pt2::new(0.0, 10.0),
        ];
        let sites = vec![Pt2::new(2.0, 5.0), Pt2::new(8.0, 5.0)];
        let cell_a = voronoi_cell(sites[0], &sites, &bound);
        let cell_b = voronoi_cell(sites[1], &sites, &bound);
        // Each cell should be ~ half the area (50)
        let aa = area(&cell_a);
        let ab = area(&cell_b);
        assert!((aa - 50.0).abs() < 1e-6, "cell A area = {aa}");
        assert!((ab - 50.0).abs() < 1e-6, "cell B area = {ab}");
    }

    #[test]
    fn inset_unit_square() {
        let sq = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(10.0, 0.0),
            Pt2::new(10.0, 10.0),
            Pt2::new(0.0, 10.0),
        ];
        let inset = inset_convex(&sq, 1.0);
        let a = area(&inset);
        // 10x10 inset by 1m = 8x8 = 64
        assert!((a - 64.0).abs() < 0.1, "inset area = {a}");
    }

    #[test]
    fn triangulate_square() {
        let sq = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(10.0, 0.0),
            Pt2::new(10.0, 10.0),
            Pt2::new(0.0, 10.0),
        ];
        let tris = triangulate(&sq);
        assert_eq!(tris.len(), 2, "square → 2 triangles");
        let total: f64 = tris.iter().map(|t| {
            let p = vec![t[0], t[1], t[2]];
            area(&p)
        }).sum();
        assert!((total - 100.0).abs() < 1e-6, "total tri area = {total}");
    }

    #[test]
    fn triangulate_u_shape() {
        // A non-convex U (10x10 with a notch in the top):
        //    .____.   ._____.
        //    |             |
        //    |             |
        //    |_____________|
        //
        // Points (CCW): (0,0)(10,0)(10,10)(7,10)(7,4)(3,4)(3,10)(0,10)
        let u = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(10.0, 0.0),
            Pt2::new(10.0, 10.0),
            Pt2::new(7.0, 10.0),
            Pt2::new(7.0, 4.0),
            Pt2::new(3.0, 4.0),
            Pt2::new(3.0, 10.0),
            Pt2::new(0.0, 10.0),
        ];
        let original_area = area(&u);
        let tris = triangulate(&u);
        assert_eq!(tris.len(), 6, "U-shape with 8 vertices → 6 triangles");
        let total: f64 = tris.iter().map(|t| {
            let p = vec![t[0], t[1], t[2]];
            area(&p)
        }).sum();
        assert!((total - original_area).abs() < 1e-6, "total tri area = {total}, expected {original_area}");
    }

    #[test]
    fn clip_convex_against_u() {
        // A 10x10 square clipped against the U from above should:
        // - Lose the notch
        // - Total clipped area = 100 - 24 (notch is 4 wide x 6 tall) = 76
        let subject = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(10.0, 0.0),
            Pt2::new(10.0, 10.0),
            Pt2::new(0.0, 10.0),
        ];
        let u = vec![
            Pt2::new(0.0, 0.0),
            Pt2::new(10.0, 0.0),
            Pt2::new(10.0, 10.0),
            Pt2::new(7.0, 10.0),
            Pt2::new(7.0, 4.0),
            Pt2::new(3.0, 4.0),
            Pt2::new(3.0, 10.0),
            Pt2::new(0.0, 10.0),
        ];
        let pieces = clip_to_polygon(&subject, &u);
        let total: f64 = pieces.iter().map(|p| area(p)).sum();
        assert!((total - 76.0).abs() < 0.5, "clipped pieces total area = {total}, expected ~76");
    }

    #[test]
    fn subtract_convex_hole_entirely_inside_subject() {
        // 10x10 square minus a centered 4x4 hole -> remaining area 100-16=84.
        let subject = vec![
            Pt2::new(0.0, 0.0), Pt2::new(10.0, 0.0), Pt2::new(10.0, 10.0), Pt2::new(0.0, 10.0),
        ];
        let hole = vec![
            Pt2::new(3.0, 3.0), Pt2::new(7.0, 3.0), Pt2::new(7.0, 7.0), Pt2::new(3.0, 7.0),
        ];
        let pieces = subtract_convex(&subject, &hole);
        let total: f64 = pieces.iter().map(|p| area(p)).sum();
        assert!((total - 84.0).abs() < 0.5, "subtracted area = {total}, expected ~84");
        // Nothing in the remaining pieces should overlap the hole.
        for piece in &pieces {
            let overlap: f64 = clip_to_polygon(&hole, piece).iter().map(|p| area(p)).sum();
            assert!(overlap < 0.5, "remaining piece should not overlap the subtracted hole, got {overlap} m² overlap");
        }
    }

    #[test]
    fn subtract_convex_hole_entirely_outside_subject_is_a_no_op() {
        let subject = vec![
            Pt2::new(0.0, 0.0), Pt2::new(1.0, 0.0), Pt2::new(1.0, 1.0), Pt2::new(0.0, 1.0),
        ];
        let hole = vec![
            Pt2::new(5.0, 5.0), Pt2::new(6.0, 5.0), Pt2::new(6.0, 6.0), Pt2::new(5.0, 6.0),
        ];
        let pieces = subtract_convex(&subject, &hole);
        let total: f64 = pieces.iter().map(|p| area(p)).sum();
        assert!((total - 1.0).abs() < 1e-6, "non-overlapping hole should leave subject fully intact, got {total}");
    }

    #[test]
    fn subtract_convex_hole_straddling_an_edge_removes_only_the_overlap() {
        // 10x10 square minus a hole straddling its right edge (x in [8,12])
        // -> remaining area 100 - (2*4) = 92 (only the x in [8,10] portion,
        // 2 wide x 4 tall, actually overlaps the subject).
        let subject = vec![
            Pt2::new(0.0, 0.0), Pt2::new(10.0, 0.0), Pt2::new(10.0, 10.0), Pt2::new(0.0, 10.0),
        ];
        let hole = vec![
            Pt2::new(8.0, 3.0), Pt2::new(12.0, 3.0), Pt2::new(12.0, 7.0), Pt2::new(8.0, 7.0),
        ];
        let pieces = subtract_convex(&subject, &hole);
        let total: f64 = pieces.iter().map(|p| area(p)).sum();
        assert!((total - 92.0).abs() < 0.5, "subtracted area = {total}, expected ~92");
    }

    #[test]
    fn rect_corridor_has_correct_area_and_is_centered_on_the_segment() {
        let p = Pt2::new(0.0, 0.0);
        let q = Pt2::new(10.0, 0.0);
        let corridor = rect_corridor(p, q, 2.0);
        assert_eq!(corridor.len(), 4);
        let a = area(&corridor);
        assert!((a - 40.0).abs() < 1e-6, "10m segment x 4m width should be 40 m², got {a}");
        // Centered: every vertex should be within 2m of the segment's y=0 line.
        for v in &corridor {
            assert!(v.y.abs() <= 2.0 + 1e-9, "corridor should be centered on the segment, vertex y={}", v.y);
        }
    }

    #[test]
    fn rect_corridor_degenerates_to_empty_for_zero_length_segment() {
        let p = Pt2::new(1.0, 1.0);
        let corridor = rect_corridor(p, p, 2.0);
        assert!(corridor.is_empty());
    }
}
