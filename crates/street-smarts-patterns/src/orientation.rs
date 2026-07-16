//! Shared "which way does the public realm lie" geometry.
//!
//! Both `p221_natural_doors_and_windows` (door placement) and
//! `p127_intimacy_gradient` (the public-to-private depth axis) need the
//! same underlying fact about a building: which direction, among the real
//! streets and open space this pipeline already has, is "outward toward
//! the public realm." Before this module existed, that logic was a private
//! function duplicated inside `p221_natural_doors_and_windows` -- with
//! `p127_intimacy_gradient` now running BEFORE P221 in the pipeline
//! (Alexander's own numbering: 107 < 127 < 129 < 131 < 221, no reordering
//! needed), a second independent copy would have meant two operators
//! solving the identical problem with code that could silently drift
//! apart. This is the one shared implementation both now call.

use street_smarts_core::geometry::{haversine_m, LngLat};
use street_smarts_core::nir::{Building, Neighborhood};

/// Nearest point on any street centerline or open-space centroid to a
/// building's centroid -- real pipeline data, not a guess, for "which
/// direction is the public realm."
pub fn nearest_public_realm_point(nbhd: &Neighborhood, b: &Building) -> Option<LngLat> {
    let bc = b.polygon.centroid();
    let mut best: Option<(f64, LngLat)> = None;
    for s in &nbhd.streets {
        for p in &s.centerline {
            let d = haversine_m(&bc, p);
            if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, *p));
            }
        }
    }
    for o in &nbhd.open_space {
        let c = o.polygon.centroid();
        let d = haversine_m(&bc, &c);
        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, c));
        }
    }
    best.map(|(_, p)| p)
}
