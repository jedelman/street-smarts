//! Measures BuildingSolid::to_mesh() cost against the real, whole Military
//! Circle site (not a synthetic fixture) -- run this after touching
//! building_mesh.rs's SDF or suggested_voxel_size() to catch a regression
//! before it ships to a phone. Runs the real pattern-language pipeline
//! itself (street_smarts_patterns::pipeline::run_corrected_pipeline)
//! against data/eastside-baseline.json, so it's reproducible from a fresh
//! clone -- no external fixture file required.
//!
//! Usage: cargo run --release -p street-smarts-godot --example scale_probe
//! (run with --release; the debug build is 5-8x slower and will give a
//! misleading number, same trap this tool itself caught once already --
//! see the git history for crates/street-smarts-godot/src/building_mesh.rs
//! around the edge-bucketing and suggested_voxel_size changes).

use std::time::Instant;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_godot::building_mesh::BuildingSolid;
use street_smarts_patterns::pipeline::run_corrected_pipeline;

const REAL_PARCEL_ID: &str = "MILITARY_CIRCLE_ASSEMBLED";
const REAL_SEED: u64 = 42;

fn main() {
    let raw = std::fs::read_to_string("data/eastside-baseline.json")
        .expect("run from the repo root -- couldn't read data/eastside-baseline.json");
    let baseline: Neighborhood = serde_json::from_str(&raw).expect("parseable fixture");

    let pipeline_start = Instant::now();
    let nir = run_corrected_pipeline(&baseline, REAL_PARCEL_ID, REAL_SEED);
    println!(
        "pattern pipeline: {} buildings from {} parcels, took {:?}",
        nir.buildings.len(),
        nir.parcels.len(),
        pipeline_start.elapsed()
    );

    let origin = LngLat::new(
        (nir.bbox_wgs84[0] + nir.bbox_wgs84[2]) / 2.0,
        (nir.bbox_wgs84[1] + nir.bbox_wgs84[3]) / 2.0,
    );

    let mesh_start = Instant::now();
    let mut total_tris = 0;
    let mut voxel_sizes = Vec::new();
    for b in &nir.buildings {
        if let Some(solid) = BuildingSolid::from_building(b, &origin) {
            let voxel = solid.suggested_voxel_size();
            voxel_sizes.push(voxel);
            total_tris += solid.to_mesh(voxel).triangles.len();
        }
    }
    voxel_sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());

    println!(
        "meshing (adaptive voxel size): {} buildings, {total_tris} total triangles, took {:?}",
        voxel_sizes.len(),
        mesh_start.elapsed()
    );
    println!(
        "  voxel sizes: min={:.3} median={:.3} max={:.3}",
        voxel_sizes[0],
        voxel_sizes[voxel_sizes.len() / 2],
        voxel_sizes[voxel_sizes.len() - 1]
    );
}
