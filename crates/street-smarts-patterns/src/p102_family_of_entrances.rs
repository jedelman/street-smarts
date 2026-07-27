//! P102 Family of Entrances — improve the similarity sub-score by narrowing door-width
//! variance across a complex's main entrances.
//!
//! From Alexander, *A Pattern Language*, Pattern 102 (p. 499), via
//! patternlanguage.cc/Patterns/Family-of-Entrances-(102):
//! > **Problem:** When a person arrives in a complex... there is a good chance
//! > they will experience confusion unless the whole collection is laid out
//! > before them...
//! > **Solution:** Lay out the entrances to form a family: (1) They form a
//! > group, are visible together, and each visible from all the others.
//! > (2) They are all broadly similar... marked by a similar kind of doorway.
//!
//! # What this generator does
//!
//! This operator **only addresses the `similarity` sub-score** of the opinion
//! `crates/street-smarts-opinions/src/pattern/p102_family_of_entrances.rs`.
//!
//! For each building, it finds the one qualifying entrance (first ground-floor,
//! outer-wall `Door`, same selection logic the opinion uses). It computes the
//! neighborhood-wide mean door width across all qualifying entrances. For every
//! door whose width deviates from that mean by MORE than `similarity_tolerance`
//! (default 25%), it nudges that door's `width_m` to the NEAREST EDGE of the
//! tolerance band around the mean:
//!
//! - If door width is too **narrow**: set `width_m = mean * (1.0 - tolerance)`
//! - If door width is too **wide**: set `width_m = mean * (1.0 + tolerance)`
//!
//! This is the minimal adjustment needed to bring outliers back into compliance
//! with the similarity threshold, not an attempt to make every door identical.
//! Only buildings whose qualifying door actually needed adjustment are emitted
//! as `replaced_buildings`.
//!
//! # What this deliberately does NOT do
//!
//! - **No clustering improvements.** Improving the `clustering` sub-score would
//!   require physically moving buildings or their entrances into tighter groups—
//!   a large geometry decision that affects collisions with streets and other
//!   buildings, way beyond this narrow operator's scope. No repositioning is
//!   attempted.
//!
//! - **No door material/style adjustment.** This schema has no door material,
//!   color, or style data (same limitation the opinion itself documents), so
//!   "a similar kind of doorway" (Alexander's own phrase) can only be checked
//!   on width, the one dimension that actually varies here.
//!
//! - **Only the first qualifying door per building.** The opinion measures one
//!   entrance per building; this generator respects that same convention and
//!   does not address buildings with multiple real public entrances.
//!
//! # Required ordering: run AFTER Main Entrance & door-adding generators
//!
//! **This operator should run AFTER:**
//! - Any Main Entrance (P110) generator, which may change which door is "the"
//!   qualifying entrance (e.g., by marking a specific door as primary).
//! - Any generator that adds new doors to previously-doorless buildings
//!   (e.g., P100 Pedestrian Street), which changes the entrance pool and
//!   positions.
//!
//! If this operator runs before those generators finalize the entrance set and
//! positions, the mean door width and outlier detection will be stale by the
//! time the final entrance opinion is scored.

use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::{Neighborhood, OpeningKind};
use street_smarts_core::opinion::SourceCitation;

/// Matches the real opinion's own `SIMILARITY_TOLERANCE` exactly.
const DEFAULT_SIMILARITY_TOLERANCE: f64 = 0.25;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P102Params {
    /// Threshold for door-width deviation from the neighborhood mean. A door
    /// whose width is within +/- this fraction of the mean is considered
    /// similar; outliers are nudged to the tolerance band edge.
    /// Default 0.25 (25%) matches the opinion's own constant.
    pub similarity_tolerance: f64,
}

impl Parameters for P102Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "similarity_tolerance",
                "How much a door's width can deviate from the neighborhood mean before being adjusted. \
                 Default 0.25 (25%) matches the opinion's own threshold.",
                0.05,
                0.5,
                DEFAULT_SIMILARITY_TOLERANCE,
            )
        ]
    }
    fn defaults() -> Self {
        Self {
            similarity_tolerance: DEFAULT_SIMILARITY_TOLERANCE,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.similarity_tolerance]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.similarity_tolerance = s.clamp(*x);
        }
        p
    }
}

pub struct P102FamilyOfEntrances;

impl PatternOperator for P102FamilyOfEntrances {
    type Params = P102Params;

    fn name(&self) -> &'static str {
        "p102_family_of_entrances"
    }
    fn description(&self) -> &'static str {
        "Improves door-width similarity across a complex's entrances by adjusting outliers to the tolerance band, per P102 Family of Entrances."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p102".into(),
            display: "Alexander et al., A Pattern Language, Pattern 102 (Family of Entrances)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Family-of-Entrances-(102)".into()),
        }
    }

    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err(
                "p102_family_of_entrances only supports parcel_id \"*\" -- it improves door similarity across all buildings in the neighborhood.".into(),
            );
        }

        // Find the one qualifying entrance per building: first ground-floor,
        // outer-wall Door. Mirror the opinion's logic exactly.
        let entrances: Vec<(String, usize, f64)> = nbhd
            .buildings
            .iter()
            .filter_map(|b| {
                let door_idx = b
                    .openings
                    .iter()
                    .position(|o| o.kind == OpeningKind::Door && !o.on_hole && o.floor == 0)?;
                let door = &b.openings[door_idx];
                Some((b.id.clone(), door_idx, door.width_m))
            })
            .collect();

        if entrances.len() < 2 {
            return Err(format!(
                "{} building(s) with a real ground-floor entrance -- a family needs at least two to compare.",
                entrances.len()
            ));
        }

        // Compute neighborhood-wide mean door width (matching the opinion exactly).
        let mean_width = entrances.iter().map(|(_, _, w)| w).sum::<f64>() / entrances.len() as f64;

        // Find which doors are outliers (deviation from mean exceeds tolerance).
        let mut doors_needing_adjustment: Vec<(String, usize, f64, f64)> = Vec::new();
        for (building_id, door_idx, width) in &entrances {
            let deviation = (width - mean_width).abs() / mean_width;
            if deviation > params.similarity_tolerance {
                // Determine which band edge to nudge it to.
                let new_width = if width < &mean_width {
                    // Too narrow: pull up to lower edge of tolerance band.
                    mean_width * (1.0 - params.similarity_tolerance)
                } else {
                    // Too wide: pull down to upper edge of tolerance band.
                    mean_width * (1.0 + params.similarity_tolerance)
                };
                doors_needing_adjustment.push((building_id.clone(), *door_idx, *width, new_width));
            }
        }

        // If no adjustments needed, return early with explanation.
        if doors_needing_adjustment.is_empty() {
            return Err(format!(
                "p102_family_of_entrances: {} door(s) checked against a {:.2}m mean width; \
                 all fall within {:.0}% tolerance band -- nothing to adjust.",
                entrances.len(),
                mean_width,
                params.similarity_tolerance * 100.0
            ));
        }

        // Apply adjustments using clone-mutate-replace idiom.
        let mut new_buildings: Vec<street_smarts_core::nir::Building> = Vec::new();
        let mut replaced_building_ids: Vec<String> = Vec::new();

        for (building_id, door_idx, _old_width, new_width) in doors_needing_adjustment.iter() {
            if let Some(b) = nbhd.buildings.iter().find(|b| &b.id == building_id) {
                let mut nb = b.clone();
                nb.openings[*door_idx].width_m = *new_width;
                new_buildings.push(nb);
                replaced_building_ids.push(building_id.clone());
            }
        }

        let n_adjusted = doors_needing_adjustment.len();
        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!(
                "Adjusted door widths on {} building(s) to bring outliers within {:.0}% of the {:.2}m mean.",
                n_adjusted,
                params.similarity_tolerance * 100.0,
                mean_width
            ),
            steps: vec![format!(
                "{} door(s) that deviated beyond {:.0}% of the {:.2}m neighborhood mean were pulled to the nearest \
                 tolerance band edge; {} building(s) remain unchanged.",
                n_adjusted,
                params.similarity_tolerance * 100.0,
                mean_width,
                entrances.len() - n_adjusted
            )],
            caveats: vec![
                "This generator addresses only the similarity sub-score, not clustering. Improving clustering would \
                 require repositioning buildings or entrances into tighter groups, way beyond this operator's scope."
                    .into(),
                "Similarity only compares door WIDTH -- this schema has no door material, color, or style data, so \
                 'a similar kind of doorway' (Alexander's own phrase) can only be checked on the one dimension that varies."
                    .into(),
                "One entrance per building (the first ground-floor outer door found) -- doesn't account for buildings \
                 with multiple real public entrances."
                    .into(),
                "Should run AFTER any Main Entrance (P110) generator and AFTER any generator that adds new doors to \
                 previously-doorless buildings (e.g., P100 Pedestrian Street), so the entrance set and positions are final."
                    .into(),
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
            new_fields: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids,
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Building, NeighborhoodMeta, Opening};

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.02, 0.02],
            parcels: vec![],
            buildings,
            streets: vec![],
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P102 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn building_at(id: &str, x_m: f64, y_m: f64, door_width_m: f64) -> Building {
        let m = 1.0 / 111_320.0;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(x_m * m, y_m * m),
                LngLat::new((x_m + 10.0) * m, y_m * m),
                LngLat::new((x_m + 10.0) * m, (y_m + 10.0) * m),
                LngLat::new(x_m * m, (y_m + 10.0) * m),
                LngLat::new(x_m * m, y_m * m),
            ]),
            height_m: Some(6.0),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: None,
            floors: Some(2),
            openings: vec![Opening {
                kind: OpeningKind::Door,
                ring_index: 0,
                on_hole: false,
                t: 0.5,
                width_m: door_width_m,
                sill_height_m: 0.0,
                head_height_m: 2.1,
                floor: 0,
            }],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        }
    }

    #[test]
    fn fewer_than_two_qualifying_entrances_is_err() {
        let n = nbhd(vec![building_at("B1", 0.0, 0.0, 1.0)]);
        let params = P102Params::defaults();
        let result = P102FamilyOfEntrances.apply(&n, "*", &params, 42);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("a family needs at least two"));
    }

    #[test]
    fn all_doors_within_tolerance_returns_err() {
        // All doors 1.0m, mean 1.0m, 0% deviation from mean
        let n = nbhd(vec![
            building_at("B1", 0.0, 0.0, 1.0),
            building_at("B2", 50.0, 0.0, 1.0),
            building_at("B3", 100.0, 0.0, 1.0),
        ]);
        let params = P102Params::defaults();
        let result = P102FamilyOfEntrances.apply(&n, "*", &params, 42);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("all fall within"));
    }

    #[test]
    fn one_door_far_too_narrow_gets_adjusted() {
        // B1: 0.75m (too narrow), B2: 1.0m, B3: 1.0m
        // Mean: (0.75 + 1.0 + 1.0) / 3 = 0.917
        // Deviation of B1: |0.75 - 0.917| / 0.917 = 0.182 / 0.917 = 0.198 < 0.25 (within)
        // So we need a more extreme example...
        // B1: 0.5m, B2: 1.0m, B3: 1.0m
        // Mean: (0.5 + 1.0 + 1.0) / 3 = 0.833
        // Deviation: |0.5 - 0.833| / 0.833 = 0.333 / 0.833 = 0.4 > 0.25 (outlier!)
        let n = nbhd(vec![
            building_at("NARROW", 0.0, 0.0, 0.5),
            building_at("B2", 50.0, 0.0, 1.0),
            building_at("B3", 100.0, 0.0, 1.0),
        ]);
        let params = P102Params::defaults();
        let result = P102FamilyOfEntrances.apply(&n, "*", &params, 42);
        assert!(result.is_ok());
        let sub = result.unwrap();
        assert_eq!(sub.new_buildings.len(), 1);
        assert_eq!(sub.replaced_building_ids.len(), 1);
        assert_eq!(sub.replaced_building_ids[0], "NARROW");

        // Check that the width was adjusted to the lower tolerance band edge.
        let adjusted_building = &sub.new_buildings[0];
        let mean = (0.5 + 1.0 + 1.0) / 3.0;
        let expected_width = mean * (1.0 - params.similarity_tolerance);
        assert!((adjusted_building.openings[0].width_m - expected_width).abs() < 1e-9);
    }

    #[test]
    fn one_door_far_too_wide_gets_adjusted() {
        // B1: 2.0m (too wide), B2: 1.0m, B3: 1.0m
        // Mean: (2.0 + 1.0 + 1.0) / 3 = 1.333
        // Deviation: |2.0 - 1.333| / 1.333 = 0.667 / 1.333 = 0.5 > 0.25 (outlier!)
        let n = nbhd(vec![
            building_at("WIDE", 0.0, 0.0, 2.0),
            building_at("B2", 50.0, 0.0, 1.0),
            building_at("B3", 100.0, 0.0, 1.0),
        ]);
        let params = P102Params::defaults();
        let result = P102FamilyOfEntrances.apply(&n, "*", &params, 42);
        assert!(result.is_ok());
        let sub = result.unwrap();
        assert_eq!(sub.new_buildings.len(), 1);
        assert_eq!(sub.replaced_building_ids.len(), 1);
        assert_eq!(sub.replaced_building_ids[0], "WIDE");

        // Check that the width was adjusted to the upper tolerance band edge.
        let adjusted_building = &sub.new_buildings[0];
        let mean = (2.0 + 1.0 + 1.0) / 3.0;
        let expected_width = mean * (1.0 + params.similarity_tolerance);
        assert!((adjusted_building.openings[0].width_m - expected_width).abs() < 1e-9);
    }

    #[test]
    fn mixed_compliant_and_non_compliant_doors() {
        // B1: 0.5m (too narrow), B2: 1.0m (compliant), B3: 1.0m (compliant), B4: 2.0m (too wide)
        // Mean: (0.5 + 1.0 + 1.0 + 2.0) / 4 = 1.125
        // Deviation of B1: |0.5 - 1.125| / 1.125 = 0.625 / 1.125 = 0.556 > 0.25 (outlier!)
        // Deviation of B2: |1.0 - 1.125| / 1.125 = 0.125 / 1.125 = 0.111 < 0.25 (compliant)
        // Deviation of B3: |1.0 - 1.125| / 1.125 = 0.125 / 1.125 = 0.111 < 0.25 (compliant)
        // Deviation of B4: |2.0 - 1.125| / 1.125 = 0.875 / 1.125 = 0.778 > 0.25 (outlier!)
        let n = nbhd(vec![
            building_at("NARROW", 0.0, 0.0, 0.5),
            building_at("B2", 50.0, 0.0, 1.0),
            building_at("B3", 100.0, 0.0, 1.0),
            building_at("WIDE", 150.0, 0.0, 2.0),
        ]);
        let params = P102Params::defaults();
        let result = P102FamilyOfEntrances.apply(&n, "*", &params, 42);
        assert!(result.is_ok());
        let sub = result.unwrap();
        // Only NARROW and WIDE should be in new_buildings
        assert_eq!(sub.new_buildings.len(), 2);
        assert_eq!(sub.replaced_building_ids.len(), 2);
        assert!(sub.replaced_building_ids.contains(&"NARROW".to_string()));
        assert!(sub.replaced_building_ids.contains(&"WIDE".to_string()));
        assert!(!sub.replaced_building_ids.contains(&"B2".to_string()));
        assert!(!sub.replaced_building_ids.contains(&"B3".to_string()));
    }

    #[test]
    fn custom_similarity_tolerance_changes_outlier_detection() {
        // B1: 0.8m, B2: 1.0m, B3: 1.0m
        // Mean: (0.8 + 1.0 + 1.0) / 3 = 0.933
        // Deviation of B1: |0.8 - 0.933| / 0.933 = 0.143 / 0.933 = 0.153
        // With default 0.25: 0.153 < 0.25 → compliant (no adjustment)
        // With strict 0.1: 0.153 > 0.1 → outlier (adjustment needed)

        let n = nbhd(vec![
            building_at("B1", 0.0, 0.0, 0.8),
            building_at("B2", 50.0, 0.0, 1.0),
            building_at("B3", 100.0, 0.0, 1.0),
        ]);

        // With default tolerance: should be Err (no outliers).
        let params_default = P102Params::defaults();
        let result_default = P102FamilyOfEntrances.apply(&n, "*", &params_default, 42);
        assert!(result_default.is_err());

        // With strict tolerance: should be Ok (B1 is an outlier).
        let params_strict = P102Params {
            similarity_tolerance: 0.1,
        };
        let result_strict = P102FamilyOfEntrances.apply(&n, "*", &params_strict, 42);
        assert!(result_strict.is_ok());
        let sub = result_strict.unwrap();
        assert_eq!(sub.new_buildings.len(), 1);
        assert_eq!(sub.replaced_building_ids[0], "B1");
    }

    #[test]
    fn wrong_parcel_id_returns_err() {
        let n = nbhd(vec![
            building_at("B1", 0.0, 0.0, 0.5),
            building_at("B2", 50.0, 0.0, 2.0),
        ]);
        let params = P102Params::defaults();
        let result = P102FamilyOfEntrances.apply(&n, "some_parcel_id", &params, 42);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("only supports parcel_id"));
    }
}
