//! P29 Density Rings — the site's density should vary by distance from its
//! own center, not be flat everywhere.
//!
//! From Alexander, *A Pattern Language*, Pattern 29:
//! > Arrange the housing so that it steps down in density, from the center
//! > of the town...
//!
//! # Where this actually runs -- now Alexander's own canonical position
//!
//! Alexander's own numbering puts Density Rings (29) well before House
//! Cluster (37) -- deciding the site's overall density gradient before
//! carving individual clusters. This USED to be impossible in this
//! pipeline: the old schema had no way to annotate undivided raw land with
//! a zone/tier, so this operator ran on P37's `BLOCK_n` children instead,
//! tagging each block directly. That was a practical adaptation to a real
//! schema limit, not a claim to match Alexander's literal sequencing --
//! and it's exactly the bug `PATTERN_ORDERING_AUDIT.md` (repo root, §4.1)
//! names: a larger, prior pattern forced to wait on a smaller one purely
//! because there was nowhere else to attach its own output.
//!
//! `street_smarts_core::nir::DensityField` closes that gap: a real,
//! sampleable potential over the RAW site parcel's own polygon, computed
//! and attached to `Neighborhood.pattern_fields` by this operator alone --
//! no blocks required. This operator now runs on the SAME raw site
//! `parcel_id` `p37_house_cluster` is about to carve (not `"*"` -- there's
//! one field for the whole site, not one per parcel), genuinely BEFORE
//! P37 in the real pipeline. `p37_house_cluster` samples the field (via
//! `sample_density_field` below) at each new block's own centroid as it
//! creates it, stamping `density_tier`/`target_stories` directly -- the
//! individuation moment, not a separate later pass. If P29 hasn't run (no
//! field present), P37 leaves both `None`, exactly like before this
//! change.
//!
//! # Approach
//! Center = the raw site parcel's own vertex-averaged centroid (the same
//! style every other operator in this crate uses for a polygon's own
//! "origin" -- see `p107_wings_of_light`/`p95_building_complex`), shifted
//! by `eccentricity_frac` toward the parcel's own farthest vertex (see
//! "v0.2" below -- this used to shift toward the farthest BLOCK; blocks
//! don't exist yet at this operator's new position, so the farthest real
//! extremity of the raw footprint itself is the direct analog). Radius =
//! distance from that center to the farthest vertex. A sample at any
//! point is bucketed into one of `n_rings` equal-radius bands (nearest
//! third core, outermost third edge, for the `n_rings = 3` default; more
//! rings interpolate linearly between the core and edge target-story
//! values) -- unchanged math from the pre-field version, just evaluated
//! against the raw footprint's own vertices instead of block centroids,
//! and evaluated LATER, at sample time, instead of once up front.
//!
//! # What this deliberately does NOT do
//! - Equal-RADIUS rings, not equal-AREA rings -- simpler, but means the
//!   outer ring covers much more site area than the inner one for a
//!   roughly-circular site. An honest first approximation, not a claim
//!   this matches real population-ring math.
//! - The "center" is a geometric centroid of the raw parcel's own
//!   vertices, not a real civic/activity center (transit, main street, a
//!   plaza). This pipeline doesn't have a reliable signal for that yet;
//!   `activity_nodes` exists in the NIR schema but nothing populates it
//!   for these fixtures.
//! - Doesn't look at the site's actual shape (an elongated site gets the
//!   same circular rings as a compact one).
//!
//! # v0.2: `eccentricity_frac`, closing P28 Eccentric Nucleus's real gap
//!
//! `p28_eccentric_nucleus` checks whether the real `Core`-tier peak this
//! operator produces sits at a genuinely OFF-CENTER position relative to
//! the site's own bounding-box center -- Alexander's own "eccentric," not
//! dead center. `eccentricity_frac` (default 0.35) shifts the real field
//! center a real fraction of the way from the plain vertex-averaged
//! centroid TOWARD the single vertex farthest from it (the real "direction
//! of most room to grow," not an arbitrary pick) -- `0.0` keeps the old
//! dead-center behavior exactly, higher values push the density peak
//! measurably off-center.
//!
//! # v0.3: field-based, closing this operator's own real ordering bug
//!
//! Everything above this section describes the CURRENT, field-based
//! design. Previously (see git history before this change): this operator
//! ran on P37's `BLOCK_n` children, computing center/radius from BLOCK
//! centroids and stamping `density_tier`/`target_stories` directly onto
//! each block. That version's `run_native` ALSO wrote a `DensityTier`
//! component per block. Both responsibilities move to `p37_house_cluster`
//! now (its own `apply`/`run_native` sample this operator's field as each
//! block is individuated) -- this operator no longer touches blocks at
//! all, so it has nothing left for its own native port to dual-write. See
//! `p37_house_cluster`'s own module doc for the other half of this split.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{DensityField, Neighborhood, PatternField};
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
    /// How far the real field center shifts from the raw site parcel's
    /// plain vertex-averaged centroid toward the single farthest vertex --
    /// `0.0` is dead-center (the old behavior), higher values push the
    /// density peak measurably off-center (P28 Eccentric Nucleus).
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
                "Fraction of the way the field center shifts from the plain vertex centroid toward the farthest vertex (P28 Eccentric Nucleus).",
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
        "Compute a real density-ring field over the raw site parcel, higher near its own density center -- sampled by P37 as it individuates blocks."
    }

    /// `parcel_id` must be a specific raw site parcel id (the SAME one
    /// `p37_house_cluster` is about to carve) -- there is one field for
    /// the whole site, not a per-parcel or wildcard notion. Unlike most
    /// operators in this crate, `"*"` is rejected here rather than
    /// supported, since a field has no natural "for every parcel" reading.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id == "*" {
            return Err("p29_density_rings needs a specific raw site parcel id (the same one passed to p37_house_cluster), not \"*\" -- it computes one real field for the whole site, not per-parcel.".into());
        }
        let parcel = nbhd.parcels.iter().find(|p| p.id == parcel_id).ok_or_else(|| {
            format!("p29_density_rings: no parcel '{parcel_id}' found in this neighborhood.")
        })?;
        let ring = &parcel.polygon.outer;
        if ring.len() < 3 {
            return Err(format!("p29_density_rings: parcel '{parcel_id}' has a degenerate polygon (fewer than 3 vertices)."));
        }

        // Plain vertex-averaged centroid of the raw parcel -- same "origin"
        // convention every other operator in this crate uses for a
        // polygon (see p107_wings_of_light/p95_building_complex), not an
        // area-weighted one (there's only one polygon here, not many to
        // weight against each other the way the pre-field block version
        // had).
        let plain_center = LngLat::new(
            ring.iter().map(|p| p.lng).sum::<f64>() / ring.len() as f64,
            ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64,
        );

        let m_per_deg_lat = 110_540.0;
        let m_per_deg_lng = 111_320.0 * plain_center.lat.to_radians().cos();
        let dist_m = |a: LngLat, b: LngLat| -> f64 {
            let dx = (a.lng - b.lng) * m_per_deg_lng;
            let dy = (a.lat - b.lat) * m_per_deg_lat;
            (dx * dx + dy * dy).sqrt()
        };

        // P28 Eccentric Nucleus: shift the real field center a real
        // fraction of the way from plain_center toward the single
        // farthest real vertex of the raw footprint -- see this file's
        // own "v0.2" module doc. eccentricity_frac == 0.0 reproduces the
        // old dead-center behavior exactly.
        let center = if params.eccentricity_frac > 0.0 && ring.len() > 1 {
            let far = ring.iter().copied()
                .map(|v| (v, dist_m(plain_center, v)))
                .fold((plain_center, 0.0), |best, cur| if cur.1 > best.1 { cur } else { best })
                .0;
            LngLat::new(
                plain_center.lng + params.eccentricity_frac * (far.lng - plain_center.lng),
                plain_center.lat + params.eccentricity_frac * (far.lat - plain_center.lat),
            )
        } else {
            plain_center
        };

        let radius_m = ring.iter().copied()
            .map(|v| dist_m(center, v))
            .fold(0.0_f64, f64::max)
            .max(1.0);

        let n_rings = params.n_rings.round().max(1.0) as u32;
        let field = DensityField {
            center,
            radius_m,
            core_target_stories: params.core_target_stories,
            edge_target_stories: params.edge_target_stories,
            n_rings,
        };

        let trace = SubdivisionTrace {
            operator_name: "p29_density_rings".into(),
            operator_source: self.source(),
            headline: format!(
                "Computed a real density field over '{parcel_id}' ({n_rings} ring(s), radius {radius_m:.0}m), for blocks to sample as they're carved.",
            ),
            steps: vec![format!(
                "center=({:.6},{:.6}), radius_m={radius_m:.1}, core={:.1} stories, edge={:.1} stories, n_rings={n_rings}",
                center.lng, center.lat, params.core_target_stories, params.edge_target_stories
            )],
            caveats: vec![
                "The field center is the raw site parcel's own plain vertex-averaged centroid, \
                 shifted toward its own farthest vertex by eccentricity_frac -- not a real civic or \
                 activity center; this pipeline has no reliable signal for the latter yet (see the \
                 module doc comment).".into(),
                "The eccentric shift always points toward whichever real vertex sits farthest from \
                 the plain centroid -- a real, deterministic direction, not Alexander's own reasoning \
                 for WHY a nucleus forms where it does (transit, an existing landmark); this pipeline \
                 has no such signal to shift toward instead.".into(),
                "Rings are equal-RADIUS bands, not equal-area -- the outer ring covers \
                 disproportionately more site area than the inner one on a roughly circular \
                 site.".into(),
                "This operator only computes and attaches the field -- density_tier/target_stories \
                 aren't set on any parcel until p37_house_cluster samples it while creating blocks. \
                 If P37 never runs (or runs on a different parcel_id), the field is simply never \
                 sampled.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings: vec![],
            new_streets: vec![],
            new_activity_nodes: vec![],
            new_boundaries: vec![],
            new_fields: vec![PatternField::Density(field)],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
        })
    }
}

/// Real sample of `field` at `point`: local-meter distance from
/// `field.center` (the same flat lng/lat-specific conversion this crate's
/// `planar` module uses elsewhere, for consistency with every other
/// distance computation in the pipeline -- NOT haversine, a small,
/// deliberate, honestly-noted departure from a geodesically-exact
/// distance that's negligible at site scale), bucketed into one of
/// `field.n_rings` equal-radius bands, and linearly interpolated between
/// `core_target_stories` and `edge_target_stories` ACROSS RING INDEX (not
/// raw distance -- the same stepped-then-interpolated shape the pre-field
/// per-block version used, preserved exactly here so a real fixture's
/// numbers don't silently drift just because the computation moved).
/// Returns the real `(density_tier label, target_stories)` pair -- exactly
/// the two values `Parcel.density_tier`/`target_stories` have always
/// carried.
pub fn sample_density_field(field: &DensityField, point: LngLat) -> (String, f64) {
    let (ring_idx, n_rings, target_stories) = sample_density_field_ring(field, point);
    (street_smarts_core::ring_tier_label(ring_idx, n_rings), target_stories)
}

/// The same real sample as `sample_density_field`, before the ring index
/// is projected into a string label -- `p37_house_cluster`'s own native
/// `System` port uses this directly to write a `DensityTier` component
/// (`DensityTier::from_ring`) from the SAME `(ring_idx, n_rings)` pair the
/// string path used, not by parsing the string back out afterward. Same
/// "two independent projections of one computation" discipline this
/// operator's own pre-field version established.
pub fn sample_density_field_ring(field: &DensityField, point: LngLat) -> (usize, usize, f64) {
    let m_per_deg_lat = 110_540.0;
    let m_per_deg_lng = 111_320.0 * field.center.lat.to_radians().cos();
    let dx = (point.lng - field.center.lng) * m_per_deg_lng;
    let dy = (point.lat - field.center.lat) * m_per_deg_lat;
    let d = (dx * dx + dy * dy).sqrt();

    let n_rings = (field.n_rings as usize).max(1);
    let normalized = if field.radius_m > 1e-6 { (d / field.radius_m).clamp(0.0, 1.0) } else { 1.0 };
    let ring_idx = ((normalized * n_rings as f64) as usize).min(n_rings - 1);
    let t = if n_rings > 1 { ring_idx as f64 / (n_rings - 1) as f64 } else { 0.0 };
    let target_stories = field.core_target_stories + t * (field.edge_target_stories - field.core_target_stories);
    (ring_idx, n_rings, target_stories)
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{NeighborhoodMeta, Parcel};

    fn m() -> f64 { 111_320.0 }

    fn square_ring(half_side: f64) -> Vec<LngLat> {
        let s = half_side / m();
        vec![
            LngLat::new(-s, -s), LngLat::new(s, -s),
            LngLat::new(s, s), LngLat::new(-s, s), LngLat::new(-s, -s),
        ]
    }

    fn raw_parcel(id: &str, half_side: f64) -> Parcel {
        Parcel {
            id: id.into(),
            polygon: Polygon::from_ring(square_ring(half_side)),
            area_acres: 0.0,
            use_category: None,
            ownership: None,
            is_eda: false,
            spec: None,
            density_tier: None,
            target_stories: None,
        }
    }

    fn nbhd(parcels: Vec<Parcel>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels, buildings: vec![], streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![], pattern_fields: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P29 unit fixture".into(),
            },
        }
    }

    #[test]
    fn wildcard_parcel_id_is_rejected() {
        let n = nbhd(vec![raw_parcel("SITE", 100.0)]);
        assert!(P29DensityRings.apply(&n, "*", &P29Params::defaults(), 0).is_err());
    }

    #[test]
    fn unknown_parcel_id_is_an_error_not_a_silent_no_op() {
        let n = nbhd(vec![raw_parcel("SITE", 100.0)]);
        assert!(P29DensityRings.apply(&n, "NOPE", &P29Params::defaults(), 0).is_err());
    }

    #[test]
    fn a_real_field_is_attached_to_the_neighborhood() {
        let n = nbhd(vec![raw_parcel("SITE", 100.0)]);
        let sub = P29DensityRings.apply(&n, "SITE", &P29Params::defaults(), 0).unwrap();
        assert_eq!(sub.new_fields.len(), 1);
        let PatternField::Density(field) = &sub.new_fields[0];
        assert!(field.radius_m > 50.0, "radius should reflect the real 100m half-side square, got {}", field.radius_m);
        assert_eq!(field.n_rings, 3);
        // No parcels/buildings touched -- this operator only attaches a field.
        assert!(sub.new_parcels.is_empty());
        assert!(sub.replaced_parcel_ids.is_empty());
    }

    #[test]
    fn sampling_at_the_field_center_gives_the_full_core_value() {
        let n = nbhd(vec![raw_parcel("SITE", 100.0)]);
        let params = P29Params { eccentricity_frac: 0.0, ..P29Params::defaults() };
        let sub = P29DensityRings.apply(&n, "SITE", &params, 0).unwrap();
        let PatternField::Density(field) = &sub.new_fields[0];
        let (label, stories) = sample_density_field(field, field.center);
        assert_eq!(label, "core");
        assert!((stories - params.core_target_stories).abs() < 1e-9);
    }

    #[test]
    fn sampling_far_beyond_the_radius_clamps_to_the_edge_value() {
        let n = nbhd(vec![raw_parcel("SITE", 100.0)]);
        let params = P29Params { eccentricity_frac: 0.0, ..P29Params::defaults() };
        let sub = P29DensityRings.apply(&n, "SITE", &params, 0).unwrap();
        let PatternField::Density(field) = &sub.new_fields[0];
        let far = LngLat::new(field.center.lng + 10.0, field.center.lat);
        let (label, stories) = sample_density_field(field, far);
        assert_eq!(label, "edge");
        assert!((stories - params.edge_target_stories).abs() < 1e-9);
    }

    #[test]
    fn eccentricity_shifts_the_center_off_the_plain_centroid() {
        let n = nbhd(vec![raw_parcel("SITE", 100.0)]);
        let dead_center = P29DensityRings.apply(&n, "SITE", &P29Params { eccentricity_frac: 0.0, ..P29Params::defaults() }, 0).unwrap();
        let shifted = P29DensityRings.apply(&n, "SITE", &P29Params { eccentricity_frac: 0.5, ..P29Params::defaults() }, 0).unwrap();
        let PatternField::Density(d0) = &dead_center.new_fields[0];
        let PatternField::Density(d1) = &shifted.new_fields[0];
        assert!(
            (d0.center.lng - d1.center.lng).abs() > 1e-9 || (d0.center.lat - d1.center.lat).abs() > 1e-9,
            "eccentricity_frac > 0 should move the field center off the plain centroid"
        );
    }

    #[test]
    fn ring_bucketing_matches_the_pre_field_stepped_interpolation() {
        // A regression pin for the exact math: 5 rings, sampling exactly at
        // the midpoint (normalized distance 0.5) should land in ring 2 of
        // 0..4 (since (0.5 * 5) as usize == 2), and its target_stories
        // should be the linear interpolation at t = 2/4 = 0.5.
        let field = DensityField {
            center: LngLat::new(0.0, 0.0),
            radius_m: 100.0,
            core_target_stories: 6.0,
            edge_target_stories: 2.0,
            n_rings: 5,
        };
        let m_per_deg_lng = 111_320.0;
        let point = LngLat::new(field.center.lng + 50.0 / m_per_deg_lng, field.center.lat);
        let (ring_idx, n_rings, target_stories) = sample_density_field_ring(&field, point);
        assert_eq!(n_rings, 5);
        assert_eq!(ring_idx, 2);
        assert!((target_stories - 4.0).abs() < 1e-9, "expected t=0.5 interpolation (6.0 + 0.5*(2.0-6.0) = 4.0), got {target_stories}");
    }
}
