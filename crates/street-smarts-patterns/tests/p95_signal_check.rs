//! Cheap sanity check, precursor to the CMA-ES validation experiment.
//!
//! Question: does ANY registered opinion's score move when P95's parameter
//! vector moves from an obviously-good setting (the pattern's own intent —
//! break a monolithic parcel into a complex) to an obviously-bad one (force
//! a single monolithic building — the literal thing P95 exists to prevent)?
//!
//! This does NOT run CMA-ES. It's the thing to check before CMA-ES is worth
//! running at all: if scores don't move here, there's no gradient to climb.

use std::fs;
use street_smarts_conflict::build_report;
use street_smarts_core::nir::Neighborhood;
use street_smarts_opinions::evaluate_all;
use street_smarts_patterns::{apply_subdivision, Parameters, PatternOperator};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};

const MALL_PARCEL_ID: &str = "00001129";

fn score_summary(label: &str, n: &Neighborhood) {
    let evaluated = evaluate_all(n);
    let report = build_report(evaluated);
    eprintln!("\n========== {} ==========", label);
    eprintln!("Parcels: {}, Open space: {}", n.parcels.len(), n.open_space.len());
    eprintln!("Geometric chorus: {}", report.geometric_summary.headline);
    for ev in &report.opinions {
        if let street_smarts_core::opinion::OpinionOutput::Value { value, sub_scores, .. } = &ev.output {
            eprintln!("  {}: {:.3}  (sub-scores: {:?})", ev.opinion.name, value, sub_scores);
        } else {
            eprintln!("  {}: abstained", ev.opinion.name);
        }
    }
}

#[test]
fn p95_signal_check_good_vs_monolithic() {
    let raw = fs::read_to_string("../../data/eastside-baseline.json").expect("fixture present");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    assert!(
        baseline.parcels.iter().any(|p| p.id == MALL_PARCEL_ID),
        "expected mall parcel {} in fixture",
        MALL_PARCEL_ID
    );

    // "Good": P95's own defaults. Moderate density, real inset, largest-cell courtyard.
    let good_params = P95Params::defaults();

    // "Bad": as few buildings as the operator can actually produce.
    //
    // NOTE: min=max=1 was tried first and always fails — target_seeds =
    // n_buildings + 1, and stratified_seeds hard-requires >= 3 seeds, so
    // n_buildings must be >= 2. The operator's parameter space has a floor
    // at 2 buildings; it cannot currently express the single-monolithic-
    // building case that P95's own doc comment names as the thing to avoid.
    // min=max=2 is the closest reachable approximation.
    let monolithic_params = P95Params {
        min_buildings: 2.0,
        max_buildings: 2.0,
        ..P95Params::defaults()
    };

    let op = P95BuildingComplex;

    let good_sub = op
        .apply(&baseline, MALL_PARCEL_ID, &good_params, 42)
        .expect("good params should produce a subdivision");
    let bad_sub = op
        .apply(&baseline, MALL_PARCEL_ID, &monolithic_params, 42)
        .expect("monolithic params should still produce a subdivision");

    eprintln!(
        "\ngood: {} new parcels, {} open space",
        good_sub.new_parcels.len(),
        good_sub.new_open_space.len()
    );
    eprintln!(
        "monolithic: {} new parcels, {} open space",
        bad_sub.new_parcels.len(),
        bad_sub.new_open_space.len()
    );

    let good_nbhd = apply_subdivision(&baseline, &good_sub);
    let bad_nbhd = apply_subdivision(&baseline, &bad_sub);

    score_summary("GOOD (P95 defaults — decomposed complex)", &good_nbhd);
    score_summary("MONOLITHIC (min=max=1 building — the anti-pattern)", &bad_nbhd);

    // Round-trip sanity: as_vector/from_vector should reproduce params (this
    // is exactly the projection CMA-ES will rely on).
    // BUG FOUND: max_buildings' schema bounds are (3.0, 40.0, 14.0) while
    // min_buildings' are (1.0, 20.0, 3.0) — asymmetric floors. from_vector's
    // clamp silently pushes any max_buildings < 3 back up to 3, so the
    // vector-projection path (what CMA-ES would actually search over) can
    // never express max_buildings in [1,2], even though direct struct
    // construction can. Documenting via a passing assertion on the ACTUAL
    // (buggy) behavior, not the behavior we want:
    let vec = monolithic_params.as_vector();
    let roundtrip = P95Params::from_vector(&vec);
    assert_eq!(roundtrip.min_buildings, 2.0, "min_buildings clamp ok");
    assert_eq!(
        roundtrip.max_buildings, 3.0,
        "max_buildings schema floor (3.0) silently overrides the requested 2.0 — \
         asymmetric with min_buildings' floor of 1.0. Fix schema bounds before CMA-ES."
    );
}
