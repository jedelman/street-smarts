//! Building shape — places a building footprint inside each building-pad parcel.
//!
//! v0.1 implementation is intentionally simple: inscribe a rectangle inside
//! each pad, oriented along its longest axis, leaving a ~3m perimeter yard.
//! No daylight wings yet, no entrance placement yet, no courtyard awareness.
//! Calling it "v0.1 of P102/P106/P107" would overclaim — it's just enough
//! geometry to demonstrate the layered pipeline (P95 → BuildingShape) and
//! make buildings visible on the map.
//!
//! What this operator IS:
//! - A second-pass operator that consumes the output of P95 (or any operator
//!   that produces parcels tagged `use_category: "p95_building_pad"`)
//! - A simple inscribed-rectangle generator that produces a `Building` NIR
//!   entity per pad
//!
//! What this operator IS NOT, yet:
//! - P107 Wings of Light (max wing width for daylight) — needs L/U/H shapes
//! - P102 Family of Entrances (entrance facing courtyard) — needs front detection
//! - P106 Positive Outdoor Space (the leftover yard should feel shaped) — needs
//!   companion outdoor-space generation
//!
//! Those land in a later session. Marked clearly in the operator's caveats.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{
    area, centroid, inset_convex, local_to_ring, ring_to_local, Pt2,
};
use crate::prng::Prng;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Building, Neighborhood, Parcel};
use street_smarts_core::opinion::SourceCitation;

/// Tunable parameters for building-shape inscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingShapeParams {
    /// Setback (perimeter yard) in metres on each side of the building.
    /// Below 1.5m gets you fire-code violations; above 8m the yard dominates.
    pub setback_m: f64,
    /// Minimum pad area to bother shaping. Below this we skip the pad —
    /// it's left as is (raw building pad, no footprint).
    pub min_pad_area_m2: f64,
    /// Coverage ratio — fraction of post-setback pad area that becomes
    /// building footprint. 1.0 = footprint fills the post-setback envelope;
    /// 0.5 = footprint is half of it (the other half becomes additional yard).
    pub coverage_ratio: f64,
    /// Assumed building height in metres. Stored on the Building entity so
    /// downstream operators (and the chorus) can reason about massing later.
    pub assumed_height_m: f64,
}

impl Parameters for BuildingShapeParams {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "setback_m",
                "Perimeter yard width around the building footprint.",
                0.5, 10.0, 3.0,
            ).with_unit("m"),
            ParamSpec::float(
                "min_pad_area_m2",
                "Don't try to shape pads smaller than this.",
                50.0, 500.0, 120.0,
            ).with_unit("m²"),
            ParamSpec::float(
                "coverage_ratio",
                "Fraction of the inset pad area that becomes building.",
                0.2, 1.0, 0.75,
            ),
            ParamSpec::float(
                "assumed_height_m",
                "Assumed building height for the NIR Building entity.",
                3.0, 30.0, 9.0,
            ).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self {
            setback_m: 3.0,
            min_pad_area_m2: 120.0,
            coverage_ratio: 0.75,
            assumed_height_m: 9.0,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.setback_m, self.min_pad_area_m2, self.coverage_ratio, self.assumed_height_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.setback_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.min_pad_area_m2 = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.coverage_ratio = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) { p.assumed_height_m = s.clamp(*x); }
        p
    }
}

pub struct BuildingShape;

impl PatternOperator for BuildingShape {
    type Params = BuildingShapeParams;

    fn name(&self) -> &'static str { "building_shape" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p102_p106_p107_v01".into(),
            display: "Alexander et al., APL — Patterns 102 / 106 / 107 (v0.1 stub: simple inscribed rectangle, daylight-wings deferred)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl107/apl107.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Inscribe a building footprint inside each building-pad parcel."
    }

    /// `parcel_id` is interpreted specially here: if it is the literal string
    /// `"*"` the operator runs on EVERY parcel whose `use_category` is
    /// `"p95_building_pad"`. Otherwise it runs on the specific parcel.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        let targets: Vec<&Parcel> = if parcel_id == "*" {
            nbhd.parcels
                .iter()
                .filter(|p| p.use_category.as_deref() == Some("p95_building_pad"))
                .collect()
        } else {
            nbhd.parcels.iter().filter(|p| p.id == parcel_id).collect()
        };

        if targets.is_empty() {
            return Err(format!(
                "no matching parcel(s) for '{}' — building_shape expects either a specific parcel id or '*' to target all P95 pads.",
                parcel_id
            ));
        }

        let mut prng = Prng::new(seed);
        let mut buildings: Vec<Building> = Vec::new();
        let mut new_parcels: Vec<Parcel> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut shaped = 0;
        let mut skipped_small = 0;
        let mut skipped_other = 0;

        for parcel in &targets {
            let pad_area_m2 = if parcel.area_acres > 0.0 {
                parcel.area_acres * 4046.86
            } else {
                parcel.polygon.area_m2()
            };
            if pad_area_m2 < params.min_pad_area_m2 {
                skipped_small += 1;
                continue;
            }
            let origin = LngLat::new(
                average_lng(&parcel.polygon.outer),
                average_lat(&parcel.polygon.outer),
            );
            let local = ring_to_local(&parcel.polygon.outer, &origin);
            if local.len() < 3 {
                skipped_other += 1;
                continue;
            }
            // Inset by the setback distance.
            let envelope = inset_convex(&local, params.setback_m);
            if envelope.len() < 3 || area(&envelope) < 50.0 {
                skipped_small += 1;
                continue;
            }
            // Coverage ratio: shrink the envelope further if coverage < 1.
            // Simple approach: scale toward centroid.
            let footprint = if (params.coverage_ratio - 1.0).abs() < 0.01 {
                envelope
            } else {
                shrink_toward_centroid(&envelope, params.coverage_ratio)
            };
            if footprint.len() < 3 { skipped_other += 1; continue; }

            // Convert back to WGS84 ring.
            let ring = local_to_ring(&footprint, &origin);
            let footprint_area_m2 = area(&footprint);

            // Build the Building entity. Slight per-building height jitter so
            // future massing-aware opinions have something to chew on.
            let height_jitter = (prng.next_f64() - 0.5) * 1.5;
            let id = format!("{}_building", parcel.id);
            buildings.push(Building {
                id: id.clone(),
                polygon: Polygon::from_ring(ring),
                height_m: Some((params.assumed_height_m + height_jitter).max(2.5)),
                typology: Some("p95_inscribed_v01".into()),
                year_built: None,
                parcel_id: Some(parcel.id.clone()),
                floors: None,
                openings: vec![],
            });

            // Update the parcel's use_category to indicate a building exists.
            // We replace the parcel (same id, same polygon) with the new tag.
            let mut updated = (*parcel).clone();
            updated.use_category = Some("p95_pad_with_building".into());
            new_parcels.push(updated);
            replaced.push(parcel.id.clone());
            shaped += 1;

            let _ = footprint_area_m2;
        }

        if shaped == 0 {
            return Err(format!(
                "building_shape: 0 pads were large enough to shape ({} too small, {} other). \
                 Lower min_pad_area_m2 or use a denser P95 first.",
                skipped_small, skipped_other
            ));
        }

        steps.push(format!(
            "Shaped {} pad(s); skipped {} too-small; skipped {} other.",
            shaped, skipped_small, skipped_other
        ));

        let trace = SubdivisionTrace {
            operator_name: "building_shape".into(),
            operator_source: self.source(),
            headline: format!(
                "Placed building footprints in {} pad{}.",
                shaped, if shaped == 1 { "" } else { "s" }
            ),
            steps,
            caveats: vec![
                "This is the v0.1 stub of P102/P106/P107. It just inscribes a rectangle-ish \
                 shape (the pad inset by the setback). It does NOT yet honor Wings of Light \
                 (no L/U/H shapes for daylight), Family of Entrances (no entrance placement), \
                 or Positive Outdoor Space (no awareness of the leftover yard's shape).".into(),
                "Building height is a guess (assumed_height_m param) with mild jitter. Not \
                 derived from program, zoning, or anything else real.".into(),
                "Future work: take the building's relationship to the nearest courtyard or \
                 plaza into account — orient the front of the building toward shared open space.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels,
            new_open_space: vec![],
            new_buildings: buildings,
            new_streets: vec![],
            replaced_parcel_ids: replaced,
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            trace,
        })
    }
}

fn average_lng(ring: &[LngLat]) -> f64 {
    if ring.is_empty() { return 0.0; }
    ring.iter().map(|p| p.lng).sum::<f64>() / ring.len() as f64
}
fn average_lat(ring: &[LngLat]) -> f64 {
    if ring.is_empty() { return 0.0; }
    ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64
}

/// Shrink a polygon toward its centroid by a uniform `ratio`. ratio=1 is no-op.
fn shrink_toward_centroid(poly: &[Pt2], ratio: f64) -> Vec<Pt2> {
    if poly.is_empty() || ratio <= 0.0 { return vec![]; }
    if ratio >= 1.0 { return poly.to_vec(); }
    // Area scales with ratio², so the linear factor is sqrt(ratio).
    let linear = ratio.sqrt();
    let c = centroid(poly);
    poly.iter().map(|p| Pt2 {
        x: c.x + (p.x - c.x) * linear,
        y: c.y + (p.y - c.y) * linear,
    }).collect()
}
