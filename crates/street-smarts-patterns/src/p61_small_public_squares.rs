//! P61 Small Public Squares — keep public squares small enough to feel
//! intimate, not desolate.
//!
//! From Alexander, *A Pattern Language*, Pattern 61:
//! > A square which is more than about 60 feet [~18m] across... will
//! > never feel comfortable or intimate, unless it is extremely crowded.
//!
//! # v0.4 approach
//! Scans existing `OpenSpace` entities of kind `Plaza` -- the courtyards
//! P95 and P107 already produce are the natural candidates here, since
//! this pipeline doesn't yet place squares anywhere else. For each:
//! - If its longer bounding-box dimension is already <= `max_dimension_m`,
//!   leave it alone -- P61 is already satisfied.
//! - If it's too large, break it into a grid of smaller squares (Voronoi
//!   seeds on a regular grid, clipped to the plaza's real -- possibly
//!   non-convex -- boundary), each within `max_dimension_m`, and link them
//!   with an honest MST path backbone (`planar::kruskal_mst`, shared with
//!   P52's PathNetwork) -- the fewest connecting segments that still reach
//!   every new square, not a full mesh.
//!
//! v0.2 tiled the WHOLE plaza this way, which is wrong at real scale: a
//! multi-acre courtyard genuinely needs 40+ squares to tile exhaustively at
//! an 18.3m cap -- no amount of smarter packing math changes that, since
//! it's an area argument, not a layout one. That's not what "a few smaller
//! connected squares" describes, and it's not how real precedent for this
//! pattern actually works: Barcelona's *superilles* place a small number of
//! plaza nodes per reclaimed superblock (roughly one per several acres),
//! not one every 18m: most of the reclaimed area stays ordinary street/
//! green space. Lisbon's Alfama praças are the same -- sparse, at natural
//! convergence points, even though the surrounding fabric is tight and
//! irregular. So v0.3 capped the square count at `max_squares` (default 4 --
//! Alexander's own "a few") and kept the largest-area candidates (maximizes
//! usable public space for a fixed count); v0.4 goes one step further and
//! emits everything the cap (and the `min_meaningful_area_m2` sliver filter)
//! leaves out as real `OpenSpaceKind::Undecided` geometry, not a summed
//! float in a trace string -- so a future P106 (Positive Outdoor Space)
//! check has actual polygons to scan instead of having to trust that
//! "uncovered" land was resolved somewhere. The old oversized plaza is
//! still REPLACED (via `replaced_open_space_ids`) by everything this
//! operator produces -- both the kept squares and the Undecided remainder.
//!
//! # v0.5: raw-land placement, not just resizing an existing plaza
//!
//! Everything above assumes a `Plaza` already exists to shrink or partition
//! -- true when P95/P107 ran first. Alexander's own numbering (52 < 61 < 95)
//! says P61 should run BEFORE P95, though, and there's no existing plaza to
//! resize on raw, undeveloped block land. So: when `apply` is given a
//! specific `parcel_id` (not `"*"`) and no existing Plaza overlaps that
//! parcel, it places a handful of NEW compliant squares directly on the raw
//! land instead of erroring -- up to `max_squares`, each stamped at
//! `max_dimension_m` and clipped to the parcel's real boundary, linked by
//! the same honest MST backbone the resize path already uses. It does NOT
//! modify or replace the target parcel's own polygon -- the new squares and
//! connector streets are added alongside it, and P95's reserved-land
//! subtraction (`reserved_holes_for_part`) is what actually makes P95 build
//! around them later. `parcel_id == "*"` still means exactly what it always
//! did: scan every existing Plaza in the neighborhood, ignore parcels
//! entirely. That path is unchanged.
//!
//! # v0.6: site-scale square budget, not one `max_squares` cluster per block
//!
//! v0.5 fixed WHERE squares get placed (raw block land, not just resized
//! plazas) but not WHAT SCALE they're placed at: the corrected pipeline was
//! calling `apply()` once per P37 block, each call placing up to
//! `max_squares` (default 4) new squares -- so a 12-block site could get on
//! the order of 40+ squares total, each one a P95 subtraction hole on its
//! own block. That's the wrong scale for this pattern: Alexander's "a few"
//! public squares describes a handful across a whole neighborhood/site, the
//! same tier as P37 (House Cluster) and P52 (Network of Paths) run once,
//! not something nested inside every individual house cluster. It was also
//! the biggest single contributor to the block-level fragmentation tracked
//! separately (P95's `subtract_convex` holes compounding across several
//! same-block squares plus street corridors).
//!
//! `crate::pipeline::run_corrected_pipeline` now calls `place_new_squares_n`
//! (below) directly, once per block, but with an explicit count allocated
//! by area across the WHOLE site so the total stays at `max_squares`, not
//! `max_squares` times the block count -- most blocks get zero squares,
//! same as real precedent (a superilla-scale plaza serves several blocks,
//! not one per block). `apply()` itself is unchanged for the case where a
//! caller (the web UI's single-operator picker, a direct registry call)
//! wants `max_squares` placed on one specific parcel.
//!
//! # What this deliberately does NOT do
//! - Connector segments are geometry-only path centerlines (same
//!   abstraction PathNetwork uses). Alexander's real edge treatment for the
//!   links between squares -- colonnades, trees, level changes -- is not
//!   modeled; that's a materials/design decision downstream of geometry.
//! - Which candidate squares survive the `max_squares` cap is decided by
//!   area alone (biggest first), not by position -- real superilla/praça
//!   placement responds to where people actually converge (transit, main
//!   entrances, street corners), which this operator has no model of.
//! - Undecided geometry is exactly that -- undecided. This operator marks
//!   it, it doesn't resolve it. What it becomes (a second pattern, a park,
//!   merged into a neighbor) is the kind of question Alexander's Positive
//!   Outdoor Space (P106) exists to ask; no P106 check reads this yet.
//! - The grid partition is a rectangular heuristic. Alexander's own
//!   sketches for this pattern are organic, hand-drawn subdivisions; a grid
//!   is an honest first approximation, not a claim to match his intent
//!   exactly.
//!
//! # v0.7: a real ActivityNode per square, closing P126's real gap
//!
//! `Neighborhood.activity_nodes` is a real, typed field -- before this,
//! no operator anywhere in this pipeline ever populated it (the same
//! class of real-field-no-producer gap `p33_night_life`'s own module doc
//! documents for `use_category: "commercial"`). Every square this
//! operator places now gets one real `ActivityNode` (`ActivityKind::Civic`)
//! seeded from a real, deterministic `Prng` jitter off the square's own
//! centroid -- "roughly, not exactly, in the middle," Alexander's own
//! P126 Something Roughly in the Middle instruction, not dead-center by
//! construction.

use crate::p95_building_complex::stratified_seeds;
use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{
    area, bbox, centroid, clip_to_polygon, clip_to_polygon_largest, kruskal_mst, local_to_lnglat,
    local_to_ring, point_in_polygon, ring_to_local, scale_toward_centroid, union_pieces,
    voronoi_cell, MstResult, Pt2,
};
use crate::prng::Prng;
use crate::subdivision::{apply_subdivision, PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::components::StreetClassification;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{ActivityKind, ActivityNode, Neighborhood, OpenSpace, OpenSpaceKind, Parcel, Street};
use street_smarts_core::opinion::SourceCitation;
use street_smarts_core::world::World;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P61Params {
    /// Alexander's own number: ~60 feet (18.3m). Squares larger than this
    /// rarely feel intimate.
    pub max_dimension_m: f64,
    /// Don't bother touching plazas already this small or smaller, and drop
    /// (rather than emit) any partitioned sub-square piece this small --
    /// nothing useful to shrink or to call a square.
    pub min_meaningful_area_m2: f64,
    /// Right-of-way width for the path segments linking sibling squares
    /// produced by a partition. Deliberately narrower than PathNetwork's
    /// vehicular default -- these are pedestrian links between intimate
    /// squares, not streets.
    pub connector_width_m: f64,
    /// Cap on how many squares a single oversized plaza gets split into --
    /// Alexander's own "a few," and the same order of magnitude real
    /// precedent (Barcelona superilles, Lisbon's Alfama praças) actually
    /// uses per reclaimed area. Prevents exhaustively tiling a multi-acre
    /// courtyard into dozens of postage-stamp squares; excess candidate
    /// area is reported as uncovered, not fabricated or silently dropped.
    pub max_squares: f64,
}

impl Parameters for P61Params {
    fn schema() -> Vec<ParamSpec> {
        vec![
            ParamSpec::float(
                "max_dimension_m",
                "Max comfortable public square dimension (Alexander: ~60ft/18m).",
                8.0, 30.0, 18.3,
            ).with_unit("m"),
            ParamSpec::float(
                "min_meaningful_area_m2",
                "Skip plazas, and drop partitioned pieces, this small or smaller.",
                5.0, 200.0, 20.0,
            ).with_unit("m²"),
            ParamSpec::float(
                "connector_width_m",
                "Right-of-way for the pedestrian links between sibling squares.",
                1.5, 8.0, 3.0,
            ).with_unit("m"),
            ParamSpec::float(
                "max_squares",
                "Cap on squares per oversized plaza -- Alexander's 'a few,' not exhaustive tiling.",
                1.0, 20.0, 4.0,
            ),
        ]
    }
    fn defaults() -> Self {
        Self { max_dimension_m: 18.3, min_meaningful_area_m2: 20.0, connector_width_m: 3.0, max_squares: 4.0 }
    }
    fn as_vector(&self) -> Vec<f64> {
        vec![self.max_dimension_m, self.min_meaningful_area_m2, self.connector_width_m, self.max_squares]
    }
    fn from_vector(v: &[f64]) -> Self {
        let schema = Self::schema();
        let mut p = Self::defaults();
        if let (Some(s), Some(x)) = (schema.get(0), v.get(0)) { p.max_dimension_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(1), v.get(1)) { p.min_meaningful_area_m2 = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(2), v.get(2)) { p.connector_width_m = s.clamp(*x); }
        if let (Some(s), Some(x)) = (schema.get(3), v.get(3)) { p.max_squares = s.clamp(*x); }
        p
    }
}

pub struct P61SmallPublicSquares;

/// How far, as a fraction of the square's own local bounding-box short
/// side, a real ActivityNode jitters off the square's centroid -- "roughly,
/// not exactly, in the middle" (P126 Something Roughly in the Middle).
const ACTIVITY_NODE_JITTER_FRAC: f64 = 0.2;

/// One real, jittered-off-center ActivityNode for a newly placed square --
/// see this file's own "v0.7" module doc. `sq_local` is the square's own
/// ring in the same local-meter frame `centroid`/`bbox` already use;
/// `origin` converts back to real lng/lat. `activity_fit` and
/// `publicness` are left at their honest empty/`None` defaults -- this
/// function only knows it placed a generic plaza marker, not what real
/// activity belongs there or how public the setting reads; inventing
/// plausible-looking values for either would be exactly the kind of
/// fabrication this pipeline's opinions consistently refuse to do
/// elsewhere (see `p33_night_life.rs`'s own module doc on the same
/// principle for `use_category`).
fn activity_node_for_square(id: String, sq_local: &[Pt2], origin: &LngLat, prng: &mut Prng) -> ActivityNode {
    let c = centroid(sq_local);
    let (min_pt, max_pt) = bbox(sq_local);
    let short_side = (max_pt.x - min_pt.x).min(max_pt.y - min_pt.y);
    let jitter_m = short_side * ACTIVITY_NODE_JITTER_FRAC;
    let angle = prng.range(0.0, std::f64::consts::TAU);
    let jittered = Pt2::new(c.x + angle.cos() * jitter_m, c.y + angle.sin() * jitter_m);
    ActivityNode {
        id,
        location: local_to_lnglat(jittered, origin),
        kind: ActivityKind::Civic,
        intensity: None,
        label: None,
        activity_fit: Default::default(),
        publicness: None,
    }
}

/// Break an oversized plaza into a grid of Voronoi-seeded sub-squares, each
/// clipped to the plaza's real (possibly non-convex) boundary, sized so the
/// grid alone gets every piece within `max_dimension_m` along both axes. A
/// caller still needs to re-check each returned piece's own bounding box --
/// clipping against a non-convex boundary can distort a grid cell -- and
/// filter/shrink pieces as needed.
fn partition_plaza(local: &[Pt2], max_dimension_m: f64) -> Vec<Vec<Pt2>> {
    let (min_pt, max_pt) = bbox(local);
    let width = (max_pt.x - min_pt.x).max(1e-6);
    let height = (max_pt.y - min_pt.y).max(1e-6);
    let nx = (width / max_dimension_m).ceil().max(1.0) as usize;
    let ny = (height / max_dimension_m).ceil().max(1.0) as usize;
    let cell_w = width / nx as f64;
    let cell_h = height / ny as f64;

    let mut seeds: Vec<Pt2> = Vec::with_capacity(nx * ny);
    for iy in 0..ny {
        for ix in 0..nx {
            seeds.push(Pt2::new(
                min_pt.x + (ix as f64 + 0.5) * cell_w,
                min_pt.y + (iy as f64 + 0.5) * cell_h,
            ));
        }
    }

    let square_bound = vec![
        Pt2::new(min_pt.x, min_pt.y),
        Pt2::new(max_pt.x, min_pt.y),
        Pt2::new(max_pt.x, max_pt.y),
        Pt2::new(min_pt.x, max_pt.y),
    ];

    let mut pieces: Vec<Vec<Pt2>> = Vec::new();
    for &seed in &seeds {
        let cell = voronoi_cell(seed, &seeds, &square_bound);
        if cell.len() < 3 { continue; }
        let clipped = clip_to_polygon(&cell, local);
        if clipped.is_empty() { continue; }
        pieces.extend(union_pieces(&clipped));
    }
    pieces
}

impl PatternOperator for P61SmallPublicSquares {
    type Params = P61Params;

    fn name(&self) -> &'static str { "p61_small_public_squares" }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p61".into(),
            display: "Alexander et al., A Pattern Language, Pattern 61 (Small Public Squares)".into(),
            url: Some("https://patternlanguage.com/apl/aplsample/apl61/apl61.htm".into()),
        }
    }
    fn description(&self) -> &'static str {
        "Place a few small squares on raw block land, or break an oversized existing plaza into several within Alexander's ~18m intimacy threshold."
    }

    /// `parcel_id == "*"`: scan every existing `Plaza`-kind `OpenSpace` in
    /// the neighborhood, ignore parcels entirely -- unchanged from earlier
    /// versions.
    ///
    /// `parcel_id` = a specific parcel: scoped to `Plaza`s overlapping that
    /// parcel. If there are none, falls through to `place_new_squares` --
    /// raw-land placement, see the module doc comment's v0.5 section.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        let target_parcel: Option<&Parcel> = if parcel_id == "*" {
            None
        } else {
            Some(
                nbhd.parcels
                    .iter()
                    .find(|p| p.id == parcel_id)
                    .ok_or_else(|| format!("parcel {} not found", parcel_id))?,
            )
        };

        let plazas: Vec<&OpenSpace> = nbhd
            .open_space
            .iter()
            .filter(|o| o.kind == OpenSpaceKind::Plaza)
            .filter(|o| match target_parcel {
                None => true,
                Some(tp) => {
                    let origin = parcel_origin(tp);
                    let local_parcel = ring_to_local(&tp.polygon.outer, &origin);
                    let local_plaza_centroid = centroid(&ring_to_local(&o.polygon.outer, &origin));
                    point_in_polygon(local_plaza_centroid, &local_parcel)
                }
            })
            .collect();

        if plazas.is_empty() {
            if let Some(tp) = target_parcel {
                return place_new_squares(nbhd, tp, params, seed, self.source());
            }
            return Err("p61_small_public_squares: no Plaza-kind open space found. Run P95/P107 first, or pass a specific raw parcel_id to place new squares directly.".into());
        }

        let mut prng = Prng::new(seed);
        let mut new_open: Vec<OpenSpace> = Vec::new();
        let mut new_streets: Vec<Street> = Vec::new();
        let mut new_activity_nodes: Vec<ActivityNode> = Vec::new();
        let mut replaced_ids: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut n_partitioned = 0;
        let mut n_already_ok = 0;
        let mut n_skipped_tiny = 0;
        let mut n_squares_total = 0;
        let mut n_undecided_total = 0;
        let mut n_connectors_total = 0;
        let mut any_dropped_slivers = false;
        let mut any_capped = false;

        for plaza in &plazas {
            let plaza_area_m2 = plaza.polygon.area_m2();
            if plaza_area_m2 < params.min_meaningful_area_m2 {
                n_skipped_tiny += 1;
                continue;
            }

            let origin = LngLat::new(
                plaza.polygon.outer.iter().map(|p| p.lng).sum::<f64>() / plaza.polygon.outer.len() as f64,
                plaza.polygon.outer.iter().map(|p| p.lat).sum::<f64>() / plaza.polygon.outer.len() as f64,
            );
            let local = ring_to_local(&plaza.polygon.outer, &origin);
            if local.len() < 3 { continue; }

            let (min_pt, max_pt) = bbox(&local);
            let longer_side = (max_pt.x - min_pt.x).max(max_pt.y - min_pt.y);

            if longer_side <= params.max_dimension_m {
                n_already_ok += 1;
                steps.push(format!(
                    "{}: {:.1}m across, already within {:.1}m -- unchanged.",
                    plaza.id, longer_side, params.max_dimension_m
                ));
                continue;
            }

            // Oversized: break into several smaller connected squares
            // rather than shrinking the whole plaza and abandoning the
            // remainder.
            let raw_pieces = partition_plaza(&local, params.max_dimension_m);
            let mut squares: Vec<Vec<Pt2>> = Vec::new();
            // Real leftover geometry, not just a summed float -- P106
            // (Positive Outdoor Space) needs actual polygons to scan, so
            // every piece this operator declines to make a square out of
            // is kept and emitted as `OpenSpaceKind::Undecided` below,
            // instead of only being described in the trace text.
            let mut undecided_pieces: Vec<Vec<Pt2>> = Vec::new();
            for piece in raw_pieces {
                if piece.len() < 3 { continue; }
                let piece_area = area(&piece);
                if piece_area < params.min_meaningful_area_m2 {
                    undecided_pieces.push(piece);
                    continue;
                }
                // The grid guarantees compliance for a convex plaza, but
                // clipping against a non-convex boundary can distort a
                // cell -- re-check and, if needed, pull the outlier back
                // into compliance rather than silently emitting an
                // oversized "small square."
                let (pmin, pmax) = bbox(&piece);
                let longer = (pmax.x - pmin.x).max(pmax.y - pmin.y);
                let final_piece = if longer > params.max_dimension_m {
                    scale_toward_centroid(&piece, params.max_dimension_m / longer)
                } else {
                    piece
                };
                squares.push(final_piece);
            }
            let dropped_area: f64 = undecided_pieces.iter().map(|p| area(p)).sum();

            if squares.is_empty() {
                // Degenerate partition (e.g. a sliver-thin plaza where
                // every grid cell clipped to nothing meaningful) -- fall
                // back to shrinking the whole plaza rather than losing the
                // open space entirely. Known gap: the ring between the
                // original plaza and the shrunk square isn't computed here
                // (would need real polygon subtraction, not just convex
                // clipping) so it does NOT get emitted as Undecided the way
                // the normal partition path's leftovers do -- rare in
                // practice (no test exercises it on real geometry), said
                // plainly rather than silently narrowed.
                let factor = params.max_dimension_m / longer_side;
                squares.push(scale_toward_centroid(&local, factor));
                steps.push(format!(
                    "{}: partition degenerated to zero usable pieces -- fell back to a single shrunk square.",
                    plaza.id
                ));
            }

            // Cap at max_squares -- "a few," not exhaustive tiling. Keep the
            // largest-area candidates (maximizes usable public space for a
            // fixed count) and move whatever the cap left over into the
            // same real Undecided geometry as dropped slivers, rather than
            // tiling over the rest of a multi-acre plaza or discarding it.
            let max_squares = params.max_squares.round().max(1.0) as usize;
            let mut capped_area = 0.0;
            if squares.len() > max_squares {
                squares.sort_by(|a, b| area(b).partial_cmp(&area(a)).unwrap_or(std::cmp::Ordering::Equal));
                for extra in squares.split_off(max_squares) {
                    capped_area += area(&extra);
                    undecided_pieces.push(extra);
                }
            }

            let plaza_id = plaza.id.clone();
            for (idx, piece_local) in undecided_pieces.iter().enumerate() {
                let ring = local_to_ring(piece_local, &origin);
                new_open.push(OpenSpace {
                    id: format!("{plaza_id}_p61_undecided{idx}"),
                    polygon: street_smarts_core::geometry::Polygon::from_ring(ring),
                    kind: OpenSpaceKind::Undecided,
                });
            }
            for (idx, sq_local) in squares.iter().enumerate() {
                let ring = local_to_ring(sq_local, &origin);
                new_open.push(OpenSpace {
                    id: format!("{plaza_id}_p61_sq{idx}"),
                    polygon: street_smarts_core::geometry::Polygon::from_ring(ring),
                    kind: OpenSpaceKind::Plaza,
                });
                new_activity_nodes.push(activity_node_for_square(
                    format!("{plaza_id}_p61_sq{idx}_activity"), sq_local, &origin, &mut prng,
                ));
            }
            n_squares_total += squares.len();
            n_undecided_total += undecided_pieces.len();

            // Link the new squares with an honest MST backbone -- the same
            // "fewest edges that still connect everything" reasoning as
            // P52's PathNetwork, not a full mesh between every pair.
            let mut n_connectors = 0;
            if squares.len() > 1 {
                let centers_local: Vec<Pt2> = squares.iter().map(|s| centroid(s)).collect();
                let centers_wgs: Vec<LngLat> =
                    centers_local.iter().map(|c| local_to_lnglat(*c, &origin)).collect();
                let MstResult { mst_edges, .. } = kruskal_mst(&centers_local);
                for (i, j, _d) in &mst_edges {
                    new_streets.push(Street {
                        id: format!("{plaza_id}_p61_link_{i}_{j}"),
                        centerline: vec![centers_wgs[*i], centers_wgs[*j]],
                        classification: Some(StreetClassification::Pedestrian.to_label().into()),
                        row_width_m: Some(params.connector_width_m),
                        surface: Some("grass_pavers".into()),
                    });
                }
                n_connectors = mst_edges.len();
                n_connectors_total += n_connectors;
            }

            replaced_ids.push(plaza.id.clone());
            n_partitioned += 1;

            let mut leftover_note = String::new();
            if dropped_area > 1.0 {
                any_dropped_slivers = true;
                leftover_note.push_str(&format!(" ({:.0}m² too small to be a square, marked Undecided not fabricated as one)", dropped_area));
            }
            if capped_area > 1.0 {
                any_capped = true;
                leftover_note.push_str(&format!(
                    " (capped at {} square(s): {:.0}m² of otherwise-compliant candidate squares marked UNCOVERED/Undecided, not tiled over)",
                    max_squares, capped_area
                ));
            }
            steps.push(format!(
                "{}: {:.1}m across ({:.0}m²) -> split into {} connected square(s) linked by {} path segment(s){}.",
                plaza.id, longer_side, plaza_area_m2, squares.len(), n_connectors, leftover_note
            ));
        }

        if n_partitioned == 0 && n_already_ok == 0 {
            return Err(format!(
                "p61_small_public_squares: all {} plaza(s) were below min_meaningful_area_m2 -- nothing to evaluate.",
                n_skipped_tiny
            ));
        }

        steps.insert(0, format!(
            "{} plaza(s) already compliant, {} split into {} total square(s) + {} Undecided piece(s) ({} connector segments), {} too small to bother with.",
            n_already_ok, n_partitioned, n_squares_total, n_undecided_total, n_connectors_total, n_skipped_tiny
        ));

        let mut caveats = vec![
            "Connector segments are geometry-only path centerlines (same abstraction \
             PathNetwork uses), straight between square centroids -- not routed, and not \
             materialized with real edge treatment (colonnades, trees, level changes) the way \
             Alexander's own text calls for between linked squares.".into(),
            "Only breaks up oversized squares. Does not grow undersized ones -- doing that \
             would require claiming adjacent land this operator has no basis to take.".into(),
            "Partitioning uses a regular grid of Voronoi-seeded cells, clipped to the plaza's \
             real boundary. That's an honest first approximation, not a claim to match \
             Alexander's own organic, hand-drawn subdivisions.".into(),
            "Each square's ActivityNode is a real Prng jitter off its own centroid, not a real \
             'where paths cross' computation (P126's own literal instruction) -- no path-crossing- \
             a-square concept is checked here, only that it isn't placed dead-center.".into(),
        ];
        if any_dropped_slivers {
            caveats.push(
                "Some partitioned pieces were too small to count as a usable square -- emitted \
                 as real OpenSpaceKind::Undecided geometry (not a Plaza, not fabricated land \
                 use), so a future P106 (Positive Outdoor Space) check has actual polygons to \
                 scan instead of a bare number.".into()
            );
        }
        if any_capped {
            caveats.push(
                "At least one plaza had more compliant candidate squares than max_squares \
                 allows. The excess is emitted as real OpenSpaceKind::Undecided geometry, not \
                 tiled or fabricated as more squares -- real precedent (Barcelona's superilles, \
                 Lisbon's Alfama praças) places a small number of square nodes per reclaimed \
                 area, not one every 18m, so exhaustive tiling was the wrong default even though \
                 it was geometrically achievable. Which candidates survive the cap is decided by \
                 area alone (biggest first), not by where people would actually converge -- this \
                 operator has no model of that. The Undecided land, like dropped slivers, is a \
                 P106 (Positive Outdoor Space) question this version doesn't resolve, just makes \
                 scannable.".into()
            );
        }

        let trace = SubdivisionTrace {
            operator_name: "p61_small_public_squares".into(),
            operator_source: self.source(),
            headline: format!(
                "{} of {} plaza(s) exceeded {:.1}m and were split into {} connected square(s).",
                n_partitioned, plazas.len(), params.max_dimension_m, n_squares_total
            ),
            steps,
            caveats,
            seed,
            params: params.as_map(),
        };

        Ok(Subdivision {
            new_parcels: vec![],
            new_open_space: new_open,
            new_buildings: vec![],
            new_streets,
            new_activity_nodes,
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: replaced_ids,
            replaced_building_ids: vec![],
            entity_provenance: std::collections::BTreeMap::new(),
            trace,
        })
    }
}

fn parcel_origin(p: &Parcel) -> LngLat {
    LngLat::new(
        p.polygon.outer.iter().map(|q| q.lng).sum::<f64>() / p.polygon.outer.len() as f64,
        p.polygon.outer.iter().map(|q| q.lat).sum::<f64>() / p.polygon.outer.len() as f64,
    )
}

/// Raw-land placement mode: no existing `Plaza` overlaps `target_parcel`,
/// so place up to `max_squares` new compliant squares directly on it,
/// stamped at `max_dimension_m` and clipped to the parcel's real (possibly
/// non-convex) boundary, linked by an MST backbone -- same connector logic
/// the resize path already uses. Does NOT touch `target_parcel`'s own
/// polygon; P95's reserved-land subtraction is what makes building pads
/// build around these squares later, not anything this function does.
///
/// Delegates to `place_new_squares_n` with `n_target` taken straight from
/// `params.max_squares` -- the right thing when `apply()` is called
/// directly on a single parcel via the registry/web UI's "run one
/// operator" path. The corrected pipeline (`crate::pipeline`) does NOT use
/// this: it calls `place_new_squares_n` directly with a per-block count
/// allocated across the whole site, not `max_squares` repeated on every
/// block -- see that module's doc comment for why.
fn place_new_squares(
    nbhd: &Neighborhood,
    target_parcel: &Parcel,
    params: &P61Params,
    seed: u64,
    source: SourceCitation,
) -> Result<Subdivision, String> {
    let n_target = params.max_squares.round().max(1.0) as usize;
    place_new_squares_n(nbhd, target_parcel, n_target, params, seed, source)
}

/// Same as `place_new_squares`, but with the target square count passed in
/// explicitly instead of always taking it from `params.max_squares`. `pub`
/// so `crate::pipeline` can allocate a total "a few squares" budget across
/// an entire site's blocks (by area) instead of stamping `max_squares` full
/// squares onto every individual block.
///
/// Avoids land `nbhd.open_space` already reserves on this parcel (P37's
/// common land, chiefly) via `p95_building_complex::reserved_holes_for_part`
/// -- the same subtraction P95 does before seeding pads -- so a square
/// doesn't land directly on top of a cluster's common land. Seeds within
/// the single largest remaining piece after subtraction, not scattered
/// across every leftover fragment; with only one or two squares to place
/// per block that's a reasonable simplification, not an attempt to use
/// every last buildable scrap the way P95's per-piece loop does.
pub fn place_new_squares_n(
    nbhd: &Neighborhood,
    target_parcel: &Parcel,
    n_target: usize,
    params: &P61Params,
    seed: u64,
    source: SourceCitation,
) -> Result<Subdivision, String> {
    let origin = parcel_origin(target_parcel);
    let local_parcel = ring_to_local(&target_parcel.polygon.outer, &origin);
    if local_parcel.len() < 3 {
        return Err(format!(
            "p61_small_public_squares: parcel {} has degenerate geometry, nothing to place squares on.",
            target_parcel.id
        ));
    }

    let holes = crate::p95_building_complex::reserved_holes_for_part(nbhd, &local_parcel, &origin);
    let mut free_pieces = vec![local_parcel.clone()];
    for hole in &holes {
        free_pieces = free_pieces.into_iter().flat_map(|p| crate::planar::subtract_convex(&p, hole)).collect();
    }
    let placement_zone = free_pieces
        .into_iter()
        .max_by(|a, b| area(a).partial_cmp(&area(b)).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or_else(|| local_parcel.clone());
    if placement_zone.len() < 3 {
        return Err(format!(
            "p61_small_public_squares: parcel {} has no free land left after subtracting already-reserved land (e.g. P37 common land).",
            target_parcel.id
        ));
    }

    let mut prng = Prng::new(seed);
    // Jitter isn't exposed as its own P61 param (would be a knob nobody has
    // asked to tune yet) -- 0.5 matches the default other operators
    // (P37, P95) use for their own stratified seeding.
    let nodes = stratified_seeds(&placement_zone, n_target, 0.5, &mut prng);
    if nodes.is_empty() {
        return Err(format!(
            "p61_small_public_squares: parcel {} too small or too concave to place any squares on.",
            target_parcel.id
        ));
    }

    let half_side = params.max_dimension_m / 2.0;
    let mut new_open: Vec<OpenSpace> = Vec::new();
    let mut new_activity_nodes: Vec<ActivityNode> = Vec::new();
    let mut square_centers: Vec<Pt2> = Vec::new();
    let mut n_skipped_tiny = 0;
    for (idx, &node) in nodes.iter().enumerate() {
        let square_local = vec![
            Pt2::new(node.x - half_side, node.y - half_side),
            Pt2::new(node.x + half_side, node.y - half_side),
            Pt2::new(node.x + half_side, node.y + half_side),
            Pt2::new(node.x - half_side, node.y + half_side),
        ];
        let clipped = clip_to_polygon_largest(&square_local, &placement_zone);
        if clipped.len() < 3 || area(&clipped) < params.min_meaningful_area_m2 {
            n_skipped_tiny += 1;
            continue;
        }
        square_centers.push(centroid(&clipped));
        new_open.push(OpenSpace {
            id: format!("{}_p61_new_sq{idx}", target_parcel.id),
            polygon: street_smarts_core::geometry::Polygon::from_ring(local_to_ring(&clipped, &origin)),
            kind: OpenSpaceKind::Plaza,
        });
        new_activity_nodes.push(activity_node_for_square(
            format!("{}_p61_new_sq{idx}_activity", target_parcel.id), &clipped, &origin, &mut prng,
        ));
    }

    if new_open.is_empty() {
        return Err(format!(
            "p61_small_public_squares: every candidate square on parcel {} clipped to nothing usable (too close to the boundary, or too concave).",
            target_parcel.id
        ));
    }

    let mut new_streets: Vec<Street> = Vec::new();
    if square_centers.len() > 1 {
        let centers_wgs: Vec<LngLat> = square_centers.iter().map(|c| local_to_lnglat(*c, &origin)).collect();
        let MstResult { mst_edges, .. } = kruskal_mst(&square_centers);
        for (i, j, _d) in &mst_edges {
            new_streets.push(Street {
                id: format!("{}_p61_new_link_{i}_{j}", target_parcel.id),
                centerline: vec![centers_wgs[*i], centers_wgs[*j]],
                classification: Some(StreetClassification::Pedestrian.to_label().into()),
                row_width_m: Some(params.connector_width_m),
                surface: Some("grass_pavers".into()),
            });
        }
    }

    let steps = vec![format!(
        "{}: raw land, no existing plaza -- placed {} new compliant square(s) directly ({} candidate(s) too small after clipping to the real boundary), linked by {} connector(s).",
        target_parcel.id, new_open.len(), n_skipped_tiny, new_streets.len()
    )];

    let trace = SubdivisionTrace {
        operator_name: "p61_small_public_squares".into(),
        operator_source: source,
        headline: format!(
            "Placed {} new square(s) directly on raw parcel {} (no existing plaza to resize).",
            new_open.len(), target_parcel.id
        ),
        steps,
        caveats: vec![
            "Squares are placed at stratified-random node positions, not at real convergence \
             points (transit, entrances, street corners) -- this operator has no model of where \
             people would actually gather, same limitation the resize path's caveats already \
             state.".into(),
            "Does not modify the target parcel's own polygon. Downstream operators (P95) need to \
             read these squares back via reserved-land subtraction to actually build around them \
             -- this function only places geometry, it doesn't claim the land for real.".into(),
            "Connector segments are straight geometry-only centerlines between square centroids, \
             same abstraction PathNetwork and the resize path already use -- not routed, not \
             materialized with real edge treatment.".into(),
            "Seeds within the single largest piece of free land after subtracting whatever \
             nbhd.open_space already reserves on this parcel (P37's common land, chiefly) -- \
             assumes reserved holes are convex (same assumption reserved_holes_for_part already \
             makes for P95), and doesn't spread squares across every leftover fragment if the \
             subtraction splits the parcel into several.".into(),
            "Each square's ActivityNode is a real Prng jitter off its own centroid, not a real \
             'where paths cross' computation (P126's own literal instruction) -- no path-crossing- \
             a-square concept is checked here, only that it isn't placed dead-center.".into(),
        ],
        seed,
        params: params.as_map(),
    };

    Ok(Subdivision {
        new_parcels: vec![],
        new_open_space: new_open,
        new_buildings: vec![],
        new_streets,
        new_activity_nodes,
        replaced_parcel_ids: vec![],
        replaced_open_space_ids: vec![],
        replaced_building_ids: vec![],
        entity_provenance: std::collections::BTreeMap::new(),
        trace,
    })
}

/// Native `System` port of `place_new_squares_n` -- the real corrected
/// pipeline's actual P61 call site (`pipeline.rs`'s per-block loop calls
/// this free function directly, not `PatternOperator::apply`; see this
/// module's own "v0.6" doc section). Not a method on
/// `P61SmallPublicSquares` since `place_new_squares_n` itself isn't one --
/// same free-function shape, extended the same way `System`'s other native
/// ports are (see `system.rs`'s own module doc). Every street this
/// function emits gets the same constant `StreetClassification::Pedestrian`
/// -- no per-street branching to preserve, so (unlike `PathNetwork`'s two-
/// class MST/loop split) dual-write here is just: run the computation
/// unchanged, then tag every NEW street id with that one constant.
pub fn place_new_squares_n_native(
    world: &World,
    target_parcel: &Parcel,
    n_target: usize,
    params: &P61Params,
    seed: u64,
    source: SourceCitation,
) -> Result<World, String> {
    let nbhd = world.to_neighborhood();
    let sub = place_new_squares_n(&nbhd, target_parcel, n_target, params, seed, source)?;
    let new_street_ids: Vec<String> = sub.new_streets.iter().map(|s| s.id.clone()).collect();
    let new_nbhd = apply_subdivision(&nbhd, &sub);
    let mut new_world = World::from_neighborhood(&new_nbhd);
    for id in new_street_ids {
        new_world.street_classifications.insert(id, StreetClassification::Pedestrian);
    }
    Ok(new_world)
}
