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

use crate::parameters::{ParamSpec, Parameters};
use crate::planar::{
    area, bbox, centroid, clip_to_polygon, kruskal_mst, local_to_lnglat, local_to_ring,
    ring_to_local, scale_toward_centroid, union_pieces, voronoi_cell, MstResult, Pt2,
};
use crate::subdivision::{PatternOperator, Subdivision, SubdivisionTrace};
use serde::{Deserialize, Serialize};
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Neighborhood, OpenSpace, OpenSpaceKind, Street};
use street_smarts_core::opinion::SourceCitation;

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
        "Break oversized plazas into several smaller connected squares within Alexander's ~18m intimacy threshold."
    }

    /// Operates on every `OpenSpace` of kind `Plaza` in the neighborhood.
    /// `parcel_id` is unused (this operator works on open space, not
    /// parcels) but kept for `PatternOperator` trait consistency; pass `"*"`.
    fn apply(
        &self,
        nbhd: &Neighborhood,
        _parcel_id: &str,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String> {
        let plazas: Vec<&OpenSpace> = nbhd
            .open_space
            .iter()
            .filter(|o| o.kind == OpenSpaceKind::Plaza)
            .collect();

        if plazas.is_empty() {
            return Err("p61_small_public_squares: no Plaza-kind open space found. Run P95/P107 first.".into());
        }

        let mut new_open: Vec<OpenSpace> = Vec::new();
        let mut new_streets: Vec<Street> = Vec::new();
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
                        classification: Some("pedestrian".into()),
                        row_width_m: Some(params.connector_width_m),
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
            replaced_parcel_ids: vec![],
            replaced_open_space_ids: replaced_ids,
            trace,
        })
    }
}
