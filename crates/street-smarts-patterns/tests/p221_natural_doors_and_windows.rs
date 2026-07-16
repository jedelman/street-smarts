use street_smarts_core::nir::{Neighborhood, OpeningKind};
use street_smarts_patterns::apply_subdivision;
use street_smarts_patterns::p107_wings_of_light::{P107Params, P107WingsOfLight};
use street_smarts_patterns::p221_natural_doors_and_windows::{P221NaturalDoorsAndWindows, P221Params};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::{Parameters, PatternOperator};

/// Chain P95 -> P107 -> P221 on real fixture data (same fixture P107's own
/// `real_p95_pads_from_mall_parcel_shape_without_error` test uses), the same
/// real-data smoke test every stage in this pipeline gets, not just a
/// synthetic rectangle.
#[test]
fn real_p95_pads_get_openings_after_p107() {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let sub95 = P95BuildingComplex.apply(&baseline, "00001129", &P95Params::defaults(), 42).unwrap();
    let with_pads = apply_subdivision(&baseline, &sub95);

    let sub107 = P107WingsOfLight.apply(&with_pads, "*", &P107Params::defaults(), 42).unwrap();
    let with_buildings = apply_subdivision(&with_pads, &sub107);
    assert!(!with_buildings.buildings.is_empty(), "P107 should have produced real buildings");
    assert!(
        with_buildings.buildings.iter().all(|b| b.openings.is_empty()),
        "buildings shouldn't have openings before P221 runs"
    );

    let sub221 = P221NaturalDoorsAndWindows
        .apply(&with_buildings, "*", &P221Params::defaults(), 42)
        .expect("P221 should place openings on real P107 output");
    let with_openings = apply_subdivision(&with_buildings, &sub221);

    assert_eq!(
        with_openings.buildings.len(),
        with_buildings.buildings.len(),
        "P221 should replace each building in place (same count), not duplicate or drop any"
    );

    let mut n_with_windows = 0;
    let mut n_with_doors = 0;
    for b in &with_openings.buildings {
        assert!(b.floors.is_some(), "every building should have a floor count after P221");
        if b.openings.iter().any(|o| o.kind == OpeningKind::Window) {
            n_with_windows += 1;
        }
        if b.openings.iter().any(|o| o.kind == OpeningKind::Door) {
            n_with_doors += 1;
        }
    }
    eprintln!(
        "P221 on real P107 output: {} buildings, {} with windows, {} with doors",
        with_openings.buildings.len(), n_with_windows, n_with_doors
    );
    assert!(n_with_windows > 0, "at least some real buildings should have gotten windows");
    assert!(n_with_doors > 0, "at least some real buildings should have gotten a door");
}

#[test]
fn params_roundtrip() {
    let p = P221Params { room_width_m: 5.0, size_falloff_per_floor: 0.9, ..P221Params::defaults() };
    let v = p.as_vector();
    let back = P221Params::from_vector(&v);
    assert_eq!(back.room_width_m, 5.0);
    assert_eq!(back.size_falloff_per_floor, 0.9);
}
