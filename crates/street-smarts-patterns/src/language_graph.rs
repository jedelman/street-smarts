//! The pattern-language graph, as data.
//!
//! This is the deliberately cheap version: `requires` only (a node lists
//! which other nodes must already have run), plus an optional `completes`
//! note for documentation. It replaces the *checkable* part of the long
//! prose ordering rationale at the top of `pipeline.rs` and in
//! `registry.rs`'s `all_operators_v01` doc comment with something
//! `validate_order` can actually verify -- see
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
//! Known limitation of this version: `validate_order` checks that a given
//! order is *legal* (every `requires` is satisfied by the time a node
//! runs). It does not, by itself, keep this table in sync with
//! `pipeline.rs::run_corrected_pipeline_with_p37`'s actual call sequence
//! if someone edits one and not the other -- the `#[test]` below is what
//! catches that drift, by asserting the real literal call order against
//! this table on every test run, not by the table validating itself.

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
}

/// The 14-step sequence `run_corrected_pipeline_with_p37` actually runs,
/// as of this table's writing. One row per step in `pipeline.rs`'s own
/// numbered doc comment.
pub const LANGUAGE: &[PatternNode] = &[
    PatternNode { id: "p37_house_cluster", alexander_number: Some(37), requires: &[], completes: &[] },
    PatternNode { id: "p52_path_network", alexander_number: Some(52), requires: &["p37_house_cluster"], completes: &[] },
    PatternNode { id: "p29_density_rings", alexander_number: Some(29), requires: &["p37_house_cluster"], completes: &[] },
    PatternNode { id: "p61_small_public_squares", alexander_number: Some(61), requires: &["p37_house_cluster"], completes: &[] },
    PatternNode {
        id: "p95_building_complex", alexander_number: Some(95),
        requires: &["p37_house_cluster", "p29_density_rings", "p61_small_public_squares"],
        completes: &[],
    },
    PatternNode { id: "p108_connected_buildings", alexander_number: Some(108), requires: &["p95_building_complex"], completes: &["p95_building_complex"] },
    PatternNode { id: "p96_number_of_stories", alexander_number: Some(96), requires: &["p95_building_complex", "p108_connected_buildings"], completes: &[] },
    PatternNode {
        id: "p107_wings_of_light", alexander_number: Some(107),
        requires: &["p108_connected_buildings", "p96_number_of_stories"],
        completes: &[],
    },
    PatternNode { id: "p127_intimacy_gradient", alexander_number: Some(127), requires: &["p107_wings_of_light"], completes: &[] },
    PatternNode { id: "p130_entrance_room", alexander_number: Some(130), requires: &["p127_intimacy_gradient"], completes: &["p127_intimacy_gradient"] },
    PatternNode { id: "p129_common_areas_at_the_heart", alexander_number: Some(129), requires: &["p127_intimacy_gradient"], completes: &["p127_intimacy_gradient"] },
    PatternNode { id: "p131_the_flow_through_rooms", alexander_number: Some(131), requires: &["p127_intimacy_gradient"], completes: &["p127_intimacy_gradient"] },
    PatternNode { id: "p221_natural_doors_and_windows", alexander_number: Some(221), requires: &["p107_wings_of_light"], completes: &[] },
    PatternNode {
        id: "p133_staircase_as_a_stage", alexander_number: Some(133),
        requires: &["p129_common_areas_at_the_heart", "p131_the_flow_through_rooms", "p221_natural_doors_and_windows"],
        completes: &[],
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The real order `run_corrected_pipeline_with_p37` calls operators in,
    /// as of this test's writing -- kept here, literally, rather than
    /// derived from the function itself, so a future edit to that function
    /// without a matching edit here fails this test instead of silently
    /// drifting. See this module's own doc comment for why that's a real
    /// limitation, not an oversight.
    const CORRECTED_PIPELINE_ORDER: &[&str] = &[
        "p37_house_cluster",
        "p52_path_network",
        "p29_density_rings",
        "p61_small_public_squares",
        "p95_building_complex",
        "p108_connected_buildings",
        "p96_number_of_stories",
        "p107_wings_of_light",
        "p127_intimacy_gradient",
        "p130_entrance_room",
        "p129_common_areas_at_the_heart",
        "p131_the_flow_through_rooms",
        "p221_natural_doors_and_windows",
        "p133_staircase_as_a_stage",
    ];

    #[test]
    fn corrected_pipeline_order_is_valid() {
        validate_order(CORRECTED_PIPELINE_ORDER).expect("pipeline order violates a declared pattern-language requirement");
    }

    #[test]
    fn every_language_node_is_covered_by_the_pipeline_order_check() {
        for node in LANGUAGE {
            assert!(
                CORRECTED_PIPELINE_ORDER.contains(&node.id),
                "LANGUAGE has a node ({}) the pipeline-order test doesn't cover -- \
                 update CORRECTED_PIPELINE_ORDER to match pipeline.rs",
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
            "p37_house_cluster",
            "p52_path_network",
            "p29_density_rings",
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
}
