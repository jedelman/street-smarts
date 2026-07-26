//! Builds a real 3D solid from a `Building` NIR record via constructive SDF
//! (footprint extrusion + P221 aperture cuts) and extracts it to a triangle
//! mesh via `street_smarts_core::surface_nets`.
//!
//! Scope, honestly: massing-level only. This extrudes the real footprint
//! polygon (outer ring minus any real `polygon.holes` -- a courtyard
//! building's actual courtyard void, subtracted the same way a P221
//! opening is) to `height_m`, and punches each real ground-floor `Opening`
//! as a notch at its actual `ring_index`/`t`/`width_m`/`sill_height_m`/
//! `head_height_m`. It does NOT model `wall_thickness_m` as a hollow
//! shell -- there's no interior cavity behind a punch, so this proves the
//! aperture-cut mechanism, not a full walkable interior. `roof` (both
//! `Shed` and `Flat`, the only two shapes any real generator assigns
//! today -- see `RoofExtrusion::height_at_z`) is now consumed: a Shed
//! roof extends the solid's top surface as a sloped plane from
//! `ridge_height_m` at the footprint's south edge down to `eave_height_m`
//! at its north edge (P162's real "slopes down to true north"), and a
//! Flat roof extends it as a flat plane at `ridge_height_m` (the field's
//! own "highest edge" reading -- `p118_roof_garden` carries the old shed
//! ridge/eave through unchanged when it overwrites a roof to `Flat`, so
//! `eave_height_m` isn't a second, independent flat height to pick
//! between). Modeled as solid volume all the way up, same as the walls
//! below it (no hollow attic/loft cavity) -- consistent with this whole
//! struct not modeling any interior voids yet. `roof_segments`,
//! `canopies`, and `wall_niches` are still not consumed.
//! Openings on a hole ring (`on_hole = true` -- a door/window facing INTO
//! a courtyard) are still skipped, not approximated: only the courtyard
//! VOID itself is real here, not its own interior-facing openings.
//! Openings above the ground floor (`floor > 0`) are skipped too:
//! `Building` carries no floor-to-floor height, so there's no real value
//! to place them at.

use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Building, RoofShape};
use street_smarts_core::sdf::{sdf_box, sdf_difference, Vec3};
use street_smarts_core::{extract_surface_nets_bounds, Mesh};
use street_smarts_patterns::planar::{ring_to_local, Pt2};

/// Signed distance to a (possibly non-convex) simple polygon in the local
/// (u, v) plane, plus which edge (`ring_index` into `poly`, i.e. the edge
/// from `poly[i]` to `poly[i+1]`) was nearest. Negative inside, positive
/// outside. Ported from Inigo Quilez's `sdPolygon` (closed-form
/// nearest-edge distance + parity-based inside/outside test via edge
/// crossing) -- chosen because, like Surface Nets, every step is a direct
/// geometric computation with no case table to misremember. The nearest-
/// edge index is a free byproduct of the same loop that already computes
/// the nearest-edge distance -- see `BuildingSolid::sdf`'s own doc for why
/// a caller wants it (bucketing openings by wall, so a real building with
/// hundreds of them doesn't need a linear scan of all of them per sample).
pub(crate) fn sdf_polygon_2d_with_edge(u: f64, v: f64, poly: &[Pt2]) -> (f64, usize) {
    let n = poly.len();
    let v0 = poly[0];
    let mut d2_min = (u - v0.x).powi(2) + (v - v0.y).powi(2);
    let mut nearest_edge = n - 1; // edge (n-1 -> 0), matches j on the first iteration
    let mut s = 1.0f64;
    let mut j = n - 1;
    for i in 0..n {
        let vi = poly[i];
        let vj = poly[j];
        let ex = vj.x - vi.x;
        let ey = vj.y - vi.y;
        let wx = u - vi.x;
        let wy = v - vi.y;
        let ee = ex * ex + ey * ey;
        let t = if ee > 1e-12 { ((wx * ex + wy * ey) / ee).clamp(0.0, 1.0) } else { 0.0 };
        let bx = wx - ex * t;
        let by = wy - ey * t;
        let d2 = bx * bx + by * by;
        if d2 < d2_min {
            d2_min = d2;
            // Edge j->i in this loop's own traversal is the SAME edge
            // `Opening.ring_index = j` describes (poly[j] -> poly[j+1]),
            // since j is always i's immediate predecessor (wrapping).
            nearest_edge = j;
        }

        let c1 = v >= vi.y;
        let c2 = v < vj.y;
        let c3 = ex * wy > ey * wx;
        if (c1 && c2 && c3) || (!c1 && !c2 && !c3) {
            s = -s;
        }
        j = i;
    }
    (s * d2_min.sqrt(), nearest_edge)
}

/// A real roof extruded on top of `BuildingSolid`'s walls, carrying just
/// enough of `RoofForm` to shape the solid's top surface: which shape,
/// the ridge/eave heights, and the footprint's own north/south extent a
/// Shed roof's slope runs across (computed once here, not re-scanned from
/// `footprint` on every `sdf()` call).
struct RoofExtrusion {
    shape: RoofShape,
    ridge_height_m: f64,
    eave_height_m: f64,
    /// Footprint bounding-box Z extent in local meters (increasing Z =
    /// increasing latitude = north, this pipeline's own planar convention
    /// -- see `ring_to_local`). `z_south` is the min (ridge/high side),
    /// `z_north` is the max (eave/low side), matching P162's "slopes down
    /// to true north".
    z_south: f64,
    z_north: f64,
}

impl RoofExtrusion {
    /// The solid's top height at a given local Z. For `Flat`, constant at
    /// `ridge_height_m` across the whole footprint. For `Shed` (and any
    /// other/reserved shape, treated the same as an honest default rather
    /// than silently falling back to a flat top), linearly interpolated
    /// from `ridge_height_m` at `z_south` down to `eave_height_m` at
    /// `z_north`, clamped so samples outside the footprint's own Z extent
    /// (which can't happen for a point already inside the footprint SDF,
    /// but keep the function total) don't extrapolate past either end.
    fn height_at_z(&self, z: f64) -> f64 {
        if self.shape == RoofShape::Flat {
            return self.ridge_height_m;
        }
        let span = self.z_north - self.z_south;
        if span < 1e-9 {
            return self.eave_height_m;
        }
        let t = ((z - self.z_south) / span).clamp(0.0, 1.0);
        self.ridge_height_m + (self.eave_height_m - self.ridge_height_m) * t
    }

    /// The highest point this roof ever reaches -- always `ridge_height_m`
    /// (documented as the roof's own highest edge for every shape), used
    /// to size the Surface Nets extraction grid so the roof isn't clipped.
    fn max_height(&self) -> f64 {
        self.ridge_height_m
    }
}

struct OpeningCut {
    center_u: f64,
    center_w: f64,
    tangent: (f64, f64),
    normal: (f64, f64),
    half_width: f64,
    center_y: f64,
    half_height: f64,
    half_depth: f64,
}

impl OpeningCut {
    fn sdf(&self, p: Vec3) -> f64 {
        let rel_u = p.x - self.center_u;
        let rel_w = p.z - self.center_w;
        let local_u = rel_u * self.tangent.0 + rel_w * self.tangent.1;
        let local_w = rel_u * self.normal.0 + rel_w * self.normal.1;
        let local_v = p.y - self.center_y;
        sdf_box(
            Vec3::new(local_u, local_v, local_w),
            Vec3::new(self.half_width, self.half_height, self.half_depth),
        )
    }
}

/// A real solid built from one `Building`'s footprint, height, and
/// ground-floor openings. `origin` is the WGS84 point local coordinates are
/// projected relative to (shared across a whole neighborhood, so buildings
/// stay correctly positioned relative to each other).
pub struct BuildingSolid {
    footprint: Vec<Pt2>,
    /// Real courtyard/atrium voids (`Building.polygon.holes`) subtracted
    /// from the outer footprint -- a `p107_courtyard_v01` typology
    /// building's `outer` ring traces its FULL perimeter as if solid, with
    /// the actual courtyard carved out via a hole ring, same as any
    /// polygon-with-holes convention. Never consuming this meant a
    /// courtyard building rendered as a solid block filling its own
    /// courtyard -- confirmed against a real device screenshot showing a
    /// real OpenSpace courtyard plaza's ground geometry visually
    /// conflicting with that wrongly-solid fill (`p108_merged_1_building`,
    /// typology `p107_courtyard_v01`, `polygon.holes.len() == 1`, on the
    /// real Military Circle site) -- not a rendering glitch, a real,
    /// specific field this struct just never read.
    holes: Vec<Vec<Pt2>>,
    height: f64,
    openings: Vec<OpeningCut>,
    /// `openings` indices grouped by the wall edge (`ring_index`) they sit
    /// on -- `by_edge[e]` lists every opening on edge `e`. A real merged
    /// block can carry hundreds of openings (P108 Connected Buildings
    /// produces a handful of these on the real Military Circle site, one
    /// with 845); `sdf_polygon_2d_with_edge` already tells `sdf()` which
    /// edge a sample point is nearest, so it only needs to scan THAT
    /// edge's openings (plus its two neighbors, for points near a corner
    /// that are genuinely closest to an opening on the adjoining wall)
    /// instead of every opening on the building -- see `sdf()`'s own doc.
    by_edge: Vec<Vec<usize>>,
    /// `None` for a flat-topped box, same as every fixture predating roof
    /// support (including every synthetic test in this module) -- keeps
    /// the old flat-top behavior exactly for a building whose `roof` field
    /// is unset, rather than fabricating a roof shape nothing assigned.
    roof: Option<RoofExtrusion>,
}

impl BuildingSolid {
    /// Returns `None` if the building has no assigned `height_m` (nothing
    /// to extrude) or a degenerate footprint (< 3 points) -- never
    /// fabricates a placeholder height.
    pub fn from_building(building: &Building, origin: &LngLat) -> Option<Self> {
        let height = building.height_m?;
        if height <= 0.0 {
            return None;
        }
        let footprint = ring_to_local(&building.polygon.outer, origin);
        if footprint.len() < 3 {
            return None;
        }
        let holes: Vec<Vec<Pt2>> = building
            .polygon
            .holes
            .iter()
            .map(|ring| ring_to_local(ring, origin))
            .filter(|hole| hole.len() >= 3)
            .collect();

        let n = footprint.len();
        let mut openings = Vec::new();
        let mut by_edge: Vec<Vec<usize>> = vec![Vec::new(); n];
        for opening in &building.openings {
            if opening.on_hole || opening.floor != 0 {
                continue;
            }
            if opening.ring_index >= n {
                continue;
            }
            let a = footprint[opening.ring_index];
            let b = footprint[(opening.ring_index + 1) % n];
            let ex = b.x - a.x;
            let ey = b.y - a.y;
            let len = (ex * ex + ey * ey).sqrt();
            if len < 1e-9 {
                continue;
            }
            let tangent = (ex / len, ey / len);
            let normal = (-tangent.1, tangent.0);
            let t = opening.t.clamp(0.0, 1.0);
            let center_u = a.x + ex * t;
            let center_w = a.y + ey * t;
            let half_height = ((opening.head_height_m - opening.sill_height_m) / 2.0).abs();
            let center_y = opening.sill_height_m + half_height;
            // Half-depth generous enough to punch fully through any
            // realistic wall even without a resolved wall_thickness_m.
            let half_depth = building.wall_thickness_m.unwrap_or(0.3).max(0.15);

            by_edge[opening.ring_index].push(openings.len());
            openings.push(OpeningCut {
                center_u,
                center_w,
                tangent,
                normal,
                half_width: (opening.width_m / 2.0).max(0.01),
                center_y,
                half_height: half_height.max(0.01),
                half_depth,
            });
        }

        let roof = building.roof.as_ref().map(|r| {
            let mut z_south = f64::MAX;
            let mut z_north = f64::MIN;
            for p in &footprint {
                z_south = z_south.min(p.y);
                z_north = z_north.max(p.y);
            }
            RoofExtrusion {
                shape: r.shape,
                ridge_height_m: r.ridge_height_m,
                eave_height_m: r.eave_height_m,
                z_south,
                z_north,
            }
        });

        Some(Self { footprint, holes, height, openings, by_edge, roof })
    }

    /// The real signed distance at `p` (local meters, Y = height above
    /// ground): negative inside the built solid, positive outside.
    ///
    /// Only scans openings on the wall edge nearest `p` (plus its two
    /// ring-neighbors) rather than every opening on the building --
    /// `sdf_polygon_2d_with_edge`'s nearest-edge result is exactly the
    /// bucket key `by_edge` was built with. Without this, a building with
    /// hundreds of openings (a real, not hypothetical, case -- see
    /// `by_edge`'s own doc) would scan all of them at every one of a
    /// Surface Nets grid's corner samples: measured at 7.8s for one such
    /// real building (845 openings) before this change, on a desktop CPU,
    /// for ONE building out of 35 on the real site.
    pub fn sdf(&self, p: Vec3) -> f64 {
        let (outer_d, nearest_edge) = sdf_polygon_2d_with_edge(p.x, p.z, &self.footprint);
        let mut footprint_d = outer_d;
        for hole in &self.holes {
            let (hole_d, _edge) = sdf_polygon_2d_with_edge(p.x, p.z, hole);
            footprint_d = sdf_difference(footprint_d, hole_d);
        }
        let top_height = self.roof.as_ref().map_or(self.height, |r| r.height_at_z(p.z));
        let slab_d = (-p.y).max(p.y - top_height);
        let solid = footprint_d.max(slab_d);
        if self.openings.is_empty() {
            return solid;
        }

        let n = self.by_edge.len();
        let prev_edge = (nearest_edge + n - 1) % n;
        let next_edge = (nearest_edge + 1) % n;

        let mut opening_min = f64::MAX;
        for &edge in &[prev_edge, nearest_edge, next_edge] {
            for &idx in &self.by_edge[edge] {
                opening_min = opening_min.min(self.openings[idx].sdf(p));
            }
        }
        sdf_difference(solid, opening_min)
    }

    /// Local-meter bounding box, padded by nothing -- callers pad for the
    /// extraction grid.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut min_u = f64::MAX;
        let mut max_u = f64::MIN;
        let mut min_w = f64::MAX;
        let mut max_w = f64::MIN;
        for p in &self.footprint {
            min_u = min_u.min(p.x);
            max_u = max_u.max(p.x);
            min_w = min_w.min(p.y);
            max_w = max_w.max(p.y);
        }
        let max_y = self.roof.as_ref().map_or(self.height, |r| r.max_height());
        (Vec3::new(min_u, 0.0, min_w), Vec3::new(max_u, max_y, max_w))
    }

    /// Extracts a triangle mesh at `voxel_size` meters per cell.
    pub fn to_mesh(&self, voxel_size: f64) -> Mesh {
        let (min, max) = self.bounds();
        extract_surface_nets_bounds(|p| self.sdf(p), min, max, voxel_size)
    }

    /// A voxel size scaled to this building's own footprint, targeting
    /// roughly a fixed cell count across its longest horizontal extent
    /// regardless of the building's real size -- extraction cost is
    /// O(volume / voxel_size^3), so a single fixed voxel size across every
    /// building means a real P108-merged block (measured up to 160m x
    /// 122m on the real Military Circle site) costs on the order of a
    /// THOUSAND times more than a typical ~15m building at the same
    /// resolution, not proportionally more. Clamped to
    /// `[MIN_VOXEL_M, MAX_VOXEL_M]`: never so fine a huge block hangs,
    /// never so coarse a small building loses its openings entirely.
    /// Measured end to end: the real site's biggest building (78m x 110m,
    /// 845 openings) dropped from 3.68s at a flat 0.3m to ~0.2s at its own
    /// adaptive size; the whole real 35-building site meshes in ~4.4s on
    /// a desktop CPU. Coarser cells on the huge blocks is a real,
    /// visible quality tradeoff (their doors/windows read as smaller,
    /// blockier notches) -- the honest alternative to either hanging on
    /// them or silently skipping them.
    pub fn suggested_voxel_size(&self) -> f64 {
        const TARGET_CELLS_ACROSS: f64 = 150.0;
        const MIN_VOXEL_M: f64 = 0.15;
        // 0.5 m, not the 1.5 m this used to be. The smallest real P221
        // opening on the real site is 1.0 m wide x 1.06 m tall (measured,
        // not assumed) -- at a 1.5 m cap the grid cell was LARGER than the
        // feature being cut, so Surface Nets shredded window bands into
        // ragged, half-open tears and produced clusters of
        // inverted-winding triangles right where the cuts were (up to
        // 0.78% of one building's triangles). Measured across the whole
        // real site: dropping the cap 1.5 -> 0.5 takes inverted triangles
        // from 1091 to 178 (6x better) and roughly doubles meshing time
        // (9.2s -> 21.3s on this dev CPU, 1.96M -> 3.26M triangles).
        // Going finer still helps (0.25 m: 179 inverted, 10.4M triangles)
        // but costs 102s, which is not worth it on a phone. Even at 0.5 m
        // a 1 m window is only ~2 cells across, so openings on the biggest
        // P108-merged blocks stay visibly coarse -- the real fix for those
        // is drawing openings as surface panels instead of SDF cuts, so
        // the whole building doesn't have to be voxelized at window
        // resolution. Not attempted here.
        const MAX_VOXEL_M: f64 = 0.5;
        let (min, max) = self.bounds();
        let dx = max.x - min.x;
        let dz = max.z - min.z;
        let diagonal = (dx * dx + dz * dz).sqrt();
        (diagonal / TARGET_CELLS_ACROSS).clamp(MIN_VOXEL_M, MAX_VOXEL_M)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{Opening, OpeningKind, RoofForm, RoofShape};

    fn rect_building(width_m: f64, depth_m: f64, height_m: f64, origin: &LngLat) -> Building {
        // Build a rectangle directly in local meters, then project it BACK
        // to lng/lat around `origin` via the same equirectangular transform
        // `ring_to_local` inverts, so `BuildingSolid::from_building` sees
        // exactly the footprint the test intends, not an approximation.
        use street_smarts_patterns::planar::local_to_ring;
        let local = [
            Pt2::new(0.0, 0.0),
            Pt2::new(width_m, 0.0),
            Pt2::new(width_m, depth_m),
            Pt2::new(0.0, depth_m),
        ];
        let ring = local_to_ring(&local, origin);
        Building {
            id: "B1".into(),
            polygon: Polygon::from_ring(ring),
            height_m: Some(height_m),
            typology: None,
            year_built: None,
            parcel_id: None,
            floors: None,
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: Some(0.3),
            roof: None,
            roof_segments: vec![],
            canopies: vec![],
            wall_niches: vec![],
        }
    }

    fn courtyard_building(outer_side_m: f64, hole_side_m: f64, height_m: f64, origin: &LngLat) -> Building {
        use street_smarts_patterns::planar::local_to_ring;
        let margin = (outer_side_m - hole_side_m) / 2.0;
        let outer_local = [
            Pt2::new(0.0, 0.0),
            Pt2::new(outer_side_m, 0.0),
            Pt2::new(outer_side_m, outer_side_m),
            Pt2::new(0.0, outer_side_m),
        ];
        let hole_local = [
            Pt2::new(margin, margin),
            Pt2::new(margin + hole_side_m, margin),
            Pt2::new(margin + hole_side_m, margin + hole_side_m),
            Pt2::new(margin, margin + hole_side_m),
        ];
        let mut polygon = Polygon::from_ring(local_to_ring(&outer_local, origin));
        polygon.holes = vec![local_to_ring(&hole_local, origin)];
        Building {
            id: "COURTYARD_B1".into(),
            polygon,
            height_m: Some(height_m),
            typology: Some("p107_courtyard_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: None,
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: Some(0.3),
            roof: None,
            roof_segments: vec![],
            canopies: vec![],
            wall_niches: vec![],
        }
    }

    #[test]
    fn shed_roof_slopes_down_from_south_ridge_to_north_eave() {
        let origin = LngLat::new(-76.1, 36.8);
        let mut building = rect_building(10.0, 20.0, 5.0, &origin);
        // eave_height_m == height_m always, per p117_sheltering_roof's real
        // assignment; ridge is 3.0m above it here (the default is 2.0, an
        // arbitrary-but-plausible value works just as well for this test).
        building.roof = Some(RoofForm {
            shape: RoofShape::Shed,
            ridge_height_m: 8.0,
            eave_height_m: 5.0,
            slope_azimuth_deg: 0.0,
            occupiable: false,
        });
        let solid = BuildingSolid::from_building(&building, &origin).expect("valid roofed building");

        // Near the south edge (z=1, close to z_south=0, close to the
        // ridge): a height between eave and ridge should still read solid.
        let near_ridge = Vec3::new(5.0, 7.0, 1.0);
        assert!(solid.sdf(near_ridge) < 0.0, "expected solid near the south ridge edge at y=7.0, got {}", solid.sdf(near_ridge));

        // Same height, near the north edge (z=19, close to z_north=20,
        // close to the eave, which sits at the building's own height_m of
        // 5.0): should read OUTSIDE -- the roof has already sloped down
        // below y=7.0 by here.
        let near_eave = Vec3::new(5.0, 7.0, 19.0);
        assert!(solid.sdf(near_eave) >= 0.0, "expected void near the north eave edge at y=7.0, got {}", solid.sdf(near_eave));

        // The wall base stays solid everywhere regardless of the roof --
        // the slope only affects the top surface.
        let wall_base = Vec3::new(5.0, 1.0, 19.0);
        assert!(solid.sdf(wall_base) < 0.0, "wall base should stay solid under a sloped roof, got {}", solid.sdf(wall_base));

        // The extraction grid must reach the full ridge height, not just
        // the building's own height_m, or the roof gets clipped.
        let (_min, max) = solid.bounds();
        assert!((max.y - 8.0).abs() < 1e-9, "bounds should extend to ridge_height_m (8.0), got {}", max.y);
    }

    #[test]
    fn flat_roof_is_a_flat_plane_at_ridge_height_not_sloped() {
        let origin = LngLat::new(-76.1, 36.8);
        let mut building = rect_building(10.0, 20.0, 5.0, &origin);
        // Mirrors what p118_roof_garden actually does: overwrite a prior
        // shed roof to Flat while carrying its ridge/eave numbers through
        // unchanged, so ridge != eave even though the shape is Flat.
        building.roof = Some(RoofForm {
            shape: RoofShape::Flat,
            ridge_height_m: 10.0,
            eave_height_m: 8.0,
            slope_azimuth_deg: 0.0,
            occupiable: true,
        });
        let solid = BuildingSolid::from_building(&building, &origin).expect("valid roofed building");

        // Unlike a shed roof, a flat roof holds the SAME height (the
        // ridge value) at both the south and north edges -- no slope.
        let south = Vec3::new(5.0, 9.5, 1.0);
        let north = Vec3::new(5.0, 9.5, 19.0);
        assert!(solid.sdf(south) < 0.0, "expected solid near the south edge under a flat roof, got {}", solid.sdf(south));
        assert!(solid.sdf(north) < 0.0, "expected solid near the north edge under a flat roof too (flat, not sloped), got {}", solid.sdf(north));

        // Above the ridge height, everywhere, should read outside.
        let above = Vec3::new(5.0, 10.5, 10.0);
        assert!(solid.sdf(above) >= 0.0, "expected void above ridge_height_m, got {}", solid.sdf(above));

        let (_min, max) = solid.bounds();
        assert!((max.y - 10.0).abs() < 1e-9, "flat roof bounds should extend to ridge_height_m (10.0), got {}", max.y);
    }

    #[test]
    fn courtyard_hole_is_a_real_void_not_a_solid_fill() {
        let origin = LngLat::new(-76.1, 36.8);
        let building = courtyard_building(20.0, 8.0, 4.0, &origin);
        let solid = BuildingSolid::from_building(&building, &origin).expect("valid outer + hole footprint");

        // Dead center of the courtyard (10,10 in local coords, well inside
        // the 8x8 hole centered there): must read OUTSIDE. This is the
        // literal claim the real device screenshot showed failing --
        // a courtyard's own center rendering as solid building mass.
        let courtyard_center = Vec3::new(10.0, 2.0, 10.0);
        let d_courtyard = solid.sdf(courtyard_center);
        assert!(d_courtyard >= 0.0, "courtyard center should be a real void (sdf >= 0), got {d_courtyard}");

        // Between the outer wall and the hole (in the real solid ring):
        // must still read INSIDE.
        let in_the_wall_ring = Vec3::new(3.0, 2.0, 10.0);
        let d_wall = solid.sdf(in_the_wall_ring);
        assert!(d_wall < 0.0, "the real solid ring around the courtyard should stay solid, got {d_wall}");

        // Volume should match outer box minus hole box (both extruded to
        // height), not the outer box alone.
        let mesh = solid.to_mesh(0.2);
        let expected_volume = (20.0 * 20.0 - 8.0 * 8.0) * 4.0;
        let actual_volume = mesh.signed_volume();
        let rel_err = (actual_volume - expected_volume).abs() / expected_volume;
        assert!(
            rel_err < 0.05,
            "courtyard building volume off by {:.1}%: got {actual_volume}, expected {expected_volume}",
            rel_err * 100.0
        );
    }

    #[test]
    fn sdf_polygon_2d_center_is_negative_half_side_for_a_square() {
        let poly = [Pt2::new(0.0, 0.0), Pt2::new(10.0, 0.0), Pt2::new(10.0, 10.0), Pt2::new(0.0, 10.0)];
        let (d, _edge) = sdf_polygon_2d_with_edge(5.0, 5.0, &poly);
        assert!((d - (-5.0)).abs() < 1e-6, "expected -5.0 at square center, got {d}");
    }

    #[test]
    fn sdf_polygon_2d_is_positive_outside() {
        let poly = [Pt2::new(0.0, 0.0), Pt2::new(10.0, 0.0), Pt2::new(10.0, 10.0), Pt2::new(0.0, 10.0)];
        let (d, _edge) = sdf_polygon_2d_with_edge(15.0, 5.0, &poly);
        assert!((d - 5.0).abs() < 1e-6, "expected +5.0 at 5m outside the right edge, got {d}");
    }

    #[test]
    fn sdf_polygon_2d_with_edge_reports_the_edge_the_point_is_actually_nearest() {
        // Square with edges: 0=(0,0)->(10,0) [south], 1=(10,0)->(10,10) [east],
        // 2=(10,10)->(0,10) [north], 3=(0,10)->(0,0) [west].
        let poly = [Pt2::new(0.0, 0.0), Pt2::new(10.0, 0.0), Pt2::new(10.0, 10.0), Pt2::new(0.0, 10.0)];
        let (_d, edge) = sdf_polygon_2d_with_edge(5.0, 0.2, &poly);
        assert_eq!(edge, 0, "a point just inside the south wall should be nearest edge 0");
        let (_d, edge) = sdf_polygon_2d_with_edge(9.8, 5.0, &poly);
        assert_eq!(edge, 1, "a point just inside the east wall should be nearest edge 1");
    }

    #[test]
    fn suggested_voxel_size_clamps_small_buildings_to_the_min_and_scales_huge_ones() {
        let origin = LngLat::new(-76.1, 36.8);
        let small = BuildingSolid::from_building(&rect_building(10.0, 6.0, 3.0, &origin), &origin).unwrap();
        assert_eq!(small.suggested_voxel_size(), 0.15, "a small building's diagonal/150 is well under the min clamp");

        // A mid-size building lands strictly between the clamps and scales
        // with its own diagonal: sqrt(40^2+30^2) = 50; /150 = 0.333.
        let mid = BuildingSolid::from_building(&rect_building(40.0, 30.0, 15.0, &origin), &origin).unwrap();
        let mid_voxel = mid.suggested_voxel_size();
        assert!(mid_voxel > 0.15 && mid_voxel < 0.5, "a mid-size building should land between the clamps, got {mid_voxel}");
        assert!((mid_voxel - 0.333).abs() < 0.01, "expected ~0.333, got {mid_voxel}");

        // A huge P108-merged-scale block saturates the max clamp. That
        // ceiling is deliberately below the smallest real P221 opening
        // (1.0 m wide) so a window is never smaller than the grid cell
        // cutting it -- see suggested_voxel_size's own doc for the
        // measured tearing/inverted-winding this prevents.
        let huge = BuildingSolid::from_building(&rect_building(160.0, 120.0, 15.0, &origin), &origin).unwrap();
        let voxel = huge.suggested_voxel_size();
        assert_eq!(voxel, 0.5, "a huge building should saturate the max clamp, got {voxel}");
        assert!(voxel < 1.0, "the voxel ceiling must stay under the smallest real opening width (1.0 m)");
    }

    #[test]
    fn solid_rectangular_massing_has_the_right_volume() {
        let origin = LngLat::new(-76.1, 36.8);
        let building = rect_building(10.0, 6.0, 3.0, &origin);
        let solid = BuildingSolid::from_building(&building, &origin).expect("building has height + footprint");
        let mesh = solid.to_mesh(0.25);
        let expected = 10.0 * 6.0 * 3.0;
        let actual = mesh.signed_volume();
        let rel_err = (actual - expected).abs() / expected;
        assert!(rel_err < 0.05, "massing volume off by {:.1}%: got {actual}, expected {expected}", rel_err * 100.0);
    }

    #[test]
    fn a_real_p221_opening_punches_a_real_hole_at_its_real_position() {
        let origin = LngLat::new(-76.1, 36.8);
        let mut building = rect_building(10.0, 6.0, 3.0, &origin);
        // Door on ring edge 0 ((0,0)->(10,0)), centered (t=0.5), 1m wide, 0-2.1m tall.
        building.openings.push(Opening {
            kind: OpeningKind::Door,
            ring_index: 0,
            on_hole: false,
            t: 0.5,
            width_m: 1.0,
            sill_height_m: 0.0,
            head_height_m: 2.1,
            floor: 0,
        });
        let solid = BuildingSolid::from_building(&building, &origin).expect("building has height + footprint");

        // Center of the door, at wall depth: should read OUTSIDE (punched through).
        let at_door = Vec3::new(5.0, 1.0, 0.0);
        let d_door = solid.sdf(at_door);
        assert!(d_door >= 0.0, "expected the door position to be punched through (sdf >= 0), got {d_door}");

        // Same height, same wall, well away from the door: should still read SOLID.
        let away_from_door = Vec3::new(1.0, 1.0, 0.0);
        let d_wall = solid.sdf(away_from_door);
        assert!(d_wall < 0.0, "expected the wall away from the door to stay solid, got {d_wall}");

        // The opening should measurably reduce enclosed volume vs. a windowless twin.
        let blank = rect_building(10.0, 6.0, 3.0, &origin);
        let blank_solid = BuildingSolid::from_building(&blank, &origin).unwrap();
        let v_with_door = solid.to_mesh(0.2).signed_volume();
        let v_blank = blank_solid.to_mesh(0.2).signed_volume();
        assert!(
            v_with_door < v_blank - 0.1,
            "expected the door to carve out measurable volume: with_door={v_with_door}, blank={v_blank}"
        );
    }
}

/// A building's 2D footprint (outer ring minus courtyard holes) kept
/// around after meshing purely for walk-mode collision queries.
///
/// Deliberately NOT a Godot physics body. The obvious route --
/// `MeshInstance3D::create_trimesh_collision()` on the generated massing --
/// would hand the physics server the full extracted mesh: 3.26M triangles
/// site-wide, roughly 117 MB of face data before its BVH, rebuilt every
/// time the pipeline re-runs. For a walker pinned to the ground plane none
/// of that 3D detail is reachable: what actually matters is the 2D
/// question "is this (x, z) inside a building?", which the real footprint
/// polygon answers exactly, at a few hundred bytes per building, with no
/// voxelization error. Godot's physics engine is the right tool the moment
/// anything needs real 3D contact (falling, stairs, thrown objects); it
/// isn't the right tool for this.
pub struct FootprintCollider {
    outer: Vec<Pt2>,
    holes: Vec<Vec<Pt2>>,
    min_x: f64,
    max_x: f64,
    min_z: f64,
    max_z: f64,
}

impl FootprintCollider {
    pub fn from_building(building: &Building, origin: &LngLat) -> Option<Self> {
        let outer = ring_to_local(&building.polygon.outer, origin);
        if outer.len() < 3 {
            return None;
        }
        let holes: Vec<Vec<Pt2>> = building
            .polygon
            .holes
            .iter()
            .map(|r| ring_to_local(r, origin))
            .filter(|h| h.len() >= 3)
            .collect();
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_z = f64::MAX;
        let mut max_z = f64::MIN;
        for p in &outer {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_z = min_z.min(p.y);
            max_z = max_z.max(p.y);
        }
        Some(Self { outer, holes, min_x, max_x, min_z, max_z })
    }

    /// Signed distance to this building's footprint in the ground plane:
    /// negative inside the solid mass, positive outside it (and positive
    /// inside a courtyard, which is real walkable ground).
    pub fn distance(&self, x: f64, z: f64) -> f64 {
        // Cheap bbox reject -- outside the box, the true distance is at
        // least the box distance, which is all a caller comparing against
        // a small body radius needs.
        if x < self.min_x || x > self.max_x || z < self.min_z || z > self.max_z {
            let dx = (self.min_x - x).max(0.0).max(x - self.max_x);
            let dz = (self.min_z - z).max(0.0).max(z - self.max_z);
            return (dx * dx + dz * dz).sqrt();
        }
        let (mut d, _edge) = sdf_polygon_2d_with_edge(x, z, &self.outer);
        for hole in &self.holes {
            let (hd, _e) = sdf_polygon_2d_with_edge(x, z, hole);
            d = sdf_difference(d, hd);
        }
        d
    }
}
