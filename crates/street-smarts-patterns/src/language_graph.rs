//! The pattern-language graph, as data.
//!
//! This is the deliberately cheap version: `requires` only (a node lists
//! which other nodes must already have run), plus an optional `completes`
//! note for documentation. It replaces the *checkable* part of the long
//! prose ordering rationale that used to live duplicated in both
//! `pipeline.rs`'s module header and `registry.rs`'s `all_operators_v01`
//! doc comment (two files' worth of near-identical prose describing the
//! same 21-step sequence -- exactly the "read-and-infer" cost
//! PATTERN_LANGUAGE_SIMULATION.md §3.4 named) with something
//! `validate_order` can actually verify. `pipeline.rs`'s header stays the
//! one authoritative narrative home (implementation detail, bug history,
//! specific thresholds); `registry.rs` now just points here. Run
//! `cargo run -p street-smarts-patterns --example dump_pattern_language`
//! for the generated, always-in-sync version of that ordering doc -- see
//! PATTERN_LANGUAGE_SIMULATION.md §3.4/§4.1.
//!
//! What this version deliberately does NOT have: `preserves`/`invalidates`
//! (the relation that would catch "pass X quietly breaks an assumption
//! pass Y already made," which is what most of the prose comments this
//! replaces are actually about -- P108 running before P96/P107 so
//! daylight-depth shaping doesn't see stale, about-to-be-merged pad
//! boundaries is exactly that class of bug). That's `PRIMITIVES_SPEC.md`
//! §5's pass manager, a deliberate upgrade of this same table, not a
//! separate thing to build from scratch -- see this crate's own
//! `IMPLEMENTATION_PLAN.md` Phase 5.
//!
//! Previously-known limitation of this version, now closed: `validate_order`
//! checks that a given order is *legal* (every `requires` is satisfied by
//! the time a node runs), but by itself doesn't keep this table in sync
//! with `pipeline.rs::run_corrected_pipeline_with_p37`'s actual call
//! sequence if someone edits one and not the other. The test below no
//! longer guards against that with a second, independently-maintained
//! literal order (which could itself silently drift) -- it calls
//! `run_corrected_pipeline_with_p37_traced`, which returns the REAL,
//! literal sequence of operator ids the pipeline actually executed
//! (respecting every `if let Ok(...)` skip), and validates that. There is
//! now exactly one place a future edit to the pipeline's order needs to
//! happen for this check to still mean something: the pipeline itself.

/// One node in the pattern language. `id` matches the string every
/// operator already uses as its own name (`PatternOperator::name` /
/// `DynOperator::name`) so this table can be checked against real
/// operator names, not a second parallel naming scheme.
#[derive(Debug, Clone, Copy)]
pub struct PatternNode {
    pub id: &'static str,
    pub alexander_number: Option<u32>,
    /// Other node ids that must already have run before this one.
    pub requires: &'static [&'static str],
    /// Other node ids this one is a component of / extends -- documentation
    /// only, not checked by `validate_order`. `None` where there isn't a
    /// clean single answer.
    pub completes: &'static [&'static str],
    /// One-line "why here" -- the queryable replacement for the prose
    /// ordering rationale in `pipeline.rs`/`registry.rs`. Not exhaustive
    /// (see `pipeline.rs`'s module doc for full detail, bug history, and
    /// specific numeric thresholds); this is the lookup-not-read version.
    pub why: &'static str,
}

/// The 21-step sequence `run_corrected_pipeline_with_p37` actually runs,
/// as of this table's writing. One row per step in `pipeline.rs`'s own
/// numbered doc comment. `id`s are checked against real operator names by
/// this module's own tests (`language_ids_match_real_operator_names`).
pub const LANGUAGE: &[PatternNode] = &[
    PatternNode {
        id: "p29_density_rings", alexander_number: Some(29), requires: &[], completes: &[],
        why: "computes a real density FIELD over the raw site parcel's own polygon -- Alexander's own true canonical position (29 < 37), needs nothing but the raw parcel every pipeline already starts with. See PATTERN_ORDERING_AUDIT.md for the real bug this closes (this used to run AFTER p37_house_cluster, tagging its blocks directly, purely because the old schema had nowhere else to attach an undivided-land value)",
    },
    PatternNode {
        id: "p37_house_cluster", alexander_number: Some(37), requires: &[], completes: &[],
        why: "carves the raw parcel into human-scaled blocks; every later stage operates on P37's BLOCK_n parcels, not the raw site. Samples p29_density_rings's own field (if present) at each new block's centroid to set density_tier/target_stories directly -- not a hard requirement (works fine, just untagged, if P29 didn't run), so not listed as a `requires` entry",
    },
    PatternNode {
        id: "path_network", alexander_number: Some(52), requires: &["p37_house_cluster"], completes: &[],
        why: "connects P37's blocks to each other; needs real BLOCK_n parcels to route between",
    },
    PatternNode {
        id: "p61_small_public_squares", alexander_number: Some(61), requires: &["p37_house_cluster"], completes: &[],
        why: "seeds a site-wide budget of public squares across P37's blocks, before P95 builds pads around whatever land is left",
    },
    PatternNode {
        id: "p95_building_complex", alexander_number: Some(95),
        requires: &["p37_house_cluster", "p29_density_rings", "p61_small_public_squares"],
        completes: &[],
        why: "builds pads around whatever P37/P61 left on each block; each pad inherits its block's P29 density tier",
    },
    PatternNode {
        id: "p21_four_story_limit", alexander_number: Some(21),
        requires: &["p95_building_complex"],
        completes: &[],
        why: "caps every pad's field-inherited target at the ordinary ceiling -- a pure per-pad read that needs nothing from p108_connected_buildings, so it runs right after p95_building_complex, before P108 merges any pads (PATTERN_ORDERING_AUDIT.md §4.2, splitting this half out of what used to be a single p96_number_of_stories pass)",
    },
    PatternNode {
        id: "p108_connected_buildings", alexander_number: Some(108), requires: &["p95_building_complex"], completes: &["p95_building_complex"],
        why: "merges P95 pads separated only by a construction joint into one footprint, before P96/P107 read pad geometry -- they'd see stale, about-to-be-merged boundaries otherwise",
    },
    PatternNode {
        id: "p96_number_of_stories", alexander_number: Some(96),
        requires: &["p95_building_complex", "p108_connected_buildings"],
        completes: &[],
        why: "picks the very few pads that get to exceed P21's ordinary cap, \"placed with great care... widely spaced\" -- genuinely needs P108's final merged footprint to rank and space them (PATTERN_ORDERING_AUDIT.md §4.2), unlike P21's own ordinary-cap half above. Doesn't strictly require p21_four_story_limit to have run (falls back to its own default_target_stories the same way it always has if P21 didn't), so not listed as a requires entry",
    },
    PatternNode {
        id: "p107_wings_of_light", alexander_number: Some(107),
        requires: &["p108_connected_buildings", "p96_number_of_stories"],
        completes: &[],
        why: "shapes every pad for daylight depth using P96's story count for real height; needs P108's merged footprints and P96's story assignment",
    },
    PatternNode {
        id: "p117_sheltering_roof", alexander_number: Some(117),
        requires: &["p107_wings_of_light"],
        completes: &[],
        why: "assigns every real P107/P96 building with a real height_m a real shed roof sloped to true north; also closes P162 North Face's own specifically-north claim by the same geometry (P162 has no separate generator of its own to list here) -- needs real height_m, which P107 already guarantees (via P96's story count or its own assumed_height_m fallback). No real dependency on P124/P127/P197 either direction (PATTERN_ORDERING_AUDIT.md §4.3/§4.4: a free reorder) -- placed here to match Alexander's own ascending numbering (117 < 124) at zero cost",
    },
    PatternNode {
        id: "p118_roof_garden", alexander_number: Some(118),
        requires: &["p117_sheltering_roof"],
        completes: &[],
        why: "overwrites P117's sloped shed roof with a flat, occupiable one on the site's tallest buildings, ranked by real Building.floors -- P107 derives floors itself now (PATTERN_ORDERING_AUDIT.md §4.7), transitively guaranteed the moment p117_sheltering_roof (which itself requires p107_wings_of_light) has run, so this no longer needs p221_natural_doors_and_windows -- placed right after P117 to match Alexander's own ascending numbering (117 < 118) at zero real cost",
    },
    PatternNode {
        id: "p124_activity_pockets", alexander_number: Some(124),
        requires: &["p107_wings_of_light", "p61_small_public_squares"],
        completes: &[],
        why: "carves a real pocket from buildings bordering a real Plaza; needs P107's real final footprints and P61's real Plazas, and must run before P127/P197/P119/P221 read the final footprint",
    },
    PatternNode {
        id: "p119_arcades", alexander_number: Some(119),
        requires: &["p107_wings_of_light"],
        completes: &[],
        why: "places a real ground-floor arcade (P119) plus a gallery at every real upper story (P166) on the building's street/open-space-facing wall; needs the FINAL footprint (whatever P124 left it as) and real Building.floors, transitively guaranteed via p107_wings_of_light -- no longer needs p221_natural_doors_and_windows (PATTERN_ORDERING_AUDIT.md §4.7). Doesn't strictly REQUIRE p124_activity_pockets to have run -- same real reason p197_thick_walls doesn't, see that node's own why: P124 is a real but skippable step (currently a no-op on the real eastside-baseline fixture), just needs to run after it if it did, which pipeline.rs's own real call order already guarantees",
    },
    PatternNode {
        id: "p127_intimacy_gradient", alexander_number: Some(127), requires: &["p107_wings_of_light"], completes: &[],
        why: "partitions each P107 building's ground floor into a depth-ordered cell sequence; needs real building footprints. No real dependency on P197 either direction (PATTERN_ORDERING_AUDIT.md §4.4: a free reorder) -- placed here to match Alexander's own ascending numbering (127 < 197) at zero cost",
    },
    PatternNode {
        id: "p197_thick_walls", alexander_number: Some(197), requires: &["p107_wings_of_light"], completes: &[],
        why: "assigns every real P107/P124 building a real wall_thickness_m, capped relative to its own footprint; every downstream stage clones-and-mutates from here, so the field survives untouched. Doesn't strictly REQUIRE p124_activity_pockets to have run (P124 is a real but skippable step, not every fixture has a building close enough to a Plaza to qualify) -- just needs to run after it if it did, which pipeline.rs's own real call order already guarantees",
    },
    PatternNode {
        id: "p116_cascade_of_roofs", alexander_number: Some(116),
        requires: &["p117_sheltering_roof", "p127_intimacy_gradient"],
        completes: &[],
        why: "partitions each P117 whole-building roof into per-cell RoofSegments whose ridge height cascades with P127's own real depth gradient; needs both P117's real roof (possibly P118-flattened by now) and P127's real interior_cells to exist first",
    },
    PatternNode {
        id: "p129_common_areas_at_the_heart", alexander_number: Some(129), requires: &["p127_intimacy_gradient"], completes: &["p127_intimacy_gradient"],
        why: "marks the P127 cell nearest the plan's center of gravity; needs P127's cells. No real dependency on P130 either direction (PATTERN_ORDERING_AUDIT.md §4.6 -- P130 never changes cell geometry or count) -- placed here to match BOTH Alexander's own ascending numbering (129 < 130) AND his own cited textual sequence (127 -> 128 -> 129 -> 130 -> 131...) at zero cost",
    },
    PatternNode {
        id: "p130_entrance_room", alexander_number: Some(130), requires: &["p127_intimacy_gradient"], completes: &["p127_intimacy_gradient"],
        why: "labels the P127 cell at depth 0.0 as the entrance -- a label only, needs P127's cells to exist",
    },
    PatternNode {
        id: "p131_the_flow_through_rooms", alexander_number: Some(131), requires: &["p127_intimacy_gradient"], completes: &["p127_intimacy_gradient"],
        why: "connects P127's cells into a chain or loop; needs P127's cells to connect",
    },
    PatternNode {
        id: "p133_staircase_as_a_stage", alexander_number: Some(133),
        requires: &["p129_common_areas_at_the_heart", "p131_the_flow_through_rooms"],
        completes: &[],
        why: "carves a stair core from the common-area cell of multi-story buildings; needs P129/P131's cell structure to carve from and real Building.floors, transitively guaranteed via p107_wings_of_light -- no longer needs p221_natural_doors_and_windows (PATTERN_ORDERING_AUDIT.md §4.7), restoring Alexander's own exact canonical position (131 < 133)",
    },
    PatternNode {
        id: "p221_natural_doors_and_windows", alexander_number: Some(221), requires: &["p107_wings_of_light"], completes: &[],
        why: "places real window/door openings using P107's building geometry; reads (not writes) the real Building.floors P107 already derived, for its own per-floor window-shrinking rule",
    },
    PatternNode {
        id: "p160_building_edge", alexander_number: Some(160),
        requires: &["p221_natural_doors_and_windows"],
        completes: &[],
        why: "places a real wall niche flanking every real door P221 already placed, on the same wall edge; needs P221's real door Opening data to flank -- a genuine, non-free dependency (PATTERN_ORDERING_AUDIT.md §4.8), unlike P118/P119/P133 above",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderViolation {
    pub node: &'static str,
    pub missing_requirement: &'static str,
}

/// Check that `order` (a sequence of node ids) satisfies every node's
/// `requires` -- each requirement must already be in the "available" set
/// (every node run so far) by the time its dependent runs. Unknown ids in
/// `order` are ignored (lets a caller pass a real pipeline's full call
/// list even if it includes a step not yet in `LANGUAGE`).
pub fn validate_order(order: &[&str]) -> Result<(), Vec<OrderViolation>> {
    let mut available: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut violations = Vec::new();
    for &id in order {
        if let Some(node) = LANGUAGE.iter().find(|n| n.id == id) {
            for req in node.requires {
                if !available.contains(req) {
                    violations.push(OrderViolation { node: node.id, missing_requirement: req });
                }
            }
        }
        available.insert(id);
    }
    if violations.is_empty() { Ok(()) } else { Err(violations) }
}

/// Render `LANGUAGE` as a human-readable, always-in-sync ordering doc --
/// the generated documentation PATTERN_LANGUAGE_SIMULATION.md §4.1 (b)
/// describes, used by both `examples/dump_pattern_language.rs` and (in
/// short form) `registry.rs`'s own doc comment.
pub fn render_language_doc() -> String {
    let mut out = String::new();
    for (i, node) in LANGUAGE.iter().enumerate() {
        let num = node
            .alexander_number
            .map(|n| format!("P{n}"))
            .unwrap_or_else(|| "(unnumbered)".to_string());
        out.push_str(&format!("{}. {} [{}]\n", i + 1, node.id, num));
        if node.requires.is_empty() {
            out.push_str("   requires: (nothing -- runs first)\n");
        } else {
            out.push_str(&format!("   requires: {}\n", node.requires.join(", ")));
        }
        if !node.completes.is_empty() {
            out.push_str(&format!("   completes: {}\n", node.completes.join(", ")));
        }
        out.push_str(&format!("   why: {}\n", node.why));
    }
    out
}

/// The same real `LANGUAGE` order as `render_language_doc`, collapsed to a
/// compact "P37 -> P52 -> ... " arrow chain -- what a caption naming the
/// pipeline's own real stage order actually wants (`render_language_doc`'s
/// full requires/why detail is the wrong shape for a one-line summary).
/// `examples/dump_pipeline_chain.rs` prints exactly this, so a build step
/// (`scripts/vibe-render.sh`) can substitute the real, current order into
/// `public/index.html` at deploy time instead of a hand-typed string that
/// silently stops matching the real pipeline the moment a stage is added,
/// removed, or reordered -- confirmed happening for real (this function
/// was added specifically because `index.html`'s own caption had drifted:
/// missing P124/P117/P197 even before P118/P119/P160 existed).
pub fn render_arrow_chain() -> String {
    LANGUAGE
        .iter()
        .map(|node| node.alexander_number.map(|n| format!("P{n}")).unwrap_or_else(|| node.id.to_string()))
        .collect::<Vec<_>>()
        .join(" \u{2192} ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p37_house_cluster::P37Params;
    use crate::pipeline::run_corrected_pipeline_with_p37_traced;
    use crate::Parameters;

    #[test]
    fn language_ids_match_real_operator_names() {
        // Guards against exactly the drift this table warns about in its
        // own doc comment: an id here that doesn't match any real
        // operator's `PatternOperator::name()` would silently never be
        // satisfiable by a real pipeline trace. Cross-checked against
        // `registry.rs::all_operators_v01` (the actual operator instances)
        // rather than a third hand-copied name list.
        use crate::registry::all_operators_v01;
        let real_names: std::collections::HashSet<&str> =
            all_operators_v01().iter().map(|op| op.name()).collect();
        for node in LANGUAGE {
            assert!(
                real_names.contains(node.id),
                "LANGUAGE node {:?} doesn't match any real operator's name() -- \
                 real names are: {real_names:?}",
                node.id
            );
        }
    }

    #[test]
    fn corrected_pipeline_real_traced_order_is_valid() {
        // The real, literal execution trace -- not a hand-copied guess at
        // what the function does. If a future edit to
        // `run_corrected_pipeline_with_p37` reorders a stage without a
        // matching `LANGUAGE` update, this is what catches it, by
        // validating what the pipeline actually did on a real fixture.
        let baseline = eastside_baseline_fixture();
        let (_nbhd, trace) = run_corrected_pipeline_with_p37_traced(
            &baseline,
            "MILITARY_CIRCLE_ASSEMBLED",
            42,
            &P37Params::defaults(),
        );
        validate_order(&trace).unwrap_or_else(|violations| {
            panic!("real pipeline trace violates a declared pattern-language requirement: {violations:?}")
        });
    }

    #[test]
    fn real_traced_order_covers_every_language_node() {
        let baseline = eastside_baseline_fixture();
        let (_nbhd, trace) = run_corrected_pipeline_with_p37_traced(
            &baseline,
            "MILITARY_CIRCLE_ASSEMBLED",
            42,
            &P37Params::defaults(),
        );
        // p124_activity_pockets is a real, deliberate exception: after it
        // was rewritten to bump OUTWARD (matching Alexander's own literal
        // "jut forward into the open space" text) rather than carve
        // inward, and after fixing a real edge-selection bug that let a
        // bump point away from its own plaza, this fixture's own real
        // candidates produce ZERO qualifying pockets -- 32/33 real
        // bordering buildings sit at only ~0.10m from their own plaza
        // edge (too tight for any real, non-degenerate depth), and the
        // one candidate with real room faces away from the plaza, not
        // toward it. Not a stale LANGUAGE entry or a broken generator --
        // see p124_activity_pockets's own module doc and
        // cascade_contracts.rs's note on why it has no self-pair contract
        // right now.
        let allowed_absent: std::collections::HashSet<&str> = ["p124_activity_pockets"].into_iter().collect();
        for node in LANGUAGE {
            if allowed_absent.contains(node.id) {
                continue;
            }
            assert!(
                trace.contains(&node.id),
                "LANGUAGE has a node ({}) the real pipeline trace never ran on the \
                 MILITARY_CIRCLE_ASSEMBLED fixture -- either LANGUAGE has a stale \
                 entry, or this fixture doesn't exercise every stage",
                node.id
            );
        }
    }

    #[test]
    fn reordering_p108_after_p96_p107_is_caught() {
        // The exact bug class pipeline.rs's own prose describes having
        // found and fixed by hand: P108 (which merges pads) must run
        // BEFORE P96/P107 read pad geometry, or they see stale boundaries
        // about to be erased. Confirm the graph would catch it if
        // reintroduced.
        let bad_order = &[
            "p29_density_rings",
            "p37_house_cluster",
            "path_network",
            "p61_small_public_squares",
            "p95_building_complex",
            "p96_number_of_stories",
            "p107_wings_of_light",
            "p108_connected_buildings", // moved to the end, wrong
        ];
        let result = validate_order(bad_order);
        assert!(result.is_err(), "expected reordering P108 after P96/P107 to be caught");
        let violations = result.unwrap_err();
        assert!(violations.iter().any(|v| v.node == "p96_number_of_stories" && v.missing_requirement == "p108_connected_buildings"));
    }

    #[test]
    fn arrow_chain_names_every_language_node_in_order() {
        let chain = render_arrow_chain();
        let parts: Vec<&str> = chain.split(" \u{2192} ").collect();
        assert_eq!(parts.len(), LANGUAGE.len(), "arrow chain should have exactly one entry per LANGUAGE node, got: {chain}");
        for (part, node) in parts.iter().zip(LANGUAGE) {
            let expected = node.alexander_number.map(|n| format!("P{n}")).unwrap_or_else(|| node.id.to_string());
            assert_eq!(*part, expected, "arrow chain entry doesn't match LANGUAGE's own real order");
        }
    }

    /// The real `eastside-baseline.json` fixture, used instead of a
    /// hand-built synthetic one so the traced-order tests exercise the
    /// same data the rest of the pipeline's test suite already trusts.
    fn eastside_baseline_fixture() -> street_smarts_core::nir::Neighborhood {
        let raw = std::fs::read_to_string("../../data/eastside-baseline.json")
            .expect("fixture present -- run from crates/street-smarts-patterns");
        serde_json::from_str(&raw).expect("parseable")
    }
}
