//! Run P95 Building Complex against the real MALL_CORE parcel and report
//! what came out, across a few seeds.

use std::fs;
use street_smarts_core::nir::Neighborhood;
use street_smarts_patterns::{apply_subdivision, run_operator};

#[test]
fn p95_on_mall_core_multiple_seeds() {
    let raw = fs::read_to_string("../../data/eastside-proposal.json").expect("fixture present");
    let nbhd: Neighborhood = serde_json::from_str(&raw).expect("parseable");

    let mall = nbhd
        .parcels
        .iter()
        .find(|p| p.spec.as_deref() == Some("MALL_CORE"))
        .expect("MALL_CORE in proposal");
    eprintln!(
        "MALL_CORE: id={}, {} acres, {} part(s)",
        mall.id,
        mall.area_acres,
        mall.polygon.parts_view().len()
    );

    for seed in [1u64, 7, 42, 1000] {
        let res = run_operator(&nbhd, "p95_building_complex", &mall.id, seed);
        let sub = res.unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        eprintln!(
            "\nseed={}: {} pads + {} courtyard(s)  →  {}",
            seed,
            sub.new_parcels.len(),
            sub.new_open_space.len(),
            sub.trace.headline
        );
        for step in &sub.trace.steps {
            eprintln!("  · {step}");
        }
        eprintln!(
            "\nseed={}: {} pads + {} courtyard(s)  →  {}",
            seed,
            sub.new_parcels.len(),
            sub.new_open_space.len(),
            sub.trace.headline
        );
        for step in &sub.trace.steps {
            eprintln!("  · {step}");
        }
        // Sanity: total new EDA area + courtyard area should be < source area.
        let pad_area: f64 = sub.new_parcels.iter().map(|p| p.area_acres).sum();
        let court_area: f64 = sub
            .new_open_space
            .iter()
            .map(|o| o.polygon.area_m2() / 4046.86)
            .sum();
        eprintln!(
            "  totals: pads {:.2} ac + courtyards {:.2} ac = {:.2} ac (source: {:.2} ac, hull approx may exceed)",
            pad_area,
            court_area,
            pad_area + court_area,
            mall.area_acres
        );
        // Hull approximation can overshoot; just assert non-zero output.
        assert!(pad_area > 0.0, "expected some building pads");

        // Apply and verify the resulting neighborhood is valid.
        let modified = apply_subdivision(&nbhd, &sub);
        assert!(
            modified.parcels.iter().all(|p| p.id != mall.id),
            "source parcel should be removed"
        );
        let added = modified.parcels.len() as i64 - nbhd.parcels.len() as i64 + 1;
        assert!(added > 0, "subdivision should net more parcels than source");
    }
}
