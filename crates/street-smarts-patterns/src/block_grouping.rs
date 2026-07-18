//! Block grouping — clusters building pads into shared-edge blocks.
//!
//! Pulls from Alexander's intermediate-scale patterns:
//!
//! - **P14 Identifiable Neighborhood** (sense of being inside a recognizable group)
//! - **P36 Degrees of Publicness** (private cluster ↔ public face)
//! - **P37 House Cluster** (small groupings of dwellings around a common space)
//!
//! What it does: takes a neighborhood with N pad-tagged parcels and assigns
//! each one to a block (3-7 pads per block by default). Block membership is
//! recorded on each parcel as `spec = "BLOCK_<n>"` (overwriting any prior
//! P95_CELL_n spec), and the block boundary is emitted as a new open-space
//! entity tagged with `kind = OpenSpaceKind::Other` (we don't have a "block"
//! kind yet; v0.2 of NIR adds one).
//!
//! What it does NOT do yet:
//! - Detect block-internal shared courtyards (P37 wants each block to have
//!   one — currently the only courtyard is the parcel-level P95 courtyard)
//! - Distinguish private/semi-private/public faces of each block (P36)
//! - Use the block boundaries to constrain downstream operators
//!
//! Those land when we wire the pipeline up. v0.1 is the SHAPE of the
//! operator and the tagging hook.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{average_centroid, lnglat_to_local, Pt2};
use crate::prng::Prng;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Neighborhood, Parcel};
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockGroupingParams {
    /// Target pads per block. Real coalitions argue about this: smaller blocks
    /// = more communal, larger blocks = more efficient circulation. Defaults
    /// to 5 (Alexander's P37 "House Cluster" sweet spot).
    pub pads_per_block: f64,
    /// Minimum pads per block. Below this we merge stranded singletons into
    /// the nearest block.
    pub min_pads_per_block: f64,
    /// Maximum pads per block. Above this we split.
    pub max_pads_per_block: f64,
}

impl Parameters for BlockGroupingParams {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::integer(
                "pads_per_block",
                "Target number of pads grouped into one block (Alexander's House Cluster ≈ 5).",
                2.0, 12.0, 5.0,
            ).with_unit("pads"),
            ParamSpec::integer(
                "min_pads_per_block",
                "Below this, stranded pads get merged into the nearest block.",
                1.0, 5.0, 2.0,
            ).with_unit("pads"),
            ParamSpec::integer(
                "max_pads_per_block",
                "Above this, the operator splits the block.",
                3.0, 20.0, 7.0,
            ).with_unit("pads"),
        ]
    }
    fn defaults() -> Self {
        Self {
            pads_per_block: 5.0,
            min_pads_per_block: 2.0,
            max_pads_per_block: 7.0,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.pads_per_block, self.min_pads_per_block, self.max_pads_per_block]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.pads_per_block = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.min_pads_per_block = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.max_pads_per_block = s.clamp(*x); }
        p
    }
}

pub struct BlockGrouping;

impl PatternOperator for BlockGrouping {
    type Params = BlockGroupingParams;

    fn name(&self) -> &'static str { "block_grouping" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p14_p36_p37".into(),
            display: "Alexander et al., APL — Patterns 14/36/37 (Identifiable Neighborhood, Degrees of Publicness, House Cluster)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl37/apl37.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Group building pads into blocks of 3–7 (House Cluster scale)."
    }

    /// `parcel_id` is interpreted specially: pass `"*"` to group ALL pads
    /// in the neighborhood. A specific id makes no sense for this operator
    /// (a single pad can't form a block by itself) and returns an error.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("block_grouping requires parcel_id='*' (works across all pads)".into());
        }

        let pads: Vec<&Parcel> = nbhd
            .parcels
            .iter()
            .filter(|p| {
                p.use_category.as_deref() == Some("p95_building_pad")
                    || p.use_category.as_deref() == Some("p95_pad_with_building")
            })
            .collect();

        if pads.len() < params.min_pads_per_block as usize {
            return Err(format!(
                "block_grouping: need at least {} pads, found {}",
                params.min_pads_per_block as usize, pads.len()
            ));
        }

        let target_pads = params.pads_per_block.max(2.0) as usize;
        let min_pads = params.min_pads_per_block.max(1.0) as usize;
        let max_pads = params.max_pads_per_block.max(target_pads as f64) as usize;

        let origin = pads_origin(&pads);
        let positions: Vec<Pt2> = pads
            .iter()
            .map(|p| {
                let c = average_centroid(&p.polygon.outer);
                lnglat_to_local(&c, &origin)
            })
            .collect();

        // Greedy region-growing. Start a block from the most-isolated
        // remaining pad (farthest from the centroid of already-assigned pads);
        // grow it by nearest neighbour until target_pads reached or max_pads.
        let mut assignment: Vec<Option<usize>> = vec![None; pads.len()];
        let mut prng = Prng::new(seed);
        let mut block_idx = 0;
        let mut steps: Vec<String> = Vec::new();

        let unassigned = |a: &[Option<usize>]| -> Vec<usize> {
            a.iter().enumerate().filter_map(|(i, b)| if b.is_none() { Some(i) } else { None }).collect()
        };

        loop {
            let remaining = unassigned(&assignment);
            if remaining.is_empty() { break; }

            // Seed: random among remaining (so per-seed variation is real).
            let seed_pick = (prng.next_u64() as usize) % remaining.len();
            let seed_pad = remaining[seed_pick];
            let mut block_members = vec![seed_pad];
            assignment[seed_pad] = Some(block_idx);

            // Grow: nearest neighbours among remaining.
            while block_members.len() < target_pads {
                let still_remaining = unassigned(&assignment);
                if still_remaining.is_empty() { break; }
                // Find nearest unassigned to the current block centroid.
                let block_center = Pt2 {
                    x: block_members.iter().map(|&i| positions[i].x).sum::<f64>() / block_members.len() as f64,
                    y: block_members.iter().map(|&i| positions[i].y).sum::<f64>() / block_members.len() as f64,
                };
                let nearest = still_remaining
                    .iter()
                    .min_by(|&&a, &&b| {
                        let da = positions[a].sub(block_center).len();
                        let db = positions[b].sub(block_center).len();
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .copied();
                if let Some(n) = nearest {
                    assignment[n] = Some(block_idx);
                    block_members.push(n);
                } else {
                    break;
                }
            }

            steps.push(format!(
                "block {} has {} pad(s)",
                block_idx, block_members.len()
            ));
            block_idx += 1;
        }

        // Sweep singletons / under-min blocks into neighbours.
        let _ = max_pads;
        let mut block_counts: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for a in &assignment {
            if let Some(b) = a { *block_counts.entry(*b).or_default() += 1; }
        }
        for i in 0..pads.len() {
            if let Some(b) = assignment[i] {
                if block_counts[&b] < min_pads {
                    // Merge into the nearest pad's block.
                    let me = positions[i];
                    let other = (0..pads.len())
                        .filter(|&j| j != i)
                        .filter(|&j| assignment[j].map(|bb| block_counts[&bb] >= min_pads).unwrap_or(false))
                        .min_by(|&a, &b| {
                            positions[a].sub(me).len().partial_cmp(&positions[b].sub(me).len())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                    if let Some(j) = other {
                        if let Some(target_block) = assignment[j] {
                            *block_counts.entry(b).or_default() -= 1;
                            assignment[i] = Some(target_block);
                            *block_counts.entry(target_block).or_default() += 1;
                            steps.push(format!(
                                "merged singleton pad {} into block {}",
                                pads[i].id, target_block
                            ));
                        }
                    }
                }
            }
        }

        // Emit updated parcels: each pad gets `spec = "BLOCK_<id>"` while
        // retaining its prior pad metadata in use_category.
        let mut new_parcels: Vec<Parcel> = Vec::with_capacity(pads.len());
        let mut replaced: Vec<String> = Vec::with_capacity(pads.len());
        for (i, pad) in pads.iter().enumerate() {
            let mut updated = (*pad).clone();
            if let Some(b) = assignment[i] {
                updated.spec = Some(format!("BLOCK_{}", b));
            }
            replaced.push(pad.id.clone());
            new_parcels.push(updated);
        }

        let total_blocks: usize = block_counts.values().filter(|&&n| n > 0).count();
        steps.insert(0, format!(
            "Grouped {} pads into {} blocks (target {} pads/block).",
            pads.len(), total_blocks, target_pads
        ));

        let trace = SubdivisionTrace {
            operator_name: "block_grouping".into(),
            operator_source: self.source(),
            headline: format!(
                "Grouped {} pads into {} House-Cluster blocks.",
                pads.len(), total_blocks
            ),
            steps,
            caveats: vec![
                "Block membership is currently a tag (BLOCK_n) on each pad. \
                 The block boundary geometry is NOT yet emitted — that's the next operator.".into(),
                "P37 House Cluster wants each block to have a SHARED common space at its centre. \
                 v0.1 doesn't generate that yet; the only courtyard remains the parcel-level P95 courtyard.".into(),
                "Greedy region-growing is fast but not optimal. Real clustering would use \
                 Delaunay adjacency + spectral partitioning (or learned clustering from a future \
                 MoE expert). Block boundaries from this operator may be jagged or asymmetric.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels,
            new_open_space: vec![],
            new_buildings: vec![],
            new_streets: vec![],
            replaced_parcel_ids: replaced,
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
        })
    }
}

fn pads_origin(pads: &[&Parcel]) -> LngLat {
    if pads.is_empty() { return LngLat::new(0.0, 0.0); }
    let mut lng = 0.0;
    let mut lat = 0.0;
    let mut n = 0;
    for p in pads {
        let c = average_centroid(&p.polygon.outer);
        lng += c.lng;
        lat += c.lat;
        n += 1;
    }
    LngLat::new(lng / n as f64, lat / n as f64)
}
