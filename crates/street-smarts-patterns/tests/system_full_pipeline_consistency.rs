//! Closing verification for Phase 5B (IMPLEMENTATION_PLAN.md's "the real
//! migration -- touches every operator that currently reads
//! spec/use_category/density_tier"): runs the REAL, full 14-step
//! corrected pipeline (not a synthetic fixture, not one operator in
//! isolation) on the real Military Circle baseline, converts the result to
//! `World`, and checks every component map is fully and correctly
//! populated from whatever the pipeline actually produced -- proving the
//! generic derivation genuinely covers the whole operator set, not just
//! the handful with native ports.

use street_smarts_core::components::{BuildingTypology, DensityTier, PadRole, StreetClassification};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::world::World;
use street_smarts_patterns::p37_house_cluster::P37Params;
use street_smarts_patterns::pipeline::run_corrected_pipeline_with_p37;
use street_smarts_patterns::Parameters;

fn eastside_baseline() -> Neighborhood {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    serde_json::from_str(&raw).expect("parseable")
}

#[test]
fn every_component_map_matches_the_real_full_pipeline_output() {
    let baseline = eastside_baseline();
    let final_nbhd = run_corrected_pipeline_with_p37(
        &baseline,
        "MILITARY_CIRCLE_ASSEMBLED",
        42,
        &P37Params::defaults(),
    );
    // Sanity: this really did run the full sequence, not a truncated one --
    // real blocks, pads, buildings, AND streets all present.
    assert!(final_nbhd.parcels.iter().any(|p| p.density_tier.is_some()), "expected at least one density-tagged block");
    assert!(!final_nbhd.buildings.is_empty(), "expected real buildings from a full pipeline run");
    assert!(!final_nbhd.streets.is_empty(), "expected real streets from a full pipeline run");

    let world = World::from_neighborhood(&final_nbhd);

    // density_tiers: every parcel with a real, parseable density_tier
    // string must have a matching component entry, and vice versa --
    // exact set equality, not just "some entries exist".
    let expected_density: std::collections::BTreeMap<String, DensityTier> = final_nbhd.parcels.iter()
        .filter_map(|p| Some((p.id.clone(), DensityTier::from_label(p.density_tier.as_deref()?)?)))
        .collect();
    assert_eq!(world.density_tiers, expected_density);
    assert!(!expected_density.is_empty(), "expected at least one real density-tagged block on this fixture");

    // pad_roles: same exact-match check against Parcel.use_category.
    let expected_pad_roles: std::collections::BTreeMap<String, PadRole> = final_nbhd.parcels.iter()
        .filter_map(|p| Some((p.id.clone(), PadRole::from_label(p.use_category.as_deref()?)?)))
        .collect();
    assert_eq!(world.pad_roles, expected_pad_roles);
    assert!(!expected_pad_roles.is_empty(), "expected at least one real pad-tagged parcel on this fixture");

    // building_typologies: same exact-match check against Building.typology.
    let expected_typologies: std::collections::BTreeMap<String, BuildingTypology> = final_nbhd.buildings.iter()
        .filter_map(|b| Some((b.id.clone(), BuildingTypology::from_label(b.typology.as_deref()?)?)))
        .collect();
    assert_eq!(world.building_typologies, expected_typologies);
    assert!(!expected_typologies.is_empty(), "expected at least one real building on this fixture");
    // Real variety, not a degenerate single-typology result -- this site
    // is large enough that both branches should fire.
    let has_solid = expected_typologies.values().any(|t| !t.is_courtyard());
    let has_courtyard = expected_typologies.values().any(|t| t.is_courtyard());
    assert!(has_solid && has_courtyard, "expected both solid and courtyard buildings on a site this size");

    // street_classifications: same exact-match check against
    // Street.classification.
    let expected_classifications: std::collections::BTreeMap<String, StreetClassification> = final_nbhd.streets.iter()
        .filter_map(|s| Some((s.id.clone(), StreetClassification::from_label(s.classification.as_deref()?)?)))
        .collect();
    assert_eq!(world.street_classifications, expected_classifications);
    assert!(!expected_classifications.is_empty(), "expected at least one real street on this fixture");
}

#[test]
fn world_round_trips_the_real_full_pipeline_output_content_wise() {
    // The Phase A guarantee (from_neighborhood/to_neighborhood preserve
    // content) still has to hold on real, full-pipeline output -- not just
    // the synthetic/raw-baseline fixtures world.rs's own tests already
    // cover -- since that's the actual shape System::run relies on.
    let baseline = eastside_baseline();
    let final_nbhd = run_corrected_pipeline_with_p37(
        &baseline,
        "MILITARY_CIRCLE_ASSEMBLED",
        42,
        &P37Params::defaults(),
    );
    let world = World::from_neighborhood(&final_nbhd);
    let round_tripped = world.to_neighborhood();

    let mut a = final_nbhd;
    let mut b = round_tripped;
    a.parcels.sort_by(|x, y| x.id.cmp(&y.id));
    b.parcels.sort_by(|x, y| x.id.cmp(&y.id));
    a.buildings.sort_by(|x, y| x.id.cmp(&y.id));
    b.buildings.sort_by(|x, y| x.id.cmp(&y.id));
    a.streets.sort_by(|x, y| x.id.cmp(&y.id));
    b.streets.sort_by(|x, y| x.id.cmp(&y.id));
    a.open_space.sort_by(|x, y| x.id.cmp(&y.id));
    b.open_space.sort_by(|x, y| x.id.cmp(&y.id));

    assert_eq!(a.parcels, b.parcels);
    assert_eq!(a.buildings, b.buildings);
    assert_eq!(a.streets, b.streets);
    assert_eq!(a.open_space, b.open_space);
}
