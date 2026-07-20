//! P197 Thick Walls — assigns every real building a real, nonzero exterior
//! wall depth, closing the schema's own zero-depth-membrane gap.
//!
//! From Alexander, *A Pattern Language*, Pattern 197 (p. 908), via
//! patternlanguage.cc/Patterns/Thick-Walls-(197):
//! > **Problem:** Houses with smooth hard walls made of prefabricated
//! > panels, concrete, gypsum, steel, aluminum, or glass always stay
//! > impersonal and dead.
//! > **Solution:** Open your mind to the possibility that the walls of
//! > your building can be thick, can occupy a substantial volume -- even
//! > actual usable space -- and need not be merely thin membranes which
//! > have no depth.
//!
//! # A real gap, a real (scalar-only) fix
//!
//! `p197_thick_walls`'s own opinion module doc: no wall-thickness field
//! ever existed anywhere in this pipeline's schema -- `Building` modeled a
//! zero-depth membrane, matching `render.py`'s own documented caveat that a
//! punch just pierces solid mass. This operator closes that by assigning
//! every real building a real `wall_thickness_m` value.
//!
//! `wall_thickness_m`'s default (0.3m / ~1ft) is a plausible real
//! exterior-wall-assembly figure (structural block or timber frame plus
//! insulation/furring) -- Alexander's own cited problem/solution text for
//! this pattern doesn't give a precise dimension, so this is NOT presented
//! as his literal number, unlike `p133_staircase_as_a_stage`'s
//! `stair_width_m` (which does cite a literal figure from P195's own
//! text). Capped at `max_fraction_of_min_dimension` (default 0.15) of the
//! building's own real bounding-box minimum dimension, so a small pad
//! never gets an absurdly thick wall relative to its own footprint -- a
//! real geometric constraint, not an arbitrary one.
//!
//! # Known, honestly-stated limitation
//!
//! This is a SCALAR-only fix: `wall_thickness_m` is a uniform real number,
//! not real carved geometry. Alexander's own richer claim -- "even actual
//! usable space" (alcoves, window seats, niches carved INTO the wall
//! depth) -- is NOT modeled here; that would need a genuinely new interior
//! geometry mechanism (carving a real usable volume out of the wall band),
//! which is a separate, larger, not-yet-built lift. `p197_thick_walls`'s
//! own opinion is explicit that it only checks the scalar thickness claim.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{bbox, ring_to_local};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::{Building, Neighborhood};
use street_smarts_core::opinion::SourceCitation;

pub struct P197ThickWalls;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P197Params {
    /// Uniform exterior wall depth assigned to every real building, before
    /// the per-building cap. A plausible real construction figure, not
    /// Alexander's own literal number -- see this module's own doc.
    pub wall_thickness_m: f64,
    /// A building's own wall thickness never exceeds this fraction of its
    /// real bounding-box minimum dimension -- a real geometric constraint
    /// (a wall thicker than a big chunk of the building it belongs to
    /// isn't buildable), not an arbitrary cap.
    pub max_fraction_of_min_dimension: f64,
}

impl Parameters for P197Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "wall_thickness_m",
                "Uniform exterior wall depth assigned to every real building, before the per-building cap.",
                0.15, 0.6, 0.3,
            ).with_unit("m"),
            ParamSpec::float(
                "max_fraction_of_min_dimension",
                "A building's wall thickness never exceeds this fraction of its own real bounding-box minimum dimension.",
                0.05, 0.3, 0.15,
            ),
        ]
    }
    fn defaults() -> Self {
        Self { wall_thickness_m: 0.3, max_fraction_of_min_dimension: 0.15 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.wall_thickness_m, self.max_fraction_of_min_dimension]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.wall_thickness_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.max_fraction_of_min_dimension = s.clamp(*x); }
        p
    }
}

/// Real bounding-box minimum dimension in meters, local-projected around
/// the ring's own centroid -- same technique
/// `p60_accessible_green`/`p106_positive_outdoor_space` use for their own
/// real-shape proxies.
fn min_bbox_dimension_m(ring: &street_smarts_core::geometry::Ring) -> f64 {
    if ring.is_empty() {
        return 0.0;
    }
    let lat0 = ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64;
    let lng0 = ring.iter().map(|p| p.lng).sum::<f64>() / ring.len() as f64;
    let origin = street_smarts_core::geometry::LngLat::new(lng0, lat0);
    let local = ring_to_local(ring, &origin);
    let (min, max) = bbox(&local);
    (max.x - min.x).min(max.y - min.y)
}

impl PatternOperator for P197ThickWalls {
    type Params = P197Params;

    fn name(&self) -> &'static str {
        "p197_thick_walls"
    }
    fn description(&self) -> &'static str {
        "Assigns every real building a real, nonzero exterior wall depth, capped relative to its own footprint."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p197".into(),
            display: "Alexander et al., A Pattern Language, Pattern 197 (Thick Walls)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Thick-Walls-(197)".into()),
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
            return Err("p197_thick_walls only supports parcel_id \"*\" -- it assigns wall thickness to every building in one pass.".into());
        }
        if nbhd.buildings.is_empty() {
            return Err("p197_thick_walls: no buildings found -- run p107_wings_of_light (or building_shape) first.".into());
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced_building_ids: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut n_capped = 0usize;

        for b in &nbhd.buildings {
            let min_dim = min_bbox_dimension_m(&b.polygon.outer);
            let cap = min_dim * params.max_fraction_of_min_dimension;
            let thickness = if cap > 0.0 && params.wall_thickness_m > cap {
                n_capped += 1;
                cap
            } else {
                params.wall_thickness_m
            };
            let mut nb = b.clone();
            nb.wall_thickness_m = Some(thickness);
            new_buildings.push(nb);
            replaced_building_ids.push(b.id.clone());
        }

        steps.push(format!(
            "Assigned wall_thickness_m to {} building(s) (default {:.2}m, capped to {:.0}% of each building's own real min bounding-box dimension).",
            new_buildings.len(), params.wall_thickness_m, params.max_fraction_of_min_dimension * 100.0
        ));
        if n_capped > 0 {
            steps.push(format!("{n_capped} building(s) were capped below the default -- their own footprint was too narrow for the full default thickness."));
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!("Assigned real wall thickness to {} building(s).", new_buildings.len()),
            steps,
            caveats: vec![
                "wall_thickness_m is a uniform scalar per building, not carved geometry -- \
                 Alexander's own richer claim ('even actual usable space' -- alcoves, window \
                 seats, niches within the wall depth) is NOT modeled. See this operator's own \
                 module doc.".into(),
                "The default (0.3m) is a plausible real construction figure, not Alexander's own \
                 literal number -- his cited problem/solution text for this pattern doesn't give \
                 a precise dimension.".into(),
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

    fn building(id: &str, side_m: f64) -> Building {
        let m = 1.0 / 111_320.0;
        let s = (side_m / 2.0) * m;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-s, -s), LngLat::new(s, -s), LngLat::new(s, s), LngLat::new(-s, s), LngLat::new(-s, -s),
            ]),
            height_m: Some(7.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
        }
    }

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![] as Vec<Street>, open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P197 unit fixture".into(),
            },
        }
    }

    #[test]
    fn no_buildings_is_an_error_not_a_silent_no_op() {
        let n = nbhd(vec![]);
        assert!(P197ThickWalls.apply(&n, "*", &P197Params::defaults(), 0).is_err());
    }

    #[test]
    fn a_normal_sized_building_gets_the_full_default_thickness() {
        let n = nbhd(vec![building("B1", 20.0)]);
        let sub = P197ThickWalls.apply(&n, "*", &P197Params::defaults(), 0).unwrap();
        assert_eq!(sub.new_buildings.len(), 1);
        assert_eq!(sub.new_buildings[0].wall_thickness_m, Some(0.3));
        assert_eq!(sub.replaced_building_ids, vec!["B1".to_string()]);
    }

    #[test]
    fn a_tiny_building_gets_capped_below_the_default() {
        // 1m-square footprint: default 0.3m thickness would be 30% of the
        // building's own min dimension -- well past the 15% cap.
        let n = nbhd(vec![building("TINY", 1.0)]);
        let sub = P197ThickWalls.apply(&n, "*", &P197Params::defaults(), 0).unwrap();
        let t = sub.new_buildings[0].wall_thickness_m.unwrap();
        assert!(t < 0.3, "expected a capped thickness below the 0.3m default, got {t}");
        assert!((t - 0.15).abs() < 0.01, "expected ~15% of the 1m footprint (0.15m), got {t}");
    }

    #[test]
    fn params_roundtrip() {
        let p = P197Params { wall_thickness_m: 0.45, max_fraction_of_min_dimension: 0.2 };
        let v = p.as_vector();
        let back = P197Params::from_vector(&v);
        assert_eq!(back.wall_thickness_m, 0.45);
        assert_eq!(back.max_fraction_of_min_dimension, 0.2);
    }
}
