//! Levels of Scale — Alexander's first fundamental property of wholeness.
//!
//! From Salingaros 2025 (Frontiers of Architectural Research 14(6): 1491–1515),
//! canonical LLM-ready description of the property:
//!
//! > Successful designs incorporate different scales, from the dimension of
//! > the smallest detail up to the largest element, creating a sense of harmony.
//! > Each scale must be distinct, with scales in a hierarchy spaced closely enough
//! > in size (magnification) for scaling coherence, but not too close to blur
//! > the distinction between the next larger and smaller scales. Optimal
//! > magnification factors range between approximately 2 to 5.
//!
//! Implementation (this opinion's *encoded interpretation*, not a measurement):
//!
//! 1. Collect all sized features (parcels by area, buildings by footprint, open spaces by area).
//! 2. Sort, take a logarithmically-spaced sample of representative scales.
//! 3. Compute consecutive magnification ratios.
//! 4. Score 1.0 if all ratios are in [2, 5]; penalize ratios <1.5 (too close) or >5 (jumps).
//! 5. If too few features to form a hierarchy, return `NoView`.
//!
//! The author of THIS opinion is the algorithm below.
//! Other authors will encode the same property differently. The chorus disagrees.

use std::collections::BTreeMap;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct LevelsOfScale;

impl Opinion for LevelsOfScale {
    fn name(&self) -> &'static str { "levels_of_scale" }
    fn family(&self) -> OpinionFamily { OpinionFamily::Geometric }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "salingaros_2025_p1".into(),
            display: "Salingaros 2025, Property 1 of 15 (Levels of Scale)".into(),
            url: Some("https://doi.org/10.1016/j.foar.2025.01.002".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) { (0.0, 1.0) }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let mut sized: Vec<(f64, String, &'static str)> = Vec::new();
        for p in &n.parcels {
            let a = if p.area_acres > 0.0 {
                p.area_acres * 4046.86
            } else {
                p.polygon.area_m2()
            };
            if a > 0.0 { sized.push((a, p.id.clone(), "parcel")); }
        }
        for b in &n.buildings {
            let a = b.polygon.area_m2();
            if a > 0.0 { sized.push((a, b.id.clone(), "building")); }
        }
        for o in &n.open_space {
            let a = o.polygon.area_m2();
            if a > 0.0 { sized.push((a, o.id.clone(), "open_space")); }
        }

        if sized.len() < 4 {
            return OpinionOutput::NoView {
                reason: format!(
                    "Only {} sized features found; need at least 4 to discuss a scale hierarchy.",
                    sized.len()
                ),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        sized.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let min = sized.first().unwrap().0;
        let max = sized.last().unwrap().0;
        let log_min = min.ln();
        let log_max = max.ln();
        let span = log_max - log_min;

        if span < 0.5 {
            let mut details = BTreeMap::new();
            details.insert("smallest_m2".into(), format!("{:.0}", min));
            details.insert("largest_m2".into(), format!("{:.0}", max));
            details.insert("total_features".into(), sized.len().to_string());
            return OpinionOutput::Value {
                value: 0.0,
                method_summary: format!(
                    "All {} features fall within a single scale band (span {:.2}× from smallest to largest); no hierarchy of levels.",
                    sized.len(), max / min.max(1e-9)
                ),
                sub_scores: BTreeMap::new(),
                details,
                caveats: vec![
                    "This opinion sees only feature areas. It does not see building height, facade detail, or street-tree spacing.".into(),
                ],
                contributing_features: vec![],
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let target_band_log = (3.0_f64).ln();
        let n_bands = ((span / target_band_log).round() as usize).clamp(2, 8);

        let mut representatives: Vec<f64> = Vec::new();
        let mut exemplar_ids: Vec<String> = Vec::new();
        for band in 0..n_bands {
            let lo = log_min + (band as f64) * span / (n_bands as f64);
            let hi = log_min + ((band + 1) as f64) * span / (n_bands as f64);
            let in_band: Vec<&(f64, String, &str)> = sized
                .iter()
                .filter(|t| t.0.ln() >= lo && t.0.ln() < hi + 1e-9)
                .collect();
            if !in_band.is_empty() {
                let log_mean: f64 = in_band.iter().map(|t| t.0.ln()).sum::<f64>() / in_band.len() as f64;
                representatives.push(log_mean.exp());
                let exemplar = in_band.iter().max_by(|a, b| a.0.partial_cmp(&b.0).unwrap()).unwrap();
                exemplar_ids.push(exemplar.1.clone());
            }
        }

        if representatives.len() < 2 {
            return OpinionOutput::NoView {
                reason: format!(
                    "Features are concentrated in {} effective band; can't speak to between-level ratios.",
                    representatives.len()
                ),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let per_ratio_score = |r: f64| -> f64 {
            if r < 1.0 { return 0.0; }
            if r >= 2.0 && r <= 5.0 { 1.0 }
            else if r < 2.0 { ((r - 1.0) / 1.0).clamp(0.0, 1.0) }
            else { ((10.0 - r) / 5.0).clamp(0.0, 1.0) }
        };

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        let mut ratios = Vec::new();
        for (i, w) in representatives.windows(2).enumerate() {
            let r = w[1] / w[0];
            ratios.push(r);
            let s = per_ratio_score(r);
            sub_scores.insert(format!("ratio_L{}_to_L{}", i + 1, i + 2), s);
        }
        let value = sub_scores.values().sum::<f64>() / sub_scores.len() as f64;

        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("smallest_m2".into(), format!("{:.0}", representatives.first().copied().unwrap_or(0.0)));
        details.insert("largest_m2".into(), format!("{:.0}", representatives.last().copied().unwrap_or(0.0)));
        details.insert("n_levels".into(), representatives.len().to_string());
        details.insert("n_features".into(), sized.len().to_string());
        for (i, r) in ratios.iter().enumerate() {
            details.insert(format!("magnification_L{}_L{}", i + 1, i + 2), format!("{:.2}×", r));
        }

        let summary = format!(
            "Across {} effective levels (smallest {:.0} m² → largest {:.0} m²), the consecutive magnification ratios are {}. Salingaros's ideal range is 2–5×.",
            representatives.len(),
            representatives.first().copied().unwrap_or(0.0),
            representatives.last().copied().unwrap_or(0.0),
            ratios.iter().map(|r| format!("{:.1}×", r)).collect::<Vec<_>>().join(", "),
        );

        OpinionOutput::Value {
            value,
            method_summary: summary,
            sub_scores,
            details,
            caveats: vec![
                "Sees only feature areas (parcels, building footprints, open spaces). Does not see building height, facade detail, ornament, or street-tree spacing — Alexander emphasized these.".into(),
                "Treats all feature types as fungible by area. A 47-acre mall site and a 47-acre living-fabric cluster look identical here.".into(),
            ],
            contributing_features: exemplar_ids,
            runtime_ms: timer.elapsed_ms(),
        }
    }
}
