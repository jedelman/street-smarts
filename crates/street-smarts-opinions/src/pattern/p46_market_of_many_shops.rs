//! P46 Market of Many Shops — a market should be many small, autonomous
//! shops clustered together, not one large single-management store.
//!
//! From Alexander, *A Pattern Language*, Pattern 46, via
//! patternlanguage.cc/Patterns/Market-of-Many-Shops-(46):
//! > **Problem:** It is natural and convenient to want a market where all
//! > the different foods and household goods you need can be bought under
//! > a single roof. But when the market has a single management, like a
//! > supermarket, the foods are bland, and there is no joy in going there.
//! > **Solution:** Instead of modern supermarkets, establish frequent
//! > marketplaces, each one made up of many smaller shops which are
//! > autonomous and specialized... Build the structure of the market as a
//! > minimum, which provides no more than a roof, columns which define
//! > aisles, and basic services.
//!
//! # A real, checkable cluster proxy
//!
//! This schema has no per-stall subdivision (`Parcel`/`Building` don't
//! break a market into individual shop units), so this opinion checks the
//! nearest available proxy: real commercial-use `Parcel`s that are both
//! small (`<= max_shop_area_m2`, default 500m² -- a modest single-shop
//! footprint, not a supermarket box) AND clustered (`>= min_cluster_size`,
//! default 5, other small-commercial parcels within `cluster_threshold_m`,
//! default 30m). `value` = fraction of commercial parcels meeting both.
//!
//! Cannot check "many smaller shops... autonomous and specialized" (no
//! per-shop ownership or specialty data), the aisle-width or stall-size
//! specifications, or whether the structure is genuinely a shared
//! minimal-roof frame rather than several separate buildings that happen
//! to sit close together.

use std::collections::BTreeMap;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P46MarketOfManyShops;

const MAX_SHOP_AREA_M2: f64 = 500.0;
const CLUSTER_THRESHOLD_M: f64 = 30.0;
const MIN_CLUSTER_SIZE: usize = 5;

impl Opinion for P46MarketOfManyShops {
    fn name(&self) -> &'static str {
        "p46_market_of_many_shops"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p46".into(),
            display: "Alexander et al., A Pattern Language, Pattern 46 (Market of Many Shops)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Market-of-Many-Shops-(46)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let small_commercial: Vec<_> = n.parcels.iter()
            .filter(|p| p.use_category.as_deref() == Some("commercial") && p.polygon.area_m2() <= MAX_SHOP_AREA_M2)
            .collect();
        if small_commercial.is_empty() {
            return OpinionOutput::NoView {
                reason: "No small commercial-use parcels to check market clustering against yet.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut in_market: Vec<bool> = Vec::new();
        let mut isolated: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for p in &small_commercial {
            let c = p.polygon.centroid();
            let cluster_size = 1 + small_commercial.iter().filter(|o| o.id != p.id && haversine_m(&c, &o.polygon.centroid()) <= CLUSTER_THRESHOLD_M).count();
            let ok = cluster_size >= MIN_CLUSTER_SIZE;
            in_market.push(ok);
            details.insert(format!("{}.cluster_size", p.id), cluster_size.to_string());
            if !ok {
                isolated.push(p.id.clone());
            }
        }

        let value = in_market.iter().filter(|b| **b).count() as f64 / in_market.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("in_a_real_market_cluster".into(), value);
        details.insert("n_small_commercial_parcels".into(), small_commercial.len().to_string());
        details.insert("min_cluster_size".into(), MIN_CLUSTER_SIZE.to_string());
        details.insert("cluster_threshold_m".into(), format!("{CLUSTER_THRESHOLD_M:.0}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} small commercial parcel(s) (<= {:.0}m²); {:.0}% belong to a real cluster of \
                 {} or more within {:.0}m.",
                small_commercial.len(), MAX_SHOP_AREA_M2, value * 100.0, MIN_CLUSTER_SIZE, CLUSTER_THRESHOLD_M
            ),
            sub_scores,
            details,
            caveats: vec![
                "No per-stall subdivision exists in this schema -- 'many shops' is approximated \
                 by counting nearby small commercial PARCELS, not confirmed individual stalls \
                 under one shared roof.".into(),
                "Cannot check 'autonomous and specialized' (no per-shop ownership or specialty \
                 data), the aisle-width (6-12ft) or stall-size (~6x9ft) specifications, or \
                 whether the cluster is a genuine shared minimal-roof structure.".into(),
            ],
            contributing_features: isolated,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, Parcel};

    fn nbhd(parcels: Vec<Parcel>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels, buildings: vec![], streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P46 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn small_shop(id: &str, x_m: f64, y_m: f64) -> Parcel {
        let m = 1.0 / 111_320.0;
        let side = 8.0;
        Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x_m * m, y_m * m), LngLat::new((x_m + side) * m, y_m * m),
                LngLat::new((x_m + side) * m, (y_m + side) * m), LngLat::new(x_m * m, (y_m + side) * m),
                LngLat::new(x_m * m, y_m * m),
            ]),
            area_acres: 0.02, use_category: Some("commercial".into()), ownership: None, is_eda: false, spec: None, density_tier: None, target_stories: None,
        }
    }

    #[test]
    fn no_small_commercial_parcels_is_no_view() {
        assert!(matches!(P46MarketOfManyShops.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_tight_cluster_of_five_shops_scores_full() {
        let parcels: Vec<Parcel> = (0..5).map(|i| small_shop(&format!("S{i}"), i as f64 * 5.0, 0.0)).collect();
        let out = P46MarketOfManyShops.evaluate(&nbhd(parcels));
        match out {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-6, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn an_isolated_shop_is_flagged() {
        let mut parcels: Vec<Parcel> = (0..5).map(|i| small_shop(&format!("S{i}"), i as f64 * 10.0, 0.0)).collect();
        parcels.push(small_shop("LONER", 5000.0, 5000.0));
        let out = P46MarketOfManyShops.evaluate(&nbhd(parcels));
        match out {
            OpinionOutput::Value { contributing_features, .. } => {
                assert!(contributing_features.contains(&"LONER".to_string()));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
