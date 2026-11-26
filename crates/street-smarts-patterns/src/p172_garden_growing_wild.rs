//! P172 Garden Growing Wild -- the site's unbuilt ground let grow as meadow.
//!
//! From Alexander, *A Pattern Language*, Pattern 172 (p. 801), via
//! patternlanguage.cc/Patterns/Garden-Growing-Wild-(172):
//! > **Problem:** A garden which grows true to its own laws is not a
//! > wilderness, yet not entirely artificial either.
//! > **Solution:** Grow grasses, mosses, bushes, flowers, and trees in a way
//! > which comes close to the way that they occur in nature: intermingled,
//! > without barriers between them, without bare earth, without formal
//! > flower beds...
//!
//! # What this generator does
//!
//! Finds the site's own perimeter (the closed `Jurisdictional` boundary the
//! assembled-site fixtures carry as `site_perimeter`) and adds ONE
//! `GroundCover` of kind "meadow" with exactly that ring as its polygon. "Without bare earth" is the operative phrase: every square
//! metre of the site that no building, street, square or common stands on
//! is meadow, not a gap in the model.
//!
//! The meadow underlies everything rather than being cut around it: this
//! schema has no general polygon difference (see `planar.rs`), and a
//! ground layer that the rest of the plan stands on is also what a meadow
//! is. Renderers draw it just below grade, so it shows only where nothing
//! else does.
//!
//! # Why it's ground cover, not open space
//!
//! Several opinions and operators sum open-space area. A site-sized meadow
//! stored as an `OpenSpace` swamps the real squares and commons: on the
//! real fixture it took P114 Hierarchy of Open Space from 0.485 to 0.148
//! (caught by `tests/pattern_cascade.rs`). A meadow is what the ground is,
//! not a place on it, so it lives in `Neighborhood.ground_cover`, where no
//! open-space reader looks. It runs after every other stage anyway (only
//! P171 Tree Places follows, reading the meadow's ring as the site's
//! extent).
//!
//! # Precondition errors
//!
//! `Err` if the neighborhood has no closed jurisdictional boundary ring to
//! treat as the site, or if a meadow already exists (idempotent: run twice,
//! get one meadow).
//!
//! # Caveats
//!
//! - One meadow for the whole site, not one per block: the pipeline's P37
//!   blocks don't survive to the end of the run (P95/P108 replace them).
//! - "Bushes, flowers, mosses" are not modeled. The meadow is one kind of
//!   ground; P171 Tree Places plants the trees.
//! - The meadow's real edge treatment ("rough stone, brick or wood," in the
//!   pattern's own solution) isn't modeled either.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Boundary, BoundaryKind, GroundCover, Neighborhood};

/// `GroundCover.kind` for P172's meadow.
pub const MEADOW: &str = "meadow";
use street_smarts_core::opinion::SourceCitation;

/// The site's own extent, as a closed WGS84 ring: the meadow's ring if P172
/// already ran, else the first closed `Jurisdictional` boundary (preferring
/// one with id `site_perimeter`). `None` if neither exists. P171 Tree Places
/// uses this too, so both operators agree on where the site is.
pub fn site_ring(nbhd: &Neighborhood) -> Option<Vec<LngLat>> {
    if let Some(m) = nbhd.ground_cover.iter().find(|g| g.kind == MEADOW) {
        return Some(m.polygon.outer.clone());
    }
    site_boundary(nbhd).map(|b| b.centerline.clone())
}

/// The closed `Jurisdictional` boundary that marks the site: `site_perimeter`
/// if there is one, else the first closed one.
fn site_boundary(nbhd: &Neighborhood) -> Option<&Boundary> {
    let closed = |r: &[LngLat]| r.len() >= 4 && r.first() == r.last();
    let mut candidates: Vec<&Boundary> = nbhd
        .boundaries
        .iter()
        .filter(|b| b.kind == BoundaryKind::Jurisdictional && closed(&b.centerline))
        .collect();
    candidates.sort_by_key(|b| b.id != "site_perimeter");
    candidates.into_iter().next()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P172Params {}

impl Parameters for P172Params {
    fn schema() -> Vec<ParamSpec> {
        vec![]
    }
    fn defaults() -> Self {
        Self {}
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![]
    }
    fn from_vector(_v: &[f64]) -> Self {
        Self {}
    }
}

pub struct P172GardenGrowingWild;

impl PatternOperator for P172GardenGrowingWild {
    type Params = P172Params;

    fn name(&self) -> &'static str {
        "p172_garden_growing_wild"
    }

    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p172".into(),
            display: "Alexander et al., A Pattern Language, Pattern 172 (Garden Growing Wild)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Garden-Growing-Wild-(172)".into()),
        }
    }

    fn description(&self) -> &'static str {
        "Let the site's unbuilt ground grow as one meadow under everything else: no bare earth."
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        _parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if nbhd.ground_cover.iter().any(|g| g.kind == MEADOW) {
            return Err("p172_garden_growing_wild: this site already has a meadow.".into());
        }
        let boundary = site_boundary(nbhd).ok_or_else(|| {
            "p172_garden_growing_wild: no closed jurisdictional boundary to treat as the site's extent.".to_string()
        })?;
        let source_id = boundary.id.clone();
        let meadow = GroundCover {
            id: format!("{source_id}_p172_meadow"),
            polygon: Polygon::from_ring(boundary.centerline.clone()),
            kind: MEADOW.into(),
        };
        let area = meadow.polygon.area_m2();
        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!("The site's {:.1} ha of ground grows as meadow wherever nothing else stands.", area / 10_000.0),
            steps: vec![format!(
                "{}: one meadow on the ring of boundary {source_id} ({:.0} m2), drawn below everything else.",
                meadow.id, area
            )],
            caveats: vec![
                "One meadow for the whole site: the P37 blocks don't survive to the end of the pipeline.".into(),
                "Bushes, flowers, mosses and the meadow's edges (rough stone, brick or wood) aren't modeled.".into(),
                "Runs last so no other pattern counts it as open space.".into(),
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
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
            new_fields: vec![],
            new_trees: vec![],
            new_ground_cover: vec![meadow],
            new_transit: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subdivision::apply_subdivision;
    use street_smarts_core::nir::NeighborhoodMeta;

    pub(crate) fn square_site(side_m: f64) -> Neighborhood {
        let d = side_m / 111_320.0;
        let ring = vec![
            LngLat::new(0.0, 0.0),
            LngLat::new(d, 0.0),
            LngLat::new(d, d),
            LngLat::new(0.0, d),
            LngLat::new(0.0, 0.0),
        ];
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, d, d],
            parcels: vec![],
            buildings: vec![],
            streets: vec![],
            open_space: vec![],
            boundaries: vec![Boundary { id: "site_perimeter".into(), centerline: ring, kind: BoundaryKind::Jurisdictional }],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P172 unit fixture".into(),
            },
            pattern_fields: vec![],
            trees: vec![],
            ground_cover: vec![],
            transit: vec![],
        }
    }

    #[test]
    fn grows_one_meadow_on_the_site_ring() {
        let n = square_site(100.0);
        let sub = P172GardenGrowingWild.apply(&n, "*", &P172Params::defaults(), 1).unwrap();
        assert!(sub.new_open_space.is_empty(), "a meadow is ground cover, not open space");
        assert_eq!(sub.new_ground_cover.len(), 1);
        let m = &sub.new_ground_cover[0];
        assert_eq!(m.kind, MEADOW);
        assert_eq!(m.id, "site_perimeter_p172_meadow");
        assert!((m.polygon.area_m2() - 10_000.0).abs() < 200.0, "area {}", m.polygon.area_m2());
    }

    #[test]
    fn is_idempotent() {
        let n = square_site(100.0);
        let sub = P172GardenGrowingWild.apply(&n, "*", &P172Params::defaults(), 1).unwrap();
        let once = apply_subdivision(&n, &sub);
        assert!(P172GardenGrowingWild.apply(&once, "*", &P172Params::defaults(), 1).is_err());
        assert_eq!(site_ring(&once), site_ring(&n));
    }

    #[test]
    fn no_site_boundary_is_an_error() {
        let mut n = square_site(100.0);
        n.boundaries.clear();
        assert!(P172GardenGrowingWild.apply(&n, "*", &P172Params::defaults(), 1).is_err());
    }
}
