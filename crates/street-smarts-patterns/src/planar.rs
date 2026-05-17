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

/// Clip a convex polygon (e.g. a Voronoi cell) against a non-convex polygon
/// boundary using the Weiler-Atherton-style intersection.
///
/// For our use case the Voronoi cell is convex and the parcel boundary is
/// arbitrary. We approximate this by clipping the convex cell against each
/// edge of the parcel boundary using half-plane clipping. This is correct
/// when the parcel boundary is convex; for non-convex parcels the result
/// is a CONVEX subset of the true intersection (we lose any concave
/// "fingers" of the cell that reach into concavities of the parcel).
///
/// For neighborhood-scale parcel shapes this is acceptable for v0.1.
/// Marked with a clear caveat.
pub fn clip_convex_to_polygon(convex_cell: &[Pt2], polygon: &[Pt2]) -> Vec<Pt2> {
    if convex_cell.is_empty() || polygon.len() < 3 { return vec![]; }
    // Ensure polygon is CCW for the half-plane semantics to keep the interior.
    let signed = {
        let n = polygon.len();
        let mut s = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            s += polygon[i].x * polygon[j].y - polygon[j].x * polygon[i].y;
        }
        s
    };
    let poly: Vec<Pt2> = if signed > 0.0 {
        polygon.to_vec()
    } else {
        polygon.iter().rev().copied().collect()
    };

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
