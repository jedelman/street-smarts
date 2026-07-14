//! Prototype: Alexander's own pattern numbering IS the intended sequence --
//! larger/more-fixed patterns first, smaller ones nested inside what came
//! before. The production pipeline currently runs P95 (Building Complex,
//! #95) before P52 (Network of Paths, #52) and P61 (Small Public Squares,
//! #61) -- backwards. This test does NOT fix the production pipeline (see
//! the tracked follow-up work for that); it's a standalone experiment,
//! reusing existing primitives, to check whether reordering actually helps
//! before committing to rewriting P95 around it.
//!
//! What this proves: placing P52 (a path skeleton) and P61 (a handful of
//! small squares, stamped directly at their own ~18m scale) FIRST on raw
//! parcel land produces real, non-Undecided geometry with zero leftover-land
//! bookkeeping -- because nothing here is "leftover." Every square is a
//! deliberate placement, not a Voronoi-cell remnant capped down after the
//! fact. Compare: the OLD order's P61 had to retrofit squares into P95's
//! single 12,494 m^2 "largest cell" courtyard, and even after capping,
//! P106 measured 71% of the mall's open space as still Undecided.
//!
//! What this does NOT prove yet: that P95 can be cleanly reworked to build
//! around this pre-placed land. This test runs P95 UNCHANGED, on the SAME
//! raw parcel, and measures how many of its own pads land near the
//! pre-placed squares -- a real conflict a subtraction-based P95 rework
//! would need to resolve, not a finished fix. Real general polygon
//! subtraction isn't in this codebase's toolkit yet; that's the actual
//! scope of the follow-up "rework P95" task, not something to fake here.

use street_smarts_core::geometry::{LngLat, Polygon};
use street_smarts_core::nir::{Neighborhood, OpenSpace, OpenSpaceKind, Street};
use street_smarts_patterns::p95_building_complex::{stratified_seeds, P95BuildingComplex, P95Params};
use street_smarts_patterns::planar::{
    area, clip_to_polygon, clip_to_polygon_largest, kruskal_mst, local_to_lnglat,
    local_to_ring, ring_to_local, MstResult, Pt2,
};
use street_smarts_patterns::prng::Prng;
use street_smarts_patterns::{Parameters, PatternOperator};

#[test]
fn prototype_p52_and_p61_before_p95_on_raw_parcel() {
    let raw = std::fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let parcel = baseline
        .parcels
        .iter()
        .find(|p| p.id == "00001129")
        .expect("mall parcel present in baseline fixture");

    let origin = LngLat::new(
        parcel.polygon.outer.iter().map(|p| p.lng).sum::<f64>() / parcel.polygon.outer.len() as f64,
        parcel.polygon.outer.iter().map(|p| p.lat).sum::<f64>() / parcel.polygon.outer.len() as f64,
    );
    let local_parcel = ring_to_local(&parcel.polygon.outer, &origin);
    assert!(local_parcel.len() >= 3, "parcel should have real geometry");

    let mut prng = Prng::new(42);

    // === P52 first: a real path skeleton on raw land, before any building. ===
    let path_nodes = stratified_seeds(&local_parcel, 6, 0.4, &mut prng);
    assert!(path_nodes.len() >= 3, "need enough path nodes for a meaningful skeleton, got {}", path_nodes.len());

    let MstResult { mst_edges, .. } = kruskal_mst(&path_nodes);
    let mut proto_streets: Vec<Street> = Vec::new();
    for (i, j, _d) in &mst_edges {
        proto_streets.push(Street {
            id: format!("proto_p52_link_{i}_{j}"),
            centerline: vec![local_to_lnglat(path_nodes[*i], &origin), local_to_lnglat(path_nodes[*j], &origin)],
            classification: Some("local".into()),
            row_width_m: Some(4.0),
        });
    }

    // === P61 first: a HANDFUL of path nodes become real, fixed-size small
    // squares -- stamped directly at Alexander's ~18m scale, not derived by
    // dividing the parcel by a seed count. This is the actual point: a
    // square this size was never going to emerge cleanly from a shared
    // Voronoi field sized for a dozen-odd buildings across dozens of
    // acres -- the average cell in that field is orders of magnitude too
    // big (see the real numbers printed below). It has to be placed
    // directly, at its own scale, independent of building-pad density.
    let square_half_side = 18.3 / 2.0;
    let n_squares = (path_nodes.len() / 2).clamp(1, 4);
    let mut proto_open: Vec<OpenSpace> = Vec::new();
    let mut square_polys_local: Vec<Vec<Pt2>> = Vec::new();
    for (idx, &node) in path_nodes.iter().take(n_squares).enumerate() {
        let square_local = vec![
            Pt2::new(node.x - square_half_side, node.y - square_half_side),
            Pt2::new(node.x + square_half_side, node.y - square_half_side),
            Pt2::new(node.x + square_half_side, node.y + square_half_side),
            Pt2::new(node.x - square_half_side, node.y + square_half_side),
        ];
        // clip_to_polygon_largest's result is guaranteed convex: it's the
        // largest piece from intersecting a convex subject against ONE
        // triangle of the clip polygon's triangulation, and convex ∩
        // triangle is always convex. Safe to reuse as a convex_subject below
        // for real overlap-area measurement against P95's pads.
        let clipped = clip_to_polygon_largest(&square_local, &local_parcel);
        if clipped.len() < 3 || area(&clipped) < 20.0 {
            continue;
        }
        proto_open.push(OpenSpace {
            id: format!("proto_p61_sq{idx}"),
            polygon: Polygon::from_ring(local_to_ring(&clipped, &origin)),
            kind: OpenSpaceKind::Plaza,
        });
        square_polys_local.push(clipped);
    }
    assert!(!proto_open.is_empty(), "should place at least one real square directly");
    for sq in &proto_open {
        assert!(sq.polygon.area_m2() <= 18.3 * 18.3 + 1.0, "every directly-stamped square should already comply -- nothing to cap");
    }

    // === P95, UNCHANGED, run on the SAME raw parcel. Not a fix -- a
    // measurement of the conflict a real rework needs to resolve. ===
    let p95 = P95BuildingComplex;
    let sub95 = p95
        .apply(&baseline, "00001129", &P95Params::defaults(), 42)
        .expect("P95 should still run on the unmodified parcel");

    // Real overlap AREA (via clip_to_polygon's actual polygon intersection),
    // not a coarse centroid-distance proxy -- measure the conflict, don't
    // estimate it.
    let mut overlap_pad_count = 0;
    let mut overlap_area = 0.0;
    for p in &sub95.new_parcels {
        let pad_local = ring_to_local(&p.polygon.outer, &origin);
        let mut pad_overlap = 0.0;
        for sq_poly in &square_polys_local {
            for piece in clip_to_polygon(sq_poly, &pad_local) {
                pad_overlap += area(&piece);
            }
        }
        if pad_overlap > 0.5 {
            overlap_pad_count += 1;
            overlap_area += pad_overlap;
        }
    }

    let parcel_area = area(&local_parcel);
    let squares_total_area: f64 = proto_open.iter().map(|o| o.polygon.area_m2()).sum();
    let old_courtyard_area: f64 = sub95.new_open_space.iter().map(|o| o.polygon.area_m2()).sum();
    let mean_p95_cell_area = parcel_area / (sub95.new_parcels.len() + sub95.new_open_space.len()) as f64;

    eprintln!("=== Pattern-order prototype: P52+P61 placed first vs. P95's old single courtyard ===");
    eprintln!("Raw parcel area: {:.0} m^2", parcel_area);
    eprintln!(
        "P52 (first): {} path nodes, {} MST street segment(s) -- real topology laid on raw land, zero area cost.",
        path_nodes.len(), proto_streets.len()
    );
    eprintln!(
        "P61 (first): {} real compliant square(s) stamped directly, {:.0} m^2 total, ZERO Undecided land -- nothing here is leftover, every square was placed on purpose.",
        proto_open.len(), squares_total_area
    );
    eprintln!(
        "P95 (OLD, unmodified, for comparison): {} pad(s) + {} courtyard(s) ({:.0} m^2) from a shared Voronoi field averaging {:.0} m^2/cell -- {}x too big for an 18.3m square to emerge from that field on its own.",
        sub95.new_parcels.len(), sub95.new_open_space.len(), old_courtyard_area, mean_p95_cell_area,
        (mean_p95_cell_area / (18.3 * 18.3)).round() as i64
    );
    eprintln!(
        "Conflict a real P95 rework must resolve: {} of P95's {} pads actually overlap a pre-placed square, {:.0} m^2 of real polygon intersection -- land P95 would need to build around, not through (measured via clip_to_polygon's real intersection, not a centroid-distance guess).",
        overlap_pad_count, sub95.new_parcels.len(), overlap_area
    );
    eprintln!(
        "Compare to the OLD full pipeline's real result: once P61 retrofitted squares into P95's single leftover courtyard, P106 measured 71% of the mall's open space (17,166 of 24,210 m^2) as still Undecided. This prototype's P61 produces 0% Undecided by construction."
    );
}
