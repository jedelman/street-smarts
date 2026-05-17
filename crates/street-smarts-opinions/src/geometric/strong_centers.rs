//! Strong Centers — Alexander's second fundamental property of wholeness.
//!
//! From Salingaros 2025 (Frontiers of Architectural Research 14(6): 1491–1515):
//!
//! > A center is a coherent zone that draws attention because of its internal
//! > organization and its differentiation from surroundings. Living structures
//! > contain multiple strong centers, organized in nested hierarchies, where
//! > smaller centers support larger ones. A strong center is reinforced by its
//! > boundary, by symmetries within it, by gradients leading toward it, and by
//! > other adjacent centers that echo or contrast with it.
//!
//! Implementation (this opinion's encoded interpretation, not a measurement):
//!
//! Centers come in three forms in NIR data we have:
//! - Civic / activity nodes (explicit centers of attention)
//! - Plaza / public open space (geometric centers)
//! - Distinctive-spec parcels (CIVIC_*, MAIN_ST_*, MALL_CORE — named landmarks)
//!
//! We count these, weight by their area / intensity, and score on:
//! 1. Whether there are any (zero centers → 0.0)
//! 2. How well they form a hierarchy (a few big + several small > one giant)
//! 3. Distribution across the bbox (concentrated in one corner < spread out)
//!
//! NoView if the data lacks both activity_nodes and any spec-named civic parcels.

use std::collections::BTreeMap;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::{ActivityKind, Neighborhood, OpenSpaceKind};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct StrongCenters;

#[derive(Clone)]
struct Center {
    location: LngLat,
    weight: f64,    // area in m² or normalized intensity
    source_id: String,
    label: String,
}

impl Opinion for StrongCenters {
    fn name(&self) -> &'static str { "strong_centers" }
    fn family(&self) -> OpinionFamily { OpinionFamily::Geometric }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "salingaros_2025_p2".into(),
            display: "Salingaros 2025, Property 2 of 15 (Strong Centers)".into(),
            url: Some("https://doi.org/10.1016/j.foar.2025.01.002".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) { (0.0, 1.0) }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();
        let mut centers: Vec<Center> = Vec::new();

        // 1. Activity nodes are explicit centers.
        for node in &n.activity_nodes {
            let weight = node.intensity.unwrap_or(match node.kind {
                ActivityKind::Civic | ActivityKind::Transit => 1000.0,
                ActivityKind::Commerce | ActivityKind::School => 500.0,
                _ => 300.0,
            });
            centers.push(Center {
                location: node.location,
                weight,
                source_id: node.id.clone(),
                label: node.label.clone().unwrap_or_else(|| format!("{:?}", node.kind)),
            });
        }

        // 2. Plazas and parks (geometric centers).
        for os in &n.open_space {
            if matches!(os.kind, OpenSpaceKind::Plaza | OpenSpaceKind::Park) {
                let centroid = os.polygon.centroid();
                let weight = os.polygon.area_m2();
                if weight > 0.0 {
                    centers.push(Center {
                        location: centroid,
                        weight,
                        source_id: os.id.clone(),
                        label: format!("{:?}", os.kind),
                    });
                }
            }
        }

        // 3. Distinctive-spec parcels: anything tagged with a "spec" code is a
        //    designed landmark by intent. Civic, main-street, and CLT parcels especially.
        for p in &n.parcels {
            if let Some(spec) = &p.spec {
                let is_landmark = spec.starts_with("CIVIC_")
                    || spec.starts_with("MAIN_ST_")
                    || spec.starts_with("MALL_")
                    || spec.starts_with("CLT_");
                if is_landmark {
                    let weight = if p.area_acres > 0.0 {
                        p.area_acres * 4046.86
                    } else {
                        p.polygon.area_m2()
                    };
                    if weight > 0.0 {
                        centers.push(Center {
                            location: p.polygon.centroid(),
                            weight,
                            source_id: p.id.clone(),
                            label: spec.clone(),
                        });
                    }
                }
            }
        }

        if centers.is_empty() {
            return OpinionOutput::NoView {
                reason: "No activity nodes, plazas, parks, or spec-named landmarks present. \
                        This opinion has no way to discuss centers in this data.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        if centers.len() == 1 {
            let mut details = BTreeMap::new();
            details.insert("n_centers".into(), "1".into());
            return OpinionOutput::Value {
                value: 0.2,
                method_summary: format!(
                    "Found exactly one center ({}). Alexander argued living structures have *multiple* nested centers; a single center is not a hierarchy.",
                    centers[0].label
                ),
                sub_scores: BTreeMap::from([
                    ("presence".into(), 0.2),
                    ("hierarchy".into(), 0.0),
                    ("distribution".into(), 0.0),
                ]),
                details,
                caveats: vec![
                    "This opinion sees only NIR-tagged centers. It cannot detect emergent centers \
                     in building massing, façade composition, or pedestrian flow.".into(),
                ],
                contributing_features: vec![centers[0].source_id.clone()],
                runtime_ms: timer.elapsed_ms(),
            };
        }

        // Hierarchy score: weights should span at least 2× from smallest to largest.
        let mut weights: Vec<f64> = centers.iter().map(|c| c.weight).collect();
        weights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let w_min = weights.first().copied().unwrap();
        let w_max = weights.last().copied().unwrap();
        let hierarchy_ratio = w_max / w_min.max(1e-9);
        // 1× → 0, 3× → 0.7, 10× → 1.0
        let hierarchy_score = (hierarchy_ratio.ln() / 3.0_f64.ln()).clamp(0.0, 1.0);

        // Distribution score: pairwise centroid distances. We want centers
        // spread, not clustered. Compute mean nearest-neighbor distance, then
        // compare to the bbox diagonal.
        let mut nn_distances = Vec::new();
        for i in 0..centers.len() {
            let mut best = f64::INFINITY;
            for j in 0..centers.len() {
                if i == j { continue; }
                let d = haversine_m(&centers[i].location, &centers[j].location);
                if d < best { best = d; }
            }
            if best.is_finite() {
                nn_distances.push(best);
            }
        }
        let mean_nn = if nn_distances.is_empty() {
            0.0
        } else {
            nn_distances.iter().sum::<f64>() / nn_distances.len() as f64
        };

        let bbox = &n.bbox_wgs84;
        let diag_m = haversine_m(
            &LngLat::new(bbox[0], bbox[1]),
            &LngLat::new(bbox[2], bbox[3]),
        );
        // Aim for nearest-neighbor distance roughly 0.05–0.15 of the diagonal.
        // Too close → centers blur; too far → isolated.
        let nn_ratio = if diag_m > 0.0 { mean_nn / diag_m } else { 0.0 };
        let distribution_score = if nn_ratio >= 0.05 && nn_ratio <= 0.15 {
            1.0
        } else if nn_ratio < 0.05 {
            (nn_ratio / 0.05).clamp(0.0, 1.0)
        } else {
            ((0.30 - nn_ratio) / 0.15).clamp(0.0, 1.0)
        };

        // Combine: presence (have multiple centers), hierarchy, distribution.
        let presence_score = (centers.len() as f64 / 5.0).min(1.0);
        let value = (presence_score * 0.4 + hierarchy_score * 0.3 + distribution_score * 0.3).clamp(0.0, 1.0);

        // Pick top-5 centers by weight for contributing_features (don't dump all 16+).
        let mut by_weight: Vec<&Center> = centers.iter().collect();
        by_weight.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
        let top_features: Vec<String> = by_weight.iter().take(5).map(|c| c.source_id.clone()).collect();

        let summary = format!(
            "Found {} centers; weight hierarchy spans {:.1}× (smallest→largest); mean nearest-neighbor distance is {:.0}m, which is {:.1}% of the bounding-box diagonal. Presence/hierarchy/distribution scored as {:.2}/{:.2}/{:.2}.",
            centers.len(),
            hierarchy_ratio,
            mean_nn,
            nn_ratio * 100.0,
            presence_score, hierarchy_score, distribution_score,
        );

        let mut sub_scores = BTreeMap::new();
        sub_scores.insert("presence".into(), presence_score);
        sub_scores.insert("hierarchy".into(), hierarchy_score);
        sub_scores.insert("distribution".into(), distribution_score);

        let mut details = BTreeMap::new();
        details.insert("n_centers".into(), centers.len().to_string());
        details.insert("hierarchy_ratio".into(), format!("{:.1}×", hierarchy_ratio));
        details.insert("mean_nearest_neighbor_m".into(), format!("{:.0} m", mean_nn));
        details.insert("bbox_diagonal_m".into(), format!("{:.0} m", diag_m));
        details.insert("nn_distance_as_pct_of_bbox".into(), format!("{:.1}%", nn_ratio * 100.0));
        details.insert("target_nn_pct_range".into(), "5–15%".into());

        OpinionOutput::Value {
            value,
            method_summary: summary,
            sub_scores,
            details,
            caveats: vec![
                "Counts only NIR-tagged centers (activity nodes, plazas/parks, spec-named landmarks). \
                 An emergent center created by, say, a clustering of small shops would be invisible to this opinion."
                    .into(),
                "Treats civic-government buildings the same as community-owned commons buildings as 'centers' — \
                 but the lived experience of those is very different. The activist `ownership` opinion sees that difference; this one does not."
                    .into(),
            ],
            contributing_features: top_features,
            runtime_ms: timer.elapsed_ms(),
        }
    }
}
