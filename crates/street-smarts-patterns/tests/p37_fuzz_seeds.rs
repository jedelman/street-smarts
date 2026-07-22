//! P37 House Cluster's own invariants (from `tests/p37_house_cluster.rs`'s
//! `carves_a_large_parcel_into_several_blocks`), generalized across many
//! seeds via the shared harness in `tests/common/mod.rs`. See
//! PATTERN_LANGUAGE_SIMULATION.md §4.4.

mod common;

use common::{assert_invariant_across_seeds, DEFAULT_SEEDS};
use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, NeighborhoodMeta, Parcel};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::{Parameters, PatternOperator};

fn square_parcel_neighborhood(side_m: f64, id: &str) -> Neighborhood {
    let m_per_deg = 111_320.0;
    let s = side_m / m_per_deg;
    let ring = vec![
        LngLat::new(0.0, 0.0),
        LngLat::new(s, 0.0),
        LngLat::new(s, s),
        LngLat::new(0.0, s),
    ];
    Neighborhood {
        id: "test".into(),
        bbox_wgs84: [0.0, 0.0, s, s],
        parcels: vec![Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(ring),
            area_acres: (side_m * side_m) / 4046.86,
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
            label: "P37 fuzz fixture".into(),
        },
            pattern_fields: vec![],
        }
}

#[test]
fn block_count_and_area_invariants_hold_across_many_seeds() {
    let nbhd = square_parcel_neighborhood(300.0, "MEGA_1");
    let params = P37Params::defaults();

    assert_invariant_across_seeds(DEFAULT_SEEDS, |seed| {
        let sub = P37HouseCluster
            .apply(&nbhd, "MEGA_1", &params, seed)
            .map_err(|e| format!("apply failed: {e}"))?;

        if sub.new_parcels.len() < 2 {
            return Err(format!("expected multiple blocks, got {}", sub.new_parcels.len()));
        }
        if sub.new_parcels.len() > params.max_blocks as usize {
            return Err(format!(
                "expected at most max_blocks={} blocks, got {}",
                params.max_blocks as usize,
                sub.new_parcels.len()
            ));
        }

        for p in &sub.new_parcels {
            if !p.spec.as_deref().unwrap_or("").starts_with("BLOCK_") {
                return Err(format!("block {} not tagged BLOCK_n (spec={:?})", p.id, p.spec));
            }
            if p.use_category.as_deref() != Some("house_cluster_block") {
                return Err(format!("block {} has wrong use_category {:?}", p.id, p.use_category));
            }
            let area = p.polygon.area_m2();
            if area <= 0.0 {
                return Err(format!("block {} has non-positive area {area}", p.id));
            }
            if area >= 90_000.0 {
                return Err(format!("block {} ({area} m²) swallowed the whole parcel", p.id));
            }
        }

        let total_area: f64 = sub.new_parcels.iter().map(|p| p.polygon.area_m2()).sum();
        if total_area <= 90_000.0 * 0.5 {
            return Err(format!("blocks retained too little area after inset: {total_area} m²"));
        }

        Ok(())
    });
}
