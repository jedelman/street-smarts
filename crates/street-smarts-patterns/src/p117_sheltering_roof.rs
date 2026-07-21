//! P117 Sheltering Roof — assigns every real building a real, sloped shed
//! roof, closing this pipeline's "no roof geometry at all" gap.
//!
//! From Alexander, *A Pattern Language*, Pattern 117 (p. 569), via
//! patternlanguage.cc/Patterns/Sheltering-Roof-(117):
//! > **Problem:** The roof plays a primal role in our lives. If the roof
//! > is hidden, if its presence cannot be felt around the building, or
//! > if it cannot be used, then people will lack a fundamental sense of
//! > shelter.
//! > **Solution:** Slope the roof or make a vault of it, make its entire
//! > surface visible, and bring the eaves of the roof down low, as low
//! > as 6'0" or 6'6" at places like the entrance.
//!
//! # Also closes P162 North Face, by the same real geometry
//!
//! From Alexander, *A Pattern Language*, Pattern 162 (p. 761), via
//! patternlanguage.cc/Patterns/North-Face-(162):
//! > **Problem:** Look at the north sides of the buildings which you
//! > know. Almost everywhere you will find that these are the spots
//! > which are dead and dank, gloomy and useless.
//! > **Solution:** Make the north face of the building a cascade which
//! > slopes down to the ground, so that the sun which normally casts a
//! > long shadow to the north strikes the ground immediately beside the
//! > building.
//!
//! P162's own claim is stronger than a generic "low eave somewhere": it
//! names a SPECIFIC face (north) that must be the low side, so sun
//! reaches the ground beside it. A SYMMETRIC roof (a plain gable, equal
//! eave height on every side) wouldn't honestly satisfy that -- it
//! doesn't single out north as the low side any more than south. A
//! `RoofShape::Shed` (one real sloped plane, high on one side and low on
//! the other) does, when its low side is set to true north -- this
//! pipeline's real lng/lat makes "true north" an exact bearing
//! (`slope_azimuth_deg = 0.0`), not an approximation.
//!
//! # A real correction: `eave_height_m` is relative to the building, not
//! # an absolute ground height
//!
//! An earlier version of this operator set `eave_height_m` to Alexander's
//! own literal P117 figure (6'0"-6'6", ~1.9m) as an ABSOLUTE height above
//! the site datum -- which is architecturally wrong for any real
//! multi-story building: it would mean the entire north WALL is only
//! ~1.9m tall, with everything above it replaced by roof slope, rather
//! than a normal multi-story facade with a real roof CAP on top.
//! Alexander's own "as low as 6'0"" figure describes a LOCAL condition
//! "at places like the entrance" (a covered porch/overhang), not a claim
//! that a whole building's north wall should be person-height -- a real,
//! different (and not-yet-built) feature closer to P119 Arcades' own
//! projecting-canopy family than to this operator's whole-building roof
//! slope. Caught before this ever reached the renderer, by reasoning
//! through what render.py would actually have to draw (a "building" that
//! is mostly a giant ramp) rather than just checking the numbers
//! round-tripped.
//!
//! Corrected: `eave_height_m` is now always the building's own real
//! `height_m` (the low/north side sits at the SAME height every other
//! wall in this pipeline already reaches), and `ridge_height_m` is
//! `height_m + roof_rise_m` -- the roof adds a real, modest amount of
//! extra height above the nominal building height, the standard
//! architectural reading of "building height" (eave/parapet height, with
//! the ridge rising somewhat above it). `roof_rise_m` is a plausible real
//! roof-pitch figure -- NOT Alexander's own literal entrance-eave number
//! (which describes a different, local condition this whole-building
//! single-slope form doesn't reach) -- same category as `p95_building_
//! complex`'s `pad_inset_m` or `p221_natural_doors_and_windows`'s
//! `sill_frac`.
//!
//! # What this deliberately does NOT do
//! - **No per-wing cascade.** Alexander's P116 Cascade of Roofs asks for
//!   roofs that step down "following the social hierarchy" of the rooms
//!   below -- a real, richer claim needing `RoofForm` to carry per-wing
//!   segments keyed to `p127_intimacy_gradient`'s own cell graph. This
//!   operator assigns ONE shed roof per whole building, which is an
//!   honest degenerate case of that (a single-segment cascade), not a
//!   fake stand-in for it -- P116 stays a real, separate, not-yet-built
//!   gap, not silently claimed here.
//! - **No flat-roof/roof-garden option.** `RoofShape::Flat` exists in the
//!   schema for P118 Roof Garden's own future use, but P118's own opinion
//!   doc is explicit that it needs P116/117 to land first -- this
//!   operator never assigns `Flat`.
//! - **No canopy/arcade/porch geometry.** P119 Arcades, P166 Gallery
//!   Surround, and a real "low eave at the entrance" reading of P117
//!   itself are all a different real primitive (a projecting canopy at a
//!   building's edge, not a whole-roof slope) -- out of scope here, see
//!   the correction above.
//!
//! `ridge_height_m`/`eave_height_m` are never independent parameters:
//! `eave_height_m` is always the building's own real `height_m` (P96/
//! P107's own real assignment), and `ridge_height_m` is derived from it
//! plus `roof_rise_m` -- so the roof's low edge always matches whatever
//! height this pipeline already gave the building, not a second,
//! disconnected number.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::{Building, Neighborhood, RoofForm, RoofShape};
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P117Params {
    /// Real additional height the roof's own ridge adds above the
    /// building's own real `height_m` -- a plausible real roof-pitch
    /// figure, not Alexander's own literal entrance-eave number (see this
    /// module's own doc for why that figure doesn't apply at whole-building
    /// scale).
    pub roof_rise_m: f64,
}

impl Parameters for P117Params {
    fn schema() -> Vec<ParamSpec> {
        vec![ParamSpec::float(
            "roof_rise_m",
            "Real additional height the roof's own ridge adds above the building's own real height_m.",
            1.0,
            4.0,
            2.0,
        )
        .with_unit("m")]
    }
    fn defaults() -> Self {
        Self { roof_rise_m: 2.0 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.roof_rise_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.roof_rise_m = s.clamp(*x);
        }
        p
    }
}

pub struct P117ShelteringRoof;

impl PatternOperator for P117ShelteringRoof {
    type Params = P117Params;

    fn name(&self) -> &'static str {
        "p117_sheltering_roof"
    }
    fn description(&self) -> &'static str {
        "Assigns every real building a real shed roof, ridge above its own height_m, sloped down to true north, per P117 Sheltering Roof and P162 North Face."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p117".into(),
            display: "Alexander et al., A Pattern Language, Pattern 117 (Sheltering Roof)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Sheltering-Roof-(117)".into()),
        }
    }

    /// `parcel_id` must be `"*"` -- runs on every real building in one pass.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p117_sheltering_roof only supports parcel_id \"*\" -- it roofs every real building in one pass.".into());
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut n_skipped_no_height = 0usize;

        for b in &nbhd.buildings {
            let Some(height_m) = b.height_m else {
                n_skipped_no_height += 1;
                continue;
            };
            let mut nb = b.clone();
            nb.roof = Some(RoofForm {
                shape: RoofShape::Shed,
                ridge_height_m: height_m + params.roof_rise_m,
                eave_height_m: height_m,
                slope_azimuth_deg: 0.0,
            occupiable: false, });
            new_buildings.push(nb);
            replaced.push(b.id.clone());
        }

        if new_buildings.is_empty() {
            return Err("p117_sheltering_roof: no real building has a real height_m yet -- run P96/P107 first.".into());
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!("Roofed {} real building(s) with a real shed roof sloping to true north.", new_buildings.len()),
            steps: vec![format!(
                "{} real building(s) roofed (eave = each building's own real height_m, ridge = \
                 that plus {:.2}m of real roof rise); {} skipped (no real height_m yet).",
                new_buildings.len(), params.roof_rise_m, n_skipped_no_height
            )],
            caveats: vec![
                "One shed roof per whole building -- P116 Cascade of Roofs' own real 'steps down \
                 following the social hierarchy below' claim needs per-wing roof segments this \
                 operator doesn't build yet; see this operator's own module doc.".into(),
                "roof_rise_m is a plausible real roof-pitch figure, not Alexander's own literal \
                 P117 entrance-eave number -- that figure describes a local, porch-like condition \
                 this whole-building single-slope form doesn't reach. See this operator's own \
                 module doc for the real correction history.".into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings,
            new_streets: vec![],
            new_activity_nodes: vec![],
            new_boundaries: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids: replaced,
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::NeighborhoodMeta;

    fn m() -> f64 { 111_320.0 }

    fn square_ring(cx: f64, cy: f64, half_side: f64) -> Vec<LngLat> {
        let s = half_side / m();
        let x = cx / m();
        let y = cy / m();
        vec![
            LngLat::new(x - s, y - s), LngLat::new(x + s, y - s),
            LngLat::new(x + s, y + s), LngLat::new(x - s, y + s), LngLat::new(x - s, y - s),
        ]
    }

    fn building(id: &str, height_m: Option<f64>) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(square_ring(0.0, 0.0, 10.0)),
            height_m, typology: None, year_built: None, parcel_id: None, floors: None,
            openings: vec![], interior_cells: vec![], wall_thickness_m: None, roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P117 unit fixture".into(),
            },
        }
    }

    #[test]
    fn no_buildings_is_a_real_error_not_a_silent_no_op() {
        let n = nbhd(vec![]);
        assert!(P117ShelteringRoof.apply(&n, "*", &P117Params::defaults(), 0).is_err());
    }

    #[test]
    fn a_real_building_gets_a_real_shed_roof_sloping_true_north() {
        let n = nbhd(vec![building("B1", Some(9.0))]);
        let sub = P117ShelteringRoof.apply(&n, "*", &P117Params::defaults(), 0).unwrap();
        assert_eq!(sub.new_buildings.len(), 1);
        assert_eq!(sub.replaced_building_ids, vec!["B1".to_string()]);
        let roof = sub.new_buildings[0].roof.as_ref().expect("real roof assigned");
        assert_eq!(roof.shape, RoofShape::Shed);
        // Eave sits at the building's own real height -- NOT Alexander's
        // literal ~1.9m entrance figure (see this module's own doc for why).
        assert!((roof.eave_height_m - 9.0).abs() < 1e-9);
        assert!((roof.ridge_height_m - 11.0).abs() < 1e-9, "ridge should be height_m + default roof_rise_m (2.0)");
        assert!((roof.slope_azimuth_deg - 0.0).abs() < 1e-9);
        assert!(roof.ridge_height_m > roof.eave_height_m, "ridge must sit above eave for a real slope");
    }

    #[test]
    fn a_building_with_no_real_height_is_skipped_not_faked() {
        let n = nbhd(vec![building("NOHEIGHT", None)]);
        assert!(P117ShelteringRoof.apply(&n, "*", &P117Params::defaults(), 0).is_err());
    }

    #[test]
    fn params_roundtrip() {
        let p = P117Params { roof_rise_m: 3.0 };
        let v = p.as_vector();
        let back = P117Params::from_vector(&v);
        assert_eq!(back.roof_rise_m, 3.0);
    }
}
