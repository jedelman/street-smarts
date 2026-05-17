//! Path network — lays paths/streets between blocks.
//!
//! Pulls from:
//! - **P52 Network of Paths and Cars** (the path system is the spine)
//! - **P98 Circulation Realms** (distinct walking vs driving zones)
//! - **P120 Paths and Goals** (paths go where people actually want to walk)
//!
//! What it does: from a neighborhood whose pads carry `spec = "BLOCK_<n>"`,
//! emit a `Street` segment between every pair of adjacent blocks. Adjacency
//! = block centroids within a threshold distance proportional to the
//! neighborhood's overall scale.
//!
//! What it does NOT do yet:
//! - Connect to the existing street grid (Princess Anne Rd, etc.) — needs
//!   knowledge of which boundary parcels touch existing streets
//! - Distinguish pedestrian vs vehicular (P98 Circulation Realms) — every
//!   path is currently "pedestrian"
//! - Route paths around obstacles (currently they're straight segments
//!   between block centroids)
//! - Aggregate adjacent paths into named streets (each segment is its own
//!   `Street` entity with an auto-id)
//!
//! v0.1 produces a coarse graph that demonstrates the layered pipeline.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{average_centroid, lnglat_to_local, Pt2};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Neighborhood, Parcel, Street};
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathNetworkParams {
    /// Two blocks are "adjacent" if their centroids are within this multiple
    /// of the median inter-block distance. 1.5× is the default; lower
    /// = sparser network, higher = denser (more paths drawn).
    pub adjacency_multiplier: f64,
    /// Right-of-way width in metres for each path segment.
    pub path_width_m: f64,
    /// Classification stored on each Street entity ("pedestrian", "local", etc.)
    /// — currently always "pedestrian" in v0.1. Reserved for P98.
    pub classification_mode: f64, // 0.0 = pedestrian, 1.0 = mixed
}

impl Parameters for PathNetworkParams {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "adjacency_multiplier",
                "Two blocks are linked if centroids closer than median × this. Higher = denser network.",
                0.8, 3.0, 1.5,
            ),
            ParamSpec::float(
                "path_width_m",
                "Right-of-way width per path segment.",
                2.0, 12.0, 4.0,
            ).with_unit("m"),
            ParamSpec::float(
                "classification_mode",
                "0=all pedestrian, 1=mixed pedestrian/vehicular. (v0.1 stub for P98.)",
                0.0, 1.0, 0.0,
            ),
        ]
    }
    fn defaults() -> Self {
        Self {
            adjacency_multiplier: 1.5,
            path_width_m: 4.0,
            classification_mode: 0.0,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.adjacency_multiplier, self.path_width_m, self.classification_mode]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.adjacency_multiplier = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.path_width_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.classification_mode = s.clamp(*x); }
        p
    }
}

pub struct PathNetwork;

impl PatternOperator for PathNetwork {
    type Params = PathNetworkParams;

    fn name(&self) -> &'static str { "path_network" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p52_p98_p120".into(),
            display: "Alexander et al., APL — Patterns 52/98/120 (Network of Paths, Circulation Realms, Paths and Goals)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl52/apl52.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Lay paths between adjacent blocks (path graph, v0.1)."
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        _seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("path_network requires parcel_id='*' (operates on the block graph)".into());
        }

        // Group pads by BLOCK_n.
        let mut blocks: HashMap<String, Vec<&Parcel>> = HashMap::new();
        for p in &nbhd.parcels {
            if let Some(spec) = &p.spec {
                if spec.starts_with("BLOCK_") {
                    blocks.entry(spec.clone()).or_default().push(p);
                }
            }
        }
        if blocks.len() < 2 {
            return Err(format!(
                "path_network: need at least 2 blocks (found {}). Run block_grouping first.",
                blocks.len()
            ));
        }

        // Anchor projection at the mean of all block centroids.
        let mut all_lng = 0.0;
        let mut all_lat = 0.0;
        let mut n = 0;
        for parcels in blocks.values() {
            for p in parcels {
                let c = average_centroid(&p.polygon.outer);
                all_lng += c.lng;
                all_lat += c.lat;
                n += 1;
            }
        }
        let origin = LngLat::new(all_lng / n as f64, all_lat / n as f64);

        // Compute each block's centroid in local meters AND lng/lat.
        let mut block_ids: Vec<String> = blocks.keys().cloned().collect();
        block_ids.sort(); // deterministic order
        let mut centers_local: Vec<Pt2> = Vec::with_capacity(block_ids.len());
        let mut centers_wgs: Vec<LngLat> = Vec::with_capacity(block_ids.len());
        for bid in &block_ids {
            let parcels = &blocks[bid];
            let mut lng = 0.0;
            let mut lat = 0.0;
            for p in parcels {
                let c = average_centroid(&p.polygon.outer);
                lng += c.lng;
                lat += c.lat;
            }
            let avg = LngLat::new(lng / parcels.len() as f64, lat / parcels.len() as f64);
            centers_local.push(lnglat_to_local(&avg, &origin));
            centers_wgs.push(avg);
        }

        // Compute all pairwise distances; the median sets the adjacency threshold.
        let mut all_dists: Vec<f64> = Vec::new();
        let nb = centers_local.len();
        for i in 0..nb {
            for j in (i + 1)..nb {
                all_dists.push(centers_local[i].dist(centers_local[j]));
            }
        }
        all_dists.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = if all_dists.is_empty() {
            0.0
        } else {
            all_dists[all_dists.len() / 2]
        };
        let threshold = median * params.adjacency_multiplier;

        // Emit Street segments between adjacent block pairs.
        let mut streets: Vec<Street> = Vec::new();
        let mut adj_count = 0;
        for i in 0..nb {
            for j in (i + 1)..nb {
                let d = centers_local[i].dist(centers_local[j]);
                if d <= threshold {
                    let id = format!("path_{}_to_{}", block_ids[i], block_ids[j]);
                    let classification = if params.classification_mode >= 0.5 && j % 2 == 0 {
                        "local"
                    } else {
                        "pedestrian"
                    };
                    streets.push(Street {
                        id,
                        centerline: vec![centers_wgs[i], centers_wgs[j]],
                        classification: Some(classification.into()),
                        row_width_m: Some(params.path_width_m),
                    });
                    adj_count += 1;
                }
            }
        }

        if streets.is_empty() {
            return Err(format!(
                "path_network: 0 adjacency pairs found (threshold {:.0}m, median {:.0}m). \
                 Try a higher adjacency_multiplier.",
                threshold, median
            ));
        }

        let mut steps = vec![
            format!(
                "{} blocks, {} adjacency pairs at threshold {:.0}m (median centroid distance {:.0}m)",
                nb, adj_count, threshold, median
            ),
        ];
        steps.push(format!("Emitted {} Street segments at {}m right-of-way", streets.len(), params.path_width_m as u32));

        let trace = SubdivisionTrace {
            operator_name: "path_network".into(),
            operator_source: self.source(),
            headline: format!(
                "Laid {} path segments between {} blocks.",
                streets.len(), nb
            ),
            steps,
            caveats: vec![
                "Paths are straight segments between block centroids. They do not yet route \
                 around obstacles, follow shared edges, or connect to the existing street grid.".into(),
                "Every path is currently classified pedestrian by default. P98 Circulation Realms \
                 would distinguish pedestrian/vehicular/transit; that's a separate v0.2 operator.".into(),
                "Adjacent paths are not aggregated into named streets. Each segment is its own \
                 NIR Street entity. v0.2 will graph-aggregate runs into named ways.".into(),
            ],
            seed: 0,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings: vec![],
            new_streets: streets,
            replaced_parcel_ids: vec![],
            trace,
        })
    }
}
