//! Geometric primitives. Coordinates are WGS84 lng/lat unless otherwise documented.
//! For metric operations (areas, distances), project to a local UTM frame
//! using `geometry::project_to_local_meters`.

use serde::{Deserialize, Serialize};

/// A WGS84 longitude/latitude pair.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LngLat {
    pub lng: f64,
    pub lat: f64,
}

impl LngLat {
    pub fn new(lng: f64, lat: f64) -> Self {
        Self { lng, lat }
    }
}

/// A linear ring of WGS84 points. By convention, first point == last point.
pub type Ring = Vec<LngLat>;

/// A polygon. May be a single ring, a ring-with-holes, or a multi-polygon
/// (several disjoint parts under one record — e.g. MALL_CORE has two: the
/// mall footprint plus its detached parking annex).
///
/// `outer` and `holes` mirror `parts[0]` for backward compatibility with the
/// existing opinion code that only needs a single representative shape.
/// New code should iterate `parts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polygon {
    pub outer: Ring,
    #[serde(default)]
    pub holes: Vec<Ring>,
    /// All disjoint parts. May be empty in legacy fixtures; in that case
    /// callers should fall back to `outer`/`holes`. `parts[0].outer` always
    /// equals `outer` when non-empty.
    #[serde(default)]
    pub parts: Vec<PolygonPart>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolygonPart {
    pub outer: Ring,
    #[serde(default)]
    pub holes: Vec<Ring>,
}

impl Polygon {
    /// Single-ring polygon (no holes, single part).
    pub fn from_ring(outer: Ring) -> Self {
        Self {
            parts: vec![PolygonPart { outer: outer.clone(), holes: vec![] }],
            outer,
            holes: vec![],
        }
    }

    /// Multi-part polygon. Sorts parts by area descending; mirrors the largest
    /// into `outer`/`holes`.
    pub fn from_parts(mut parts: Vec<PolygonPart>) -> Self {
        if parts.is_empty() {
            return Self { outer: vec![], holes: vec![], parts: vec![] };
        }
        parts.sort_by(|a, b| {
            let area_a = ring_abs_area(&a.outer);
            let area_b = ring_abs_area(&b.outer);
            area_b.partial_cmp(&area_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        let primary = parts[0].clone();
        Self { outer: primary.outer, holes: primary.holes, parts }
    }

    /// Returns parts; if the polygon was deserialized from a legacy fixture
    /// without `parts`, synthesizes a one-element view from `outer`/`holes`.
    pub fn parts_view(&self) -> Vec<PolygonPart> {
        if !self.parts.is_empty() {
            self.parts.clone()
        } else {
            vec![PolygonPart { outer: self.outer.clone(), holes: self.holes.clone() }]
        }
    }
    /// Approximate centroid in WGS84. Good enough for fixture-scale neighborhoods;
    /// for metric work, project first.
    pub fn centroid(&self) -> LngLat {
        let n = self.outer.len();
        if n == 0 {
            return LngLat::new(0.0, 0.0);
        }
        let (lng_sum, lat_sum) = self.outer.iter().fold((0.0_f64, 0.0_f64), |(a, b), p| {
            (a + p.lng, b + p.lat)
        });
        LngLat::new(lng_sum / n as f64, lat_sum / n as f64)
    }

    /// Approximate area in square meters using equirectangular projection
    /// centered on the polygon's centroid. Sums all `parts` (so multi-polygon
    /// parcels report their full extent). Subtracts holes within each part.
    pub fn area_m2(&self) -> f64 {
        let parts = self.parts_view();
        if parts.is_empty() {
            return 0.0;
        }
        let c = self.centroid();
        let mlat = (c.lat * std::f64::consts::PI / 180.0).cos();
        let project = |p: &LngLat| -> (f64, f64) {
            let x = (p.lng - c.lng) * mlat * 111_320.0;
            let y = (p.lat - c.lat) * 110_540.0;
            (x, y)
        };
        let mut total = 0.0;
        for part in &parts {
            let outer = shoelace(&part.outer, &project).abs();
            let inner: f64 = part.holes.iter().map(|h| shoelace(h, &project).abs()).sum();
            total += (outer - inner).max(0.0);
        }
        total
    }

    /// Approximate perimeter in meters (outer ring only).
    pub fn perimeter_m(&self) -> f64 {
        if self.outer.len() < 2 {
            return 0.0;
        }
        let c = self.centroid();
        let mlat = (c.lat * std::f64::consts::PI / 180.0).cos();
        let mut sum = 0.0;
        for w in self.outer.windows(2) {
            let dx = (w[1].lng - w[0].lng) * mlat * 111_320.0;
            let dy = (w[1].lat - w[0].lat) * 110_540.0;
            sum += (dx * dx + dy * dy).sqrt();
        }
        sum
    }
}

fn shoelace(ring: &[LngLat], project: &dyn Fn(&LngLat) -> (f64, f64)) -> f64 {
    let mut sum = 0.0;
    let n = ring.len();
    if n < 3 {
        return 0.0;
    }
    for i in 0..n {
        let (x1, y1) = project(&ring[i]);
        let (x2, y2) = project(&ring[(i + 1) % n]);
        sum += x1 * y2 - x2 * y1;
    }
    sum * 0.5
}

/// Coarse, projection-free ring area for sorting (lng/lat units squared,
/// fine for ordering by magnitude).
fn ring_abs_area(ring: &[LngLat]) -> f64 {
    let n = ring.len();
    if n < 3 { return 0.0; }
    let mut s = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        s += ring[i].lng * ring[j].lat - ring[j].lng * ring[i].lat;
    }
    (s * 0.5).abs()
}

/// Approximate haversine distance between two lng/lat points, in meters.
pub fn haversine_m(a: &LngLat, b: &LngLat) -> f64 {
    const R: f64 = 6_371_000.0;
    let lat1 = a.lat.to_radians();
    let lat2 = b.lat.to_radians();
    let dlat = (b.lat - a.lat).to_radians();
    let dlng = (b.lng - a.lng).to_radians();
    let h = (dlat / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * (dlng / 2.0).sin().powi(2);
    2.0 * R * h.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_square_area() {
        // Roughly 1 m on each side near the equator
        let m_per_deg = 111_320.0;
        let d = 1.0 / m_per_deg;
        let ring = vec![
            LngLat::new(0.0, 0.0),
            LngLat::new(d, 0.0),
            LngLat::new(d, d),
            LngLat::new(0.0, d),
            LngLat::new(0.0, 0.0),
        ];
        let poly = Polygon::from_ring(ring);
        let area = poly.area_m2();
        assert!((area - 1.0).abs() < 0.05, "area was {area}");
    }

    #[test]
    fn multi_part_area_sums() {
        let m_per_deg = 111_320.0;
        let d = 1.0 / m_per_deg;
        // Two disjoint 1m squares
        let p1 = PolygonPart {
            outer: vec![
                LngLat::new(0.0, 0.0), LngLat::new(d, 0.0),
                LngLat::new(d, d), LngLat::new(0.0, d),
                LngLat::new(0.0, 0.0),
            ],
            holes: vec![],
        };
        let p2 = PolygonPart {
            outer: vec![
                LngLat::new(10.0 * d, 0.0), LngLat::new(11.0 * d, 0.0),
                LngLat::new(11.0 * d, d), LngLat::new(10.0 * d, d),
                LngLat::new(10.0 * d, 0.0),
            ],
            holes: vec![],
        };
        let poly = Polygon::from_parts(vec![p1, p2]);
        let area = poly.area_m2();
        // Two 1m² squares = ~2m². Allow generous slack for projection imprecision.
        assert!((area - 2.0).abs() < 0.2, "area was {area}, expected ~2.0");
    }
}
