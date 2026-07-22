//! Real integration tests for P29 Density Rings, against the real baseline
//! mall parcel -- complements the synthetic unit tests in
//! `p29_density_rings.rs`'s own `#[cfg(test)]` module (which cover the
//! sampling math precisely on simple squares) with checks against real,
//! irregular site geometry, and the real end-to-end story: P29 attaches a
//! field to the RAW parcel, `p37_house_cluster` samples it into real
//! blocks.
//!
//! Was built around the pre-field version, which ran P29 on already-carved
//! `BLOCK_n` parcels directly (`parcel_id == "*"`) -- see
//! `PATTERN_ORDERING_AUDIT.md` item 1 and `p29_density_rings.rs`'s own
//! "v0.3" module doc for why that changed. Rewritten for the real, current
//! contract: P29 takes the specific raw site parcel_id and only ever
//! attaches a `DensityField`; it never touches a parcel directly anymore.

use street_smarts_core::nir::{Neighborhood, PatternField};
use street_smarts_patterns::p29_density_rings::{sample_density_field, P29DensityRings, P29Params};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};

const MALL_PARCEL_ID: &str = "00001129";

fn real_baseline() -> Neighborhood {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    serde_json::from_str(&raw).expect("parseable")
}

fn attach_field(nbhd: &Neighborhood, params: &P29Params) -> Neighborhood {
    let sub = P29DensityRings.apply(nbhd, MALL_PARCEL_ID, params, 7).expect("P29 should compute a field for the real mall parcel");
    apply_subdivision(nbhd, &sub)
}

#[test]
fn field_is_attached_with_a_real_center_and_radius() {
    let baseline = real_baseline();
    let with_field = attach_field(&baseline, &P29Params::defaults());
    assert_eq!(with_field.pattern_fields.len(), 1);
    let PatternField::Density(field) = &with_field.pattern_fields[0];
    assert!(field.radius_m > 10.0, "a real 47-acre site should have a real, non-trivial radius, got {}", field.radius_m);
    assert_eq!(field.n_rings, 3);
}

/// P28 Eccentric Nucleus: with `eccentricity_frac > 0`, the field's own
/// real center should shift measurably away from the raw parcel's plain
/// vertex-averaged centroid.
#[test]
fn eccentricity_frac_measurably_shifts_the_field_center() {
    let baseline = real_baseline();
    let centered = attach_field(&baseline, &P29Params { eccentricity_frac: 0.0, ..P29Params::defaults() });
    let eccentric = attach_field(&baseline, &P29Params { eccentricity_frac: 0.6, ..P29Params::defaults() });

    let PatternField::Density(d0) = &centered.pattern_fields[0];
    let PatternField::Density(d1) = &eccentric.pattern_fields[0];
    let shift_m = street_smarts_core::geometry::haversine_m(&d0.center, &d1.center);
    assert!(shift_m > 1.0, "a higher eccentricity_frac should measurably shift the real field center, got {shift_m:.2}m");
}

#[test]
fn respects_custom_core_and_edge_targets() {
    let baseline = real_baseline();
    let params = P29Params { n_rings: 2.0, core_target_stories: 10.0, edge_target_stories: 1.0, ..P29Params::defaults() };
    let with_field = attach_field(&baseline, &params);
    let PatternField::Density(field) = &with_field.pattern_fields[0];

    let (label_at_center, stories_at_center) = sample_density_field(field, field.center);
    assert_eq!(label_at_center, "core");
    assert!((stories_at_center - 10.0).abs() < 1e-6);

    let far = street_smarts_core::geometry::LngLat::new(field.center.lng + 1.0, field.center.lat);
    let (label_far, stories_far) = sample_density_field(field, far);
    assert_eq!(label_far, "edge");
    assert!((stories_far - 1.0).abs() < 1e-6);
}

#[test]
fn wildcard_parcel_id_errors_instead_of_silently_doing_nothing() {
    let baseline = real_baseline();
    let result = P29DensityRings.apply(&baseline, "*", &P29Params::defaults(), 1);
    assert!(result.is_err());
}

#[test]
fn unknown_parcel_id_errors_instead_of_silently_doing_nothing() {
    let baseline = real_baseline();
    let result = P29DensityRings.apply(&baseline, "not_a_real_parcel_id", &P29Params::defaults(), 1);
    assert!(result.is_err());
}

/// The full real story: P29 attaches a field to the real raw parcel, then
/// `p37_house_cluster` samples it as it carves real blocks -- every block
/// gets a real `density_tier`/`target_stories`, and there's real variety
/// across them (not one flat value).
#[test]
fn p37_samples_the_field_into_a_real_gradient_across_real_blocks() {
    let baseline = real_baseline();
    let with_field = attach_field(&baseline, &P29Params::defaults());

    let sub37 = P37HouseCluster.apply(&with_field, MALL_PARCEL_ID, &P37Params::defaults(), 42).unwrap();
    assert!(sub37.new_parcels.len() >= 5, "expected several real blocks from the fixture");

    for p in &sub37.new_parcels {
        assert!(p.density_tier.is_some(), "{} should have a real density_tier", p.id);
        assert!(p.target_stories.is_some(), "{} should have a real target_stories", p.id);
    }

    let distinct_tiers: std::collections::BTreeSet<String> =
        sub37.new_parcels.iter().filter_map(|p| p.density_tier.clone()).collect();
    assert!(distinct_tiers.len() >= 2, "a real site with several blocks should span more than one density tier, got {distinct_tiers:?}");

    let core_stories = sub37.new_parcels.iter().find(|p| p.density_tier.as_deref() == Some("core")).and_then(|p| p.target_stories);
    assert_eq!(core_stories, Some(6.0), "the block nearest the density center should get the default core target");
}
