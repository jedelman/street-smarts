//! P29 Density Rings — the site's density should vary by distance from its
//! own center, not be flat everywhere.
//!
//! From Alexander, *A Pattern Language*, Pattern 29:
//! > Arrange the housing so that it steps down in density, from the center
//! > of the town...
//!
//! # Where this actually runs
//! Alexander's own numbering puts Density Rings (29) well before House
//! Cluster (37) -- deciding the site's overall density gradient before
//! carving individual clusters. This codebase's schema has no way to
//! annotate undivided raw land with a zone/tier -- there's nothing to
//! attach the tag to until real parcels exist. So this operator runs on
//! P37's `BLOCK_n` children instead (`parcel_id == "*"`, same convention
//! `PathNetwork` uses), tagging each block's `density_tier`/`target_stories`
//! from its own centroid's distance from the site's density center. This is
//! a practical adaptation to what the schema can express, not a claim to
//! match Alexander's literal sequencing -- said plainly, same as every
//! other honest scale-approximation in this pipeline.
//!
//! # Approach
//! Center = area-weighted centroid of every `BLOCK_n` parcel (a proxy for
//! "town center" -- this pipeline has no separate notion of a civic center
//! or main activity node to use instead; see caveats). Each block's
//! normalized distance from that center (its own distance / the farthest
//! block's distance) buckets it into `n_rings` equal-radius bands: nearest
//! third gets `core_target_stories`, next third `mid_target_stories`,
//! outermost third `edge_target_stories` (for `n_rings = 3`, the default;
//! more rings interpolate linearly between the core and edge values).
//!
//! Sets `density_tier` (a label: "core"/"ring_1"/.../"edge") and
//! `target_stories` (a number) on each block -- REPLACES each `BLOCK_n`
//! parcel with an identical copy carrying those two fields, same
//! non-destructive "replace with an annotated copy" mechanism used
//! elsewhere. Geometry is untouched.
//!
//! `target_stories` here is a BLOCK-level goal, not a promise every
//! building on that block gets exactly that many stories -- P96 Number of
//! Stories is what turns it into real per-pad assignments, respecting
//! P21's four-story-ordinary-building constraint and its "a few, widely
//! spaced" exception allowance.
//!
//! # What this deliberately does NOT do
//! - Equal-RADIUS rings, not equal-AREA rings -- simpler, but means the
//!   outer ring covers much more site area than the inner one for a
//!   roughly-circular site. An honest first approximation, not a claim
//!   this matches real population-ring math.
//! - The "center" is a geometric centroid of the blocks themselves, not a
//!   real civic/activity center (transit, main street, a plaza). This
//!   pipeline doesn't have a reliable signal for that yet; `activity_nodes`
//!   exists in the NIR schema but nothing populates it for these fixtures.
//! - Doesn't look at the site's actual shape (an elongated site gets the
//!   same circular rings as a compact one).
//!
//! # v0.2: `eccentricity_frac`, closing P28 Eccentric Nucleus's real gap
//!
//! `p28_eccentric_nucleus` checks whether the real `Core`-tier peak this
//! operator produces sits at a genuinely OFF-CENTER position relative to
//! the site's own bounding-box center -- Alexander's own "eccentric," not
//! dead center. The plain area-weighted block centroid used as the ring
//! center is, for a roughly compact block layout, close to that
//! bounding-box center by construction -- the opposite of eccentric.
//! `eccentricity_frac` (default 0.35) now shifts the real ring center a
//! real fraction of the way from that plain centroid TOWARD the single
//! block farthest from it (the real "direction of most room to grow," not
//! an arbitrary pick) -- `0.0` keeps the old dead-center behavior exactly,
//! higher values push the density peak measurably off-center.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{apply_subdivision, PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::components::DensityTier;
use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::{Neighborhood, Parcel};
use street_smarts_core::world::World;
use street_smarts_core::Scope;
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P29Params {
    /// Number of concentric density bands.
    pub n_rings: f64,
    /// Target story count for the innermost ring (nearest the density
    /// center). Intentionally allowed above P21's ordinary 4-story cap --
    /// that's the "a few, widely spaced" exception P21 itself describes,
    /// which P96 is responsible for actually respecting.
    pub core_target_stories: f64,
    /// Target story count for the outermost ring.
    pub edge_target_stories: f64,
    /// How far the real ring center shifts from the blocks' plain
    /// area-weighted centroid toward the single farthest block -- `0.0`
    /// is dead-center (the old behavior), higher values push the density
    /// peak measurably off-center (P28 Eccentric Nucleus).
    pub eccentricity_frac: f64,
}

impl Parameters for P29Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::integer(
                "n_rings",
                "Number of concentric density bands from center to edge.",
                2.0, 6.0, 3.0,
            ).with_unit("rings"),
            ParamSpec::float(
                "core_target_stories",
                "Target stories for the innermost ring (may exceed the ordinary 4-story cap).",
                2.0, 12.0, 6.0,
            ).with_unit("stories"),
            ParamSpec::float(
                "edge_target_stories",
                "Target stories for the outermost ring.",
                1.0, 4.0, 2.0,
            ).with_unit("stories"),
            ParamSpec::float(
                "eccentricity_frac",
                "Fraction of the way the ring center shifts from the plain block centroid toward the farthest block (P28 Eccentric Nucleus).",
                0.0, 0.8, 0.35,
            ),
        ]
    }
    fn defaults() -> Self {
        Self { n_rings: 3.0, core_target_stories: 6.0, edge_target_stories: 2.0, eccentricity_frac: 0.35 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.n_rings, self.core_target_stories, self.edge_target_stories, self.eccentricity_frac]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.n_rings = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.core_target_stories = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.edge_target_stories = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) { p.eccentricity_frac = s.clamp(*x); }
        p
    }
}

pub struct P29DensityRings;

impl PatternOperator for P29DensityRings {
    type Params = P29Params;

    fn name(&self) -> &'static str { "p29_density_rings" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p29".into(),
            display: "Alexander et al., A Pattern Language, Pattern 29 (Density Rings)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl29/apl29.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Tag each BLOCK_n parcel with a density tier and target story count, higher near the site's own density center."
    }

    /// `parcel_id == "*"` (the only mode supported): tags every `BLOCK_n`
    /// parcel currently in the neighborhood.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p29_density_rings only supports parcel_id \"*\" -- it tags every BLOCK_n parcel in one pass, same convention as PathNetwork.".into());
        }

        let assignments = compute_ring_assignments(nbhd, params)?;

        let n_rings = params.n_rings.round().max(1.0) as usize;
        let mut new_parcels: Vec<Parcel> = Vec::with_capacity(assignments.len());
        let mut replaced: Vec<String> = Vec::with_capacity(assignments.len());
        let mut tier_counts: Vec<usize> = vec![0; n_rings];

        for a in &assignments {
            tier_counts[a.ring_idx] += 1;
            let tier = street_smarts_core::ring_tier_label(a.ring_idx, a.n_rings);

            let mut updated = a.block.clone();
            updated.density_tier = Some(tier);
            updated.target_stories = Some(a.target_stories);
            new_parcels.push(updated);
            replaced.push(a.block.id.clone());
        }

        let mut steps: Vec<String> = Vec::new();
        for (i, count) in tier_counts.iter().enumerate() {
            let label = street_smarts_core::ring_tier_label(i, n_rings);
            steps.push(format!("{label}: {count} block(s)"));
        }

        let trace = SubdivisionTrace {
            operator_name: "p29_density_rings".into(),
            operator_source: self.source(),
            headline: format!(
                "Tagged {} block(s) across {} density ring(s), centered on their own area-weighted centroid.",
                new_parcels.len(), n_rings
            ),
            steps,
            caveats: vec![
                "The density center is the blocks' own area-weighted centroid, shifted toward the \
                 farthest block by eccentricity_frac -- not a real civic or activity center; this \
                 pipeline has no reliable signal for the latter yet (see the module doc comment).".into(),
                "The eccentric shift always points toward whichever block sits farthest from the \
                 plain centroid -- a real, deterministic direction, not Alexander's own reasoning \
                 for WHY a nucleus forms where it does (transit, an existing landmark); this \
                 pipeline has no such signal to shift toward instead.".into(),
                "Rings are equal-RADIUS bands, not equal-area -- the outer ring covers \
                 disproportionately more site area than the inner one on a roughly circular \
                 site.".into(),
                "target_stories here is a block-level GOAL, not a per-building assignment. P96 \
                 Number of Stories is what turns it into real pad-level story counts, respecting \
                 P21's four-story-ordinary-building constraint.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels,
            new_open_space: vec![],
            new_buildings: vec![],
            new_streets: vec![],
            new_activity_nodes: vec![],
            new_boundaries: vec![],
            replaced_parcel_ids: replaced,
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
        })
    }
}

/// One block's real assignment: the exact `(ring_idx, n_rings)` pair and
/// derived `target_stories` this operator computes for it. Shared by
/// `apply()` (which projects `ring_idx`/`n_rings` into the string label via
/// `ring_tier_label`) and `run_native()` (which projects the SAME
/// `ring_idx`/`n_rings` into `DensityTier` via `DensityTier::from_ring`) --
/// extracted so both are genuinely two views of one computation, not one
/// parsed from the other. See `system.rs`'s own module doc for why this
/// distinction is what "native" dual-write means here.
struct RingAssignment<'a> {
    block: &'a Parcel,
    ring_idx: usize,
    n_rings: usize,
    target_stories: f64,
}

fn compute_ring_assignments<'a>(
    nbhd: &'a Neighborhood,
    params: &P29Params,
) -> Result<Vec<RingAssignment<'a>>, String> {
    let blocks: Vec<&Parcel> = nbhd.select(&Scope::Block).collect();
    if blocks.is_empty() {
        return Err("p29_density_rings: no BLOCK_n parcels found -- run P37 House Cluster first.".into());
    }

    // Area-weighted centroid of the blocks as a proxy for the site's
    // "density center" -- see the module doc's caveat on this.
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut total_area = 0.0;
    let mut block_centroids: Vec<(LngLat, f64)> = Vec::with_capacity(blocks.len());
    for b in &blocks {
        let a = b.polygon.area_m2().max(1.0);
        let c = LngLat::new(
            b.polygon.outer.iter().map(|q| q.lng).sum::<f64>() / b.polygon.outer.len() as f64,
            b.polygon.outer.iter().map(|q| q.lat).sum::<f64>() / b.polygon.outer.len() as f64,
        );
        cx += c.lng * a;
        cy += c.lat * a;
        total_area += a;
        block_centroids.push((c, a));
    }
    let plain_center = LngLat::new(cx / total_area, cy / total_area);

    // P28 Eccentric Nucleus: shift the real ring center a real fraction of
    // the way from plain_center toward the single farthest block -- see
    // this file's own "v0.2" module doc. eccentricity_frac == 0.0
    // reproduces the old dead-center behavior exactly.
    let center = if params.eccentricity_frac > 0.0 && block_centroids.len() > 1 {
        let (far, _) = block_centroids.iter()
            .map(|(c, _)| (*c, haversine_m(&plain_center, c)))
            .fold((plain_center, 0.0), |best, cur| if cur.1 > best.1 { cur } else { best });
        LngLat::new(
            plain_center.lng + params.eccentricity_frac * (far.lng - plain_center.lng),
            plain_center.lat + params.eccentricity_frac * (far.lat - plain_center.lat),
        )
    } else {
        plain_center
    };

    // Local-meter distance from center for each block (reuses the same
    // lng/lat-specific meter conversion `planar` uses elsewhere).
    let m_per_deg_lat = 110_540.0;
    let m_per_deg_lng = 111_320.0 * (center.lat.to_radians().cos());
    let dist_m = |p: LngLat| -> f64 {
        let dx = (p.lng - center.lng) * m_per_deg_lng;
        let dy = (p.lat - center.lat) * m_per_deg_lat;
        (dx * dx + dy * dy).sqrt()
    };
    let max_dist = block_centroids.iter()
        .map(|(c, _)| dist_m(*c))
        .fold(0.0_f64, f64::max)
        .max(1.0);

    let n_rings = params.n_rings.round().max(1.0) as usize;
    let mut out = Vec::with_capacity(blocks.len());
    for (b, (c, _)) in blocks.iter().zip(block_centroids.iter()) {
        let normalized = (dist_m(*c) / max_dist).clamp(0.0, 1.0);
        let ring_idx = ((normalized * n_rings as f64) as usize).min(n_rings - 1);
        // Linear interpolation between core and edge targets across the
        // ring index -- ring 0 gets exactly core_target_stories, the last
        // ring gets exactly edge_target_stories.
        let t = if n_rings > 1 { ring_idx as f64 / (n_rings - 1) as f64 } else { 0.0 };
        let target_stories = params.core_target_stories + t * (params.edge_target_stories - params.core_target_stories);
        out.push(RingAssignment { block: b, ring_idx, n_rings, target_stories });
    }
    Ok(out)
}

impl P29DensityRings {
    /// The native `System` port -- see `system.rs`'s own module doc for
    /// why this is an inherent method, not a second `impl System`,  and
    /// PRIMITIVES_SPEC.md §1.5's milestone this satisfies literally
    /// ("one real pattern... is ported to System... without changing its
    /// own test file's assertions" -- `apply()`'s own test file is
    /// untouched by this addition).
    ///
    /// Builds the same `Subdivision`/string output `apply()` does (via the
    /// same shared `compute_ring_assignments` helper -- not a second,
    /// possibly-drifting reimplementation), applies it, and ALSO writes
    /// `DensityTier::from_ring` directly into the resulting `World`'s
    /// `density_tiers` map from the same `ring_idx`/`n_rings` pair used to
    /// build the string -- not by parsing the string back out afterward.
    pub fn run_native(&self, world: &World, params: &P29Params, seed: u64) -> Result<World, String> {
        let nbhd = world.to_neighborhood();
        let assignments = compute_ring_assignments(&nbhd, params)?;

        let sub = self.apply(&nbhd, "*", params, seed)?;
        let new_nbhd = apply_subdivision(&nbhd, &sub);
        let mut new_world = World::from_neighborhood(&new_nbhd);

        for a in &assignments {
            new_world.density_tiers.insert(
                a.block.id.clone(),
                DensityTier::from_ring(a.ring_idx, a.n_rings),
            );
        }
        Ok(new_world)
    }
}
