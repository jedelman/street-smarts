//! P116 Cascade of Roofs — partition each already-roofed building's roof
//! into real per-wing `RoofSegment`s that step down with `p127_intimacy_
//! gradient`'s own depth-ordered cell graph, closing the real generator gap
//! `p117_sheltering_roof` and `p116_cascade_of_roofs` (the opinion) both
//! document at length: the schema (`Building.roof_segments`) and the check
//! (`street-smarts-opinions/src/pattern/p116_cascade_of_roofs.rs`) existed
//! first; this is the generator that actually populates the field.
//!
//! From Alexander, *A Pattern Language*, Pattern 116 (p. 565), via
//! patternlanguage.cc/Patterns/Cascade-of-Roofs-(116):
//! > **Problem:** Few buildings will be structurally and socially intact,
//! > unless the floors step down toward the ends of wings, and unless the
//! > roof, accordingly, forms a cascade.
//! > **Solution:** Designers should envision the entire building as a roof
//! > system, positioning the largest and highest roofs over the most
//! > significant areas. Lesser roofs should cascade from these primary
//! > structures.
//!
//! # Where the "social hierarchy of the spaces below" comes from
//! `p127_intimacy_gradient` already computes exactly this, for a different
//! pattern: every `InteriorCell.depth` is a real 0.0 (public wall / entrance
//! bay) to 1.0 (deepest point) position in the public-to-private sequence --
//! the "most significant areas" Alexander names here ARE the shallow,
//! public-facing cells (entrance, common areas), not an independent
//! judgment this operator would otherwise have no way to make. Reusing that
//! same cell polygon partition as the roof's own wing partition keeps the
//! two "which parts of this building matter more" answers -- one under the
//! floor, one over it -- from silently disagreeing.
//!
//! # v0.1 approach
//! For every building with a real whole-building `roof` (from P117) AND 2+
//! real `interior_cells` (from P127): emit one `RoofSegment` per interior
//! cell, footprint = that cell's own polygon, `shape`/`slope_azimuth_deg`/
//! `occupiable` copied unchanged from the whole-building roof (still one
//! real building, one real roof material and orientation -- only ridge
//! height cascades), `eave_height_m` unchanged (every wing still meets the
//! same real wall height this pipeline already gave the building), and
//! `ridge_height_m` linearly interpolated by the cell's own `depth`: full
//! `roof.ridge_height_m` at depth 0 (shallowest/most significant), down to
//! `eave_height_m + (roof.ridge_height_m - roof.eave_height_m) *
//! min_ridge_fraction` at depth 1 (deepest). The whole-building `roof`
//! field is left untouched (still a valid degenerate single-segment
//! summary; render.py and other consumers that only read `roof` are
//! unaffected), `roof_segments` is additive.
//!
//! A building with 0 or 1 interior cells has nothing to cascade -- skipped,
//! not faked into a single-segment "cascade" (that's what leaving
//! `roof_segments` empty already honestly represents; see this pattern's
//! own opinion, which excludes single-segment buildings from its
//! denominator for the same reason).
//!
//! # What this deliberately does NOT do
//! - **No new footprint geometry.** Reuses P127's own cell polygons exactly
//!   -- doesn't attempt an independent "which wing is this" partition. A
//!   solid building's cells are parallel depth bands (not lobed wings in
//!   the branching-footprint sense P107's own module doc says it doesn't
//!   build); a courtyard building's cells are ring bays. Both are real
//!   sub-polygons of the roof's own footprint, which is what a "cascade of
//!   roofs" needs -- Alexander's own text doesn't require literal branching
//!   wings, just roofs that step down with the significance of what's
//!   underneath.
//! - **No slope/shape variation per segment.** Every segment keeps the
//!   parent roof's own `shape` (always `Shed` from P117 today) and
//!   `slope_azimuth_deg` -- only `ridge_height_m` cascades. A real cascade
//!   with per-wing roof shapes (a hip over the entrance, a shed over a
//!   service wing) is a further real refinement, not attempted here.
//! - **Upper floors untouched.** Same ground-floor-only scope
//!   `p127_intimacy_gradient` itself documents -- the cell graph this
//!   operator reuses only exists for floor 0.
//!
//! # v0.2: sampling P127's real `IntimacyField` instead of `cell.depth`
//!
//! The v0.1 approach above read each cell's `depth` attribute directly --
//! a real, correct dependency at the time, but `PATTERN_ORDERING_AUDIT.md`
//! §4.5 named it honestly as a self-critique: reusing a smaller, later
//! pattern's own discrete individuation (which specific band index a cell
//! got, out of however many P127 happened to slice) rather than sampling
//! the real underlying gradient directly, the same premature-individuation
//! shape the whole audit was about. `p127_intimacy_gradient` now attaches
//! a real `IntimacyField` per solid building it partitions (its own "v0.3"
//! module doc) -- this operator samples that field at each cell's own
//! centroid instead of reading `cell.depth`, when one is attached. Falls
//! back to `cell.depth` for courtyard buildings (no field: their depth
//! isn't a simple closed form) and any building without one, so this stays
//! fully backward compatible with older serialized fixtures. The
//! resulting ridge heights shift slightly from the old index-based values
//! -- expected, not a bug; see p127's own module doc for the real numbers.

use crate::p127_intimacy_gradient::sample_intimacy_field;
use crate::parameters::{ParamSpec, Parameters};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Building, InteriorCell, Neighborhood, PatternField, RoofSegment};
use street_smarts_core::opinion::SourceCitation;

/// Vertex-averaged centroid of `cell`'s own polygon -- the same convention
/// every other operator in this crate uses for a polygon's own "origin"
/// (see `p107_wings_of_light`/`p96_number_of_stories`), used as the query
/// point when sampling this building's own `IntimacyField`, if it has one.
fn cell_centroid(cell: &InteriorCell) -> LngLat {
    let ring = &cell.polygon.outer;
    LngLat::new(
        ring.iter().map(|p| p.lng).sum::<f64>() / ring.len().max(1) as f64,
        ring.iter().map(|p| p.lat).sum::<f64>() / ring.len().max(1) as f64,
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P116Params {
    /// Fraction of the parent roof's real rise (ridge_height_m -
    /// eave_height_m) the deepest (depth = 1.0) cell's segment keeps. 1.0
    /// would mean no cascade at all (every segment as tall as the whole
    /// roof); this pipeline's default keeps a real, visible step-down
    /// without flattening the deepest wing's roof into the wall plane.
    pub min_ridge_fraction: f64,
}

impl Parameters for P116Params {
    fn schema() -> Vec<ParamSpec> {
        vec![ParamSpec::float(
            "min_ridge_fraction",
            "Fraction of the parent roof's real rise the deepest cell's segment keeps at depth 1.0.",
            0.1,
            0.9,
            0.35,
        )]
    }
    fn defaults() -> Self {
        Self { min_ridge_fraction: 0.35 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.min_ridge_fraction]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.min_ridge_fraction = s.clamp(*x);
        }
        p
    }
}

pub struct P116CascadeOfRoofs;

impl PatternOperator for P116CascadeOfRoofs {
    type Params = P116Params;

    fn name(&self) -> &'static str {
        "p116_cascade_of_roofs"
    }
    fn description(&self) -> &'static str {
        "Partition each roofed building's roof into per-cell RoofSegments whose ridge height cascades with p127_intimacy_gradient's own depth, per P116 Cascade of Roofs."
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p116".into(),
            display: "Alexander et al., A Pattern Language, Pattern 116 (Cascade of Roofs)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Cascade-of-Roofs-(116)".into()),
        }
    }

    /// `parcel_id` must be `"*"` -- runs on every real building in one pass,
    /// same convention as `p127_intimacy_gradient`.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p116_cascade_of_roofs only supports parcel_id \"*\" -- it cascades every real building's roof in one pass.".into());
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut n_cascaded = 0usize;
        let mut n_skipped_no_roof = 0usize;
        let mut n_skipped_too_few_cells = 0usize;
        let mut n_field_sampled = 0usize;

        for b in &nbhd.buildings {
            let Some(roof) = &b.roof else {
                n_skipped_no_roof += 1;
                continue;
            };
            if b.interior_cells.len() < 2 {
                n_skipped_too_few_cells += 1;
                continue;
            }

            // Prefer sampling this building's own real IntimacyField (P127's
            // continuous public-to-private gradient) at each cell's own
            // centroid over reading that cell's discrete `depth` -- see this
            // file's own "v0.2" module doc for why that's a real, honest
            // improvement, not just a refactor. Falls back to `cell.depth`
            // when no field is attached (courtyard buildings, or a solid
            // building too small for P127 to have sliced at all).
            let intimacy_field = nbhd.pattern_fields.iter().find_map(|f| match f {
                PatternField::Intimacy(i) if i.building_id == b.id => Some(i),
                _ => None,
            });
            if intimacy_field.is_some() {
                n_field_sampled += 1;
            }

            let rise = roof.ridge_height_m - roof.eave_height_m;
            let segments: Vec<RoofSegment> = b
                .interior_cells
                .iter()
                .map(|cell| {
                    let depth = match intimacy_field {
                        Some(field) => sample_intimacy_field(field, cell_centroid(cell)),
                        None => cell.depth.clamp(0.0, 1.0),
                    };
                    let fraction = 1.0 - depth * (1.0 - params.min_ridge_fraction);
                    let mut form = roof.clone();
                    form.ridge_height_m = roof.eave_height_m + rise * fraction;
                    RoofSegment { footprint: cell.polygon.clone(), form }
                })
                .collect();

            let mut nb = b.clone();
            nb.roof_segments = segments;
            new_buildings.push(nb);
            replaced.push(b.id.clone());
            n_cascaded += 1;
        }

        if new_buildings.is_empty() {
            return Err(format!(
                "p116_cascade_of_roofs: 0 of {} building(s) could be cascaded ({} have no roof yet -- \
                 run P117 first; {} have fewer than 2 interior_cells -- run P127 first).",
                nbhd.buildings.len(), n_skipped_no_roof, n_skipped_too_few_cells
            ));
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!("Cascaded {} real building roof(s) into per-cell segments.", n_cascaded),
            steps: vec![format!(
                "{} building(s) cascaded, {} real roof segment(s) total (one per interior cell); {} \
                 skipped (no roof yet), {} skipped (fewer than 2 interior_cells). {} building(s) \
                 sampled a real IntimacyField for ridge height (continuous); the rest fell back to \
                 InteriorCell.depth (courtyard buildings, or no field attached).",
                n_cascaded,
                new_buildings.iter().map(|b| b.roof_segments.len()).sum::<usize>(),
                n_skipped_no_roof, n_skipped_too_few_cells, n_field_sampled
            )],
            caveats: vec![
                "Reuses p127_intimacy_gradient's own interior-cell polygons as the roof's wing \
                 partition rather than computing an independent one -- a solid building's cells are \
                 parallel depth bands, a courtyard building's are ring bays, neither a literal \
                 branching L/U/H wing footprint. See this operator's own module doc.".into(),
                "Ridge height still reads footprint geometry from P127's own cells (their polygons \
                 ARE the roof segment footprints) -- only the DEPTH VALUE used to compute ridge \
                 height is now field-sampled where a field is attached, not the partition itself. \
                 See p127_intimacy_gradient's own 'v0.3' module doc for why the partition (unlike \
                 the depth value) is a reasonable thing to still share.".into(),
                "Only ridge_height_m cascades -- every segment keeps the parent roof's own shape and \
                 slope_azimuth_deg. A real cascade with per-wing roof shapes is a further refinement, \
                 not attempted here.".into(),
                "Ground floor only, same scope p127_intimacy_gradient itself documents -- the cell \
                 graph this operator reuses only exists for floor 0.".into(),
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
            new_fields: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{InteriorCell, IntimacyField, NeighborhoodMeta, RoofForm, RoofShape};

    fn m() -> f64 { 111_320.0 }

    fn square_ring(half_side: f64) -> Vec<LngLat> {
        let s = half_side / m();
        vec![
            LngLat::new(-s, -s), LngLat::new(s, -s),
            LngLat::new(s, s), LngLat::new(-s, s), LngLat::new(-s, -s),
        ]
    }

    fn cell(id: &str, depth: f64) -> InteriorCell {
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(square_ring(3.0)),
            depth,
            is_common: false,
            kind: "room".into(),
            connects_to: vec![],
            floor: 0,
        }
    }

    /// Same as `cell`, but its real polygon is shifted `offset_x_m` east
    /// (local metres) of the origin -- used to give a cell a real, distinct
    /// physical position `sample_intimacy_field` can tell apart, independent
    /// of whatever (possibly wrong) `depth` label it also carries.
    fn cell_at(id: &str, depth: f64, offset_x_m: f64) -> InteriorCell {
        let dx = offset_x_m / m();
        let ring: Vec<LngLat> = square_ring(3.0).iter().map(|p| LngLat::new(p.lng + dx, p.lat)).collect();
        InteriorCell {
            id: id.into(),
            polygon: Polygon::from_ring(ring),
            depth,
            is_common: false,
            kind: "room".into(),
            connects_to: vec![],
            floor: 0,
        }
    }

    fn roof(eave: f64, ridge: f64) -> RoofForm {
        RoofForm { shape: RoofShape::Shed, ridge_height_m: ridge, eave_height_m: eave, slope_azimuth_deg: 0.0, occupiable: false }
    }

    fn building(id: &str, roof: Option<RoofForm>, cells: Vec<InteriorCell>) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(square_ring(10.0)),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None, floors: None,
            openings: vec![], interior_cells: cells, wall_thickness_m: None, roof,
            roof_segments: vec![], canopies: vec![], wall_niches: vec![],
        }
    }

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P116 unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    #[test]
    fn no_roof_is_an_error_not_a_silent_no_op() {
        let n = nbhd(vec![building("B1", None, vec![cell("c0", 0.0), cell("c1", 1.0)])]);
        assert!(P116CascadeOfRoofs.apply(&n, "*", &P116Params::defaults(), 0).is_err());
    }

    #[test]
    fn fewer_than_two_cells_is_skipped() {
        let n = nbhd(vec![building("B1", Some(roof(9.0, 11.0)), vec![cell("c0", 0.0)])]);
        assert!(P116CascadeOfRoofs.apply(&n, "*", &P116Params::defaults(), 0).is_err());
    }

    #[test]
    fn a_shallow_and_a_deep_cell_produce_a_real_ridge_step_down() {
        let n = nbhd(vec![building(
            "B1",
            Some(roof(9.0, 11.0)),
            vec![cell("c0", 0.0), cell("c1", 0.5), cell("c2", 1.0)],
        )]);
        let sub = P116CascadeOfRoofs.apply(&n, "*", &P116Params::defaults(), 0).unwrap();
        let b = &sub.new_buildings[0];
        assert_eq!(b.roof_segments.len(), 3);
        // depth 0 keeps the full ridge.
        assert!((b.roof_segments[0].form.ridge_height_m - 11.0).abs() < 1e-9);
        // depth 1 keeps eave + rise * min_ridge_fraction (0.35 default).
        let expected_deep = 9.0 + (11.0 - 9.0) * 0.35;
        assert!((b.roof_segments[2].form.ridge_height_m - expected_deep).abs() < 1e-9);
        // strictly decreasing across the real gradient.
        assert!(b.roof_segments[0].form.ridge_height_m > b.roof_segments[1].form.ridge_height_m);
        assert!(b.roof_segments[1].form.ridge_height_m > b.roof_segments[2].form.ridge_height_m);
        // eave height, shape, and azimuth are unchanged from the parent roof.
        for seg in &b.roof_segments {
            assert!((seg.form.eave_height_m - 9.0).abs() < 1e-9);
            assert_eq!(seg.form.shape, RoofShape::Shed);
            assert!(!seg.form.occupiable);
        }
        // the whole-building roof field is left untouched.
        assert!(b.roof.is_some());
    }

    #[test]
    fn a_field_sampled_building_uses_real_position_not_the_raw_depth_label() {
        // Both cells carry the SAME (deliberately wrong) depth label -- if
        // this operator fell back to reading it, both would get identical
        // ridge heights. Their real positions differ (east vs west of the
        // field's own origin), so a real IntimacyField sample should still
        // tell them apart correctly.
        let field = IntimacyField {
            building_id: "B1".into(),
            origin: LngLat::new(0.0, 0.0),
            axis_x: 1.0,
            axis_y: 0.0,
            s_min: -10.0,
            s_max: 10.0,
        };
        let east = cell_at("c_east", 0.5, 8.0); // near s = +8 -> shallow (close to s_max)
        let west = cell_at("c_west", 0.5, -8.0); // near s = -8 -> deep (close to s_min)
        let mut n = nbhd(vec![building("B1", Some(roof(9.0, 11.0)), vec![east, west])]);
        n.pattern_fields.push(PatternField::Intimacy(field));

        let sub = P116CascadeOfRoofs.apply(&n, "*", &P116Params::defaults(), 0).unwrap();
        let b = &sub.new_buildings[0];
        assert_eq!(b.roof_segments.len(), 2);
        assert!(
            b.roof_segments[0].form.ridge_height_m > b.roof_segments[1].form.ridge_height_m,
            "the east (shallow) cell should keep a taller ridge than the west (deep) cell, even \
             though both carried the same cell.depth label -- got {:?} vs {:?}",
            b.roof_segments[0].form.ridge_height_m, b.roof_segments[1].form.ridge_height_m
        );
    }

    #[test]
    fn no_field_attached_falls_back_to_cell_depth() {
        // Same building/cells as above, but with NO IntimacyField attached
        // -- confirms the fallback path (cell.depth) still works exactly as
        // before this change, for a courtyard building or an older
        // serialized fixture.
        let east = cell_at("c_east", 0.0, 8.0);
        let west = cell_at("c_west", 1.0, -8.0);
        let n = nbhd(vec![building("B1", Some(roof(9.0, 11.0)), vec![east, west])]);
        let sub = P116CascadeOfRoofs.apply(&n, "*", &P116Params::defaults(), 0).unwrap();
        let b = &sub.new_buildings[0];
        assert!((b.roof_segments[0].form.ridge_height_m - 11.0).abs() < 1e-9, "depth 0.0 should keep the full ridge");
        let expected_deep = 9.0 + (11.0 - 9.0) * 0.35;
        assert!((b.roof_segments[1].form.ridge_height_m - expected_deep).abs() < 1e-9, "depth 1.0 should keep the min_ridge_fraction floor");
    }

    #[test]
    fn params_roundtrip() {
        let p = P116Params { min_ridge_fraction: 0.5 };
        let v = p.as_vector();
        let back = P116Params::from_vector(&v);
        assert_eq!(back.min_ridge_fraction, 0.5);
    }
}
