//! P53 Main Gateways — mark every real site boundary with a real Gateway
//! marker where the paths that actually enter it cross.
//!
//! From Alexander, *A Pattern Language*, Pattern 53 (p. 276), via
//! patternlanguage.cc/Patterns/Main-Gateways-(53):
//! > **Problem:** Any part of town -- large or small -- which is to be
//! > identified by its inhabitants as a precinct of some kind, will be
//! > reinforced, helped in its distinctness, marked, and made more vivid,
//! > if the paths which enter it are marked by gateways where they cross
//! > the boundary.
//! > **Solution:** Mark every boundary in the city which has important
//! > human meaning -- the boundary of a building cluster, a neighborhood,
//! > a precinct -- by great gateways where the major entering paths cross
//! > the boundary.
//!
//! # The same real proxy the opinion already checks
//!
//! `crates/street-smarts-opinions/src/pattern/p53_main_gateways.rs` (the
//! detector for this same pattern) has, since it shipped, scored a real
//! site `Boundary` as "gated" when a real street endpoint sits within
//! `GATEWAY_THRESHOLD_M` of the boundary's own centerline -- but nothing
//! ever generated toward that score; it could only ever measure whatever
//! `path_network`'s own site-perimeter `Boundary` happened to already
//! satisfy by chance. This operator closes that gap using the EXACT same
//! definition (same 30m default threshold, same "nearest real street
//! endpoint" measure): for each real `Boundary`, if a street endpoint
//! sits within `gateway_threshold_m`, place a real `ActivityNode`
//! (`ActivityKind::Gateway`) AT that endpoint. The generator and the
//! opinion can't silently disagree about what counts as a marked gateway,
//! because they're reading the same real geometry the same way.
//!
//! # What this deliberately does NOT do
//! - **No real gateway/arch structure.** Same honest limitation the
//!   opinion's own caveat already states: nothing in this schema models a
//!   built gateway/arch/marker structure. This places a real POINT marking
//!   where one belongs, not a real built form.
//! - **Only the single nearest street endpoint per boundary.** A real
//!   boundary with several real streets crossing near it only gets one
//!   marker, at whichever endpoint is closest -- matching the opinion's
//!   own `fold(f64::MAX, f64::min)` reduction exactly, not an attempt to
//!   mark every real crossing.
//! - **Needs `path_network` to have run first.** `Neighborhood.boundaries`
//!   only has real content once `path_network`'s own "v0.5" step has
//!   populated the site's real perimeter boundary -- an empty
//!   `boundaries`/`streets` list is a real, honest error here, not a
//!   silent no-op.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{point_to_polyline_m, LngLat};
use street_smarts_core::nir::{ActivityKind, ActivityNode, Neighborhood};
use street_smarts_core::opinion::SourceCitation;

/// Matches the real opinion's own `GATEWAY_THRESHOLD_M` exactly -- see
/// this module's own doc for why that agreement matters.
const DEFAULT_GATEWAY_THRESHOLD_M: f64 = 30.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P53Params {
    /// How close a real street endpoint must sit to a boundary's own
    /// centerline to count as marking a gateway there.
    pub gateway_threshold_m: f64,
}

impl Parameters for P53Params {
    fn schema() -> Vec<ParamSpec> {
        vec![ParamSpec::float(
            "gateway_threshold_m",
            "How close a real street endpoint must sit to a boundary's own centerline to count as a gateway.",
            5.0,
            100.0,
            DEFAULT_GATEWAY_THRESHOLD_M,
        )]
    }
    fn defaults() -> Self {
        Self { gateway_threshold_m: DEFAULT_GATEWAY_THRESHOLD_M }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.gateway_threshold_m]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.gateway_threshold_m = s.clamp(*x);
        }
        p
    }
}

pub struct P53MainGateways;

impl PatternOperator for P53MainGateways {
    type Params = P53Params;

    fn name(&self) -> &'static str {
        "p53_main_gateways"
    }
    fn description(&self) -> &'static str {
        "Marks each real site boundary with a real Gateway activity node at its nearest crossing street endpoint, per P53 Main Gateways."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p53".into(),
            display: "Alexander et al., A Pattern Language, Pattern 53 (Main Gateways)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Main-Gateways-(53)".into()),
        }
    }

    /// `parcel_id` must be `"*"` -- marks gateways across every real site
    /// boundary in one pass, same convention `p127_intimacy_gradient`/
    /// `p116_cascade_of_roofs` use.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p53_main_gateways only supports parcel_id \"*\" -- it marks gateways across every real site boundary in one pass.".into());
        }
        if nbhd.boundaries.is_empty() {
            return Err("p53_main_gateways needs at least one real Boundary to mark -- run path_network first (it populates the site's own real perimeter boundary).".into());
        }
        if nbhd.streets.is_empty() {
            return Err("p53_main_gateways needs real streets to check for a crossing gateway -- run path_network first.".into());
        }

        let mut new_activity_nodes: Vec<ActivityNode> = Vec::new();
        let mut n_marked = 0usize;
        let mut n_unmarked: Vec<String> = Vec::new();

        for b in &nbhd.boundaries {
            let mut nearest_m = f64::MAX;
            let mut nearest_point: Option<LngLat> = None;
            for s in &nbhd.streets {
                for end in s.centerline.first().into_iter().chain(s.centerline.last()) {
                    let d = point_to_polyline_m(end, &b.centerline);
                    if d < nearest_m {
                        nearest_m = d;
                        nearest_point = Some(*end);
                    }
                }
            }

            match nearest_point {
                Some(location) if nearest_m <= params.gateway_threshold_m => {
                    new_activity_nodes.push(ActivityNode {
                        id: format!("{}_p53_gateway", b.id),
                        location,
                        kind: ActivityKind::Gateway,
                        intensity: None,
                        label: Some(format!("Main gateway for boundary {}", b.id)),
                        activity_fit: Default::default(),
                        publicness: None,
                    });
                    n_marked += 1;
                }
                _ => n_unmarked.push(b.id.clone()),
            }
        }

        if new_activity_nodes.is_empty() {
            return Err(format!(
                "p53_main_gateways: 0 of {} real boundary/boundaries had a street endpoint within {:.0}m -- nothing to mark.",
                nbhd.boundaries.len(),
                params.gateway_threshold_m
            ));
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!(
                "Marked {} of {} real site boundary/boundaries with a Main Gateway.",
                n_marked,
                nbhd.boundaries.len()
            ),
            steps: vec![format!(
                "{} gateway(s) placed at the real nearest street endpoint within {:.0}m of each boundary's own centerline; {} boundary/boundaries had no real street endpoint close enough.",
                n_marked,
                params.gateway_threshold_m,
                n_unmarked.len()
            )],
            caveats: vec![
                "A gateway here is a real street endpoint marker, not a built gateway/arch structure \
                 -- no such structure concept exists anywhere in this schema."
                    .into(),
                "Only the single nearest real street endpoint per boundary is marked, even when \
                 several real streets cross near the same boundary."
                    .into(),
            ],
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings: vec![],
            new_streets: vec![],
            new_activity_nodes,
            new_boundaries: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids: vec![],
            entity_provenance: Default::default(),
            trace,
            new_fields: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::LngLat;
    use street_smarts_core::nir::{Boundary, BoundaryKind, NeighborhoodMeta, Street};

    fn nbhd(boundaries: Vec<Boundary>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![],
            buildings: vec![],
            streets,
            open_space: vec![],
            boundaries,
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P53 generator unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn boundary(id: &str) -> Boundary {
        let m = 1.0 / 111_320.0;
        Boundary {
            id: id.into(),
            centerline: vec![LngLat::new(-100.0 * m, 0.0), LngLat::new(100.0 * m, 0.0)],
            kind: BoundaryKind::Jurisdictional,
        }
    }

    #[test]
    fn no_boundaries_is_a_real_error_not_a_silent_no_op() {
        let n = nbhd(vec![], vec![]);
        assert!(P53MainGateways.apply(&n, "*", &P53Params::defaults(), 0).is_err());
    }

    #[test]
    fn no_streets_is_a_real_error() {
        let n = nbhd(vec![boundary("B1")], vec![]);
        assert!(P53MainGateways.apply(&n, "*", &P53Params::defaults(), 0).is_err());
    }

    #[test]
    fn only_supports_whole_site_target() {
        let n = nbhd(vec![boundary("B1")], vec![]);
        let err = P53MainGateways.apply(&n, "BLOCK_0", &P53Params::defaults(), 0).unwrap_err();
        assert!(err.contains("\"*\""), "expected the real target-scope error, got: {err}");
    }

    #[test]
    fn a_street_ending_at_the_boundary_gets_marked_there() {
        let m = 1.0 / 111_320.0;
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(0.0, -50.0 * m), LngLat::new(0.0, 0.0)],
            classification: Some("local".into()),
            row_width_m: Some(5.5),
            surface: None,
        };
        let n = nbhd(vec![boundary("B1")], vec![street]);
        let sub = P53MainGateways.apply(&n, "*", &P53Params::defaults(), 0).unwrap();
        assert_eq!(sub.new_activity_nodes.len(), 1);
        let node = &sub.new_activity_nodes[0];
        assert_eq!(node.kind, ActivityKind::Gateway);
        assert!((node.location.lng - 0.0).abs() < 1e-9, "gateway should sit at the real street endpoint");
        assert!((node.location.lat - 0.0).abs() < 1e-9, "gateway should sit at the real street endpoint");
    }

    #[test]
    fn a_distant_street_marks_nothing() {
        let m = 1.0 / 111_320.0;
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(500.0 * m, 500.0 * m), LngLat::new(600.0 * m, 500.0 * m)],
            classification: Some("local".into()),
            row_width_m: Some(5.5),
            surface: None,
        };
        let n = nbhd(vec![boundary("B1")], vec![street]);
        let err = P53MainGateways.apply(&n, "*", &P53Params::defaults(), 0).unwrap_err();
        assert!(err.contains("0 of 1"), "expected the real 'nothing to mark' error, got: {err}");
    }

    #[test]
    fn params_roundtrip() {
        let p = P53Params { gateway_threshold_m: 50.0 };
        let v = p.as_vector();
        let back = P53Params::from_vector(&v);
        assert_eq!(back.gateway_threshold_m, 50.0);
    }
}
