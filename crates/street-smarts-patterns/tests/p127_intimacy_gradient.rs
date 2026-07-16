use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::apply_subdivision;
use street_smarts_patterns::p107_wings_of_light::{P107Params, P107WingsOfLight};
use street_smarts_patterns::p127_intimacy_gradient::{P127IntimacyGradient, P127Params};
use street_smarts_patterns::p129_common_areas_at_the_heart::{P129CommonAreasAtTheHeart, P129Params};
use street_smarts_patterns::p131_the_flow_through_rooms::{P131Params, P131TheFlowThroughRooms};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::{Parameters, PatternOperator};

/// Chain P95 -> P107 -> P127 -> P129 -> P131 on real fixture data -- same
/// real-data smoke test every stage in this pipeline gets (mirrors
/// p221_natural_doors_and_windows.rs's own `real_p95_pads_get_openings_after_p107`).
#[test]
fn real_p107_buildings_get_a_connected_interior_gradient() {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let sub95 = P95BuildingComplex.apply(&baseline, "00001129", &P95Params::defaults(), 42).unwrap();
    let with_pads = apply_subdivision(&baseline, &sub95);

    let sub107 = P107WingsOfLight.apply(&with_pads, "*", &P107Params::defaults(), 42).unwrap();
    let with_buildings = apply_subdivision(&with_pads, &sub107);
    assert!(!with_buildings.buildings.is_empty());

    let sub127 = P127IntimacyGradient
        .apply(&with_buildings, "*", &P127Params::defaults(), 42)
        .expect("P127 should partition real P107 buildings");
    let with_cells = apply_subdivision(&with_buildings, &sub127);
    assert_eq!(with_cells.buildings.len(), with_buildings.buildings.len());
    assert!(with_cells.buildings.iter().all(|b| !b.interior_cells.is_empty()));

    let sub129 = P129CommonAreasAtTheHeart
        .apply(&with_cells, "*", &P129Params::defaults(), 42)
        .expect("P129 should mark a common cell");
    let with_common = apply_subdivision(&with_cells, &sub129);
    for b in &with_common.buildings {
        let n_common = b.interior_cells.iter().filter(|c| c.is_common).count();
        assert_eq!(n_common, 1, "{} should have exactly one common cell, got {}", b.id, n_common);
    }

    let sub131 = P131TheFlowThroughRooms
        .apply(&with_common, "*", &P131Params::defaults(), 42)
        .expect("P131 should connect real cells");
    let with_flow = apply_subdivision(&with_common, &sub131);

    let mut n_multi_cell_buildings = 0;
    let mut n_closed_loops = 0;
    for b in &with_flow.buildings {
        if b.interior_cells.len() < 2 {
            continue;
        }
        n_multi_cell_buildings += 1;
        // Every cell in a multi-cell building should connect to at least
        // one other cell -- no orphaned rooms.
        assert!(
            b.interior_cells.iter().all(|c| !c.connects_to.is_empty()),
            "{}: every cell in a multi-cell building should have at least one connection",
            b.id
        );
        // A courtyard building's ring, or a solid building's closed
        // passage loop, should give every cell exactly 2 connections.
        if b.interior_cells.iter().all(|c| c.connects_to.len() == 2) {
            n_closed_loops += 1;
        }
    }
    eprintln!(
        "P127->P129->P131 on real P107 output: {} buildings, {} multi-cell, {} fully closed loops",
        with_flow.buildings.len(), n_multi_cell_buildings, n_closed_loops
    );
    assert!(n_multi_cell_buildings > 0, "at least some real buildings should have multiple interior cells");
}

#[test]
fn params_roundtrip() {
    let p = P127Params { band_depth_m: 6.0, ..P127Params::defaults() };
    let v = p.as_vector();
    let back = P127Params::from_vector(&v);
    assert_eq!(back.band_depth_m, 6.0);

    let p131 = P131Params { passage_width_m: 2.0, ..P131Params::defaults() };
    let v131 = p131.as_vector();
    let back131 = P131Params::from_vector(&v131);
    assert_eq!(back131.passage_width_m, 2.0);
}
