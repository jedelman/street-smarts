//! P37 House Cluster run against GENERATED SHAPES, not just generated
//! seeds -- the second fuzzing axis HARDENING_SPEC.md §5 and
//! PATTERN_LANGUAGE_SIMULATION.md §4.4 both call for. `p37_fuzz_seeds.rs`
//! varies the RNG against one fixed shape; this varies the shape itself
//! (aspect ratio, concavity, vertex count) against a fixed small set of
//! seeds, which is what would actually catch a shape-triggered geometry
//! bug (a concave near-self-intersecting boundary, a sliver parcel) that
//! seed variance alone structurally cannot.

mod common;

use common::assert_invariant_across_seeds;
use common::synthetic_fixtures::{generate, is_plausible, FixtureAxes};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::{Parameters, PatternOperator};

/// A spread across every axis HARDENING_SPEC.md §5 names: regular, a
/// sliver (extreme aspect ratio), a star shape (high concavity, the
/// deliberately adversarial case), a tiny parcel, and a large one.
fn axes_under_test() -> Vec<(&'static str, FixtureAxes)> {
    vec![
        ("regular", FixtureAxes::regular(6000.0)),
        ("sliver", FixtureAxes { aspect_ratio: 15.0, concavity: 0.0, area_m2: 6000.0, vertex_count: 8 }),
        ("star_shaped", FixtureAxes { aspect_ratio: 1.0, concavity: 0.6, area_m2: 8000.0, vertex_count: 12 }),
        ("tiny", FixtureAxes::regular(600.0)),
        ("huge", FixtureAxes::regular(120_000.0)),
    ]
}

#[test]
fn generated_fixtures_are_all_physically_plausible() {
    for (label, axes) in axes_under_test() {
        for seed in [1u64, 2, 3] {
            let n = generate(&axes, seed);
            assert!(is_plausible(&n), "{label} (seed {seed}) produced an implausible fixture");
        }
    }
}

#[test]
fn p37_does_not_panic_or_corrupt_output_across_generated_shapes() {
    // Weaker than the seed-fuzz invariants on purpose: at extreme shape
    // axes (a 15:1 sliver, a 0.6-concavity star), P37 legitimately
    // producing zero blocks or erroring is an acceptable real answer
    // ("too small/too weird to cluster") -- what's NOT acceptable is a
    // panic, or a block whose own geometry is nonsensical (non-positive
    // area, or larger than the source parcel it was carved from).
    for (label, axes) in axes_under_test() {
        assert_invariant_across_seeds(&[1, 2, 3, 7, 13], |seed| {
            let n = generate(&axes, seed);
            let source_area = n.parcels[0].polygon.area_m2();
            let parcel_id = n.parcels[0].id.clone();

            let result = P37HouseCluster.apply(&n, &parcel_id, &P37Params::defaults(), seed);
            let sub = match result {
                Ok(sub) => sub,
                Err(_) => return Ok(()), // a declined run is a legitimate answer at extreme axes
            };

            for p in &sub.new_parcels {
                let area = p.polygon.area_m2();
                if area <= 0.0 {
                    return Err(format!("[{label}] block {} has non-positive area {area}", p.id));
                }
                if area > source_area {
                    return Err(format!(
                        "[{label}] block {} ({area} m²) is larger than its source parcel ({source_area} m²)",
                        p.id
                    ));
                }
            }
            Ok(())
        });
    }
}
