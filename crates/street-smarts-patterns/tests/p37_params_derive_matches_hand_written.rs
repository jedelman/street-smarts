//! Proves `#[derive(Parameters)]` on `P37Params` produces the same
//! schema/defaults/round-trip behavior the hand-written impl it replaced
//! had. See PATTERN_LANGUAGE_SIMULATION.md §3.3.

use street_smarts_patterns::p37_house_cluster::P37Params;
use street_smarts_patterns::Parameters;

#[test]
fn schema_matches_hand_written_values() {
    let schema = P37Params::schema();
    assert_eq!(schema.len(), 9);

    let by_name: std::collections::HashMap<_, _> =
        schema.iter().map(|s| (s.name.as_str(), s)).collect();

    let s = by_name["target_block_area_m2"];
    assert_eq!((s.min, s.max, s.default), (2000.0, 20000.0, 7000.0));
    assert_eq!(s.unit.as_deref(), Some("m²"));
    assert!(!s.integer);

    let s = by_name["min_blocks"];
    assert_eq!((s.min, s.max, s.default), (1.0, 10.0, 2.0));
    assert!(s.integer);

    let s = by_name["seeding_mode"];
    assert_eq!((s.min, s.max, s.default), (0.0, 1.0, 0.0));
    assert_eq!(s.unit, None);
}

#[test]
fn defaults_match_hand_written_struct() {
    let d = P37Params::defaults();
    assert_eq!(d.target_block_area_m2, 7000.0);
    assert_eq!(d.min_blocks, 2.0);
    assert_eq!(d.max_blocks, 12.0);
    assert_eq!(d.block_inset_m, 10.0);
    assert_eq!(d.seed_jitter, 0.5);
    assert_eq!(d.min_block_area_m2, 1500.0);
    assert_eq!(d.common_land_fraction, 0.12);
    assert_eq!(d.min_common_land_area_m2, 150.0);
    assert_eq!(d.seeding_mode, 0.0);
}

#[test]
fn as_vector_is_in_schema_order() {
    let d = P37Params::defaults();
    assert_eq!(
        d.as_vector(),
        vec![7000.0, 2.0, 12.0, 10.0, 0.5, 1500.0, 0.12, 150.0, 0.0]
    );
}

#[test]
fn from_vector_round_trips_and_clamps() {
    let v = vec![9000.0, 3.0, 15.0, 12.0, 0.8, 2000.0, 0.2, 200.0, 1.0];
    let p = P37Params::from_vector(&v);
    assert_eq!(p.as_vector(), v);

    // Out-of-range values clamp to the declared bounds, same as the
    // hand-written impl's `ParamSpec::clamp` calls did.
    let out_of_range = vec![999_999.0, -5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let clamped = P37Params::from_vector(&out_of_range);
    assert_eq!(clamped.target_block_area_m2, 20000.0); // clamped to max
    assert_eq!(clamped.min_blocks, 1.0); // clamped to min
}

#[test]
fn from_vector_short_vector_keeps_remaining_defaults() {
    let short = vec![8000.0, 4.0];
    let p = P37Params::from_vector(&short);
    assert_eq!(p.target_block_area_m2, 8000.0);
    assert_eq!(p.min_blocks, 4.0);
    assert_eq!(p.max_blocks, 12.0); // default, untouched
}
