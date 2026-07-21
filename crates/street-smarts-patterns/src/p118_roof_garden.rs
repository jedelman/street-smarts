//! P118 Roof Garden — flat, usable sections of roof should be developed
//! as real gardens.
//!
//! From Alexander, *A Pattern Language*, Pattern 118 (p. 575), via
//! patternlanguage.cc/Patterns/Roof-Garden-(118):
//! > **Problem:** A vast part of the earth's surface, in a town,
//! > consists of roofs... it is natural, and indeed essential, to make
//! > roofs which take advantage of the sun and air.
//! > **Solution:** Make parts of almost every roof system usable as roof
//! > gardens... always make it possible to walk directly out onto the
//! > roof garden from some lived-in part of the building.
//!
//! # A real, scoped generator -- not per-wing, not access-modeled
//!
//! `p117_sheltering_roof` assigns every real building ONE roof (always a
//! sloped `Shed`). This operator runs AFTER it and, for a real, small
//! subset of buildings, OVERWRITES that single roof to `RoofShape::Flat`
//! with `occupiable: true` -- Alexander's own literal "usable as roof
//! gardens" claim, now checkable by `p118_roof_garden`'s own opinion
//! (`street-smarts-opinions`).
//!
//! `p116_cascade_of_roofs` (per-wing roof segments) doesn't exist yet --
//! see this crate's own module doc history (`PHASE5_PATTERN_COVERAGE.md`)
//! for why a real wing-detection generator is a separate, larger lift.
//! So this operator can only choose "flat garden roof" or "sloped shed
//! roof" for a building's ENTIRE roof, not a real per-wing mix of both
//! (Alexander's own "almost every roof system" framing already
//! anticipates a building having several roof areas; this pipeline
//! doesn't model that yet). Overwrites the whole roof rather than trying
//! to carve a partial flat section out of a sloped one -- there's no
//! real geometry primitive for a partial roof yet either.
//!
//! **Real criterion, not Alexander's own figure**: the `max_gardens`
//! (default 2) TALLEST real buildings on the site get a flat garden roof;
//! every other building keeps P117's sloped shed. Deliberately a small,
//! fixed COUNT, not a fixed floor-count threshold or a fraction of the
//! site -- a real site typically singles out a small number of standout,
//! prominent buildings for a real amenity roof, not a fixed fraction of
//! every building on it. A floor-count threshold was tried first and
//! rejected: on this pipeline's own real eastside-baseline fixture, most
//! buildings cluster at 2/4 floors with a couple of real outliers at 6/7
//! -- ANY threshold below the outliers converted a real MAJORITY of
//! buildings from sloped to flat, which correctly (not a bug) tanked
//! `p117_sheltering_roof`/`p162_north_face`'s own real "is it sloped"
//! score across the whole site (caught by `tests/pattern_cascade.rs`'s
//! own real regression contract, not assumed safe). A small fixed count
//! keeps P118's own real, honest coverage without gutting P117/P162's.
//! Ties at the count boundary are broken by building id, ascending, for
//! determinism.
//!
//! **Still can't check** "always make it possible to walk directly out
//! onto the roof garden from some lived-in part of the building" -- no
//! real access point (stair/hatch from an `InteriorCell` up to the roof)
//! exists in this schema. `occupiable: true` here is a real, honest claim
//! about the roof PLANE, not a claim that it's reachable.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::{Building, Neighborhood, RoofForm, RoofShape};
use street_smarts_core::opinion::SourceCitation;

pub struct P118RoofGarden;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P118Params {
    /// How many of the site's own tallest real buildings get a flat,
    /// occupiable garden roof instead of P117's sloped shed roof. See
    /// this module's own doc for why a small fixed count, not a floor
    /// threshold or a fraction of the site.
    pub max_gardens: u32,
}

impl Parameters for P118Params {
    fn schema() -> Vec<ParamSpec> {
        vec![ParamSpec::float(
            "max_gardens",
            "How many of the site's own tallest real buildings get a flat, occupiable garden roof.",
            1.0, 6.0, 2.0,
        )]
    }
    fn defaults() -> Self {
        Self { max_gardens: 2 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.max_gardens as f64]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.max_gardens = s.clamp(*x).round() as u32;
        }
        p
    }
}

/// The ids of the `max_gardens` real buildings with the highest real
/// floor count, ties broken by id ascending for determinism.
fn tallest_building_ids(nbhd: &Neighborhood, max_gardens: u32) -> Vec<String> {
    let mut ranked: Vec<&Building> = nbhd.buildings.iter().collect();
    ranked.sort_by(|a, b| {
        b.floors.unwrap_or(1).cmp(&a.floors.unwrap_or(1)).then_with(|| a.id.cmp(&b.id))
    });
    ranked.into_iter().take(max_gardens as usize).map(|b| b.id.clone()).collect()
}

impl PatternOperator for P118RoofGarden {
    type Params = P118Params;

    fn name(&self) -> &'static str {
        "p118_roof_garden"
    }
    fn description(&self) -> &'static str {
        "Overwrites the site's own tallest real buildings' sloped P117 roof with a flat, occupiable garden roof."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p118".into(),
            display: "Alexander et al., A Pattern Language, Pattern 118 (Roof Garden)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Roof-Garden-(118)".into()),
        }
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        _seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p118_roof_garden only supports parcel_id \"*\" -- it re-scores every building in one pass.".into());
        }
        if nbhd.buildings.is_empty() {
            return Err("p118_roof_garden: no buildings found.".into());
        }

        let gardened_ids = tallest_building_ids(nbhd, params.max_gardens);
        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced_building_ids: Vec<String> = Vec::new();

        for b in &nbhd.buildings {
            if !gardened_ids.contains(&b.id) {
                continue;
            }
            let mut nb = b.clone();
            let (ridge, eave) = b.roof.as_ref().map(|r| (r.ridge_height_m, r.eave_height_m)).unwrap_or_else(|| {
                let h = b.height_m.unwrap_or(0.0);
                (h, h)
            });
            nb.roof = Some(RoofForm {
                shape: RoofShape::Flat,
                ridge_height_m: ridge,
                eave_height_m: eave,
                slope_azimuth_deg: 0.0,
                occupiable: true,
            });
            new_buildings.push(nb);
            replaced_building_ids.push(b.id.clone());
        }

        if new_buildings.is_empty() {
            return Err("p118_roof_garden: no building to garden (empty site).".into());
        }

        let n_gardened = new_buildings.len();
        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!("Gave the site's {n_gardened} tallest real building(s) a real flat, occupiable roof garden."),
            steps: vec![format!(
                "{n_gardened} of {} real building(s) (the site's own tallest, up to max_gardens={}) had their roof overwritten from a sloped shed to a flat, occupiable plane.",
                nbhd.buildings.len(), params.max_gardens
            )],
            caveats: vec![
                "Overwrites the ENTIRE roof flat -- no per-wing mix of sloped and flat sections \
                 (p116_cascade_of_roofs' own per-wing roof_segments generator doesn't exist yet).".into(),
                "occupiable: true is a claim about the roof plane, not about real reachability -- \
                 no stair/hatch access-point geometry exists in this schema yet.".into(),
                "max_gardens is a real, documented design decision (a small fixed count, not a floor \
                 threshold -- an earlier version tried that and tanked P117/P162's own real \
                 sloped-roof score site-wide, see this module's own doc), not Alexander's own literal \
                 figure -- his text gives none.".into(),
            ],
            seed: 0,
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
            replaced_building_ids,
            entity_provenance: Default::default(),
            trace,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn building(id: &str, floors: u32, roof: Option<RoofForm>) -> Building {
        let m = 1.0 / 111_320.0;
        let s = 10.0 * m;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-s, -s), LngLat::new(s, -s), LngLat::new(s, s), LngLat::new(-s, s), LngLat::new(-s, -s),
            ]),
            height_m: Some(floors as f64 * 3.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None, parcel_id: None, floors: Some(floors),
            openings: vec![], interior_cells: vec![], wall_thickness_m: None,
            roof, roof_segments: vec![], canopies: vec![], wall_niches: vec![],
        }
    }

    fn shed_roof(ridge: f64, eave: f64) -> RoofForm {
        RoofForm { shape: RoofShape::Shed, ridge_height_m: ridge, eave_height_m: eave, slope_azimuth_deg: 0.0, occupiable: false }
    }

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![] as Vec<Street>, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P118 unit fixture".into(),
            },
        }
    }

    #[test]
    fn no_buildings_is_an_error() {
        let n = nbhd(vec![]);
        assert!(P118RoofGarden.apply(&n, "*", &P118Params::defaults(), 0).is_err());
    }

    #[test]
    fn only_the_tallest_buildings_get_gardened() {
        let n = nbhd(vec![
            building("SHORT", 2, Some(shed_roof(8.0, 6.0))),
            building("MED", 4, Some(shed_roof(14.0, 9.0))),
            building("TALL", 7, Some(shed_roof(23.0, 21.0))),
        ]);
        let params = P118Params { max_gardens: 1 };
        let sub = P118RoofGarden.apply(&n, "*", &params, 0).unwrap();
        assert_eq!(sub.new_buildings.len(), 1);
        assert_eq!(sub.replaced_building_ids, vec!["TALL".to_string()]);
        let roof = sub.new_buildings[0].roof.as_ref().unwrap();
        assert_eq!(roof.shape, RoofShape::Flat);
        assert!(roof.occupiable);
    }

    #[test]
    fn a_majority_floor_count_cluster_does_not_all_get_gardened() {
        // Mirrors the real eastside-baseline shape (mostly 2/4 floors, a
        // couple of 6/7-floor outliers) -- max_gardens should stay small
        // and pick the real outliers, not a majority of the cluster.
        let n = nbhd(vec![
            building("A", 4, Some(shed_roof(14.0, 9.0))),
            building("B", 4, Some(shed_roof(14.0, 9.0))),
            building("C", 2, Some(shed_roof(8.0, 6.0))),
            building("D", 6, Some(shed_roof(20.0, 18.0))),
            building("E", 7, Some(shed_roof(23.0, 21.0))),
        ]);
        let sub = P118RoofGarden.apply(&n, "*", &P118Params::defaults(), 0).unwrap();
        assert_eq!(sub.new_buildings.len(), 2, "default max_gardens=2 should garden exactly 2 of 5 buildings");
        let mut ids = sub.replaced_building_ids.clone();
        ids.sort();
        assert_eq!(ids, vec!["D".to_string(), "E".to_string()], "the two real tallest buildings should be gardened");
    }

    #[test]
    fn ties_are_broken_deterministically_by_id() {
        let n = nbhd(vec![
            building("Z", 5, Some(shed_roof(17.0, 15.0))),
            building("A", 5, Some(shed_roof(17.0, 15.0))),
            building("M", 5, Some(shed_roof(17.0, 15.0))),
        ]);
        let params = P118Params { max_gardens: 2 };
        let sub = P118RoofGarden.apply(&n, "*", &params, 0).unwrap();
        let mut ids = sub.replaced_building_ids.clone();
        ids.sort();
        assert_eq!(ids, vec!["A".to_string(), "M".to_string()], "ties should break by id ascending");
    }

    #[test]
    fn params_roundtrip() {
        let p = P118Params { max_gardens: 4 };
        let v = p.as_vector();
        let back = P118Params::from_vector(&v);
        assert_eq!(back.max_gardens, 4);
    }
}
