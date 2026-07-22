//! Regression test for min_pad_short_side_m: a pad can clear
//! min_pad_area_m2 easily while still being an unbuildable sliver (a long
//! thin strip). Caught via real pipeline output where several standalone
//! pads had a height/width ratio over 1.3 -- fins, not buildings.

use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, NeighborhoodMeta, Parcel};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::{Parameters, PatternOperator};

const M_PER_DEG_LNG: f64 = 111_320.0;
const M_PER_DEG_LAT: f64 = 110_540.0;

fn rect_parcel_neighborhood(width_m: f64, depth_m: f64) -> Neighborhood {
    let w = width_m / M_PER_DEG_LNG;
    let d = depth_m / M_PER_DEG_LAT;
    let ring = vec![
        LngLat::new(0.0, 0.0),
        LngLat::new(w, 0.0),
        LngLat::new(w, d),
        LngLat::new(0.0, d),
    ];
    let parcel = Parcel {
        id: "RAW".into(),
        polygon: Polygon::from_ring(ring),
        area_acres: (width_m * depth_m) / 4046.86,
        use_category: None,
        ownership: None,
        is_eda: true,
        spec: None,
        density_tier: None,
        target_stories: None,
    };
    Neighborhood {
        id: "test".into(),
        bbox_wgs84: [0.0, 0.0, w, d],
        parcels: vec![parcel],
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
            label: "P95 sliver-pad fixture".into(),
        },
            pattern_fields: vec![],
        }
}

#[test]
fn a_long_thin_parcel_produces_no_real_pads_only_slivers_get_filtered() {
    // 200m x 4m: individual Voronoi cells along this strip easily clear
    // min_pad_area_m2 (120) -- a 4m x 50m cell is 200 m² -- but every one of
    // them is stuck at ~4m wide, well under min_pad_short_side_m's 7m
    // default. Not a real floor plate at any height, regardless of area.
    let nbhd = rect_parcel_neighborhood(200.0, 4.0);
    let params = P95Params { min_buildings: 3.0, max_buildings: 5.0, ..P95Params::defaults() };
    let sub = P95BuildingComplex.apply(&nbhd, "RAW", &params, 1);
    // Either it errors outright (nothing usable at all) or it succeeds with
    // zero real building pads (a courtyard cell alone, no sliver pads) --
    // both are correct; a non-empty new_parcels list would mean a sliver
    // slipped through.
    match sub {
        Err(_) => {}
        Ok(s) => assert!(s.new_parcels.is_empty(), "no sliver pad should have made it through, got {} pad(s)", s.new_parcels.len()),
    }
}

#[test]
fn a_reasonably_proportioned_parcel_still_produces_a_pad() {
    let nbhd = rect_parcel_neighborhood(40.0, 30.0);
    let sub = P95BuildingComplex.apply(&nbhd, "RAW", &P95Params::defaults(), 1).expect("a 40x30m parcel should produce real pads");
    assert!(!sub.new_parcels.is_empty());
}

#[test]
fn min_pad_short_side_m_roundtrips_through_the_param_vector() {
    let p = P95Params { min_pad_short_side_m: 9.5, ..P95Params::defaults() };
    let v = p.as_vector();
    let back = P95Params::from_vector(&v);
    assert_eq!(back.min_pad_short_side_m, 9.5);
    // Sanity: the field after it in the vector (courtyard_mode) shouldn't
    // have shifted onto the wrong index.
    assert_eq!(back.courtyard_mode, P95Params::defaults().courtyard_mode);
}
