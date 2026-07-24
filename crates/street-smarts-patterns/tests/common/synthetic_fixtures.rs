//! Procedural synthetic fixture generation — HARDENING_SPEC.md §5.
//!
//! `tests/p37_fuzz_seeds.rs` (and PATTERN_LANGUAGE_SIMULATION.md §4.4's
//! harness generally) varies the SEED axis against one real fixture
//! (Eastside Commons) or one hand-built synthetic square. Neither can
//! surface a bug that's triggered by input SHAPE rather than randomness --
//! a concave near-self-intersecting boundary, a sliver parcel, a site an
//! order of magnitude larger or smaller than what's been tested. This
//! module generates that second axis: varied parcel shapes, deterministic
//! per seed, for the same fuzz harness to run pattern operators against.
//!
//! Deliberately scoped to the geometric shape axes -- aspect ratio,
//! concavity, area, vertex count -- since those are what actually stress
//! the geometry pipeline (Voronoi seeding, ear-clipping triangulation,
//! half-plane clipping). `existing_building_density` from the original
//! spec sketch is left out of this version: P37/P95 carve raw land, they
//! don't read pre-existing `Building` entities on the source parcel, so
//! that axis wouldn't exercise anything these operators actually do.

use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, NeighborhoodMeta, Parcel};
use street_smarts_patterns::prng::Prng;

pub struct FixtureAxes {
    /// 1.0 = roughly square/regular. Larger values stretch one dimension --
    /// x scaled by sqrt(aspect_ratio), y by 1/sqrt(aspect_ratio), which
    /// preserves area under the stretch so `area_m2` stays meaningful.
    pub aspect_ratio: f64,
    /// 0.0 = a regular convex polygon. Approaching 1.0 pulls every other
    /// vertex sharply inward toward the centroid -- a star shape, the
    /// deliberately adversarial end of this axis (near-self-intersecting,
    /// exactly the class of input the ear-clipping triangulator's own
    /// "guard against infinite loops on degenerate input" comment is
    /// worried about).
    pub concavity: f64,
    /// Target parcel area in m² (approximate -- concavity distorts the
    /// exact regular-polygon area formula this starts from).
    pub area_m2: f64,
    /// Vertex count of the base polygon before concavity is applied.
    /// Must be >= 4 (concavity needs alternating in/out vertices, so an
    /// odd count still works but loses the strict alternation on the
    /// closing edge).
    pub vertex_count: usize,
}

impl FixtureAxes {
    pub fn regular(area_m2: f64) -> Self {
        Self { aspect_ratio: 1.0, concavity: 0.0, area_m2, vertex_count: 8 }
    }
}

/// Generate a single-parcel `Neighborhood` matching `axes`, deterministic
/// for a given `seed` (jitters vertex angles slightly so `vertex_count`
/// alone doesn't produce a perfectly regular, atypically well-behaved
/// polygon every time).
pub fn generate(axes: &FixtureAxes, seed: u64) -> Neighborhood {
    let mut rng = Prng::new(seed);
    let n = axes.vertex_count.max(4);

    // Regular n-gon area = (n/2) * r^2 * sin(2*pi/n) -- solve for r.
    let interior_angle = 2.0 * std::f64::consts::PI / n as f64;
    let base_radius = (2.0 * axes.area_m2 / (n as f64 * interior_angle.sin())).sqrt();

    let mut points_m: Vec<(f64, f64)> = Vec::with_capacity(n);
    for i in 0..n {
        let angle = interior_angle * i as f64 + rng.next_f64() * 0.05; // small angular jitter
        let mut r = base_radius;
        if axes.concavity > 0.0 && i % 2 == 1 {
            r *= 1.0 - axes.concavity;
        }
        let x = r * angle.cos() * axes.aspect_ratio.sqrt();
        let y = r * angle.sin() / axes.aspect_ratio.sqrt();
        points_m.push((x, y));
    }

    let m_per_deg_lng = 111_320.0;
    let m_per_deg_lat = 110_540.0;
    let origin = LngLat::new(0.0, 0.0);
    let mut ring: Vec<LngLat> = points_m
        .iter()
        .map(|(x, y)| LngLat::new(origin.lng + x / m_per_deg_lng, origin.lat + y / m_per_deg_lat))
        .collect();
    ring.push(ring[0]); // closed ring, matching this codebase's own convention

    let polygon = Polygon::from_ring(ring);
    let real_area_m2 = polygon.area_m2();

    Neighborhood {
        id: format!("synthetic_{seed}"),
        bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
        parcels: vec![Parcel {
            id: format!("SYNTH_{seed}"),
            polygon,
            area_acres: real_area_m2 / 4046.86,
            use_category: None,
            ownership: None,
            is_eda: true,
            spec: None,
            density_tier: None,
            target_stories: None,
        }],
        buildings: vec![],
        streets: vec![],
        open_space: vec![],
        boundaries: vec![],
        activity_nodes: vec![],
        metadata: NeighborhoodMeta {
            source: "synthetic".into(),
            fetched_at: "test".into(),
            license: "test".into(),
            layer_provenance: Default::default(),
            label: format!(
                "synthetic fixture (aspect={:.1}, concavity={:.2}, area={:.0}m², n={}, seed={seed})",
                axes.aspect_ratio, axes.concavity, axes.area_m2, n
            ),
        },
            pattern_fields: vec![],
        }
}

/// Minimal physical-plausibility check -- closed, positive-area, enough
/// distinct vertices to be a real polygon. Run on every generated fixture
/// before handing it to a pattern operator, so a downstream test failure
/// is attributable to the operator under test, not to this generator
/// having produced outright garbage.
pub fn is_plausible(n: &Neighborhood) -> bool {
    let Some(parcel) = n.parcels.first() else { return false };
    let ring = &parcel.polygon.outer;
    if ring.len() < 5 {
        // >= 4 distinct points + the closing repeat of the first.
        return false;
    }
    if ring.first() != ring.last() {
        return false;
    }
    parcel.polygon.area_m2() > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_axes_produce_a_plausible_fixture() {
        let n = generate(&FixtureAxes::regular(5000.0), 1);
        assert!(is_plausible(&n));
        let area = n.parcels[0].polygon.area_m2();
        assert!((area - 5000.0).abs() / 5000.0 < 0.15, "area {area} too far from target 5000");
    }

    #[test]
    fn same_seed_is_deterministic() {
        let axes = FixtureAxes::regular(4000.0);
        let a = generate(&axes, 42);
        let b = generate(&axes, 42);
        assert_eq!(a.parcels[0].polygon.outer, b.parcels[0].polygon.outer);
    }

    #[test]
    fn different_seeds_produce_different_shapes() {
        let axes = FixtureAxes::regular(4000.0);
        let a = generate(&axes, 1);
        let b = generate(&axes, 2);
        assert_ne!(a.parcels[0].polygon.outer, b.parcels[0].polygon.outer);
    }

    #[test]
    fn high_concavity_star_shape_is_still_plausible() {
        let axes = FixtureAxes { aspect_ratio: 1.0, concavity: 0.7, area_m2: 6000.0, vertex_count: 10 };
        let n = generate(&axes, 5);
        assert!(is_plausible(&n), "star-shaped fixture should still be a well-formed closed ring");
    }

    #[test]
    fn extreme_aspect_ratio_produces_a_sliver() {
        let axes = FixtureAxes { aspect_ratio: 20.0, concavity: 0.0, area_m2: 5000.0, vertex_count: 8 };
        let n = generate(&axes, 3);
        assert!(is_plausible(&n));
    }
}
