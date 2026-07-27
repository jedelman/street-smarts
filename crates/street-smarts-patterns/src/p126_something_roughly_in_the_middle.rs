//! P126 Something Roughly in the Middle — every public square needs a
//! real focal object roughly (not exactly) at its center.
//!
//! From Alexander, *A Pattern Language*, Pattern 126 (p. 606), via
//! patternlanguage.cc/Patterns/Something-Roughly-in-the-Middle-(126):
//! > **Problem:** A public space without a middle is quite likely to stay
//! > empty.
//! > **Solution:** Between the natural paths which cross a public square
//! > or courtyard or a piece of common land, choose something to stand
//! > roughly in the middle... Leave it exactly where it falls between the
//! > paths; resist the impulse to put it exactly in the middle.
//!
//! # What this generator does
//!
//! For every real `OpenSpaceKind::Plaza` in the neighborhood (with
//! `area_m2() > 0.0`):
//! 1. Compute its centroid and the check radius: `radius = bbox_short_side_m(outer) * middle_radius_fraction`.
//! 2. Check whether ANY existing `ActivityNode` in `nbhd.activity_nodes` sits
//!    within that radius of the centroid (using haversine distance).
//! 3. If yes, the plaza already has a middle — skip it (idempotent).
//! 4. If no, place one new `ActivityNode` (kind: `ActivityKind::Civic`) at a
//!    jittered-off-center location: offset the centroid by `jitter_fraction *
//!    bbox_short_side_m(outer)` meters in a random direction drawn from a
//!    deterministic `Prng::new(seed)`.
//!
//! The radius and jitter calculations use the same `bbox_short_side_m` logic
//! the opinion itself applies (replicated exactly to ensure generator and
//! opinion agree), and both default to the same thresholds the opinion uses
//! (0.6 for radius, implicitly 0.2 for jitter matching p61's convention).
//!
//! **IMPORTANT INVARIANT**: `jitter_fraction` MUST stay meaningfully smaller
//! than `middle_radius_fraction` (default 0.2 vs 0.6) — otherwise the node
//! you place might not even satisfy the opinion's own "within radius" check,
//! which would be self-defeating. A note about this constraint is included
//! in the module doc AND in the caveats.
//!
//! # Precondition errors
//!
//! Returns `Err(String)` if:
//! - No real `Plaza`-kind open space exists (area_m2() > 0.0) — same as the
//!   opinion's own first `NoView` reason; suggest running P61 or P95/P107 first.
//!
//! It's fine (NOT an error) if every plaza already has a middle — returns
//! `Ok` with empty `new_activity_nodes` and a trace explaining the count checked.
//!
//! # Caveats
//!
//! - Doesn't verify the object is "between the natural paths" that cross the
//!   square (Alexander's literal instruction) — no path-crossing-a-plaza
//!   concept exists in this schema; only centroid proximity is checked, matching
//!   the opinion's own stated limitation.
//! - The new marker is a generic `ActivityKind::Civic` node, not a real
//!   fountain/statue/bandstand/program decision — this generator has no basis
//!   to invent what specifically belongs there (same restraint
//!   `p61_small_public_squares`'s own `activity_node_for_square` documents).
//! - This generator should run LATE in the pipeline (after P61, P95, P107,
//!   P124, P53) so it sees the final state and doesn't stack a redundant node
//!   on a plaza another generator already gave one to.

use crate::parameters::{ParamSpec, Parameters};
use crate::prng::Prng;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::{ActivityKind, ActivityNode, Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::SourceCitation;

/// Matches the opinion's own MIDDLE_RADIUS_FRACTION exactly.
const MIDDLE_RADIUS_FRACTION_DEFAULT: f64 = 0.6;

/// Default jitter fraction — how far (as a fraction of bbox_short_side_m)
/// the new activity node is offset from the centroid. Must be smaller than
/// MIDDLE_RADIUS_FRACTION to ensure the jittered node still passes the
/// opinion's own "within radius" check.
const JITTER_FRACTION_DEFAULT: f64 = 0.2;

/// Compute the short side of a bounding box in meters, same logic the opinion
/// uses in `bbox_short_side_m`. This MUST exactly match the opinion's
/// implementation so generator and opinion agree on what counts as "close
/// enough to the middle."
fn bbox_short_side_m(ring: &[LngLat]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let lat0 = ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64;
    let mlat = lat0.to_radians().cos();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in ring {
        let x = p.lng * mlat * 111_320.0;
        let y = p.lat * 110_540.0;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    (max_x - min_x).min(max_y - min_y)
}

/// Jitter an activity node off the centroid by `jitter_m` meters in a random
/// direction, using lng/lat degrees directly (same meters-per-degree constants
/// the opinion's own `bbox_short_side_m` uses).
fn jitter_location(centroid: &LngLat, jitter_m: f64, angle: f64) -> LngLat {
    // Same constants as the opinion's bbox_short_side_m:
    // - 111_320.0 meters per degree longitude (scaled by cos(lat))
    // - 110_540.0 meters per degree latitude
    let lat0 = centroid.lat.to_radians().cos();
    let dlat = angle.sin() * jitter_m / 110_540.0;
    let dlng = angle.cos() * jitter_m / (lat0 * 111_320.0);
    LngLat::new(centroid.lng + dlng, centroid.lat + dlat)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P126Params {
    /// Fraction of the plaza's own bbox short side from the centroid that
    /// defines "close enough to the middle" for an activity node to count as
    /// serving the plaza. Default matches the opinion's own MIDDLE_RADIUS_FRACTION.
    pub middle_radius_fraction: f64,
    /// Fraction of the plaza's bbox short side by which to offset the activity
    /// node from the centroid (in a random direction). Must be smaller than
    /// middle_radius_fraction to ensure the jittered node still passes the
    /// opinion's own check. Default matches p61's own ACTIVITY_NODE_JITTER_FRAC.
    pub jitter_fraction: f64,
}

impl Parameters for P126Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "middle_radius_fraction",
                "Fraction of bbox short side that defines 'close enough to the middle' (opinion's MIDDLE_RADIUS_FRACTION).",
                0.2,
                1.0,
                MIDDLE_RADIUS_FRACTION_DEFAULT,
            ),
            ParamSpec::float(
                "jitter_fraction",
                "Fraction of bbox short side to offset from centroid (must be < middle_radius_fraction). Matches p61's ACTIVITY_NODE_JITTER_FRAC.",
                0.05,
                0.4,
                JITTER_FRACTION_DEFAULT,
            ),
        ]
    }

    fn defaults() -> Self {
        Self {
            middle_radius_fraction: MIDDLE_RADIUS_FRACTION_DEFAULT,
            jitter_fraction: JITTER_FRACTION_DEFAULT,
        }
    }

    fn as_vector(&self) -> Vec<f64> {
        vec![self.middle_radius_fraction, self.jitter_fraction]
    }

    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.middle_radius_fraction = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) {
            p.jitter_fraction = s.clamp(*x);
        }
        p
    }
}

pub struct P126SomethingRoughlyInTheMiddle;

impl PatternOperator for P126SomethingRoughlyInTheMiddle {
    type Params = P126Params;

    fn name(&self) -> &'static str {
        "p126_something_roughly_in_the_middle"
    }

    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p126".into(),
            display: "Alexander et al., A Pattern Language, Pattern 126 (Something Roughly in the Middle)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Something-Roughly-in-the-Middle-(126)".into()),
        }
    }

    fn description(&self) -> &'static str {
        "Place a generic activity node roughly (not exactly) at the center of plazas that lack one, closing P126's real gap."
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        _parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        // Filter to real plazas (same filter the opinion uses).
        let plazas: Vec<_> = nbhd
            .open_space
            .iter()
            .filter(|o| o.kind == OpenSpaceKind::Plaza && o.polygon.area_m2() > 0.0)
            .collect();

        if plazas.is_empty() {
            return Err(
                "p126_something_roughly_in_the_middle: No real Plaza open space in this neighborhood. \
                 Run P61 Small Public Squares, P95 Building Complex, or P107 Wings of Light first."
                    .into(),
            );
        }

        // Sanity check: jitter must be smaller than middle_radius for the nodes
        // to pass the opinion's own check. This is a debug-time warning (not a
        // hard error) to help catch configuration mistakes.
        if params.jitter_fraction >= params.middle_radius_fraction {
            eprintln!(
                "WARNING: p126 jitter_fraction ({:.3}) >= middle_radius_fraction ({:.3}); \
                 jittered nodes may not pass the opinion's own 'within radius' check.",
                params.jitter_fraction, params.middle_radius_fraction
            );
        }

        let mut prng = Prng::new(seed);
        let mut new_activity_nodes: Vec<ActivityNode> = Vec::new();
        let mut n_checked = 0;
        let mut n_already_served = 0;
        let mut n_placed = 0;
        let mut steps: Vec<String> = Vec::new();

        for plaza in &plazas {
            n_checked += 1;
            let centroid = plaza.polygon.centroid();
            let short_side = bbox_short_side_m(&plaza.polygon.outer);
            let radius = short_side * params.middle_radius_fraction;

            // Check if any existing activity node already sits within the radius.
            let nearest_m = nbhd
                .activity_nodes
                .iter()
                .map(|a| haversine_m(&centroid, &a.location))
                .fold(f64::MAX, f64::min);

            if nearest_m <= radius {
                // This plaza already has a middle.
                n_already_served += 1;
                steps.push(format!(
                    "{}: already has activity node {:.1}m from centroid (within {:.1}m threshold).",
                    plaza.id, nearest_m, radius
                ));
            } else {
                // Place a new activity node, jittered off center.
                let jitter_m = short_side * params.jitter_fraction;
                let angle = prng.range(0.0, TAU);
                let jittered_location = jitter_location(&centroid, jitter_m, angle);

                let node = ActivityNode {
                    id: format!("{}_p126_middle", plaza.id),
                    location: jittered_location,
                    kind: ActivityKind::Civic,
                    intensity: None,
                    label: None,
                    activity_fit: Default::default(),
                    publicness: None,
                };

                new_activity_nodes.push(node);
                n_placed += 1;
                steps.push(format!(
                    "{}: no activity node nearby (nearest was {:.1}m, beyond {:.1}m threshold) — placed at jittered centroid.",
                    plaza.id, nearest_m.min(9999.0), radius
                ));
            }
        }

        steps.insert(
            0,
            format!(
                "{} plaza(s) checked: {} already have a middle, {} newly placed.",
                n_checked, n_already_served, n_placed
            ),
        );

        let trace = SubdivisionTrace {
            operator_name: "p126_something_roughly_in_the_middle".into(),
            operator_source: self.source(),
            headline: format!(
                "{} plaza(s) checked; {} already served by existing activity node(s), {} new middle(s) placed.",
                n_checked, n_already_served, n_placed
            ),
            steps,
            caveats: vec![
                "Doesn't verify the object is 'between the natural paths' that cross the square — \
                 no path-crossing-a-plaza concept exists in this schema; only centroid proximity is checked, \
                 matching the opinion's own stated limitation.".into(),
                "The new marker is a generic ActivityKind::Civic node, not a real fountain/statue/bandstand/program \
                 decision — this generator has no basis to invent what specifically belongs there.".into(),
                "Should run LATE in the pipeline (after P61, P95, P107, P124, P53) so it sees the final state \
                 and doesn't stack a redundant node on a plaza another generator already gave one to.".into(),
                format!(
                    "jitter_fraction ({:.3}) must be < middle_radius_fraction ({:.3}) for the jittered node to pass \
                     the opinion's own 'within radius' check — if this constraint is violated, the generator and opinion disagree.",
                    params.jitter_fraction, params.middle_radius_fraction
                ),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings: vec![],
            new_streets: vec![],
            new_activity_nodes,
            new_boundaries: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
            new_fields: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{NeighborhoodMeta, OpenSpace};

    fn nbhd(open_space: Vec<OpenSpace>, activity_nodes: Vec<ActivityNode>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![],
            buildings: vec![],
            streets: vec![],
            open_space,
            boundaries: vec![],
            activity_nodes,
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P126 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    /// Create a plaza of a given side length (in meters), centered at origin.
    fn plaza(id: &str, side_m: f64) -> OpenSpace {
        // Convert meters to degrees (1 degree longitude at equator ≈ 111,320 m)
        let m_to_deg = 1.0 / 111_320.0;
        let half = side_m / 2.0 * m_to_deg;
        OpenSpace {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-half, -half),
                LngLat::new(half, -half),
                LngLat::new(half, half),
                LngLat::new(-half, half),
                LngLat::new(-half, -half),
            ]),
            kind: OpenSpaceKind::Plaza,
        }
    }

    #[test]
    fn no_plazas_is_error() {
        let op = P126SomethingRoughlyInTheMiddle;
        let params = P126Params::defaults();
        let n = nbhd(vec![], vec![]);
        let result = op.apply(&n, "*", &params, 42);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No real Plaza"));
    }

    #[test]
    fn plaza_with_nearby_activity_node_skipped() {
        let op = P126SomethingRoughlyInTheMiddle;
        let params = P126Params::defaults();
        let node = ActivityNode {
            id: "A1".into(),
            location: LngLat::new(0.0, 0.0),
            kind: ActivityKind::Civic,
            intensity: None,
            label: None,
            activity_fit: Default::default(),
            publicness: None,
        };
        let n = nbhd(vec![plaza("PZ1", 30.0)], vec![node]);
        let result = op.apply(&n, "*", &params, 42);
        assert!(result.is_ok());
        let sub = result.unwrap();
        // The plaza already has a nearby node, so we should place zero new ones.
        assert_eq!(sub.new_activity_nodes.len(), 0);
        assert!(sub.trace.headline.contains("already"));
    }

    #[test]
    fn plaza_without_nearby_activity_node_gets_one() {
        let op = P126SomethingRoughlyInTheMiddle;
        let params = P126Params::defaults();
        // Place a node far away from the plaza.
        let m_to_deg = 1.0 / 111_320.0;
        let far_node = ActivityNode {
            id: "A1".into(),
            location: LngLat::new(500.0 * m_to_deg, 500.0 * m_to_deg),
            kind: ActivityKind::Civic,
            intensity: None,
            label: None,
            activity_fit: Default::default(),
            publicness: None,
        };
        let n = nbhd(vec![plaza("PZ1", 30.0)], vec![far_node]);
        let result = op.apply(&n, "*", &params, 42);
        assert!(result.is_ok());
        let sub = result.unwrap();
        // The plaza should get a new node.
        assert_eq!(sub.new_activity_nodes.len(), 1);
        assert_eq!(sub.new_activity_nodes[0].id, "PZ1_p126_middle");
        assert_eq!(sub.new_activity_nodes[0].kind, ActivityKind::Civic);
    }

    #[test]
    fn jittered_node_is_not_at_exact_centroid() {
        let op = P126SomethingRoughlyInTheMiddle;
        let params = P126Params::defaults();
        let n = nbhd(vec![plaza("PZ1", 30.0)], vec![]);
        let result = op.apply(&n, "*", &params, 42);
        assert!(result.is_ok());
        let sub = result.unwrap();
        assert_eq!(sub.new_activity_nodes.len(), 1);
        let node = &sub.new_activity_nodes[0];
        let centroid = LngLat::new(0.0, 0.0); // The plaza is centered at origin.
        let distance_m = haversine_m(&centroid, &node.location);
        // The jitter should place it a nonzero distance from the centroid.
        assert!(distance_m > 0.0, "Node should be jittered, not at exact centroid");
        // But it should still be within the middle_radius.
        let short_side = bbox_short_side_m(&vec![
            LngLat::new(-15.0 / 111_320.0, -15.0 / 111_320.0),
            LngLat::new(15.0 / 111_320.0, -15.0 / 111_320.0),
            LngLat::new(15.0 / 111_320.0, 15.0 / 111_320.0),
            LngLat::new(-15.0 / 111_320.0, 15.0 / 111_320.0),
        ]);
        let radius = short_side * params.middle_radius_fraction;
        assert!(
            distance_m <= radius,
            "Jittered node at {:.1}m should be within radius {:.1}m",
            distance_m,
            radius
        );
    }

    #[test]
    fn deterministic_jitter_same_seed() {
        let op = P126SomethingRoughlyInTheMiddle;
        let params = P126Params::defaults();
        let n = nbhd(vec![plaza("PZ1", 30.0)], vec![]);
        let result1 = op.apply(&n, "*", &params, 42);
        let result2 = op.apply(&n, "*", &params, 42);
        assert!(result1.is_ok() && result2.is_ok());
        let sub1 = result1.unwrap();
        let sub2 = result2.unwrap();
        assert_eq!(sub1.new_activity_nodes.len(), 1);
        assert_eq!(sub2.new_activity_nodes.len(), 1);
        let loc1 = &sub1.new_activity_nodes[0].location;
        let loc2 = &sub2.new_activity_nodes[0].location;
        assert_eq!(loc1.lng, loc2.lng, "Same seed should produce same lng");
        assert_eq!(loc1.lat, loc2.lat, "Same seed should produce same lat");
    }

    #[test]
    fn different_seeds_produce_different_jitter() {
        let op = P126SomethingRoughlyInTheMiddle;
        let params = P126Params::defaults();
        let n = nbhd(vec![plaza("PZ1", 30.0)], vec![]);
        let result1 = op.apply(&n, "*", &params, 42);
        let result2 = op.apply(&n, "*", &params, 43);
        assert!(result1.is_ok() && result2.is_ok());
        let sub1 = result1.unwrap();
        let sub2 = result2.unwrap();
        assert_eq!(sub1.new_activity_nodes.len(), 1);
        assert_eq!(sub2.new_activity_nodes.len(), 1);
        let loc1 = &sub1.new_activity_nodes[0].location;
        let loc2 = &sub2.new_activity_nodes[0].location;
        // Different seeds should (almost always) produce different locations.
        assert!(
            loc1.lng != loc2.lng || loc1.lat != loc2.lat,
            "Different seeds should produce different jitter"
        );
    }

    #[test]
    fn mixed_plazas_served_and_unserved() {
        let op = P126SomethingRoughlyInTheMiddle;
        let params = P126Params::defaults();
        let m_to_deg = 1.0 / 111_320.0;

        // PZ1 (centered at origin) has a nearby node at the origin.
        let node1 = ActivityNode {
            id: "A1".into(),
            location: LngLat::new(0.0, 0.0),
            kind: ActivityKind::Civic,
            intensity: None,
            label: None,
            activity_fit: Default::default(),
            publicness: None,
        };

        // PZ2: create a plaza centered 1000m away (in the X direction).
        // This is far enough that the node at origin won't serve it.
        let offset_deg = 1000.0 * m_to_deg;  // ~0.009 degrees
        let half_deg = 15.0 * m_to_deg;       // 30m plaza, half-side = 15m
        let pz2_far = OpenSpace {
            id: "PZ2".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(offset_deg - half_deg, -half_deg),
                LngLat::new(offset_deg + half_deg, -half_deg),
                LngLat::new(offset_deg + half_deg, half_deg),
                LngLat::new(offset_deg - half_deg, half_deg),
                LngLat::new(offset_deg - half_deg, -half_deg),
            ]),
            kind: OpenSpaceKind::Plaza,
        };

        let n = nbhd(vec![plaza("PZ1", 30.0), pz2_far], vec![node1]);
        let result = op.apply(&n, "*", &params, 42);
        assert!(result.is_ok());
        let sub = result.unwrap();
        // Only PZ2 should get a new node (PZ1 is already served).
        assert_eq!(sub.new_activity_nodes.len(), 1);
        assert_eq!(sub.new_activity_nodes[0].id, "PZ2_p126_middle");
    }
}
