//! P221 Natural Doors and Windows — place real window and door openings on
//! every building's exterior walls, sized per room-width bay, shrinking
//! floor by floor going up.
//!
//! From Alexander, *A Pattern Language*, Pattern 221:
//! > Make a rule of thumb to avoid standard, ready-made windows and doors...
//! > place them according to the [needs of light and view from] the room
//! > and the building... make each window a different size, according to
//! > its place... the windows on the upper floors [get] smaller than those
//! > below, because the rooms get smaller, there is more daylight up
//! > there, and people need more enclosure the higher up they are.
//!
//! # Where this sits in the pattern graph
//! Fetched and confirmed against `patternlanguage.com/apl/aplsample` (not
//! the copyrighted full-text PDF -- see README's Reference section for that
//! link, kept as a citation only, never committed as a file):
//! - **P159 Light on Two Sides of Every Room** explicitly names itself as
//!   completing **P107 Wings of Light** (already built in this crate) and
//!   in turn calls for this pattern (P221) among others. `street-smarts-
//!   opinions::pattern::p159_light_on_two_sides` scores whether the result
//!   this operator produces actually achieves two-sided light -- P159 is a
//!   criterion Alexander states, not a shape rule, so it lives in the
//!   opinion/chorus architecture rather than here.
//! - P221 itself calls for three child patterns that would refine the
//!   opening's own geometry: **P222 Low Sill**, **P223 Deep Reveals**,
//!   **P239 Small Panes**. Their sample pages 404 on patternlanguage.com's
//!   free excerpt set, and the Cornell PDF mirror is too large to fetch
//!   directly -- their *names/numbers* are corroborated from two
//!   independent verified pages (P221's own "smaller patterns" list AND
//!   cross-referenced from P159's), but their exact prescribed dimensions
//!   are NOT verified. `sill_frac`/`window_width_ratio` below are
//!   reasonable placeholder geometry, not sourced numbers -- named honestly
//!   rather than hidden, same convention as every other operator's caveats
//!   in this crate.
//! - **P192 Windows Overlooking Life** is named in both P221's and P159's
//!   own reference lists (independently corroborated) and is used here,
//!   by title only, as the reasoning behind orienting the ground-floor
//!   door toward the nearest street/open space rather than picking a wall
//!   arbitrarily. Its own text is unverified; nothing beyond the
//!   self-evident title ("face activity, not a blank wall") is claimed.
//!
//! # What this operator does
//! Runs once, site-scale (`parcel_id == "*"`), over every `Building` already
//! on the neighborhood -- the first operator in this crate that primarily
//! targets `nbhd.buildings` rather than `Parcel`s, since window/door
//! placement is downstream of a real footprint+height, both of which only
//! exist once `p107_wings_of_light` (or the older `building_shape` stub)
//! has run. For each building:
//! - `floors = round(height_m / floor_to_floor_m)`.
//! - Walks the outer wall ring, and the courtyard ring too for buildings
//!   P107 shaped as `p107_courtyard_v01` -- courtyard-facing walls get
//!   windows exactly like street-facing ones, since a courtyard building's
//!   inner wall is real daylight, not decoration.
//! - Divides each wall segment long enough (`min_wall_segment_m`) into
//!   `room_width_m`-wide bays and places one `Window` opening per bay per
//!   floor, shrinking width and height by `size_falloff_per_floor` per
//!   floor above ground -- the one prescriptive P221 rule verified above.
//! - Places one `Door` opening on the ground floor of whichever wall
//!   segment's outward normal points most directly at the nearest point on
//!   `nbhd.streets`/`nbhd.open_space` (the P192-inspired heuristic).
//!
//! # What this operator deliberately does NOT do
//! - No randomness anywhere (`seed` is accepted, per the `PatternOperator`
//!   trait, but ignored -- every opening's position is a deterministic
//!   function of wall geometry, floor count, and proximity to the public
//!   realm). This is a deliberate departure from every other operator in
//!   this crate, most of which use `Prng` for jitter -- there's nothing
//!   here Alexander's own rule leaves to chance.
//! - No real room layout. A "bay" is a geometric subdivision of the
//!   exterior wall by an assumed `room_width_m`, not an actual interior
//!   partition -- there is no partition data anywhere in this pipeline.
//! - No P222/P223/P239 detail (sill depth, reveal, pane subdivision) beyond
//!   a flat sill/head height -- see the graph note above.
//! - Doesn't suppress openings on walls that happen to sit close to a
//!   DIFFERENT, unmerged building (only `p108_connected_buildings` merges
//!   are treated as party walls, by construction, since a merged building's
//!   polygon has no boundary where the party wall was). Two separate
//!   buildings standing close but not merged can each get windows facing
//!   the gap between them.

use crate::orientation::nearest_public_realm_point;
use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{centroid, lnglat_to_local, ring_to_local, Pt2};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::components::BuildingTypology;
use street_smarts_core::nir::{Building, Neighborhood, Opening, OpeningKind};
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P221Params {
    /// Assumed room width -- how far apart window bays are spaced along a
    /// wall. Same category of placeholder as P107's `max_wing_width_m`:
    /// plausible, not derived from any real room program.
    pub room_width_m: f64,
    /// Fraction of a bay's width the window itself occupies.
    pub window_width_ratio: f64,
    /// Fraction of `floor_to_floor_m` up from this floor's base where the
    /// window sill sits.
    pub window_sill_frac: f64,
    /// Fraction of `floor_to_floor_m` up from this floor's base where the
    /// window head sits.
    pub window_head_frac: f64,
    /// Multiplicative shrink applied to window width AND height per floor
    /// above ground -- P221's own verified rule ("windows on the upper
    /// floors smaller than those below").
    pub size_falloff_per_floor: f64,
    /// Floor-to-floor height. Should match `p107_wings_of_light`'s own
    /// parameter of the same name for a building's `floors` count to be
    /// consistent with the height P107 actually built.
    pub floor_to_floor_m: f64,
    /// Ground-floor door width.
    pub door_width_m: f64,
    /// Ground-floor door head height (door sill is always 0 -- a door has
    /// no sill above the floor by definition).
    pub door_head_height_m: f64,
    /// Don't bother placing openings on a wall segment shorter than this --
    /// not enough room for even one bay.
    pub min_wall_segment_m: f64,
}

impl Parameters for P221Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "room_width_m",
                "Assumed room width -- spacing between window bays along a wall.",
                2.5, 7.0, 4.0,
            ).with_unit("m"),
            ParamSpec::float(
                "window_width_ratio",
                "Fraction of a bay's width the window itself occupies.",
                0.2, 0.9, 0.55,
            ),
            ParamSpec::float(
                "window_sill_frac",
                "Fraction of floor-to-floor height where the window sill sits.",
                0.1, 0.6, 0.35,
            ),
            ParamSpec::float(
                "window_head_frac",
                "Fraction of floor-to-floor height where the window head sits.",
                0.6, 0.98, 0.85,
            ),
            ParamSpec::float(
                "size_falloff_per_floor",
                "Multiplicative shrink of window width/height per floor above ground (P221: smaller higher up).",
                0.6, 1.0, 0.92,
            ),
            ParamSpec::float(
                "floor_to_floor_m",
                "Floor-to-floor height -- should match p107_wings_of_light's own value.",
                2.5, 5.0, 3.5,
            ).with_unit("m"),
            ParamSpec::float(
                "door_width_m",
                "Ground-floor door width.",
                0.7, 2.0, 1.0,
            ).with_unit("m"),
            ParamSpec::float(
                "door_head_height_m",
                "Ground-floor door head height.",
                1.9, 2.4, 2.1,
            ).with_unit("m"),
            ParamSpec::float(
                "min_wall_segment_m",
                "Don't place openings on a wall segment shorter than this.",
                1.0, 10.0, 2.5,
            ).with_unit("m"),
        ]
    }
    fn defaults() -> Self {
        Self {
            room_width_m: 4.0,
            window_width_ratio: 0.55,
            window_sill_frac: 0.35,
            window_head_frac: 0.85,
            size_falloff_per_floor: 0.92,
            floor_to_floor_m: 3.5,
            door_width_m: 1.0,
            door_head_height_m: 2.1,
            min_wall_segment_m: 2.5,
        }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![
            self.room_width_m,
            self.window_width_ratio,
            self.window_sill_frac,
            self.window_head_frac,
            self.size_falloff_per_floor,
            self.floor_to_floor_m,
            self.door_width_m,
            self.door_head_height_m,
            self.min_wall_segment_m,
        ]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.room_width_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.window_width_ratio = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.window_sill_frac = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) { p.window_head_frac = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(4), v.get(4)) { p.size_falloff_per_floor = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(5), v.get(5)) { p.floor_to_floor_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(6), v.get(6)) { p.door_width_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(7), v.get(7)) { p.door_head_height_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(8), v.get(8)) { p.min_wall_segment_m = s.clamp(*x); }
        p
    }
}

pub struct P221NaturalDoorsAndWindows;

impl PatternOperator for P221NaturalDoorsAndWindows {
    type Params = P221Params;

    fn name(&self) -> &'static str { "p221_natural_doors_and_windows" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p221".into(),
            display: "Alexander et al., A Pattern Language, Pattern 221 (Natural Doors and Windows)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl221/apl221.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Place window and door openings on every building's exterior walls, sized per room-width bay, shrinking floor by floor going up."
    }

    /// `parcel_id` must be `"*"` -- this operator places openings on every
    /// building in one pass; it doesn't target parcels at all.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        _seed: u64,
    ) -> Result<Subdivision, String> {
        if parcel_id != "*" {
            return Err("p221_natural_doors_and_windows only supports parcel_id \"*\" -- it places openings on every building in one pass.".into());
        }
        if nbhd.buildings.is_empty() {
            return Err("p221_natural_doors_and_windows: no buildings found -- run p107_wings_of_light (or building_shape) first.".into());
        }

        let mut new_buildings: Vec<Building> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut n_shaped = 0;
        let mut n_skipped = 0;
        let mut total_windows = 0usize;
        let mut total_doors = 0usize;

        for b in &nbhd.buildings {
            if b.polygon.outer.len() < 3 {
                n_skipped += 1;
                continue;
            }
            let floors = ((b.height_m.unwrap_or(params.floor_to_floor_m)) / params.floor_to_floor_m)
                .round()
                .max(1.0) as u32;

            let origin = b.polygon.centroid();
            let outer_local = ring_to_local(&b.polygon.outer, &origin);
            if outer_local.len() < 3 {
                n_skipped += 1;
                continue;
            }

            let target = nearest_public_realm_point(nbhd, b);
            let target_local = target.map(|t| lnglat_to_local(&t, &origin));
            let door_edge = choose_door_wall(&outer_local, target_local, params.min_wall_segment_m);

            let mut openings: Vec<Opening> = Vec::new();
            place_wall_openings(&outer_local, false, door_edge, floors, params, &mut openings);

            if BuildingTypology::label_is_courtyard(b.typology.as_deref()) {
                if let Some(part) = b.polygon.parts_view().first() {
                    if let Some(hole) = part.holes.first() {
                        let hole_local = ring_to_local(hole, &origin);
                        if hole_local.len() >= 3 {
                            place_wall_openings(&hole_local, true, None, floors, params, &mut openings);
                        }
                    }
                }
            }

            if openings.is_empty() {
                n_skipped += 1;
                continue;
            }
            for o in &openings {
                match o.kind {
                    OpeningKind::Window => total_windows += 1,
                    OpeningKind::Door => total_doors += 1,
                }
            }

            let mut updated = b.clone();
            updated.floors = Some(floors);
            updated.openings = openings;
            new_buildings.push(updated);
            replaced.push(b.id.clone());
            n_shaped += 1;
        }

        if n_shaped == 0 {
            return Err(format!(
                "p221_natural_doors_and_windows: 0 of {} building(s) had a wall long enough for an opening (min_wall_segment_m = {:.1}m).",
                nbhd.buildings.len(), params.min_wall_segment_m
            ));
        }

        steps.push(format!(
            "Placed {} window(s) and {} door(s) across {} building(s); skipped {} (degenerate footprint or no wall >= {:.1}m).",
            total_windows, total_doors, n_shaped, n_skipped, params.min_wall_segment_m
        ));

        let trace = SubdivisionTrace {
            operator_name: "p221_natural_doors_and_windows".into(),
            operator_source: self.source(),
            headline: format!(
                "Placed real window/door openings on {} building(s): {} windows, {} doors.",
                n_shaped, total_windows, total_doors
            ),
            steps,
            caveats: vec![
                "room_width_m (the window-bay spacing) is a plausible placeholder, not derived \
                 from any real room program -- same category of assumption as P107's \
                 max_wing_width_m.".into(),
                "P222 Low Sill / P223 Deep Reveals / P239 Small Panes -- the child patterns P221 \
                 itself calls for -- are named here but their exact prescribed dimensions are \
                 unverified (404 on patternlanguage.com's free excerpt set; the Cornell PDF \
                 mirror is too large to fetch and isn't committed to this repo for copyright \
                 reasons -- see README). sill/head fractions here are reasonable stand-ins, not \
                 sourced numbers.".into(),
                "Door-wall selection (nearest street/open-space point) is a heuristic inspired by \
                 P192 Windows Overlooking Life's title, not a verified implementation of that \
                 pattern's actual text.".into(),
                "Doesn't suppress openings facing a DIFFERENT, unmerged nearby building -- only \
                 p108_connected_buildings merges are treated as party walls (by construction, a \
                 merged footprint has no boundary where the party wall was). Two close but \
                 separately-shaped buildings can each get windows facing the gap between them."
                    .into(),
                "No real room layout anywhere in this pipeline -- a 'bay' is a geometric \
                 subdivision of the exterior wall by room_width_m, not an actual interior \
                 partition.".into(),
            ],
            seed: _seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: vec![],
            new_buildings,
            new_streets: vec![],
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: vec![],
            replaced_building_ids: replaced,
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
        })
    }
}

/// Pick the outer-ring edge index whose outward normal points most directly
/// at `target_local`, among edges long enough to hold a door. Falls back to
/// the single longest qualifying edge when there's no target (no streets or
/// open space anywhere in the neighborhood) or none qualifies by direction.
fn choose_door_wall(ring: &[Pt2], target_local: Option<Pt2>, min_len: f64) -> Option<usize> {
    let n = ring.len();
    if n < 2 {
        return None;
    }
    let c = centroid(ring);
    let mut best_by_len: Option<(usize, f64)> = None;
    let mut best_by_facing: Option<(usize, f64)> = None;

    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        let len = a.dist(b);
        if len < min_len {
            continue;
        }
        if best_by_len.map(|(_, bl)| len > bl).unwrap_or(true) {
            best_by_len = Some((i, len));
        }
        if let Some(target) = target_local {
            let mid = Pt2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            let edge = b.sub(a);
            let mut normal = Pt2::new(edge.y, -edge.x);
            // Orient outward: flip if it points toward the ring centroid.
            if normal.dot(mid.sub(c)) < 0.0 {
                normal = Pt2::new(-normal.x, -normal.y);
            }
            let to_target = target.sub(mid);
            let dist = to_target.len();
            if dist < 1e-6 {
                continue;
            }
            let facing = normal.dot(to_target) / (normal.len() * dist);
            if facing > 0.0 {
                let score = facing / dist.max(1.0); // prefer close AND facing
                if best_by_facing.map(|(_, bs)| score > bs).unwrap_or(true) {
                    best_by_facing = Some((i, score));
                }
            }
        }
    }

    best_by_facing.or(best_by_len).map(|(i, _)| i)
}

/// Place window (and, on the chosen door edge, one door) openings along
/// every long-enough segment of `ring`, per floor, per `room_width_m` bay.
fn place_wall_openings(
    ring: &[Pt2],
    on_hole: bool,
    door_edge: Option<usize>,
    floors: u32,
    params: &P221Params,
    out: &mut Vec<Opening>,
) {
    let n = ring.len();
    if n < 2 {
        return;
    }
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        let len = a.dist(b);
        if len < params.min_wall_segment_m {
            continue;
        }
        let n_bays = (len / params.room_width_m).round().max(1.0) as usize;
        let bay_len = len / n_bays as f64;
        let is_door_edge = door_edge == Some(i);
        let door_bay = n_bays / 2;

        for k in 0..n_bays {
            let t = (k as f64 + 0.5) / n_bays as f64;
            if is_door_edge && k == door_bay {
                out.push(Opening {
                    kind: OpeningKind::Door,
                    ring_index: i,
                    on_hole,
                    t,
                    width_m: params.door_width_m.min(bay_len * 0.9),
                    sill_height_m: 0.0,
                    head_height_m: params.door_head_height_m,
                    floor: 0,
                });
                continue; // ground floor: door instead of a window in this bay
            }
            for f in 0..floors {
                let falloff = params.size_falloff_per_floor.powi(f as i32);
                let width = (bay_len * params.window_width_ratio * falloff).min(bay_len * 0.9);
                let floor_h = params.floor_to_floor_m;
                let mid_frac = (params.window_sill_frac + params.window_head_frac) / 2.0;
                let half_h = (params.window_head_frac - params.window_sill_frac) * falloff / 2.0;
                let sill = ((mid_frac - half_h) * floor_h).max(0.05);
                let head = ((mid_frac + half_h) * floor_h).min(floor_h - 0.05);
                if head <= sill || width < 0.3 {
                    continue;
                }
                out.push(Opening {
                    kind: OpeningKind::Window,
                    ring_index: i,
                    on_hole,
                    t,
                    width_m: width,
                    sill_height_m: sill,
                    head_height_m: head,
                    floor: f,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{NeighborhoodMeta, Street};

    fn nbhd(buildings: Vec<Building>, streets: Vec<Street>) -> Neighborhood {
        Neighborhood {
            id: "test".into(),
            bbox_wgs84: [0.0, 0.0, 0.001, 0.001],
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
                label: "P221 unit fixture".into(),
            },
        }
    }

    fn square_building(id: &str, side_m: f64, height_m: f64) -> Building {
        let m = 1.0 / 111_320.0;
        let s = side_m * m;
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0),
                LngLat::new(s, 0.0),
                LngLat::new(s, s),
                LngLat::new(0.0, s),
                LngLat::new(0.0, 0.0),
            ]),
            height_m: Some(height_m),
            typology: Some("p107_solid_v01".into()),
            year_built: None,
            parcel_id: Some("PAD_1".into()),
            floors: None,
            openings: vec![],
            interior_cells: vec![],
        }
    }

    #[test]
    fn places_windows_and_a_door_on_a_two_story_building() {
        let street = Street {
            id: "S1".into(),
            centerline: vec![LngLat::new(-0.0005, 0.0), LngLat::new(-0.0005, 0.0002)],
            classification: Some("local".into()),
            row_width_m: Some(8.0),
            surface: None,
        };
        let n = nbhd(vec![square_building("B1", 20.0, 7.0)], vec![street]);
        let sub = P221NaturalDoorsAndWindows
            .apply(&n, "*", &P221Params::defaults(), 1)
            .expect("should place openings");
        assert_eq!(sub.new_buildings.len(), 1);
        assert_eq!(sub.replaced_building_ids, vec!["B1".to_string()]);
        let b = &sub.new_buildings[0];
        assert_eq!(b.floors, Some(2));
        let doors: Vec<_> = b.openings.iter().filter(|o| o.kind == OpeningKind::Door).collect();
        let windows: Vec<_> = b.openings.iter().filter(|o| o.kind == OpeningKind::Window).collect();
        assert_eq!(doors.len(), 1, "expected exactly one door");
        assert!(!windows.is_empty(), "expected at least one window");
    }

    #[test]
    fn windows_shrink_going_up_floors() {
        let n = nbhd(vec![square_building("B1", 20.0, 10.5)], vec![]);
        let sub = P221NaturalDoorsAndWindows
            .apply(&n, "*", &P221Params::defaults(), 1)
            .expect("should place openings");
        let b = &sub.new_buildings[0];
        assert_eq!(b.floors, Some(3));
        let w0 = b.openings.iter().find(|o| o.kind == OpeningKind::Window && o.floor == 0).unwrap().width_m;
        let w2 = b.openings.iter().find(|o| o.kind == OpeningKind::Window && o.floor == 2).unwrap().width_m;
        assert!(w2 < w0, "top-floor window ({w2}) should be smaller than ground-floor window ({w0})");
    }

    #[test]
    fn no_buildings_is_an_error_not_a_silent_no_op() {
        let n = nbhd(vec![], vec![]);
        let err = P221NaturalDoorsAndWindows.apply(&n, "*", &P221Params::defaults(), 1);
        assert!(err.is_err());
    }
}
