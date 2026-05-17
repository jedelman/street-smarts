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

use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};

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
        let t0 = now_ms();

        let mut sized: Vec<f64> = Vec::new();
        // Parcels
        for p in &n.parcels {
            let a = if p.area_acres > 0.0 {
                p.area_acres * 4046.86 // acres → m²
            } else {
                p.polygon.area_m2()
            };
            if a > 0.0 { sized.push(a); }
        }
        // Buildings (footprint area)
        for b in &n.buildings {
            let a = b.polygon.area_m2();
            if a > 0.0 { sized.push(a); }
        }
        // Open spaces
        for o in &n.open_space {
            let a = o.polygon.area_m2();
            if a > 0.0 { sized.push(a); }
        }

        if sized.len() < 4 {
            return OpinionOutput::NoView {
                reason: format!(
                    "Only {} sized features found; need at least 4 to discuss scale hierarchy.",
                    sized.len()
                ),
                runtime_ms: (now_ms() - t0) as u32,
            };
        }

        sized.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Sample logarithmic deciles: each "level" represents an order-of-magnitude band.
        // Take the geometric mean within each band as the representative scale.
        let min = sized.first().copied().unwrap();
        let max = sized.last().copied().unwrap();
        let log_min = min.ln();
        let log_max = max.ln();
        let span = log_max - log_min;
        if span < 0.5 {
            // All features are roughly one size — no levels at all.
            return OpinionOutput::Value {
                value: 0.0,
                method_summary: format!(
                    "All {} features fall within a single scale band (span {:.2}× from smallest to largest); no hierarchy of levels.",
                    sized.len(), max / min.max(1e-9)
                ),
                caveats: vec![
                    "This opinion sees only feature areas. It does not see building height, facade detail, or street-tree spacing.".into(),
                ],
                contributing_features: vec![],
                runtime_ms: (now_ms() - t0) as u32,
            };
        }

        // Pick a number of bands so that each spans roughly ln(3) ≈ 1.1 (a ~3× factor)
        let target_band_log = (3.0_f64).ln();
        let n_bands = ((span / target_band_log).round() as usize).clamp(2, 8);

        let mut representatives: Vec<f64> = Vec::with_capacity(n_bands);
        for band in 0..n_bands {
            let lo = log_min + (band as f64) * span / (n_bands as f64);
            let hi = log_min + ((band + 1) as f64) * span / (n_bands as f64);
            let in_band: Vec<f64> = sized
                .iter()
                .filter(|&&a| a.ln() >= lo && a.ln() < hi + 1e-9)
                .copied()
                .collect();
            if !in_band.is_empty() {
                let log_mean: f64 = in_band.iter().map(|a| a.ln()).sum::<f64>() / in_band.len() as f64;
                representatives.push(log_mean.exp());
            }
        }

        if representatives.len() < 2 {
            return OpinionOutput::NoView {
                reason: format!(
                    "Features are concentrated in {} effective band; can't speak to between-level ratios.",
                    representatives.len()
                ),
                runtime_ms: (now_ms() - t0) as u32,
            };
        }

        // Score each consecutive ratio.
        let mut ratios = Vec::with_capacity(representatives.len().saturating_sub(1));
        for w in representatives.windows(2) {
            let r = w[1] / w[0];
            ratios.push(r);
        }

        // Per Salingaros: score 1 if r in [2,5]; linearly fall to 0 outside.
        let per_ratio_score = |r: f64| -> f64 {
            if r < 1.0 { return 0.0; } // shouldn't happen given sorted bands
            if r >= 2.0 && r <= 5.0 { 1.0 }
            else if r < 2.0 {
                // 1.0→0, 1.5→0.5, 2.0→1.0
                ((r - 1.0) / 1.0).clamp(0.0, 1.0)
            } else {
                // 5.0→1.0, 10.0→0
                ((10.0 - r) / 5.0).clamp(0.0, 1.0)
            }
        };

        let scores: Vec<f64> = ratios.iter().map(|&r| per_ratio_score(r)).collect();
        let value = scores.iter().sum::<f64>() / scores.len() as f64;

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
            caveats: vec![
                "Sees only feature areas (parcels, building footprints, open spaces). Does not see building height, facade detail, ornament, or street-tree spacing — Alexander emphasized these.".into(),
                "Treats all feature types as fungible by area. A 47-acre mall site and a 47-acre living-fabric cluster look identical here.".into(),
            ],
            contributing_features: vec![],
            runtime_ms: (now_ms() - t0) as u32,
        }
    }
}

fn now_ms() -> u128 {
    // Use a portable, monotonic time source.
    // In WASM the std::time::Instant is supported on recent toolchains, but
    // for safety we approximate with a no-op (the actual timing is not load-bearing for v0.1).
    0
}
