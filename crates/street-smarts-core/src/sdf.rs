//! # 3D Signed Distance Fields (SDF) & Alexandrian Spatial Math
//!
//! Provides 3D implicit geometry functions, CSG boolean operations, smooth minimums (`smin`),
//! and spatial AABB/BVH acceleration structures for mobile-efficient surface extraction
//! across micro apertures and macro (91-acre) neighborhood contexts.

use serde::{Deserialize, Serialize};

/// 3D Vector in local metric space (meters).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn sub(&self, other: Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    pub fn add(&self, other: Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    pub fn abs(&self) -> Vec3 {
        Vec3::new(self.x.abs(), self.y.abs(), self.z.abs())
    }

    pub fn max_component(&self, val: f64) -> Vec3 {
        Vec3::new(self.x.max(val), self.y.max(val), self.z.max(val))
    }
}

/// Axis-Aligned Bounding Box (AABB) for 3D spatial indexing and BVH pruning.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AABB3D {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB3D {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Checks if a 3D point is inside the bounding box.
    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x &&
        p.y >= self.min.y && p.y <= self.max.y &&
        p.z >= self.min.z && p.z <= self.max.z
    }

    /// Distance from point `p` to the nearest face of the AABB (0.0 if inside).
    pub fn distance_to_point(&self, p: Vec3) -> f64 {
        let dx = (self.min.x - p.x).max(0.0).max(p.x - self.max.x);
        let dy = (self.min.y - p.y).max(0.0).max(p.y - self.max.y);
        let dz = (self.min.z - p.z).max(0.0).max(p.z - self.max.z);
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

// ============================================================================
// Core Signed Distance Field (SDF) Primitives
// ============================================================================

/// SDF for a 3D Box centered at origin with half-extents `b`.
pub fn sdf_box(p: Vec3, b: Vec3) -> f64 {
    let q = p.abs().sub(b);
    let outside = q.max_component(0.0).length();
    let inside = q.x.max(q.y).max(q.z).min(0.0);
    outside + inside
}

/// SDF for a 3D Sphere centered at origin with radius `r`.
pub fn sdf_sphere(p: Vec3, r: f64) -> f64 {
    p.length() - r
}

/// SDF for a Vertical Cylinder centered at origin with `radius` and `height`.
pub fn sdf_cylinder(p: Vec3, radius: f64, height: f64) -> f64 {
    let d_xz = (p.x * p.x + p.z * p.z).sqrt() - radius;
    let d_y = p.y.abs() - height * 0.5;
    d_xz.max(d_y)
}

// ============================================================================
// CSG Boolean Operators
// ============================================================================

/// CSG Union: Combining two spatial volumes.
pub fn sdf_union(d1: f64, d2: f64) -> f64 {
    d1.min(d2)
}

/// CSG Intersection: Volume shared by two shapes.
pub fn sdf_intersection(d1: f64, d2: f64) -> f64 {
    d1.max(d2)
}

/// CSG Difference (Aperture Cuts P221): Subtracting volume `d2` from `d1`.
pub fn sdf_difference(d1: f64, d2: f64) -> f64 {
    d1.max(-d2)
}

/// Alexandrian Smooth Minimum (`smin`): Creates organic architectural transitions,
/// fillets, arches, and wall-to-ceiling connections with blending factor `k`.
pub fn sdf_smin(d1: f64, d2: f64, k: f64) -> f64 {
    if k <= 0.0 {
        return d1.min(d2);
    }
    let h = (0.5 + 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
    d2 * (1.0 - h) + d1 * h - k * h * (1.0 - h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdf_box_center() {
        let b = Vec3::new(2.0, 3.0, 4.0);
        let p = Vec3::new(0.0, 0.0, 0.0);
        let dist = sdf_box(p, b);
        assert!((dist - -2.0).abs() < 1e-6);
    }

    #[test]
    fn test_csg_aperture_difference() {
        // Solid wall d1, window punch d2
        let d_wall = -1.0;
        let d_window = -0.5;
        let d_punched = sdf_difference(d_wall, d_window);
        assert!((d_punched - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_smin_smooth_arch() {
        let d1 = 1.0;
        let d2 = 0.8;
        let blended = sdf_smin(d1, d2, 0.5);
        assert!(blended < d2);
    }
}
