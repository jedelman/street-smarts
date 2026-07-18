//! P28 Eccentric Nucleus — density should peak somewhere real and
//! off-center, not spread as a random or perfectly-centered pattern.
//!
//! From Alexander, *A Pattern Language*, Pattern 28, via
//! patternlanguage.cc/Patterns/Eccentric-Nucleus-(28):
//! > **Problem:** The random character of local densities confuses the
//! > identity of our communities, and also creates a chaos in the pattern
//! > of land use.
//! > **Solution:** Encourage growth and the accumulation of density to
//! > form a clear configuration of peaks and valleys.
//!
//! # Reusing a real, already-computed density signal
//!
//! `p29_density_rings` already tags every `BLOCK_n` parcel with a real
//! `DensityTier` (`Core`/`Middle`/`Edge`), radiating from its own
//! area-weighted centroid -- see `street-smarts-core::components`. This
//! opinion doesn't recompute density; it checks a real structural claim
//! about that existing gradient: does a real, non-trivial `Core` peak
//! exist at all (not every block flattened to one tier), and does it sit
//! at a meaningfully ECCENTRIC position relative to the raw site's own
//! geometric bounding-box center -- "eccentric" in Alexander's literal
//! sense (off-center), not dead center by construction.
//!
//! # Two sub-scores
//!
//! - `has_real_peak`: whether at least one, but not all, blocks are
//!   tagged `Core` (a real peak exists, distinguishable from a flat
//!   uniform density).
//! - `eccentricity`: how far the mean position of `Core`-tagged blocks
//!   sits from the site's own bounding-box center, normalized against the
//!   site's own bounding-box half-diagonal -- `0.0` at dead center, `1.0`
//!   at the bounding box's own edge.

use std::collections::BTreeMap;
use street_smarts_core::components::DensityTier;
use street_smarts_core::geometry::haversine_m;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;
use street_smarts_core::Scope;

pub struct P28EccentricNucleus;

impl Opinion for P28EccentricNucleus {
    fn name(&self) -> &'static str {
        "p28_eccentric_nucleus"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p28".into(),
            display: "Alexander et al., A Pattern Language, Pattern 28 (Eccentric Nucleus)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Eccentric-Nucleus-(28)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let blocks: Vec<_> = n.select(&Scope::Block).collect();
        if blocks.is_empty() {
            return OpinionOutput::NoView {
                reason: "No BLOCK_n parcels -- run P37 House Cluster first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        let tiers: Vec<Option<DensityTier>> = blocks.iter()
            .map(|b| b.density_tier.as_deref().and_then(DensityTier::from_label))
            .collect();
        if tiers.iter().all(|t| t.is_none()) {
            return OpinionOutput::NoView {
                reason: "No block has a real density_tier yet -- run P29 Density Rings first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let n_core = tiers.iter().filter(|t| **t == Some(DensityTier::Core)).count();
        let has_real_peak = if n_core > 0 && n_core < blocks.len() { 1.0 } else { 0.0 };

        let (mut min_lng, mut max_lng, mut min_lat, mut max_lat) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for b in &blocks {
            for p in &b.polygon.outer {
                min_lng = min_lng.min(p.lng);
                max_lng = max_lng.max(p.lng);
                min_lat = min_lat.min(p.lat);
                max_lat = max_lat.max(p.lat);
            }
        }
        let center = street_smarts_core::geometry::LngLat::new((min_lng + max_lng) / 2.0, (min_lat + max_lat) / 2.0);
        let corner = street_smarts_core::geometry::LngLat::new(max_lng, max_lat);
        let half_diagonal = haversine_m(&center, &corner).max(1.0);

        let eccentricity = if n_core == 0 {
            None
        } else {
            let core_centroids: Vec<_> = blocks.iter().zip(tiers.iter())
                .filter(|(_, t)| **t == Some(DensityTier::Core))
                .map(|(b, _)| b.polygon.centroid())
                .collect();
            let (mut cx, mut cy) = (0.0, 0.0);
            for c in &core_centroids {
                cx += c.lng;
                cy += c.lat;
            }
            let core_center = street_smarts_core::geometry::LngLat::new(cx / core_centroids.len() as f64, cy / core_centroids.len() as f64);
            Some((haversine_m(&center, &core_center) / half_diagonal).clamp(0.0, 1.0))
        };

        let value = match eccentricity {
            Some(e) => 0.5 * has_real_peak + 0.5 * e,
            None => has_real_peak,
        };

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("has_real_peak".into(), has_real_peak);
        if let Some(e) = eccentricity {
            sub_scores.insert("eccentricity".into(), e);
        }

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_blocks".into(), blocks.len().to_string());
        details.insert("n_core_blocks".into(), n_core.to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} block(s), {} tagged Core (density peak). {}",
                blocks.len(), n_core,
                eccentricity.map(|e| format!("Core center sits {:.0}% of the way to the site's own bounding-box edge from dead-center.", e * 100.0)).unwrap_or_else(|| "No Core blocks to check eccentricity against.".into()),
            ),
            sub_scores,
            details,
            caveats: vec![
                "Reuses p29_density_rings's own DensityTier output rather than an independent \
                 density measure -- if P29 hasn't run with meaningful parameters, this opinion \
                 has nothing real to check.".into(),
                "'Eccentric' is checked against the site's own bounding-box center, not a real \
                 town/region-scale center -- P29's own density center IS the blocks' area-weighted \
                 centroid, so a P29-driven peak is somewhat eccentric by construction relative to \
                 the (different) bounding-box center used here, not fully independent \
                 confirmation.".into(),
            ],
            contributing_features: vec![],
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
                layer_provenance: Default::default(), label: "P28 unit fixture".into(),
            },
        }
    }

    fn block(id: &str, x_m: f64, y_m: f64, tier: Option<&str>) -> Parcel {
        let m = 1.0 / 111_320.0;
        let half = 10.0;
        Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new((x_m - half) * m, (y_m - half) * m), LngLat::new((x_m + half) * m, (y_m - half) * m),
                LngLat::new((x_m + half) * m, (y_m + half) * m), LngLat::new((x_m - half) * m, (y_m + half) * m),
                LngLat::new((x_m - half) * m, (y_m - half) * m),
            ]),
            area_acres: 1.0, use_category: None, ownership: None, is_eda: true,
            spec: Some("BLOCK_1".into()), density_tier: tier.map(String::from), target_stories: None,
        }
    }

    #[test]
    fn no_blocks_is_no_view() {
        assert!(matches!(P28EccentricNucleus.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn no_density_tiers_is_no_view() {
        let n = nbhd(vec![block("B1", 0.0, 0.0, None)]);
        assert!(matches!(P28EccentricNucleus.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn all_blocks_core_has_no_real_peak() {
        let n = nbhd(vec![block("B1", 0.0, 0.0, Some("core")), block("B2", 100.0, 0.0, Some("core"))]);
        let out = P28EccentricNucleus.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => assert!((sub_scores["has_real_peak"] - 0.0).abs() < 1e-6),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn an_off_center_core_block_scores_high_eccentricity() {
        let n = nbhd(vec![
            block("EDGE1", -200.0, 0.0, Some("edge")), block("EDGE2", 200.0, 0.0, Some("edge")),
            block("EDGE3", 0.0, 200.0, Some("edge")), block("EDGE4", 0.0, -200.0, Some("edge")),
            block("CORE1", 180.0, 180.0, Some("core")), // near a corner, not the center
        ]);
        let out = P28EccentricNucleus.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["has_real_peak"] - 1.0).abs() < 1e-6);
                assert!(sub_scores["eccentricity"] > 0.5, "got {}", sub_scores["eccentricity"]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
