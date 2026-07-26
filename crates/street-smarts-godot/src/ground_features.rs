//! Flat ground-level geometry for `Street` and `OpenSpace` NIR records --
//! plazas, commons, and street ribbons. Both are real data the pattern
//! pipeline already produces (verified against the real Military Circle
//! site: 96 open spaces, 9 streets) that nothing rendered before this.
//!
//! Deliberately NOT built on `building_mesh`'s SDF + Surface Nets pipeline,
//! despite that being the first approach tried here: a uniform-grid
//! extractor needs its solid's thinnest dimension to span at least ~1
//! voxel cell, and a large, genuinely-flat open space (real footprints on
//! the real site range up to a 195m bounding diagonal) forces a voxel
//! size far coarser than any honest "thin pad" thickness -- the solid
//! region ends up entirely inside one cell, no sample point ever lands
//! strictly inside it, and Surface Nets silently extracts nothing. Caught
//! by this module's own tests (0 volume, not almost-right), not by
//! inspection. A flat 2D shape doesn't need volumetric extraction at all;
//! it needs a 2D triangulation, which is what this module actually does.

use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{OpenSpace, Parcel, Street};
use street_smarts_core::sdf::Vec3;
use street_smarts_core::Mesh;
use street_smarts_patterns::planar::{lnglat_to_local, ring_to_local, Pt2};

/// Height ground features sit at -- a hair above the buildings' own y=0
/// base, not exactly on it, so a plaza sharing an edge with a building
/// footprint doesn't coplanar-z-fight with it.
const GROUND_Y_M: f64 = 0.02;
const DEFAULT_ROW_WIDTH_M: f64 = 6.0;

/// A flat, double-sided (visible from above or below -- this is ground
/// decoration, not a solid whose backface culling needs to be correct)
/// polygon at a fixed height: one plaza, one common, or one street
/// segment's ribbon.
pub struct FlatPolygon {
    footprint: Vec<Pt2>,
}

impl FlatPolygon {
    pub fn to_mesh(&self) -> Mesh {
        let triangles_2d = triangulate_ear_clipping(&self.footprint);
        let positions: Vec<Vec3> = self.footprint.iter().map(|p| Vec3::new(p.x, GROUND_Y_M, p.y)).collect();
        // Placeholder -- the caller (street-smarts-godot::mesh_to_instance)
        // computes its own flat per-triangle face normals from positions
        // and never reads this field; kept populated (not empty) purely to
        // satisfy Mesh's own invariant that normals.len() == positions.len().
        let normals = vec![Vec3::new(0.0, 1.0, 0.0); positions.len()];

        // Single winding, not both: two coincident triangles at the exact
        // same 3 positions (the earlier approach, to be visible from
        // underneath too) is exactly the setup for z-fighting -- the
        // renderer has no principled way to pick which of two identical
        // triangles is "in front", and can flicker between the correctly-
        // lit one and its backwards-normal twin. street-smarts-godot's
        // rebuild_3d_mesh() disables backface culling on the open-space/
        // street materials instead, so this single winding stays visible
        // from either side without a duplicate.
        let triangles = triangles_2d.iter().map(|t| [t[0] as u32, t[1] as u32, t[2] as u32]).collect();
        Mesh { positions, normals, triangles }
    }
}

/// Builds a flat polygon for one raw `Parcel` record -- the pre-building
/// pad/block fabric a pattern operator hasn't consumed (subdivided into
/// blocks, built on, or replaced) yet. Real gap this closes: once a
/// pattern operator DOES consume a parcel, `apply_subdivision` removes it
/// from `Neighborhood.parcels` (see `replaced_parcel_ids`), so a still-
/// present parcel is, by construction, real ground nothing else has been
/// generated on top of yet -- there is no double-rendering/z-fighting
/// concern with `open_space_polygon`/building massing sharing the same
/// footprint, because a parcel and whatever eventually replaces it are
/// never both present at once. Without this, the interactive pattern-
/// stepper UI (`apply_pattern`) had nothing to actually show for the
/// early, pre-building pipeline stages (P29/P37/PathNetwork/P95 all run
/// on raw parcels) -- the site would render as an empty void until the
/// first building or open space appeared.
pub fn parcel_polygon(parcel: &Parcel, origin: &LngLat) -> Option<FlatPolygon> {
    let footprint = ring_to_local(&parcel.polygon.outer, origin);
    if footprint.len() < 3 {
        return None;
    }
    Some(FlatPolygon { footprint })
}

/// Builds a flat polygon for one `OpenSpace` record (plaza/common/etc),
/// using its outer ring only -- holes and multi-part footprints
/// (`Polygon.holes` / `.parts`) are real fields this doesn't consume yet,
/// the same "outer ring only" scope `building_mesh.rs` already has for
/// `Building.polygon`.
pub fn open_space_polygon(open_space: &OpenSpace, origin: &LngLat) -> Option<FlatPolygon> {
    let footprint = ring_to_local(&open_space.polygon.outer, origin);
    if footprint.len() < 3 {
        return None;
    }
    Some(FlatPolygon { footprint })
}

/// Builds one flat ribbon polygon per centerline segment of a `Street`,
/// each a simple rectangle of width `row_width_m` (or
/// `DEFAULT_ROW_WIDTH_M` if unset) centered on that segment. Deliberately
/// NOT a single mitered polygon along the whole centerline -- segments
/// meet with small gaps/overlaps at corners instead of a proper joined
/// offset curve. A real, honest limitation (visible at sharp turns), not
/// an oversight.
pub fn street_ribbon_segments(street: &Street, origin: &LngLat) -> Vec<FlatPolygon> {
    let half_width = street.row_width_m.unwrap_or(DEFAULT_ROW_WIDTH_M).max(0.5) / 2.0;
    let points: Vec<Pt2> = street.centerline.iter().map(|p| lnglat_to_local(p, origin)).collect();

    let mut ribbons = Vec::new();
    for pair in points.windows(2) {
        let a = pair[0];
        let b = pair[1];
        let ex = b.x - a.x;
        let ey = b.y - a.y;
        let len = (ex * ex + ey * ey).sqrt();
        if len < 1e-6 {
            continue;
        }
        // Perpendicular to the segment, scaled to half_width.
        let nx = -ey / len * half_width;
        let ny = ex / len * half_width;
        let footprint = vec![
            Pt2::new(a.x + nx, a.y + ny),
            Pt2::new(b.x + nx, b.y + ny),
            Pt2::new(b.x - nx, b.y - ny),
            Pt2::new(a.x - nx, a.y - ny),
        ];
        ribbons.push(FlatPolygon { footprint });
    }
    ribbons
}

/// Ear-clipping triangulation of a simple (non-self-intersecting), hole-
/// free polygon -- correct for non-convex input (verified by this
/// module's own tests via total triangulated area vs. the shoelace-
/// formula area of a deliberately non-convex L-shape, not just a square),
/// which real `OpenSpace`/street-ribbon footprints aren't guaranteed to
/// avoid. Indices are into `poly`, ordered CCW internally regardless of
/// the input's own winding (`to_mesh` above emits both windings anyway,
/// so this doesn't need to match any particular caller convention).
fn triangulate_ear_clipping(poly: &[Pt2]) -> Vec<[usize; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    if n == 3 {
        return vec![[0, 1, 2]];
    }

    // Ear-clipping's convexity test assumes CCW winding; flip the working
    // order if the input is CW (signed area negative) rather than
    // duplicating/mutating the caller's own point order.
    let mut indices: Vec<usize> = if signed_area2(poly) >= 0.0 { (0..n).collect() } else { (0..n).rev().collect() };

    let mut triangles = Vec::with_capacity(n - 2);
    let mut stalled_passes = 0;
    while indices.len() > 3 {
        let m = indices.len();
        let mut clipped_this_pass = false;
        let mut i = 0;
        while i < m && indices.len() > 3 {
            let count = indices.len();
            let prev = indices[(i + count - 1) % count];
            let cur = indices[i % count];
            let next = indices[(i + 1) % count];
            if is_convex_ccw(poly[prev], poly[cur], poly[next]) && !any_other_point_inside(poly, &indices, prev, cur, next) {
                triangles.push([prev, cur, next]);
                indices.remove(i % count);
                clipped_this_pass = true;
            } else {
                i += 1;
            }
        }
        if !clipped_this_pass {
            stalled_passes += 1;
            if stalled_passes > 2 {
                // Degenerate/self-intersecting input: stop rather than
                // loop forever: whatever's been clipped so far is still a
                // valid partial triangulation.
                break;
            }
        }
    }
    if indices.len() == 3 {
        triangles.push([indices[0], indices[1], indices[2]]);
    }
    triangles
}

fn signed_area2(poly: &[Pt2]) -> f64 {
    let n = poly.len();
    let mut sum = 0.0;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        sum += a.x * b.y - b.x * a.y;
    }
    sum
}

fn is_convex_ccw(prev: Pt2, cur: Pt2, next: Pt2) -> bool {
    cross_z(prev, cur, next) > 1e-12
}

/// Z-component of (b-a) x (p-a): positive when `p` is left of the
/// directed line a->b (standard CCW orientation test).
fn cross_z(a: Pt2, b: Pt2, p: Pt2) -> f64 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

fn point_in_triangle(p: Pt2, a: Pt2, b: Pt2, c: Pt2) -> bool {
    let d1 = cross_z(a, b, p);
    let d2 = cross_z(b, c, p);
    let d3 = cross_z(c, a, p);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

fn any_other_point_inside(poly: &[Pt2], indices: &[usize], prev: usize, cur: usize, next: usize) -> bool {
    for &idx in indices {
        if idx == prev || idx == cur || idx == next {
            continue;
        }
        if point_in_triangle(poly[idx], poly[prev], poly[cur], poly[next]) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat as GeomLngLat, Polygon};
    use street_smarts_patterns::planar::{local_to_lnglat, local_to_ring};

    fn triangulated_area(poly: &[Pt2]) -> f64 {
        triangulate_ear_clipping(poly)
            .iter()
            .map(|t| {
                let (a, b, c) = (poly[t[0]], poly[t[1]], poly[t[2]]);
                (cross_z(a, b, c)).abs() / 2.0
            })
            .sum()
    }

    fn polygon_area(poly: &[Pt2]) -> f64 {
        signed_area2(poly).abs() / 2.0
    }

    #[test]
    fn ear_clipping_covers_a_convex_square_exactly() {
        let square = [Pt2::new(0.0, 0.0), Pt2::new(10.0, 0.0), Pt2::new(10.0, 10.0), Pt2::new(0.0, 10.0)];
        let tris = triangulate_ear_clipping(&square);
        assert_eq!(tris.len(), 2, "a quad should clip into exactly 2 triangles");
        assert!((triangulated_area(&square) - polygon_area(&square)).abs() < 1e-6);
    }

    #[test]
    fn ear_clipping_covers_a_non_convex_l_shape_exactly() {
        // L-shape: a 10x10 square with a 5x5 notch bitten out of one corner.
        let l_shape = [
            Pt2::new(0.0, 0.0),
            Pt2::new(10.0, 0.0),
            Pt2::new(10.0, 5.0),
            Pt2::new(5.0, 5.0),
            Pt2::new(5.0, 10.0),
            Pt2::new(0.0, 10.0),
        ];
        let expected_area = 100.0 - 25.0; // 10x10 minus the 5x5 notch
        assert!((polygon_area(&l_shape) - expected_area).abs() < 1e-6, "sanity check on the shoelace formula itself");
        let tris = triangulate_ear_clipping(&l_shape);
        assert_eq!(tris.len(), 4, "a 6-vertex simple polygon should clip into exactly 4 triangles");
        assert!(
            (triangulated_area(&l_shape) - expected_area).abs() < 1e-6,
            "triangulated area {} should match the polygon's own shoelace area {}",
            triangulated_area(&l_shape),
            expected_area
        );
    }

    #[test]
    fn ear_clipping_handles_clockwise_input_the_same_as_counterclockwise() {
        let ccw = [Pt2::new(0.0, 0.0), Pt2::new(10.0, 0.0), Pt2::new(10.0, 10.0), Pt2::new(0.0, 10.0)];
        let cw: Vec<Pt2> = ccw.iter().rev().cloned().collect();
        assert!((triangulated_area(&ccw) - triangulated_area(&cw)).abs() < 1e-6);
    }

    #[test]
    fn parcel_polygon_produces_a_mesh_covering_the_real_footprint_area() {
        let origin = GeomLngLat::new(-76.1, 36.8);
        let local = [Pt2::new(0.0, 0.0), Pt2::new(20.0, 0.0), Pt2::new(20.0, 10.0), Pt2::new(0.0, 10.0)];
        let ring = local_to_ring(&local, &origin);
        let parcel = Parcel {
            id: "BLOCK_0".into(),
            polygon: Polygon::from_ring(ring),
            area_acres: 0.0,
            use_category: None,
            ownership: None,
            is_eda: false,
            spec: Some("BLOCK_0".into()),
            density_tier: None,
            target_stories: None,
        };
        let flat = parcel_polygon(&parcel, &origin).expect("valid quad footprint");
        let mesh = flat.to_mesh();
        assert_eq!(mesh.triangles.len(), 2, "a quad footprint should ear-clip into exactly 2 triangles");
        let front_face_area: f64 = mesh.triangles
            .iter()
            .map(|t| {
                let (a, b, c) = (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
                let e1 = (b.x - a.x, b.z - a.z);
                let e2 = (c.x - a.x, c.z - a.z);
                (e1.0 * e2.1 - e1.1 * e2.0).abs() / 2.0
            })
            .sum();
        assert!((front_face_area - 200.0).abs() < 1e-6, "20x10 parcel should cover 200 sq m, got {front_face_area}");
    }

    #[test]
    fn open_space_polygon_produces_a_mesh_covering_the_real_footprint_area() {
        let origin = GeomLngLat::new(-76.1, 36.8);
        let local = [Pt2::new(0.0, 0.0), Pt2::new(20.0, 0.0), Pt2::new(20.0, 10.0), Pt2::new(0.0, 10.0)];
        let ring = local_to_ring(&local, &origin);
        let open_space = OpenSpace {
            id: "PLAZA_1".into(),
            polygon: Polygon::from_ring(ring),
            kind: street_smarts_core::nir::OpenSpaceKind::Plaza,
        };
        let flat = open_space_polygon(&open_space, &origin).expect("valid quad footprint");
        let mesh = flat.to_mesh();
        assert_eq!(mesh.triangles.len(), 2, "a quad footprint should ear-clip into exactly 2 triangles");
        let front_face_area: f64 = mesh.triangles
            .iter()
            .map(|t| {
                let (a, b, c) = (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
                let e1 = (b.x - a.x, b.z - a.z);
                let e2 = (c.x - a.x, c.z - a.z);
                (e1.0 * e2.1 - e1.1 * e2.0).abs() / 2.0
            })
            .sum();
        assert!((front_face_area - 200.0).abs() < 1e-6, "20x10 plaza should cover 200 sq m, got {front_face_area}");
    }

    #[test]
    fn street_ribbon_produces_one_polygon_per_segment_at_the_right_area() {
        let origin = GeomLngLat::new(-76.1, 36.8);
        let local = [Pt2::new(0.0, 0.0), Pt2::new(30.0, 0.0), Pt2::new(30.0, 20.0)];
        let street = Street {
            id: "S1".into(),
            centerline: local.iter().map(|p| local_to_lnglat(*p, &origin)).collect(),
            classification: None,
            row_width_m: Some(8.0),
            surface: None,
        };
        let ribbons = street_ribbon_segments(&street, &origin);
        assert_eq!(ribbons.len(), 2, "3 centerline points should produce 2 segments");
        for ribbon in &ribbons {
            let mesh = ribbon.to_mesh();
            assert_eq!(mesh.triangles.len(), 2, "each rectangular segment should ear-clip into 2 triangles");
        }
    }
}
