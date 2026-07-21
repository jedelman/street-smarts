//! P107 Wings of Light — no interior point in a building should be far
//! from natural light; buildings should break into narrow wings, not
//! deep, undifferentiated masses.
//!
//! From Alexander, *A Pattern Language*, Pattern 107 (p. 524):
//! > **Problem:** Modern buildings are often shapes with no concern for
//! > natural light -- they depend almost entirely on artificial light.
//! > But buildings which displace natural light as the major source of
//! > illumination are not fit places to spend the day.
//! > **Solution:** Arrange each building so that it breaks down into
//! > wings which correspond, approximately, to the most important
//! > natural social groups within the building. Make each wing long and
//! > as narrow as you can -- never more than 25 feet wide.
//!
//! # A real, checkable shape proxy
//!
//! Alexander's literal number is 25ft (~7.5m) from an outside wall for a
//! wing lit from one side; `p107_wings_of_light.rs`'s own module doc
//! reasons a wing lit from BOTH long sides (a "double-loaded" wing) can
//! therefore be up to ~2x that, ~15m -- the exact same reasoning this
//! opinion checks against, not the generator's current parameter value
//! (so a regression in the generator's own default would still be
//! caught, same convention `p49_looped_local_roads.rs` established).
//!
//! For a SOLID building (`polygon.holes` empty): its local bounding-box
//! short side is the real, checkable proxy for wing width -- the same
//! metric the generator itself uses to decide whether a footprint is
//! already narrow enough.
//!
//! For a COURTYARD (ring) building (`polygon.holes` non-empty, one hole):
//! the ring's own width -- estimated as half the difference between the
//! outer footprint's and the inner hole's local bounding-box short sides
//! -- is the real proxy, since a ring building's OWN bbox is wide by
//! design; what matters is the ring band itself, not the whole footprint.
//!
//! `value` = fraction of buildings whose real wing width falls at or
//! under `MAX_WING_WIDTH_M` (15.0m).

use std::collections::BTreeMap;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P107WingsOfLightOpinion;

const MAX_WING_WIDTH_M: f64 = 15.0; // Alexander's 25ft (7.5m) one-side, doubled for a double-loaded wing

fn bbox_short_side_m(ring: &[LngLat]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let lat0 = ring.iter().map(|p| p.lat).sum::<f64>() / ring.len() as f64;
    let mlat = lat0.to_radians().cos();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in ring {
        let x = p.lng * mlat * 111_320.0;
        let y = p.lat * 110_540.0;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    (max_x - min_x).min(max_y - min_y)
}

/// The real wing-width proxy for one building: bbox short side for a
/// solid footprint, or estimated ring band width for a courtyard one.
fn wing_width_m(outer: &[LngLat], holes: &[Vec<LngLat>]) -> f64 {
    match holes.first() {
        None => bbox_short_side_m(outer),
        Some(hole) => {
            let outer_short = bbox_short_side_m(outer);
            let inner_short = bbox_short_side_m(hole);
            ((outer_short - inner_short) / 2.0).max(0.0)
        }
    }
}

impl Opinion for P107WingsOfLightOpinion {
    fn name(&self) -> &'static str {
        "p107_wings_of_light"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p107".into(),
            display: "Alexander et al., A Pattern Language, Pattern 107 (Wings of Light)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Wings-of-Light-(107)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        if n.buildings.is_empty() {
            return OpinionOutput::NoView {
                reason: "No buildings in this neighborhood -- run P107 Wings of Light first.".into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let widths: Vec<f64> = n.buildings.iter()
            .map(|b| wing_width_m(&b.polygon.outer, &b.polygon.holes))
            .collect();
        let n_within = widths.iter().filter(|&&w| w <= MAX_WING_WIDTH_M).count();
        let value = n_within as f64 / widths.len() as f64;

        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("within_max_wing_width".into(), value);

        let mean_width = widths.iter().sum::<f64>() / widths.len() as f64;
        let mut details: BTreeMap<String, String> = BTreeMap::new();
        details.insert("n_buildings".into(), n.buildings.len().to_string());
        details.insert("n_within_max_wing_width".into(), n_within.to_string());
        details.insert("mean_wing_width_m".into(), format!("{mean_width:.1}"));
        details.insert("max_wing_width_m".into(), format!("{MAX_WING_WIDTH_M:.1}"));

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} building(s); {}/{} ({:.0}%) have a real wing width at or under Alexander's \
                 double-loaded {MAX_WING_WIDTH_M:.0}m threshold -- mean wing width {mean_width:.1}m.",
                n.buildings.len(), n_within, widths.len(), value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Wing width is a bounding-box proxy (solid buildings) or an estimated symmetric \
                 ring-band width (courtyard buildings), not a true 'distance from every interior \
                 point to the nearest exterior wall' computation -- an irregular, non-rectangular \
                 footprint could have a misleading bbox short side.".into(),
                "The courtyard-ring estimate assumes the ring band is roughly symmetric all the \
                 way around (half the outer/inner bbox short-side difference) -- a real \
                 approximation for an off-center or non-convex courtyard.".into(),
                "Only buildings with a real polygon are checked; a pad P107 never reached (still \
                 tagged as a plain pad, not shaped into a Building) isn't counted here at all -- \
                 see p95_building_complex.rs's own opinion for whether pads got shaped in the \
                 first place.".into(),
                "Checked directly against real fixture output (clean_baseline, 37 buildings): only \
                 19% clear the threshold, mean wing width 21.3m against the 15m target -- not a \
                 detector bug. Most of the shortfall is `p107_solid_fallback_v01`-typed buildings: \
                 p107_wings_of_light.rs's own documented escape hatch, where a courtyard would have \
                 been too small to be real open space, so it falls back to a solid footprint rather \
                 than carve a token slit -- deliberately trading away the wing-width target in that \
                 case, not a mistake. A real candidate for future generator work (branching wings \
                 instead of a solid fallback), not addressed by this opinion.".into(),
            ],
            contributing_features: vec![],
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{Polygon, PolygonPart};
    use street_smarts_core::nir::{Building, NeighborhoodMeta};

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [0.0, 0.0, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P107 unit fixture".into(),
            },
        }
    }

    fn rect_ring(w: f64, h: f64) -> Vec<LngLat> {
        let m = 1.0 / 111_320.0;
        vec![
            LngLat::new(0.0, 0.0),
            LngLat::new(w * m, 0.0),
            LngLat::new(w * m, h * m),
            LngLat::new(0.0, h * m),
        ]
    }

    fn solid_building(id: &str, w: f64, h: f64) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(rect_ring(w, h)),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None,
            floors: None, openings: vec![], interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], }
    }

    #[test]
    fn no_buildings_is_no_view() {
        assert!(matches!(P107WingsOfLightOpinion.evaluate(&nbhd(vec![])), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_narrow_solid_building_clears_the_threshold() {
        // 40m long, 10m deep -- short side well under the 15m threshold.
        let n = nbhd(vec![solid_building("B1", 40.0, 10.0)]);
        let out = P107WingsOfLightOpinion.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["within_max_wing_width"] - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_deep_solid_block_fails_the_threshold() {
        // 40m x 40m -- a deep, undifferentiated mass, no wing shaping at all.
        let n = nbhd(vec![solid_building("B1", 40.0, 40.0)]);
        let out = P107WingsOfLightOpinion.evaluate(&n);
        match out {
            OpinionOutput::Value { sub_scores, .. } => {
                assert!((sub_scores["within_max_wing_width"] - 0.0).abs() < 1e-6);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_real_courtyard_ring_at_the_generators_own_carve_width_clears_the_threshold() {
        // Outer 60x60, inner hole 30x30 -- a symmetric ring band of (60-30)/2 = 15m,
        // exactly the generator's own max_wing_width_m carve depth.
        let outer = rect_ring(60.0, 60.0);
        let m = 1.0 / 111_320.0;
        let inner = vec![
            LngLat::new(15.0 * m, 15.0 * m),
            LngLat::new(45.0 * m, 15.0 * m),
            LngLat::new(45.0 * m, 45.0 * m),
            LngLat::new(15.0 * m, 45.0 * m),
        ];
        let b = Building {
            id: "B1".into(),
            polygon: Polygon::from_parts(vec![PolygonPart { outer, holes: vec![inner] }]),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None,
            floors: None, openings: vec![], interior_cells: vec![],
            wall_thickness_m: None,
            roof: None,
        canopies: vec![], roof_segments: vec![], wall_niches: vec![], };
        let out = P107WingsOfLightOpinion.evaluate(&nbhd(vec![b]));
        match out {
            OpinionOutput::Value { sub_scores, details, .. } => {
                assert!((sub_scores["within_max_wing_width"] - 1.0).abs() < 1e-6);
                // Real lng/lat round-tripping through the meter conversion
                // introduces small float noise (~14.9 vs the exact 15.0 the
                // ring geometry targets) -- check the real number with
                // tolerance, not an exact string.
                let mean: f64 = details["mean_wing_width_m"].parse().unwrap();
                assert!((mean - 15.0).abs() < 0.5, "got {mean}");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
