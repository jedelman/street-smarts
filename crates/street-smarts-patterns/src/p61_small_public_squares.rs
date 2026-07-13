//! P61 Small Public Squares — keep public squares small enough to feel
//! intimate, not desolate.
//!
//! From Alexander, *A Pattern Language*, Pattern 61:
//! > A square which is more than about 60 feet [~18m] across... will
//! > never feel comfortable or intimate, unless it is extremely crowded.
//!
//! # v0.1 approach
//! Scans existing `OpenSpace` entities of kind `Plaza` -- the courtyards
//! P95 and P107 already produce are the natural candidates here, since
//! this pipeline doesn't yet place squares anywhere else. For each:
//! - If its longer bounding-box dimension is already <= `max_dimension_m`,
//!   leave it alone -- P61 is already satisfied.
//! - If it's too large, shrink it toward its centroid by exactly the
//!   linear factor needed to bring the longer dimension down to
//!   `max_dimension_m`, and REPLACE the old plaza with the smaller one
//!   (via the new `replaced_open_space_ids` mechanism -- see subdivision.rs).
//!
//! # What this deliberately does NOT do
//! The leftover area (original footprint minus the shrunk square) is not
//! assigned anywhere. Alexander's actual guidance for a genuinely large
//! plaza is to break it into a few smaller connected squares with real
//! edges (colonnades, trees, level changes) -- not to shrink-and-abandon
//! the remainder. Deciding what the leftover land becomes (a second small
//! square? garden? nothing?) is a real design decision this operator
//! doesn't make. Said in caveats, not hidden in the geometry.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{area, bbox, ring_to_local, local_to_ring, scale_toward_centroid};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Neighborhood, OpenSpace, OpenSpaceKind};
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P61Params {
    /// Alexander's own number: ~60 feet (18.3m). Squares larger than this
    /// rarely feel intimate.
    pub max_dimension_m: f64,
    /// Don't bother touching plazas already this small or smaller -- v0.1
    /// only shrinks oversized squares, doesn't grow undersized ones (that
    /// would require land this operator doesn't have a claim to).
    pub min_meaningful_area_m2: f64,
}

impl Parameters for P61Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "max_dimension_m",
                "Max comfortable public square dimension (Alexander: ~60ft/18m).",
                8.0, 30.0, 18.3,
            ).with_unit("m"),
            ParamSpec::float(
                "min_meaningful_area_m2",
                "Skip plazas already this small or smaller -- nothing useful to shrink.",
                5.0, 200.0, 20.0,
            ).with_unit("m²"),
        ]
    }
    fn defaults() -> Self {
        Self { max_dimension_m: 18.3, min_meaningful_area_m2: 20.0 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.max_dimension_m, self.min_meaningful_area_m2]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.max_dimension_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.min_meaningful_area_m2 = s.clamp(*x); }
        p
    }
}

pub struct P61SmallPublicSquares;

impl PatternOperator for P61SmallPublicSquares {
    type Params = P61Params;

    fn name(&self) -> &'static str { "p61_small_public_squares" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p61".into(),
            display: "Alexander et al., A Pattern Language, Pattern 61 (Small Public Squares)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl61/apl61.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Shrink oversized plazas/courtyards to Alexander's ~18m intimacy threshold."
    }

    /// Operates on every `OpenSpace` of kind `Plaza` in the neighborhood.
    /// `parcel_id` is unused (this operator works on open space, not
    /// parcels) but kept for `PatternOperator` trait consistency; pass `"*"`.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        _parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        let plazas: Vec<&OpenSpace> = nbhd
            .open_space
            .iter()
            .filter(|o| o.kind == OpenSpaceKind::Plaza)
            .collect();

        if plazas.is_empty() {
            return Err("p61_small_public_squares: no Plaza-kind open space found. Run P95/P107 first.".into());
        }

        let mut new_open: Vec<OpenSpace> = Vec::new();
        let mut replaced_ids: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut n_shrunk = 0;
        let mut n_already_ok = 0;
        let mut n_skipped_tiny = 0;

        for plaza in &plazas {
            let plaza_area_m2 = plaza.polygon.area_m2();
            if plaza_area_m2 < params.min_meaningful_area_m2 {
                n_skipped_tiny += 1;
                continue;
            }

            let origin = LngLat::new(
                plaza.polygon.outer.iter().map(|p| p.lng).sum::<f64>() / plaza.polygon.outer.len() as f64,
                plaza.polygon.outer.iter().map(|p| p.lat).sum::<f64>() / plaza.polygon.outer.len() as f64,
            );
            let local = ring_to_local(&plaza.polygon.outer, &origin);
            if local.len() < 3 { continue; }

            let (min_pt, max_pt) = bbox(&local);
            let longer_side = (max_pt.x - min_pt.x).max(max_pt.y - min_pt.y);

            if longer_side <= params.max_dimension_m {
                n_already_ok += 1;
                steps.push(format!(
                    "{}: {:.1}m across, already within {:.1}m -- unchanged.",
                    plaza.id, longer_side, params.max_dimension_m
                ));
                continue;
            }

            let factor = params.max_dimension_m / longer_side;
            let shrunk_local = scale_toward_centroid(&local, factor);
            let shrunk_area = area(&shrunk_local);
            let ring = local_to_ring(&shrunk_local, &origin);

            new_open.push(OpenSpace {
                id: format!("{}_p61", plaza.id),
                polygon: street_smarts_core::geometry::Polygon::from_ring(ring),
                kind: OpenSpaceKind::Plaza,
            });
            replaced_ids.push(plaza.id.clone());
            n_shrunk += 1;

            steps.push(format!(
                "{}: {:.1}m across -> shrunk to {:.1}m ({:.0}m² -> {:.0}m², {:.0}m² unassigned leftover).",
                plaza.id, longer_side, params.max_dimension_m, plaza_area_m2, shrunk_area,
                plaza_area_m2 - shrunk_area
            ));
        }

        if n_shrunk == 0 && n_already_ok == 0 {
            return Err(format!(
                "p61_small_public_squares: all {} plaza(s) were below min_meaningful_area_m2 -- nothing to evaluate.",
                n_skipped_tiny
            ));
        }

        steps.insert(0, format!(
            "{} plaza(s) already compliant, {} shrunk, {} too small to bother with.",
            n_already_ok, n_shrunk, n_skipped_tiny
        ));

        let trace = SubdivisionTrace {
            operator_name: "p61_small_public_squares".into(),
            operator_source: self.source(),
            headline: format!(
                "{} of {} plaza(s) exceeded {:.1}m and were shrunk to comply with P61.",
                n_shrunk, plazas.len(), params.max_dimension_m
            ),
            steps,
            caveats: vec![
                "Leftover area from shrinking is NOT assigned to anything -- it's real \
                 unresolved land, not fabricated as a second open space. Alexander's actual \
                 guidance for a genuinely large plaza is to break it into a few smaller \
                 CONNECTED squares with real edges (colonnades, trees, level changes), not to \
                 shrink-and-abandon the remainder. That's a real design decision this v0.1 \
                 doesn't make.".into(),
                "Only shrinks oversized squares. Does not grow undersized ones -- doing that \
                 would require claiming adjacent land this operator has no basis to take.".into(),
                "Assumes the plaza's bounding-box longer side is a reasonable proxy for \
                 'how large it feels.' A long thin plaza and a square plaza of the same bbox \
                 diagonal don't feel the same; this doesn't distinguish them.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: new_open,
            new_buildings: vec![],
            new_streets: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: replaced_ids,
            trace,
        })
    }
}
