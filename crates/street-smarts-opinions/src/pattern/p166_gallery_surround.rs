//! P166 Gallery Surround — porches, balconies, and arcades at a
//! building's edge should let people step out into the public world at
//! every story.
//!
//! From Alexander, *A Pattern Language*, Pattern 166 (p. 777), via
//! patternlanguage.cc/Patterns/Gallery-Surround-(166):
//! > **Problem:** If people cannot walk out from the building onto
//! > balconies and terraces which look toward the outdoor space around
//! > the building, then neither they themselves nor the people outside
//! > have any medium which helps them feel the building and the larger
//! > public world are intertwined.
//! > **Solution:** Whenever possible, and at every story, build porches,
//! > galleries, arcades, balconies, niches, outdoor seats, awnings,
//! > trellised rooms.
//!
//! # A real check, now that the schema supports it -- no generator yet
//!
//! Same real schema this pattern shares with `p119_arcades.rs`:
//! `Building.canopies` (`Vec<Canopy>`, `CanopyKind::Arcade | Gallery`,
//! each with a real `floor` number). No generator populates it yet, so on
//! every real fixture this pipeline ships today, `canopies` is empty
//! everywhere, and this opinion still returns `NoView`. What changed: the
//! reason is now "no generator populates this yet", not "the schema can't
//! represent this at all" -- and the check below is REAL, not a
//! placeholder, verified against synthetic fixtures in this file's own
//! tests.
//!
//! Unlike P119 Arcades (ground-floor only), this pattern's own literal
//! claim is "at every story" -- `value` = fraction of MULTI-story
//! buildings (`floors >= 2`) whose set of real canopy floors (either
//! `Arcade` or `Gallery` kind; Alexander's own solution text lists both
//! among the same "porches, galleries, arcades, balconies" family) covers
//! every story from 0 to `floors - 1`. Single-story buildings are
//! excluded from the denominator -- "every story" is vacuously trivial
//! for one story, not a real test of this pattern's own claim.

use std::collections::BTreeMap;
use street_smarts_core::nir::{Building, Neighborhood};
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, SourceCitation};
use street_smarts_core::timer::Timer;

pub struct P166GallerySurround;

fn covers_every_story(b: &Building) -> bool {
    let floors = b.floors.unwrap_or(1);
    if floors < 2 {
        return false; // excluded from the denominator by the caller; never reached as "ok"
    }
    (0..floors).all(|floor| b.canopies.iter().any(|c| c.floor == floor))
}

impl Opinion for P166GallerySurround {
    fn name(&self) -> &'static str {
        "p166_gallery_surround"
    }
    fn family(&self) -> OpinionFamily {
        OpinionFamily::Pattern
    }
    fn source(&self) -> SourceCitation {
        SourceCitation {
            id: "alexander_apl_p166".into(),
            display: "Alexander et al., A Pattern Language, Pattern 166 (Gallery Surround)".into(),
            url: Some("https://patternlanguage.cc/Patterns/Gallery-Surround-(166)".into()),
        }
    }
    fn value_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn evaluate(&self, n: &Neighborhood) -> OpinionOutput {
        let timer = Timer::start();

        let multi_story: Vec<_> = n.buildings.iter().filter(|b| b.floors.unwrap_or(1) >= 2).collect();
        if multi_story.is_empty() {
            return OpinionOutput::NoView {
                reason: "No multi-story building in this neighborhood -- 'at every story' is \
                         vacuously trivial for a single-story building, not a real test of this \
                         pattern's own claim."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }
        if multi_story.iter().all(|b| b.canopies.is_empty()) {
            return OpinionOutput::NoView {
                reason: "No building carries a real canopy yet -- Building.canopies exists in the \
                         schema (with a real per-floor `floor` number specifically for this \
                         pattern's own 'at every story' claim), but no generator populates it. See \
                         this opinion's own module doc."
                    .into(),
                runtime_ms: timer.elapsed_ms(),
            };
        }

        let mut n_surrounded = 0usize;
        let mut incomplete: Vec<String> = Vec::new();
        let mut details: BTreeMap<String, String> = BTreeMap::new();

        for b in &multi_story {
            let floors = b.floors.unwrap_or(1);
            let n_covered = (0..floors).filter(|&f| b.canopies.iter().any(|c| c.floor == f)).count();
            let ok = covers_every_story(b);
            details.insert(format!("{}.stories_covered", b.id), format!("{n_covered}/{floors}"));
            if ok {
                n_surrounded += 1;
            } else {
                incomplete.push(b.id.clone());
            }
        }

        let value = n_surrounded as f64 / multi_story.len() as f64;
        let mut sub_scores: BTreeMap<String, f64> = BTreeMap::new();
        sub_scores.insert("every_story_covered_fraction".into(), value);
        details.insert("n_multi_story_buildings".into(), multi_story.len().to_string());

        OpinionOutput::Value {
            value,
            method_summary: format!(
                "{} multi-story building(s) checked; {} ({:.0}%) have a real canopy (arcade or \
                 gallery) at every real story.",
                multi_story.len(), n_surrounded, value * 100.0
            ),
            sub_scores,
            details,
            caveats: vec![
                "Single-story buildings are excluded from the denominator -- 'at every story' is \
                 vacuously trivial for one story.".into(),
                "No real fixture this pipeline ships today produces a canopy of any kind -- this \
                 opinion returns NoView in practice until a real generator exists.".into(),
            ],
            contributing_features: incomplete,
            runtime_ms: timer.elapsed_ms(),
            model_version: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_smarts_core::geometry::{LngLat, Polygon};
    use street_smarts_core::nir::{Canopy, CanopyKind, NeighborhoodMeta};

    fn m() -> f64 { 111_320.0 }

    fn square_ring(half_side: f64) -> Vec<LngLat> {
        let s = half_side / m();
        vec![
            LngLat::new(-s, -s), LngLat::new(s, -s),
            LngLat::new(s, s), LngLat::new(-s, s), LngLat::new(-s, -s),
        ]
    }

    fn building(id: &str, floors: u32, canopies: Vec<Canopy>) -> Building {
        Building {
            id: id.into(),
            polygon: Polygon::from_ring(square_ring(10.0)),
            height_m: Some(9.0), typology: None, year_built: None, parcel_id: None,
            floors: Some(floors),
            openings: vec![], interior_cells: vec![], wall_thickness_m: None, roof: None,
            roof_segments: vec![], canopies, wall_niches: vec![],
        }
    }

    fn canopy_at(kind: CanopyKind, floor: u32) -> Canopy {
        Canopy { kind, ring_index: 0, on_hole: false, t_start: 0.1, t_end: 0.6, depth_m: 1.2, height_m: 2.4, floor }
    }

    fn nbhd(buildings: Vec<Building>) -> Neighborhood {
        Neighborhood {
            id: "test".into(), bbox_wgs84: [-0.01, -0.01, 0.01, 0.01],
            parcels: vec![], buildings, streets: vec![], open_space: vec![],
            boundaries: vec![], activity_nodes: vec![],
            metadata: NeighborhoodMeta {
                source: "synthetic".into(), fetched_at: "test".into(), license: "test".into(),
                layer_provenance: Default::default(), label: "P166 unit fixture".into(),
            },
        }
    }

    #[test]
    fn no_multi_story_buildings_is_no_view() {
        let n = nbhd(vec![building("B1", 1, vec![])]);
        assert!(matches!(P166GallerySurround.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn no_canopies_anywhere_is_no_view() {
        let n = nbhd(vec![building("B1", 3, vec![])]);
        assert!(matches!(P166GallerySurround.evaluate(&n), OpinionOutput::NoView { .. }));
    }

    #[test]
    fn a_canopy_at_every_story_scores_full() {
        let n = nbhd(vec![building("B1", 3, vec![
            canopy_at(CanopyKind::Arcade, 0), canopy_at(CanopyKind::Gallery, 1), canopy_at(CanopyKind::Gallery, 2),
        ])]);
        match P166GallerySurround.evaluate(&n) {
            OpinionOutput::Value { value, .. } => assert!((value - 1.0).abs() < 1e-9, "got {value}"),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_story_is_flagged_incomplete() {
        let n = nbhd(vec![building("PARTIAL", 3, vec![
            canopy_at(CanopyKind::Gallery, 0), canopy_at(CanopyKind::Gallery, 1),
        ])]);
        match P166GallerySurround.evaluate(&n) {
            OpinionOutput::Value { value, contributing_features, .. } => {
                assert!((value - 0.0).abs() < 1e-9, "got {value}");
                assert_eq!(contributing_features, vec!["PARTIAL".to_string()]);
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
