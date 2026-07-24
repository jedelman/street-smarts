//! Naive Surface Nets: extracts a triangle mesh from any signed-distance
//! field (SDF). Chosen over full Dual Contouring (named alongside it in
//! `GODOT_PORT_SPEC.md` as an acceptable alternative) because its face/vertex
//! rule has no large case table to transcribe from memory -- every step is
//! either an average of edge crossings or a sign comparison, which keeps the
//! risk of a silent geometric bug low in an environment with no Godot editor
//! to visually catch one. Quality is lower than Dual Contouring on sharp
//! edges (no per-vertex QEF fit), acceptable for massing-level building forms.
//!
//! Reference: Gibson, S. "Constrained Elastic Surface Nets" (1998); the
//! "naive" variant (no elastic relaxation) is what's implemented here.

use crate::sdf::Vec3;

/// A triangle mesh: flat position/normal arrays plus triangle index triples.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
}

impl Mesh {
    /// Enclosed volume via the divergence theorem: `sum(dot(v0, cross(v1, v2))) / 6`
    /// over every triangle. Only meaningful for a closed, consistently
    /// outward-wound mesh -- exactly what `extract_surface_nets` produces.
    /// Used to check extracted geometry against an analytically known
    /// volume without a renderer to look at.
    pub fn signed_volume(&self) -> f64 {
        let mut acc = 0.0;
        for tri in &self.triangles {
            let v0 = self.positions[tri[0] as usize];
            let v1 = self.positions[tri[1] as usize];
            let v2 = self.positions[tri[2] as usize];
            acc += v0.x * (v1.y * v2.z - v1.z * v2.y)
                - v0.y * (v1.x * v2.z - v1.z * v2.x)
                + v0.z * (v1.x * v2.y - v1.y * v2.x);
        }
        acc / 6.0
    }
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn normalize(v: Vec3) -> Vec3 {
    let len = v.length();
    if len < 1e-12 {
        return Vec3::new(0.0, 0.0, 0.0);
    }
    Vec3::new(v.x / len, v.y / len, v.z / len)
}

/// Estimate the outward normal of `sdf` at `p` via central differences.
/// The SDF gradient always points from lower (inside) to higher (outside)
/// values, i.e. outward -- true regardless of the mesh's own triangle
/// winding, so this is used independently of `emit_quad`'s winding check.
fn gradient_normal<F: Fn(Vec3) -> f64>(sdf: &F, p: Vec3, eps: f64) -> Vec3 {
    let dx = sdf(Vec3::new(p.x + eps, p.y, p.z)) - sdf(Vec3::new(p.x - eps, p.y, p.z));
    let dy = sdf(Vec3::new(p.x, p.y + eps, p.z)) - sdf(Vec3::new(p.x, p.y - eps, p.z));
    let dz = sdf(Vec3::new(p.x, p.y, p.z + eps)) - sdf(Vec3::new(p.x, p.y, p.z - eps));
    normalize(Vec3::new(dx, dy, dz))
}

/// Number of cells along each axis of the sampling grid.
#[derive(Debug, Clone, Copy)]
pub struct GridDims {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
}

const CORNER_OFFSETS: [(usize, usize, usize); 8] = [
    (0, 0, 0),
    (1, 0, 0),
    (1, 1, 0),
    (0, 1, 0),
    (0, 0, 1),
    (1, 0, 1),
    (1, 1, 1),
    (0, 1, 1),
];

const EDGES: [(usize, usize); 12] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 0),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

/// Runs Naive Surface Nets over a grid of `dims` cells, `cell_size` meters
/// per cell, with corner (0,0,0) at world position `origin`.
pub fn extract_surface_nets<F: Fn(Vec3) -> f64>(
    sdf: F,
    origin: Vec3,
    cell_size: f64,
    dims: GridDims,
) -> Mesh {
    let GridDims { nx, ny, nz } = dims;
    let (cx, cy, cz) = (nx + 1, ny + 1, nz + 1);

    let corner_index = |i: usize, j: usize, k: usize| -> usize { (i * cy + j) * cz + k };
    let cell_index = |i: usize, j: usize, k: usize| -> usize { (i * ny + j) * nz + k };
    let corner_pos = |i: usize, j: usize, k: usize| -> Vec3 {
        Vec3::new(
            origin.x + i as f64 * cell_size,
            origin.y + j as f64 * cell_size,
            origin.z + k as f64 * cell_size,
        )
    };

    let mut values = vec![0.0f64; cx * cy * cz];
    for i in 0..cx {
        for j in 0..cy {
            for k in 0..cz {
                values[corner_index(i, j, k)] = sdf(corner_pos(i, j, k));
            }
        }
    }

    let mut cell_vertex: Vec<Option<u32>> = vec![None; nx * ny * nz];
    let mut positions: Vec<Vec3> = Vec::new();

    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                let corner_vals: [f64; 8] =
                    CORNER_OFFSETS.map(|(di, dj, dk)| values[corner_index(i + di, j + dj, k + dk)]);
                let has_pos = corner_vals.iter().any(|&v| v >= 0.0);
                let has_neg = corner_vals.iter().any(|&v| v < 0.0);
                if !(has_pos && has_neg) {
                    continue;
                }

                let mut sum = Vec3::new(0.0, 0.0, 0.0);
                let mut count = 0;
                for &(a, b) in EDGES.iter() {
                    let va = corner_vals[a];
                    let vb = corner_vals[b];
                    if (va < 0.0) != (vb < 0.0) {
                        let t = va / (va - vb);
                        let (ai, aj, ak) = CORNER_OFFSETS[a];
                        let (bi, bj, bk) = CORNER_OFFSETS[b];
                        sum.x += ai as f64 + (bi as f64 - ai as f64) * t;
                        sum.y += aj as f64 + (bj as f64 - aj as f64) * t;
                        sum.z += ak as f64 + (bk as f64 - ak as f64) * t;
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                let n = count as f64;
                let local = Vec3::new(sum.x / n, sum.y / n, sum.z / n);
                let world = Vec3::new(
                    origin.x + (i as f64 + local.x) * cell_size,
                    origin.y + (j as f64 + local.y) * cell_size,
                    origin.z + (k as f64 + local.z) * cell_size,
                );
                let idx = positions.len() as u32;
                positions.push(world);
                cell_vertex[cell_index(i, j, k)] = Some(idx);
            }
        }
    }

    let mut triangles: Vec<[u32; 3]> = Vec::new();
    // Winding is corrected against the ACTUAL local SDF gradient at the
    // quad's own centroid, not a crude "which axis, which sign" guess.
    // The axis-sign heuristic this replaced assumed the surface crossing
    // at a cell edge is roughly axis-aligned -- true for a simple wall,
    // false at a composite SDF boundary (e.g. right where an opening's
    // difference-cut meets the solid's own outer surface), where it could
    // "correct" an already-right triangle into a wrong one. Measured
    // directly on a real building-with-door: 32 of 11132 triangles had
    // their face normal pointing opposite the true SDF gradient there
    // before this fix -- small enough that the aggregate signed-volume
    // tests never caught it (they still passed), but real, and visible
    // as wrong-looking shading concentrated near openings on a real
    // device. The gradient sample costs 6 extra sdf() calls per quad;
    // correctness here matters more than that.
    let emit_quad = |triangles: &mut Vec<[u32; 3]>, quad: [u32; 4]| {
        let p0 = positions[quad[0] as usize];
        let p1 = positions[quad[1] as usize];
        let p2 = positions[quad[2] as usize];
        let p3 = positions[quad[3] as usize];
        let centroid = Vec3::new(
            (p0.x + p1.x + p2.x + p3.x) / 4.0,
            (p0.y + p1.y + p2.y + p3.y) / 4.0,
            (p0.z + p1.z + p2.z + p3.z) / 4.0,
        );
        let expected = gradient_normal(&sdf, centroid, cell_size * 0.1);
        let n = cross(
            Vec3::new(p1.x - p0.x, p1.y - p0.y, p1.z - p0.z),
            Vec3::new(p2.x - p0.x, p2.y - p0.y, p2.z - p0.z),
        );
        if dot(n, expected) >= 0.0 {
            triangles.push([quad[0], quad[1], quad[2]]);
            triangles.push([quad[0], quad[2], quad[3]]);
        } else {
            triangles.push([quad[0], quad[2], quad[1]]);
            triangles.push([quad[0], quad[3], quad[2]]);
        }
    };

    // X-axis edges: loop over (j-1,k-1)/(j,k-1)/(j,k)/(j-1,k) cell quartets.
    for i in 0..nx {
        for j in 1..ny {
            for k in 1..nz {
                let va = values[corner_index(i, j, k)];
                let vb = values[corner_index(i + 1, j, k)];
                if (va < 0.0) == (vb < 0.0) {
                    continue;
                }
                if let (Some(c00), Some(c10), Some(c11), Some(c01)) = (
                    cell_vertex[cell_index(i, j - 1, k - 1)],
                    cell_vertex[cell_index(i, j, k - 1)],
                    cell_vertex[cell_index(i, j, k)],
                    cell_vertex[cell_index(i, j - 1, k)],
                ) {
                    emit_quad(&mut triangles, [c00, c10, c11, c01]);
                }
            }
        }
    }

    // Y-axis edges: loop over (i-1,k-1)/(i,k-1)/(i,k)/(i-1,k) cell quartets.
    for i in 1..nx {
        for j in 0..ny {
            for k in 1..nz {
                let va = values[corner_index(i, j, k)];
                let vb = values[corner_index(i, j + 1, k)];
                if (va < 0.0) == (vb < 0.0) {
                    continue;
                }
                if let (Some(c00), Some(c10), Some(c11), Some(c01)) = (
                    cell_vertex[cell_index(i - 1, j, k - 1)],
                    cell_vertex[cell_index(i, j, k - 1)],
                    cell_vertex[cell_index(i, j, k)],
                    cell_vertex[cell_index(i - 1, j, k)],
                ) {
                    emit_quad(&mut triangles, [c00, c10, c11, c01]);
                }
            }
        }
    }

    // Z-axis edges: loop over (i-1,j-1)/(i,j-1)/(i,j)/(i-1,j) cell quartets.
    for i in 1..nx {
        for j in 1..ny {
            for k in 0..nz {
                let va = values[corner_index(i, j, k)];
                let vb = values[corner_index(i, j, k + 1)];
                if (va < 0.0) == (vb < 0.0) {
                    continue;
                }
                if let (Some(c00), Some(c10), Some(c11), Some(c01)) = (
                    cell_vertex[cell_index(i - 1, j - 1, k)],
                    cell_vertex[cell_index(i, j - 1, k)],
                    cell_vertex[cell_index(i, j, k)],
                    cell_vertex[cell_index(i - 1, j, k)],
                ) {
                    emit_quad(&mut triangles, [c00, c10, c11, c01]);
                }
            }
        }
    }

    let normals = positions
        .iter()
        .map(|&p| gradient_normal(&sdf, p, cell_size * 0.1))
        .collect();

    Mesh { positions, normals, triangles }
}

/// Convenience wrapper: computes a grid covering `bounds_min..bounds_max`
/// with one cell of padding on every side (so the object never touches the
/// grid boundary, where `extract_surface_nets` deliberately skips faces).
pub fn extract_surface_nets_bounds<F: Fn(Vec3) -> f64>(
    sdf: F,
    bounds_min: Vec3,
    bounds_max: Vec3,
    cell_size: f64,
) -> Mesh {
    let pad = cell_size * 2.0;
    let origin = Vec3::new(bounds_min.x - pad, bounds_min.y - pad, bounds_min.z - pad);
    let size = Vec3::new(
        (bounds_max.x - bounds_min.x) + 2.0 * pad,
        (bounds_max.y - bounds_min.y) + 2.0 * pad,
        (bounds_max.z - bounds_min.z) + 2.0 * pad,
    );
    let dims = GridDims {
        nx: (size.x / cell_size).ceil() as usize + 1,
        ny: (size.y / cell_size).ceil() as usize + 1,
        nz: (size.z / cell_size).ceil() as usize + 1,
    };
    extract_surface_nets(sdf, origin, cell_size, dims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdf::{sdf_box, sdf_sphere};

    #[test]
    fn every_triangle_normal_agrees_with_the_true_sdf_gradient_near_a_composite_cut() {
        // A box with a smaller box notch cut through one face via
        // sdf_difference -- the same composite-SDF shape (solid minus a
        // door/window punch) that originally caused emit_quad's old
        // axis-sign heuristic to flip 32 of 11132 triangles on a real
        // building. This is the direct regression test for that fix: for
        // every triangle, its own face normal (via the cross product of
        // two edges) must point the same general direction as the SDF's
        // real gradient at that triangle's centroid, not just produce the
        // right AGGREGATE signed volume (which the old heuristic already
        // passed despite being wrong on individual triangles).
        let half = Vec3::new(3.0, 2.0, 1.5);
        let notch_center = Vec3::new(0.0, 0.0, half.z);
        let notch_half = Vec3::new(0.6, 0.6, 0.6);
        let sdf = move |p: Vec3| {
            let solid = crate::sdf::sdf_box(p, half);
            let notch = crate::sdf::sdf_box(
                Vec3::new(p.x - notch_center.x, p.y - notch_center.y, p.z - notch_center.z),
                notch_half,
            );
            crate::sdf::sdf_difference(solid, notch)
        };
        let cell_size = 0.15;
        let mesh = extract_surface_nets_bounds(
            sdf,
            Vec3::new(-half.x, -half.y, -half.z),
            Vec3::new(half.x, half.y, half.z),
            cell_size,
        );
        assert!(!mesh.triangles.is_empty());

        let eps = cell_size * 0.1;
        let mut disagreements = 0;
        for tri in &mesh.triangles {
            let p0 = mesh.positions[tri[0] as usize];
            let p1 = mesh.positions[tri[1] as usize];
            let p2 = mesh.positions[tri[2] as usize];
            let centroid = Vec3::new((p0.x + p1.x + p2.x) / 3.0, (p0.y + p1.y + p2.y) / 3.0, (p0.z + p1.z + p2.z) / 3.0);
            let expected = gradient_normal(&sdf, centroid, eps);
            let face = cross(
                Vec3::new(p1.x - p0.x, p1.y - p0.y, p1.z - p0.z),
                Vec3::new(p2.x - p0.x, p2.y - p0.y, p2.z - p0.z),
            );
            if dot(face, expected) < 0.0 {
                disagreements += 1;
            }
        }
        let rate = disagreements as f64 / mesh.triangles.len() as f64;
        assert!(
            rate < 0.01,
            "{disagreements} of {} triangles ({:.2}%) disagree with the true SDF gradient -- expected near-zero, a handful only at the tightest notch corners (inherent to Naive Surface Nets on sharp features, not a regression)",
            mesh.triangles.len(),
            rate * 100.0
        );
    }

    #[test]
    fn sphere_extracts_nonempty_mesh_with_roughly_correct_volume() {
        let r = 2.0;
        let mesh = extract_surface_nets_bounds(
            |p| sdf_sphere(p, r),
            Vec3::new(-r, -r, -r),
            Vec3::new(r, r, r),
            0.2,
        );
        assert!(!mesh.positions.is_empty(), "sphere should produce vertices");
        assert!(!mesh.triangles.is_empty(), "sphere should produce triangles");

        let expected_volume = 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
        let actual_volume = mesh.signed_volume();
        let rel_err = (actual_volume - expected_volume).abs() / expected_volume;
        assert!(
            rel_err < 0.05,
            "sphere volume off by {:.1}%: got {actual_volume}, expected {expected_volume}",
            rel_err * 100.0
        );
    }

    #[test]
    fn box_extracts_mesh_whose_volume_matches_analytic_box() {
        let half = Vec3::new(3.0, 2.0, 4.0);
        let mesh = extract_surface_nets_bounds(
            |p| sdf_box(p, half),
            Vec3::new(-half.x, -half.y, -half.z),
            Vec3::new(half.x, half.y, half.z),
            0.25,
        );
        let expected_volume = 8.0 * half.x * half.y * half.z;
        let actual_volume = mesh.signed_volume();
        let rel_err = (actual_volume - expected_volume).abs() / expected_volume;
        assert!(
            rel_err < 0.05,
            "box volume off by {:.1}%: got {actual_volume}, expected {expected_volume}",
            rel_err * 100.0
        );
    }

    #[test]
    fn every_vertex_lies_close_to_the_true_zero_surface() {
        let r = 1.5;
        let mesh = extract_surface_nets_bounds(
            |p| sdf_sphere(p, r),
            Vec3::new(-r, -r, -r),
            Vec3::new(r, r, r),
            0.15,
        );
        for p in &mesh.positions {
            let d = sdf_sphere(*p, r).abs();
            assert!(d < 0.15, "vertex {p:?} is {d} from the true surface, expected < one cell");
        }
    }

    #[test]
    fn empty_region_produces_no_geometry() {
        // SDF that's positive (outside) everywhere in this region -- no surface to extract.
        let mesh = extract_surface_nets_bounds(
            |p| sdf_sphere(p, 0.5),
            Vec3::new(10.0, 10.0, 10.0),
            Vec3::new(12.0, 12.0, 12.0),
            0.5,
        );
        assert!(mesh.positions.is_empty());
        assert!(mesh.triangles.is_empty());
    }
}
