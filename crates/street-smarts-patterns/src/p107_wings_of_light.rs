//! P107 Wings of Light — shape a building footprint so no interior point is
//! farther than `max_wing_width_m` from an exterior wall.
//!
//! From Alexander, *A Pattern Language*, Pattern 107:
//! > Buildings which have a lot of window area on their outside walls, are
//! > best kept narrow, so that natural light can reach into every part of
//! > the room... no part of the building [should be] more than about 25
//! > feet from an outside wall.
//!
//! # v0.1 approach
//! If a pad's setback envelope is already narrow enough (its bounding-box
//! short side <= `max_wing_width_m`), it already satisfies the pattern --
//! fill it as a solid footprint, same as the old inscribed-rectangle stub.
//!
//! If it's too deep, carve a central courtyard by insetting
//! `max_wing_width_m` from the perimeter. What's left is a ring: every
//! point in it is within `max_wing_width_m` of EITHER the outer wall or the
//! new courtyard wall. This is one of Alexander's own named ways to satisfy
//! P107 -- a courtyard building -- not a branching L/U/H wing. Simpler to
//! build correctly for v0.1; true branching wings are deferred (noted in
//! caveats, not hidden).
//!
//! The courtyard itself is emitted as an `OpenSpace` (kind `Plaza`, same
//! convention P95 uses for its courtyard cell) -- it's usable open space,
//! not leftover void.
//!
//! # Known limitation
//! Uses `inset_convex` for the courtyard cut, which assumes a convex
//! envelope. A pad whose setback envelope is genuinely non-convex (possible
//! if it came from a concave source parcel) will misbehave the same way
//! `clip_convex_to_polygon` warns about on non-convex input. Not handled in
//! v0.1 -- this is a real gap, not swept under a caveat that undersells it.
//!
//! # v0.2: don't double-apply the yard setback on a P95 pad
//! `setback_m` used to inset EVERY pad's boundary before drawing the
//! footprint, regardless of where the pad came from. But a pad tagged
//! `use_category: "p95_building_pad"` was already inset by P95's own
//! `pad_inset_m` -- "room for interconnecting spaces" between neighboring
//! buildings within a complex. Applying `setback_m` again on top of that
//! shrinks the footprint by BOTH insets, stacked -- with each at its 3.0m
//! default, two neighboring buildings end up ~12m apart (2 x (pad_inset_m
//! + setback_m)) for a gap that was only ever supposed to be sized once.
//! Real urban infill -- party-wall buildings running straight to the lot
//! line with zero gap -- is the opposite of what stacking these produces:
//! isolated pavilions, each surrounded by its own moat, even when nothing
//! actually calls for one. Alexander's own P108 (Connected Buildings)
//! argues against exactly this.
//!
//! So: `setback_m` now only applies when the pad did NOT already come from
//! P95 (i.e. `use_category != "p95_building_pad"`) -- someone calling P107
//! directly on a raw parcel still gets a real yard, since nothing else
//! reserved one. A P95 pad's own inset is treated as the authoritative gap;
//! P107 builds right to that boundary.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{
    area, bbox, inset_convex, local_to_ring, ring_to_local,
};
use crate::prng::Prng;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{LngLat, Polygon, PolygonPart};
use street_smarts_core::nir::{Building, Neighborhood, OpenSpace, OpenSpaceKind, Parcel};
use street_smarts_core::Scope;
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P107Params {
    /// The daylight-depth threshold. Alexander's own number is ~7.5m (25ft)
    /// from an outside wall; a wing lit from both long sides can therefore
    /// be up to ~2x that (~15m) and still keep every interior point within
    /// range of a window wall. Default assumes double-loaded wings.
    pub max_wing_width_m: f64,
    /// Perimeter yard, same convention as the old BuildingShape stub. Only
    /// applied to a pad that did NOT already come from P95 (see the module
    /// doc's "v0.2" section) -- a P95 pad's own `pad_inset_m` is treated as
    /// the authoritative gap, not stacked with this one.
    pub setback_m: f64,
    /// Don't bother shaping pads smaller than this.
    pub min_pad_area_m2: f64,
    /// If carving a courtyard would leave a hole smaller than this, it's
    /// not meaningful open space -- fall back to a solid footprint instead.
    pub courtyard_min_area_m2: f64,
    /// Fallback height for a pad with no `target_stories` assignment (P96
    /// Number of Stories didn't run, or didn't reach this pad).
    pub assumed_height_m: f64,
    /// Floor-to-floor height used to convert a pad's `target_stories` (set
    /// by P96) into real building height. Only used when `target_stories`
    /// is present -- otherwise `assumed_height_m` applies directly.
    pub floor_to_floor_m: f64,
}

impl Parameters for P107Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "max_wing_width_m",
                "Max building depth for daylight to reach every interior point (double-loaded wing).",
                6.0, 25.0, 15.0,
            ).with_unit("m"),
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
                "courtyard_min_area_m2",
                "Minimum courtyard size worth carving; below this, fill solid instead.",
                10.0, 200.0, 30.0,
            ).with_unit("m²"),
            ParamSpec::float(
                "assumed_height_m",
                "Fallback building height when a pad has no target_stories assignment.",
                3.0, 30.0, 9.0,
            ).with_unit("m"),
            ParamSpec::float(
                "floor_to_floor_m",
                "Floor-to-floor height used to convert a pad's target_stories into real height.",
                2.5, 5.0, 3.5,
            ).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self {
            max_wing_width_m: 15.0,
            setback_m: 3.0,
            min_pad_area_m2: 120.0,
            courtyard_min_area_m2: 30.0,
            assumed_height_m: 9.0,
            floor_to_floor_m: 3.5,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![
            self.max_wing_width_m,
            self.setback_m,
            self.min_pad_area_m2,
            self.courtyard_min_area_m2,
            self.assumed_height_m,
            self.floor_to_floor_m,
        ]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.max_wing_width_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.setback_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.min_pad_area_m2 = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) { p.courtyard_min_area_m2 = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(4), v.get(4)) { p.assumed_height_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(5), v.get(5)) { p.floor_to_floor_m = s.clamp(*x); }
        p
    }
}

pub struct P107WingsOfLight;

impl PatternOperator for P107WingsOfLight {
    type Params = P107Params;

    fn name(&self) -> &'static str { "p107_wings_of_light" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p107".into(),
            display: "Alexander et al., A Pattern Language, Pattern 107 (Wings of Light)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl107/apl107.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Shape a building footprint (solid or courtyard-ring) so no interior point is farther than max_wing_width_m from an exterior wall."
    }

    /// `parcel_id` special-cases `"*"` to run on every parcel tagged
    /// `use_category: "p95_building_pad"`, same convention as BuildingShape.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        let targets: Vec<&Parcel> = if parcel_id == "*" {
            nbhd.select(&Scope::BUILDING_PAD).collect()
        } else {
            nbhd.parcels.iter().filter(|p| p.id == parcel_id).collect()
        };

        if targets.is_empty() {
            return Err(format!(
                "no matching parcel(s) for '{}' — p107_wings_of_light expects a specific pad id or '*'.",
                parcel_id
            ));
        }

        let mut prng = Prng::new(seed);
        let mut buildings: Vec<Building> = Vec::new();
        let mut new_open: Vec<OpenSpace> = Vec::new();
        let mut new_parcels: Vec<Parcel> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut n_solid = 0;
        let mut n_courtyard = 0;
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
                parcel.polygon.outer.iter().map(|p| p.lng).sum::<f64>() / parcel.polygon.outer.len() as f64,
                parcel.polygon.outer.iter().map(|p| p.lat).sum::<f64>() / parcel.polygon.outer.len() as f64,
            );
            let local = ring_to_local(&parcel.polygon.outer, &origin);
            if local.len() < 3 {
                skipped_other += 1;
                continue;
            }
            // A P95 pad already has its own gap reserved (pad_inset_m) --
            // don't inset it again for a "yard" that's already there. Only
            // apply setback_m to a pad this operator has no other reason to
            // believe already has room around it.
            let from_p95_pad = Scope::BUILDING_PAD.matches_parcel(parcel);
            let envelope = if from_p95_pad {
                local.clone()
            } else {
                inset_convex(&local, params.setback_m)
            };
            if envelope.len() < 3 || area(&envelope) < 50.0 {
                skipped_small += 1;
                continue;
            }

            let (min_pt, max_pt) = bbox(&envelope);
            let short_side = (max_pt.x - min_pt.x).min(max_pt.y - min_pt.y);

            // P96 Number of Stories may have already assigned this pad a
            // real story count -- respect it (small jitter for an organic
            // feel, not enough to cross into a different story count).
            // Falls back to the flat assumed_height_m default otherwise,
            // unchanged from before P96 existed.
            let height_m = if let Some(target_stories) = parcel.target_stories {
                let height_jitter = (prng.next_f64() - 0.5) * (params.floor_to_floor_m * 0.2);
                (target_stories * params.floor_to_floor_m + height_jitter).max(2.5)
            } else {
                let height_jitter = (prng.next_f64() - 0.5) * 1.5;
                (params.assumed_height_m + height_jitter).max(2.5)
            };

            if short_side <= params.max_wing_width_m {
                // Already narrow enough -- fill solid, P107 is satisfied
                // without needing to carve anything.
                let ring = local_to_ring(&envelope, &origin);
                buildings.push(Building {
                    id: format!("{}_building", parcel.id),
                    polygon: Polygon::from_ring(ring),
                    height_m: Some(height_m),
                    typology: Some("p107_solid_v01".into()),
                    year_built: None,
                    parcel_id: Some(parcel.id.clone()),
                    floors: None,
                    openings: vec![],
                    interior_cells: vec![],
                });
                n_solid += 1;
            } else {
                // Too deep for single-sided daylight -- carve a courtyard.
                let inner = inset_convex(&envelope, params.max_wing_width_m);
                let inner_area = if inner.len() >= 3 { area(&inner) } else { 0.0 };

                if inner.len() < 3 || inner_area < params.courtyard_min_area_m2 {
                    // Courtyard would be too small to be real open space --
                    // fall back to solid rather than carve a token slit.
                    let ring = local_to_ring(&envelope, &origin);
                    buildings.push(Building {
                        id: format!("{}_building", parcel.id),
                        polygon: Polygon::from_ring(ring),
                        height_m: Some(height_m),
                        typology: Some("p107_solid_fallback_v01".into()),
                        year_built: None,
                        parcel_id: Some(parcel.id.clone()),
                        floors: None,
                        openings: vec![],
                        interior_cells: vec![],
                    });
                    n_solid += 1;
                } else {
                    let outer_ring = local_to_ring(&envelope, &origin);
                    let inner_ring = local_to_ring(&inner, &origin);
                    buildings.push(Building {
                        id: format!("{}_building", parcel.id),
                        polygon: Polygon::from_parts(vec![PolygonPart {
                            outer: outer_ring,
                            holes: vec![inner_ring.clone()],
                        }]),
                        height_m: Some(height_m),
                        typology: Some("p107_courtyard_v01".into()),
                        year_built: None,
                        parcel_id: Some(parcel.id.clone()),
                        floors: None,
                        openings: vec![],
                        interior_cells: vec![],
                    });
                    new_open.push(OpenSpace {
                        id: format!("{}_P107_courtyard", parcel.id),
                        polygon: Polygon::from_ring(inner_ring),
                        kind: OpenSpaceKind::Plaza,
                    });
                    n_courtyard += 1;
                }
            }

            let mut updated = (*parcel).clone();
            updated.use_category = Some("p95_pad_with_building".into());
            new_parcels.push(updated);
            replaced.push(parcel.id.clone());
        }

        if n_solid == 0 && n_courtyard == 0 {
            return Err(format!(
                "p107_wings_of_light: 0 pads were large enough to shape ({} too small, {} other).",
                skipped_small, skipped_other
            ));
        }

        steps.push(format!(
            "{} solid footprint(s), {} courtyard-ring footprint(s); skipped {} too-small, {} other.",
            n_solid, n_courtyard, skipped_small, skipped_other
        ));

        let trace = SubdivisionTrace {
            operator_name: "p107_wings_of_light".into(),
            operator_source: self.source(),
            headline: format!(
                "Shaped {} building(s) ({} solid, {} courtyard-ring) for daylight depth.",
                n_solid + n_courtyard, n_solid, n_courtyard
            ),
            steps,
            caveats: vec![
                "Courtyard cut assumes the setback envelope is convex (uses inset_convex). A pad \
                 from a genuinely concave source parcel can produce a non-convex envelope, which \
                 this does not detect or handle correctly.".into(),
                "Only two configurations are implemented: solid footprint, or one central \
                 courtyard ring. True branching wings (L/U/H/T footprints), which Alexander also \
                 names as valid, are deferred.".into(),
                "max_wing_width_m assumes double-loaded wings (rooms on both sides of a central \
                 spine). Real programs vary; this is a placeholder depth, not derived from any \
                 actual room layout.".into(),
                "setback_m is skipped entirely for pads that came from P95 -- P95's own \
                 pad_inset_m already reserved that gap, and stacking a second setback on top \
                 produced isolated pavilions with far more space between buildings than either \
                 pattern called for on its own. Real party-wall infill (buildings running to the \
                 lot line with zero gap) needs more than removing the double setback -- see P108 \
                 Connected Buildings, not implemented here.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels,
            new_open_space: new_open,
            new_buildings: buildings,
            new_streets: vec![],
            replaced_parcel_ids: replaced,
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            trace,
        })
    }
}
