//! P164 Street Windows — buildings alongside real streets need real windows
//! looking out onto them, not blind walls.
//!
//! From Alexander, *A Pattern Language*, Pattern 164 (p. 769), via
//! patternlanguage.cc/Patterns/Street-Windows-(164):
//! > **Problem:** A street without windows is blind and frightening. And
//! > it is equally uncomfortable to be in a house which bounds a public
//! > street with no window at all on the street.
//! > **Solution:** Where buildings run alongside busy streets, build
//! > windows with window seats, looking out onto the street.
//!
//! # The same real proxy the opinion already checks
//!
//! `crates/street-smarts-opinions/src/pattern/p164_street_windows.rs` (the
//! detector for this same pattern) has, since it shipped, scored a building
//! as "street-windowed" when:
//!   1. Its polygon centroid sits within `STREET_ADJACENCY_M` (default 20m)
//!      of a Local or Pedestrian street's centerline
//!   2. It has at least one opening with `kind == Window && !on_hole &&
//!      floor == 0` (ground-floor, outer-ring window)
//!   3. That window's exterior point (ring-interpolated at its own `t`
//!      position) sits within `WINDOW_THRESHOLD_M` (default 15m) of the
//!      street's own centerline
//!
//! This operator closes the gap: for each real street-adjacent building that
//! is currently BLIND, pick the outer-ring edge whose MIDPOINT is nearest a
//! walkable street's centerline, and place one real ground-floor Window
//! Opening at `t: 0.5` on that edge. Before committing it, verify the new
//! opening's own exterior point actually clears `window_threshold_m` -- if
//! the nearest edge's midpoint still isn't within threshold, skip that
//! building as unfixable rather than placing a window that doesn't satisfy
//! the opinion's own check.
//!
//! # What this deliberately does NOT do
//! - **No real window seat.** Same honest limitation the opinion already
//!   states: nothing in this schema models a built window seat, alcove,
//!   or window-room. This places a real OPENING, not the full feature
//!   Alexander describes.
//! - **"Busy" collapses to Local/Pedestrian streets.** No traffic volume
//!   or pedestrian-count concept exists to distinguish a genuinely busy
//!   street from a quiet one. This treats all Local and Pedestrian streets
//!   the same.
//! - **One window per blind building, on a single edge.** This is NOT a real
//!   fenestration design. A real street-facing wall might have many windows;
//!   this places at most one on the single edge nearest the street.
//! - **Doesn't verify facing direction.** Same caveat the opinion itself
//!   declares: only checks distance, not whether the window's actual
//!   surface normal points toward the street.

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::ring_to_local;
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::{point_to_polyline_m, LngLat};
use street_smarts_core::nir::{Building, Neighborhood, Opening, OpeningKind};
use street_smarts_core::opinion::SourceCitation;

/// Matches the real opinion's own `STREET_ADJACENCY_M` exactly -- see
/// this module's own doc for why that agreement matters.
const DEFAULT_STREET_ADJACENCY_M: f64 = 20.0;

/// Matches the real opinion's own `WINDOW_THRESHOLD_M` exactly -- same reason.
const DEFAULT_WINDOW_THRESHOLD_M: f64 = 15.0;

/// Reused from the opinion module. Returns the exterior point of an opening
/// ring-interpolated at position `t` along edge `ring_index`.
fn opening_point(ring: &[LngLat], ring_index: usize, t: f64) -> Option<LngLat> {
    if ring.len() < 2 {
        return None;
    }
    let idx = ring_index.min(ring.len().saturating_sub(2));
    let a = ring[idx];
    let c = ring[(idx + 1) % ring.len()];
    Some(LngLat::new(a.lng + (c.lng - a.lng) * t, a.lat + (c.lat - a.lat) * t))
}

/// Reused from the opinion module. Check if a street classification counts
/// as "walkable" (Local or Pedestrian).
fn is_walkable(classification: &Option<String>) -> bool {
    matches!(classification.as_deref(), Some("local") | Some("pedestrian"))
}

/// Real length in meters of outer-ring edge `ring_index`, local-projected
/// around the building's own centroid -- needed to calculate edge midpoint
/// position and clamp window width.
fn edge_len_m(b: &Building, ring_index: usize) -> f64 {
    let bc = b.polygon.centroid();
    let origin = LngLat::new(bc.lng, bc.lat);
    let local = ring_to_local(&b.polygon.outer, &origin);
    let n = local.len();
    if n < 2 || ring_index >= n {
        return 0.0;
    }
    local[ring_index].dist(local[(ring_index + 1) % n])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P164Params {
    /// How close a building's own centroid must sit to a walkable street's
    /// centerline to count as street-adjacent. Matches the opinion's own
    /// STREET_ADJACENCY_M default.
    pub street_adjacency_m: f64,
    /// How close an opening's own exterior point must sit to a walkable
    /// street's centerline for the opening to count as a street window.
    /// Matches the opinion's own WINDOW_THRESHOLD_M default.
    pub window_threshold_m: f64,
    /// Fraction of the edge's length to use as the window's width.
    pub window_width_frac: f64,
    /// Minimum window width in meters -- no opening shorter than this
    /// will be placed, even if window_width_frac would produce something
    /// narrower.
    pub min_window_width_m: f64,
}

impl Parameters for P164Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "street_adjacency_m",
                "How close a building's centroid must sit to a walkable street's centerline.",
                5.0,
                50.0,
                DEFAULT_STREET_ADJACENCY_M,
            )
            .with_unit("m"),
            ParamSpec::float(
                "window_threshold_m",
                "How close a window's exterior point must sit to a walkable street's centerline.",
                5.0,
                40.0,
                DEFAULT_WINDOW_THRESHOLD_M,
            )
            .with_unit("m"),
            ParamSpec::float(
                "window_width_frac",
                "Fraction of the wall edge length to use as the window's width.",
                0.05,
                0.6,
                0.25,
            ),
            ParamSpec::float(
                "min_window_width_m",
                "Minimum window width in meters.",
                0.3,
                2.0,
                0.9,
            )
            .with_unit("m"),
        ]
    }

    fn defaults() -> Self {
        Self {
            street_adjacency_m: DEFAULT_STREET_ADJACENCY_M,
            window_threshold_m: DEFAULT_WINDOW_THRESHOLD_M,
            window_width_frac: 0.25,
            min_window_width_m: 0.9,
        }
    }

    fn as_vector(&self) -> Vec<f64> {
        vec![
            self.street_adjacency_m,
            self.window_threshold_m,
            self.window_width_frac,
            self.min_window_width_m,
        ]
    }

    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) {
            p.street_adjacency_m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) {
            p.window_threshold_m = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) {
            p.window_width_frac = s.clamp(*x);
        }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) {
            p.min_window_width_m = s.clamp(*x);
        }
        p
    }
}

pub struct P164StreetWindows;

impl PatternOperator for P164StreetWindows {
    type Params = P164Params;

    fn name(&self) -> &'static str {
        "p164_street_windows"
    }

    fn description(&self) -> &'static str {
        "Places one real ground-floor Window opening on each blind street-adjacent building, on the edge nearest a walkable street."
    }

    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p164".into(),
            display: "Alexander et al., A Pattern Language, Pattern 164 (Street Windows)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Street-Windows-(164)".into()),
        }
    }

    /// `parcel_id` must be `"*"` -- processes all buildings in one pass.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p164_street_windows only supports parcel_id \"*\" -- it adds windows to blind street-adjacent buildings across the whole site in one pass.".into());
        }

        // Filter walkable streets (Local or Pedestrian classification).
        let walk_streets: Vec<_> = nbhd
            .streets
            .iter()
            .filter(|s| is_walkable(&s.classification))
            .collect();

        if walk_streets.is_empty() {
            return Err(
                "p164_street_windows needs at least one Local or Pedestrian street -- run path_network first."
                    .into(),
            );
        }

        // Identify street-adjacent buildings.
        let mut street_adjacent: Vec<&Building> = Vec::new();
        for b in &nbhd.buildings {
            let bc = b.polygon.centroid();
            let nearest_m = walk_streets
                .iter()
                .map(|s| point_to_polyline_m(&bc, &s.centerline))
                .fold(f64::MAX, f64::min);
            if nearest_m <= params.street_adjacency_m {
                street_adjacent.push(b);
            }
        }

        if street_adjacent.is_empty() {
            return Err(
                "p164_street_windows: no building sits within the street-adjacency threshold of a Local or Pedestrian street."
                    .into(),
            );
        }

        // Filter to blind buildings (those without a street-facing window).
        let mut blind_buildings: Vec<&Building> = Vec::new();
        for b in &street_adjacent {
            let has_facing_window = b
                .openings
                .iter()
                .filter(|w| w.kind == OpeningKind::Window && !w.on_hole && w.floor == 0)
                .filter_map(|w| opening_point(&b.polygon.outer, w.ring_index, w.t))
                .any(|p| {
                    walk_streets
                        .iter()
                        .any(|s| point_to_polyline_m(&p, &s.centerline) <= params.window_threshold_m)
                });
            if !has_facing_window {
                blind_buildings.push(b);
            }
        }

        if blind_buildings.is_empty() {
            return Err(format!(
                "p164_street_windows: all {} street-adjacent building(s) already have a street window -- nothing to add.",
                street_adjacent.len()
            ));
        }

        // For each blind building, find the edge nearest to a walkable street
        // and place a window on it.
        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced_building_ids: Vec<String> = Vec::new();
        let mut n_windowed = 0usize;
        let mut n_unfixable: Vec<String> = Vec::new();

        for b in &blind_buildings {
            // Find the outer-ring edge whose midpoint is nearest a walkable street.
            let mut nearest_edge: Option<(usize, f64)> = None;
            let mut nearest_m = f64::MAX;

            for ring_index in 0..b.polygon.outer.len().saturating_sub(1) {
                // The midpoint of this edge is at t = 0.5
                if let Some(midpoint) = opening_point(&b.polygon.outer, ring_index, 0.5) {
                    let d = walk_streets
                        .iter()
                        .map(|s| point_to_polyline_m(&midpoint, &s.centerline))
                        .fold(f64::MAX, f64::min);
                    if d < nearest_m {
                        nearest_m = d;
                        nearest_edge = Some((ring_index, d));
                    }
                }
            }

            match nearest_edge {
                Some((ring_index, _midpoint_distance)) => {
                    let edge_len = edge_len_m(b, ring_index);
                    let window_width = if edge_len < params.min_window_width_m {
                        // Edge is too short to place any window on.
                        n_unfixable.push(b.id.clone());
                        continue;
                    } else {
                        // Calculate window width as a fraction of edge length,
                        // clamped to [min, edge_len * 0.9]
                        let frac_width = edge_len * params.window_width_frac;
                        frac_width
                            .max(params.min_window_width_m)
                            .min(edge_len * 0.9)
                    };

                    // Create the opening at t=0.5 (midpoint of the edge).
                    let new_opening = Opening {
                        kind: OpeningKind::Window,
                        ring_index,
                        on_hole: false,
                        t: 0.5,
                        width_m: window_width,
                        sill_height_m: 0.9,    // P221 convention
                        head_height_m: 2.1,    // P221 convention
                        floor: 0,
                    };

                    // Verify the opening's exterior point actually clears the window_threshold_m.
                    if let Some(opening_pt) = opening_point(&b.polygon.outer, ring_index, 0.5) {
                        let d_to_street = walk_streets
                            .iter()
                            .map(|s| point_to_polyline_m(&opening_pt, &s.centerline))
                            .fold(f64::MAX, f64::min);

                        if d_to_street > params.window_threshold_m {
                            // Opening doesn't satisfy the opinion's own window_threshold check.
                            n_unfixable.push(b.id.clone());
                            continue;
                        }
                    } else {
                        n_unfixable.push(b.id.clone());
                        continue;
                    }

                    // Commit the window: clone the building, add the opening, and track replacement.
                    let mut nb = (*b).clone();
                    nb.openings.push(new_opening);
                    new_buildings.push(nb);
                    replaced_building_ids.push(b.id.clone());
                    n_windowed += 1;
                }
                None => {
                    n_unfixable.push(b.id.clone());
                }
            }
        }

        if new_buildings.is_empty() {
            return Err(format!(
                "p164_street_windows: all {} blind street-adjacent building(s) had walls too short or too far from the street to place a real window.",
                blind_buildings.len()
            ));
        }

        let trace = SubdivisionTrace {
            operator_name: self.name().into(),
            operator_source: self.source(),
            headline: format!(
                "Placed {} real street window(s) on {} blind street-adjacent building(s).",
                n_windowed, n_windowed
            ),
            steps: vec![format!(
                "{} blind building(s) got a real ground-floor window on the edge nearest a Local/Pedestrian street, at {:.1}m distance threshold; {} building(s) were unfixable (wall too short or still beyond threshold distance).",
                n_windowed, params.window_threshold_m, n_unfixable.len()
            )],
            caveats: vec![
                "Cannot build Alexander's literal 'window seat' -- no furniture or interior-fixture concept exists in this schema.".into(),
                "'Busy' collapses to Street.classification's Local/Pedestrian values -- no traffic volume or pedestrian-count concept exists.".into(),
                "Places at most ONE window per blind building, on a single edge -- not a real fenestration design.".into(),
                "Doesn't verify the window's own facing direction points at the street, only that its exterior point sits within threshold distance -- same limitation the opinion itself declares.".into(),
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
            entity_provenance: Default::default(),
            trace,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::Polygon;
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn m() -> f64 {
        1.0 / 111_320.0
    }

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![],
            buildings,
            streets,
            open_space: vec![],
            boundaries: vec![],
            activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(),
                fetched_at: "test".into(),
                license: "test".into(),
                layer_provenance: Default::default(),
                label: "P164 generator unit fixture".into(),
            },
            pattern_fields: vec![],
        }
    }

    fn local_street(y_m: f64) -> Street {
        let mm = m();
        Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(-100.0 * mm, y_m * mm), LngLat::new(100.0 * mm, y_m * mm)],
            classification: Some("local".into()),
            row_width_m: Some(5.5),
            surface: None,
        }
    }

    fn blind_building(id: &str, y_m: f64) -> Building {
        let mm = m();
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-3.0 * mm, y_m * mm),
                LngLat::new(3.0 * mm, y_m * mm),
                LngLat::new(3.0 * mm, (y_m + 6.0) * mm),
                LngLat::new(-3.0 * mm, (y_m + 6.0) * mm),
                LngLat::new(-3.0 * mm, y_m * mm),
            ]),
            height_m: Some(6.0),
            typology: None,
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        }
    }

    fn building_with_street_window(id: &str, y_m: f64) -> Building {
        let mm = m();
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-3.0 * mm, y_m * mm),
                LngLat::new(3.0 * mm, y_m * mm),
                LngLat::new(3.0 * mm, (y_m + 6.0) * mm),
                LngLat::new(-3.0 * mm, (y_m + 6.0) * mm),
                LngLat::new(-3.0 * mm, y_m * mm),
            ]),
            height_m: Some(6.0),
            typology: None,
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![Opening {
                kind: OpeningKind::Window,
                ring_index: 0,
                on_hole: false,
                t: 0.5,
                width_m: 1.2,
                sill_height_m: 0.9,
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
    fn parcel_id_target_scope_check() {
        let n = nbhd(
            vec![blind_building("B1", 5.0)],
            vec![local_street(0.0)],
        );
        let err = P164StreetWindows
            .apply(&n, "SOME_BLOCK", &P164Params::defaults(), 0)
            .unwrap_err();
        assert!(
            err.contains("\"*\""),
            "expected the real target-scope error, got: {err}"
        );
    }

    #[test]
    fn no_walkable_streets_is_an_error() {
        let n = nbhd(vec![blind_building("B1", 5.0)], vec![]);
        assert!(P164StreetWindows
            .apply(&n, "*", &P164Params::defaults(), 0)
            .is_err());
    }

    #[test]
    fn no_street_adjacent_buildings_is_an_error() {
        let mm = m();
        let distant_building = Building {
            id: "B_FAR".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(-3.0 * mm, 500.0 * mm),
                LngLat::new(3.0 * mm, 500.0 * mm),
                LngLat::new(3.0 * mm, 506.0 * mm),
                LngLat::new(-3.0 * mm, 506.0 * mm),
                LngLat::new(-3.0 * mm, 500.0 * mm),
            ]),
            height_m: Some(6.0),
            typology: None,
            year_built: None,
            parcel_id: None,
            floors: Some(1),
            openings: vec![],
            interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
            canopies: vec![],
            roof_segments: vec![],
            wall_niches: vec![],
        };
        let n = nbhd(vec![distant_building], vec![local_street(0.0)]);
        let err = P164StreetWindows
            .apply(&n, "*", &P164Params::defaults(), 0)
            .unwrap_err();
        assert!(
            err.contains("no building"),
            "expected the real 'no street-adjacent buildings' error, got: {err}"
        );
    }

    #[test]
    fn all_buildings_already_have_windows_is_an_error() {
        let n = nbhd(
            vec![building_with_street_window("B1", 5.0)],
            vec![local_street(0.0)],
        );
        let err = P164StreetWindows
            .apply(&n, "*", &P164Params::defaults(), 0)
            .unwrap_err();
        assert!(
            err.contains("already have"),
            "expected the real 'nothing to do' error, got: {err}"
        );
    }

    #[test]
    fn a_blind_building_gets_a_window_placed_on_the_nearest_edge() {
        let n = nbhd(
            vec![blind_building("B1", 5.0)],
            vec![local_street(0.0)],
        );
        let sub = P164StreetWindows
            .apply(&n, "*", &P164Params::defaults(), 0)
            .unwrap();

        assert_eq!(sub.new_buildings.len(), 1, "should create one modified building");
        let modified = &sub.new_buildings[0];
        assert_eq!(
            modified.openings.len(),
            1,
            "should add exactly one opening to the blind building"
        );

        let opening = &modified.openings[0];
        assert_eq!(opening.kind, OpeningKind::Window, "opening should be a Window");
        assert!(!opening.on_hole, "opening should be on outer ring");
        assert_eq!(opening.floor, 0, "opening should be on ground floor");
        assert_eq!(opening.t, 0.5, "window should be at the midpoint of the edge");
        assert!(
            opening.width_m >= P164Params::defaults().min_window_width_m,
            "window width should respect min_window_width_m"
        );

        // Check that the opening's exterior point actually satisfies the window_threshold.
        if let Some(pt) = opening_point(&modified.polygon.outer, opening.ring_index, opening.t)
        {
            let street_centerline = &n.streets[0].centerline;
            let d = point_to_polyline_m(&pt, street_centerline);
            assert!(
                d <= P164Params::defaults().window_threshold_m,
                "opening's exterior point should be within window_threshold_m of the street, got {:.1}m",
                d
            );
        }

        // Check that the building was replaced.
        assert_eq!(sub.replaced_building_ids.len(), 1);
        assert_eq!(sub.replaced_building_ids[0], "B1");
    }

    #[test]
    fn params_roundtrip() {
        let p = P164Params {
            street_adjacency_m: 25.0,
            window_threshold_m: 12.0,
            window_width_frac: 0.3,
            min_window_width_m: 1.1,
        };
        let v = p.as_vector();
        let back = P164Params::from_vector(&v);
        assert_eq!(back.street_adjacency_m, 25.0);
        assert_eq!(back.window_threshold_m, 12.0);
        assert_eq!(back.window_width_frac, 0.3);
        assert_eq!(back.min_window_width_m, 1.1);
    }

    #[test]
    fn defaults_match_opinion_constants() {
        let defaults = P164Params::defaults();
        assert_eq!(defaults.street_adjacency_m, DEFAULT_STREET_ADJACENCY_M);
        assert_eq!(defaults.window_threshold_m, DEFAULT_WINDOW_THRESHOLD_M);
    }
}
