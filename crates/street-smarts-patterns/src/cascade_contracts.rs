//! Cascade contracts — Alexander's own cross-reference DAG through *A
//! Pattern Language* (the graph patternlanguage.cc's own site renders),
//! operationalized as a checkable build artifact instead of something only
//! verified by hand, once, in a throwaway script.
//!
//! `language_graph::LANGUAGE` already encodes one slice of that DAG: which
//! GENERATOR stage must run before which other generator stage
//! (`requires`), so the pipeline's own execution order stays legal. That
//! table says nothing about whether a generator's real output actually
//! reaches a DETECTOR (`Opinion`) on the far side of one of Alexander's own
//! real cross-references -- e.g. P52 Network of Paths and Cars real
//! `Arterial` streets are what P59 Quiet Backs and P36 Degrees of
//! Publicness both need to have a real view at all. This module is that
//! other slice: a real, checkable claim that a specific generator's real
//! output moves a specific opinion's real score, cited against the SAME
//! Alexander pattern numbers (not invented relationships) each opinion's
//! own module doc already sources.
//!
//! Every entry here started as a one-off `examples/check_*.rs` script run
//! by hand during this project's Phase 5 §D generator work, then deleted
//! once the number was read into a commit message -- real, but not
//! permanent, and nothing would have caught a later regression. This
//! module and `tests/pattern_cascade.rs` make that permanent: one real
//! pipeline run against `data/eastside-baseline.json`, checked against
//! every contract below.
//!
//! `min_value` floors are deliberately set safely BELOW the real value
//! measured at the time each contract was added (see each entry's `why`
//! for the real number) -- future generator tuning can legitimately move
//! the number without breaking this test; only a real regression back
//! toward `NoView` or near-zero should ever trip it.
//!
//! # Where the graph itself comes from
//!
//! `data/apl-pattern-graph.json` is the checked-in canonical source: the
//! complete, real cross-reference graph for all 253 patterns in *A
//! Pattern Language*, structural data only (no copyrighted Problem/
//! Solution prose). Originally built by fetching individual
//! `patternlanguage.cc` pages one at a time (and briefly, for 4 patterns,
//! by unverified recall after their pages failed to load -- a real lapse,
//! caught and fixed the same day, see that file's own `_meta.history`);
//! now built by downloading `patternlanguage.cc`'s own precomputed Quartz
//! content-link index directly and parsing it programmatically, so there
//! is no model-summarization step and no hallucination surface for the
//! graph data itself. Every entry below cites a real edge from that graph
//! -- not an invented relationship.
//!
//! Two different contract STRENGTHS appear below, both real, labeled
//! honestly rather than blurred together:
//! - **Exclusive-producer contracts** (the original six): the named
//!   generator is the ONLY real producer of the specific field the
//!   opinion reads -- without it, the opinion is structurally `NoView`.
//! - **Real-cross-reference contracts** (the rest, added when this module
//!   was hydrated against the full graph): Alexander's own text connects
//!   these two pattern numbers, AND on the real fixture, the generator
//!   stage genuinely being present is why the opinion shows the value it
//!   does -- but other stages may also legitimately contribute, so this
//!   is a real, checkable claim without an exclusivity claim this project
//!   hasn't verified. Several patterns get a same-number self-referential
//!   contract (e.g. `p127_intimacy_gradient` generator -> `p127_intimacy_gradient`
//!   opinion) when the generator is literally the code that writes the
//!   exact field its own same-numbered opinion reads -- the same shape as
//!   `p197_thick_walls`'s own pair, just not called out as "exclusive"
//!   above because a handful of these fields (interior cell depth,
//!   connects_to, is_common, openings) are in practice only ever written
//!   by that one stage too, verified by reading each generator's own code
//!   rather than assumed from the pattern number match alone.

use street_smarts_core::nir::Neighborhood;

/// One real, checkable edge in Alexander's own pattern DAG: a generator
/// stage (`language_graph::LANGUAGE`'s own `id`) whose real output a
/// specific opinion (`Opinion::name`) depends on.
pub struct CascadeContract {
    /// The `language_graph::LANGUAGE` id of the generator stage this
    /// contract depends on.
    pub generator: &'static str,
    /// Alexander's own pattern number for that generator stage.
    pub generator_pattern: u32,
    /// The opinion (detector) this contract checks.
    pub opinion: &'static str,
    /// Alexander's own pattern number for that opinion.
    pub opinion_pattern: u32,
    /// What this contract actually checks on the real pipeline output.
    pub check: CascadeCheck,
    /// One-line citation of the real Alexander cross-reference this edge
    /// encodes, and the real value measured when this contract was added.
    pub why: &'static str,
}

pub enum CascadeCheck {
    /// The opinion must return a real `Value` (not `NoView`) at or above
    /// this floor.
    MinValue(f64),
    /// The opinion's own number doesn't move on the current real fixture
    /// (a fixture-scale fact, not a structural one -- see the contract's
    /// own `why`), so this checks a direct structural fact about the
    /// final `Neighborhood` instead, proving the generator's real
    /// capability without overclaiming what the opinion shows today.
    StructuralFact(fn(&Neighborhood) -> bool),
}

pub const CASCADE_CONTRACTS: &[CascadeContract] = &[
    CascadeContract {
        generator: "path_network",
        generator_pattern: 52,
        opinion: "p36_degrees_of_publicness",
        opinion_pattern: 36,
        check: CascadeCheck::StructuralFact(|n| {
            n.streets.iter().any(|s| s.classification.as_deref() == Some("arterial"))
        }),
        why: "path_network's 'v0.6' module doc: real Arterial streets close P36's own \
              structural gap (a 'busy' bucket used to be forced to 0 on every possible input). \
              On the real eastside fixture no building's single NEAREST street happens to be \
              the one reclassified edge, so p36's own value is still 0.000 there -- a \
              fixture-scale fact, not a structural one (see p36's own module doc) -- so this \
              checks the real structural fact directly: at least one real Arterial street exists.",
    },
    CascadeContract {
        generator: "path_network",
        generator_pattern: 52,
        opinion: "p59_quiet_backs",
        opinion_pattern: 59,
        check: CascadeCheck::MinValue(0.3),
        why: "p59_quiet_backs's own module doc: 'busy' collapses to Street.classification == \
              'arterial', the only real proxy for traffic volume this schema has -- needs \
              path_network's real Arterial streets to have any view at all. Measured on the \
              real fixture: NoView -> Value 0.667 (3 buildings front the real arterial; 67% \
              clear a 40m quiet back). Floor set safely below that.",
    },
    CascadeContract {
        generator: "path_network",
        generator_pattern: 52,
        opinion: "p53_main_gateways",
        opinion_pattern: 53,
        check: CascadeCheck::MinValue(0.5),
        why: "path_network's 'v0.5' module doc: the real convex-hull site-perimeter Boundary it \
              computes is the only producer of Neighborhood.boundaries in this pipeline -- \
              p53_main_gateways needs a real Boundary to have any view at all. Measured on the \
              real fixture: NoView -> Value 1.000 (the one real boundary has a real street \
              endpoint within 30m). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p61_small_public_squares",
        generator_pattern: 61,
        opinion: "p126_something_roughly_in_the_middle",
        opinion_pattern: 126,
        check: CascadeCheck::MinValue(0.02),
        why: "p61_small_public_squares is the only real producer of Neighborhood.activity_nodes \
              in this pipeline (see its own module doc) -- p126_something_roughly_in_the_middle \
              needs a real ActivityNode to have any view at all. Measured on the real fixture: \
              NoView -> Value 0.056 (89 real plazas checked; 6% have a real activity node near \
              their own centroid). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p197_thick_walls",
        generator_pattern: 197,
        opinion: "p197_thick_walls",
        opinion_pattern: 197,
        check: CascadeCheck::MinValue(0.9),
        why: "p197_thick_walls (generator) is the only real producer of \
              Building.wall_thickness_m -- p197_thick_walls (opinion) needs a real thickness \
              value to have any view at all. Measured on the real fixture: NoView -> Value \
              1.000 (35/35 real buildings clear the real 0.2m threshold). Floor set safely \
              below that.",
    },
    CascadeContract {
        generator: "p133_staircase_as_a_stage",
        generator_pattern: 133,
        opinion: "p195_staircase_volume",
        opinion_pattern: 195,
        check: CascadeCheck::MinValue(0.3),
        why: "p133_staircase_as_a_stage's own stair_width_m range was tightened to Alexander's \
              own literal P195 figure (2-5ft / 0.61-1.52m) specifically so p195_staircase_volume \
              (a DIFFERENT Alexander pattern number checking a real cross-reference: P133's \
              stair carving is what P195's own volume claim needs to have any view at all) \
              would clear it by construction. Measured on the real fixture: Value 0.559 (34 \
              real stair cells checked; 56% fall within the real range). Floor set safely below \
              that.",
    },
    // -- path_network (P52): apl-pattern-graph.json's own smaller-patterns
    // list for 52 is [23, 31, 49, 51, 54, 55, 120, 121]; larger-patterns
    // list includes [30, 32, 38, 100]. --
    CascadeContract {
        generator: "path_network",
        generator_pattern: 52,
        opinion: "p52_network_of_paths_and_cars",
        opinion_pattern: 52,
        check: CascadeCheck::MinValue(0.15),
        why: "Same-number self-pair: path_network is the generator that lays every real Street; \
              p52_network_of_paths_and_cars checks the real crossing geometry it produces. \
              Measured on the real fixture: Value 0.321 (5 real crossings; mean perpendicularity \
              0.32). Floor set safely below that.",
    },
    CascadeContract {
        generator: "path_network",
        generator_pattern: 52,
        opinion: "p49_looped_local_roads",
        opinion_pattern: 49,
        check: CascadeCheck::MinValue(0.8),
        why: "data/apl-pattern-graph.json: P52's own smaller-patterns list includes Looped Local \
              Roads (49). path_network's 'v0.3' module doc: local_loop_budget was added \
              specifically to close this pattern's real gap. Measured on the real fixture: Value \
              1.000 (1/1 connected component contains a real loop; 100% of streets clear \
              Alexander's 17-20ft width range). Floor set safely below that.",
    },
    CascadeContract {
        generator: "path_network",
        generator_pattern: 52,
        opinion: "p50_t_junctions",
        opinion_pattern: 50,
        check: CascadeCheck::MinValue(0.5),
        why: "path_network's own 'v0.4' module doc: degree-capped loop-edge selection was added \
              specifically to close P50's real gap (avoid four-way intersections). Measured on \
              the real fixture: Value 0.754 (4 real intersections, 100% clean three-way, no \
              four-way-or-more). Floor set safely below that.",
    },
    CascadeContract {
        generator: "path_network",
        generator_pattern: 52,
        opinion: "p51_green_streets",
        opinion_pattern: 51,
        check: CascadeCheck::MinValue(0.8),
        why: "data/apl-pattern-graph.json: P52's own smaller-patterns list includes Green Streets \
              (51). path_network is the only real producer of Street.surface. Measured on the \
              real fixture: Value 1.000 (8/8 Local/Pedestrian streets tagged grass_pavers). Floor \
              set safely below that.",
    },
    CascadeContract {
        generator: "path_network",
        generator_pattern: 52,
        opinion: "p100_pedestrian_street",
        opinion_pattern: 100,
        check: CascadeCheck::MinValue(0.1),
        why: "data/apl-pattern-graph.json: P52's own larger-patterns list includes Pedestrian \
              Street (100). Measured on the real fixture: Value 0.290 (2 real pedestrian streets; \
              mean entrance density 0.29 of the 3-per-100m target). Floor set safely below that.",
    },
    CascadeContract {
        generator: "path_network",
        generator_pattern: 52,
        opinion: "p120_paths_and_goals",
        opinion_pattern: 120,
        check: CascadeCheck::MinValue(0.4),
        why: "data/apl-pattern-graph.json: P52's own smaller-patterns list includes Paths and \
              Goals (120). Measured on the real fixture: Value 0.667 (9 real streets; 67% have a \
              real goal within 15m of both endpoints). Floor set safely below that.",
    },
    CascadeContract {
        generator: "path_network",
        generator_pattern: 52,
        opinion: "p121_path_shape",
        opinion_pattern: 121,
        check: CascadeCheck::MinValue(0.8),
        why: "data/apl-pattern-graph.json: P52's own smaller-patterns list includes Path Shape \
              (121). path_network's own bulge_centerline is the only real producer of a street's \
              midpoint bulge. Measured on the real fixture: Value 1.000 (9/9 real streets have a \
              real midpoint bulge). Floor set safely below that.",
    },
    // -- p61_small_public_squares (P61): apl-pattern-graph.json's own
    // larger-patterns list is [14, 30, 31, 41]; smaller-patterns list is
    // [106, 114, 122, 123, 124, 125, 126]. --
    CascadeContract {
        generator: "p61_small_public_squares",
        generator_pattern: 61,
        opinion: "p61_small_public_squares",
        opinion_pattern: 61,
        check: CascadeCheck::MinValue(0.03),
        why: "Same-number self-pair: p61_small_public_squares (generator) places every real \
              Plaza; p61_small_public_squares (opinion) checks its own real size distribution \
              against Alexander's ~18m diameter bound. Measured on the real fixture: Value 0.070 \
              (27/89 real plazas by area stay under the bound). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p61_small_public_squares",
        generator_pattern: 61,
        opinion: "p30_activity_nodes",
        opinion_pattern: 30,
        check: CascadeCheck::MinValue(0.02),
        why: "data/apl-pattern-graph.json: P61's own larger-patterns list includes Activity Nodes \
              (30). p61_small_public_squares is the only real producer of Neighborhood.activity_nodes \
              (see its own module doc). Measured on the real fixture: Value 0.062 (89 real plazas; \
              2% sit where 2+ real streets converge). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p61_small_public_squares",
        generator_pattern: 61,
        opinion: "p41_work_community",
        opinion_pattern: 41,
        check: CascadeCheck::MinValue(0.7),
        why: "data/apl-pattern-graph.json: P61's own larger-patterns list includes Work Community \
              (41). Measured on the real fixture: Value 1.000 (17/17 real party-wall-clustered \
              courtyard buildings sit within 30m of a real public square). Floor set safely below \
              that.",
    },
    CascadeContract {
        generator: "p61_small_public_squares",
        generator_pattern: 61,
        opinion: "p106_positive_outdoor_space",
        opinion_pattern: 106,
        check: CascadeCheck::MinValue(0.7),
        why: "data/apl-pattern-graph.json: P61's own smaller-patterns list includes Positive \
              Outdoor Space (106). Measured on the real fixture: Value 1.000 (96 real open-space \
              features, area-weighted mean convexity 1.00). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p61_small_public_squares",
        generator_pattern: 61,
        opinion: "p114_hierarchy_of_open_space",
        opinion_pattern: 114,
        check: CascadeCheck::MinValue(0.2),
        why: "data/apl-pattern-graph.json: P61's own smaller-patterns list includes Hierarchy of \
              Open Space (114). Measured on the real fixture: Value 0.485 (96 real open-space \
              features; 49% by area have a meaningfully larger neighbor within 100m). Floor set \
              safely below that.",
    },
    CascadeContract {
        generator: "p61_small_public_squares",
        generator_pattern: 61,
        opinion: "p122_building_fronts",
        opinion_pattern: 122,
        check: CascadeCheck::MinValue(0.4),
        why: "data/apl-pattern-graph.json: P61's own smaller-patterns list includes Building \
              Fronts (122). Measured on the real fixture: Value 0.714 (35 real buildings; 71% \
              build right up to the path). Floor set safely below that.",
    },
    // -- p95_building_complex (P95). --
    CascadeContract {
        generator: "p95_building_complex",
        generator_pattern: 95,
        opinion: "p95_building_complex",
        opinion_pattern: 95,
        check: CascadeCheck::MinValue(0.3),
        why: "Same-number self-pair: p95_building_complex (generator) places every real pad; \
              p95_building_complex (opinion) checks its own real complex quality. Measured on the \
              real fixture: Value 0.645 (3 real complexes found; scores 0.79/0.64/0.50). Floor \
              set safely below that.",
    },
    CascadeContract {
        generator: "p95_building_complex",
        generator_pattern: 95,
        opinion: "p99_main_building",
        opinion_pattern: 99,
        check: CascadeCheck::MinValue(0.7),
        why: "data/apl-pattern-graph.json: P95's own smaller-patterns list includes Main Building \
              (99). Measured on the real fixture: Value 0.995 (the tallest of 35 real buildings \
              sits 2m from the group's real centroid). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p95_building_complex",
        generator_pattern: 95,
        opinion: "p110_main_entrance",
        opinion_pattern: 110,
        check: CascadeCheck::MinValue(0.3),
        why: "data/apl-pattern-graph.json: P95's own smaller-patterns list includes Main Entrance \
              (110). Measured on the real fixture: Value 0.634 (35 real buildings; mean entrance \
              singularity 0.60, mean street-proximity 0.67). Floor set safely below that.",
    },
    // -- p96_number_of_stories (P96). --
    CascadeContract {
        generator: "p96_number_of_stories",
        generator_pattern: 96,
        opinion: "p21_four_story_limit",
        opinion_pattern: 21,
        check: CascadeCheck::MinValue(0.6),
        why: "data/apl-pattern-graph.json: P96's own larger-patterns list includes Four-Story \
              Limit (21). p96_number_of_stories is the real generator that assigns every pad's \
              target_stories, which P107 turns into real height. Measured on the real fixture: \
              Value 0.960 (93% of real floor area at or below 4 stories; the 2 real exceptions \
              are >=80m apart). Floor set safely below that.",
    },
    // -- p107_wings_of_light (P107). --
    CascadeContract {
        generator: "p107_wings_of_light",
        generator_pattern: 107,
        opinion: "p107_wings_of_light",
        opinion_pattern: 107,
        check: CascadeCheck::MinValue(0.05),
        why: "Same-number self-pair: p107_wings_of_light (generator) shapes every real building's \
              wing width; p107_wings_of_light (opinion) checks it against Alexander's real \
              15m double-loaded threshold. Measured on the real fixture: Value 0.143 (5/35 real \
              buildings clear the threshold; mean wing width 22.7m) -- a real, low, honestly \
              low score, not a NoView. Floor set safely below that.",
    },
    CascadeContract {
        generator: "p107_wings_of_light",
        generator_pattern: 107,
        opinion: "p105_south_facing_outdoors",
        opinion_pattern: 105,
        check: CascadeCheck::MinValue(0.3),
        why: "data/apl-pattern-graph.json: P107's own smaller-patterns list includes South Facing \
              Outdoors (105). Measured on the real fixture: Value 0.589 (96 real open-space \
              features; 53% have their nearest building to the north). Floor set safely below \
              that.",
    },
    CascadeContract {
        generator: "p107_wings_of_light",
        generator_pattern: 107,
        opinion: "p106_positive_outdoor_space",
        opinion_pattern: 106,
        check: CascadeCheck::MinValue(0.7),
        why: "data/apl-pattern-graph.json: P107's own smaller-patterns list includes Positive \
              Outdoor Space (106) -- P107's own real courtyards are one of the real open-space \
              features this opinion measures convexity against. Measured on the real fixture: \
              Value 1.000. Floor set safely below that.",
    },
    CascadeContract {
        generator: "p107_wings_of_light",
        generator_pattern: 107,
        opinion: "p122_building_fronts",
        opinion_pattern: 122,
        check: CascadeCheck::MinValue(0.4),
        why: "data/apl-pattern-graph.json: P107's own larger-patterns list includes Building \
              Fronts (122). Measured on the real fixture: Value 0.714 (35 real buildings; mean \
              setback 6.73m; 71% build right up to the path). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p107_wings_of_light",
        generator_pattern: 107,
        opinion: "p159_light_on_two_sides",
        opinion_pattern: 159,
        check: CascadeCheck::MinValue(0.5),
        why: "data/apl-pattern-graph.json: P107's own larger- AND smaller-patterns lists both \
              include Light on Two Sides of Every Room (159) -- P107's real solid/courtyard \
              massing is exactly what this opinion checks window-bearing wall length against. \
              Measured on the real fixture: Value 0.821 (82% of window-bearing wall length \
              qualifies). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p107_wings_of_light",
        generator_pattern: 107,
        opinion: "p160_building_edge",
        opinion_pattern: 160,
        check: CascadeCheck::MinValue(0.3),
        why: "data/apl-pattern-graph.json: P107's own larger-patterns list includes Building Edge \
              (160). P107's real solid/courtyard footprint shaping is what this opinion's shape \
              index measures. Measured on the real fixture: Value 0.657 (35 real buildings; mean \
              shape index 1.46; 66% exceed the 1.15 threshold). Floor set safely below that.",
    },
    // -- p108_connected_buildings (P108). --
    CascadeContract {
        generator: "p108_connected_buildings",
        generator_pattern: 108,
        opinion: "p108_connected_buildings",
        opinion_pattern: 108,
        check: CascadeCheck::MinValue(0.7),
        why: "Same-number self-pair: p108_connected_buildings (generator) merges pads separated \
              only by a construction joint; p108_connected_buildings (opinion) checks for pad \
              pairs that read as still-separate despite apparently touching. Measured on the real \
              fixture: Value 0.992 (5 of 595 real pad pairs read as still-separate). Floor set \
              safely below that.",
    },
    // -- p127_intimacy_gradient (P127). --
    CascadeContract {
        generator: "p127_intimacy_gradient",
        generator_pattern: 127,
        opinion: "p127_intimacy_gradient",
        opinion_pattern: 127,
        check: CascadeCheck::MinValue(0.03),
        why: "Same-number self-pair: p127_intimacy_gradient (generator) is the only real producer \
              of InteriorCell.depth ordering; p127_intimacy_gradient (opinion) checks whether \
              real front doors open into the most-public cell. Measured on the real fixture: \
              Value 0.095 (10% of 63 real front doors). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p127_intimacy_gradient",
        generator_pattern: 127,
        opinion: "p112_entrance_transition",
        opinion_pattern: 112,
        check: CascadeCheck::MinValue(0.15),
        why: "data/apl-pattern-graph.json: P127's own larger-patterns list includes Entrance \
              Transition (112). p127_intimacy_gradient is the real generator that carves the \
              entrance-bay cell this opinion measures the area share of. Measured on the real \
              fixture: Value 0.400 (35 real buildings with a real entrance cell; 40% fall in the \
              real 3-35% target). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p127_intimacy_gradient",
        generator_pattern: 127,
        opinion: "p191_shape_of_indoor_space",
        opinion_pattern: 191,
        check: CascadeCheck::MinValue(0.05),
        why: "data/apl-pattern-graph.json: P197's own larger-patterns list connects Thick Walls to \
              The Shape of Indoor Space (191); mechanically in this pipeline, p127_intimacy_gradient \
              is the real generator that partitions every InteriorCell this opinion measures \
              rectangularity against (its own 'v0.2' cardinal-snap depth axis was added \
              specifically to improve this real score). Measured on the real fixture: Value \
              0.122 (12% of 1119 real interior cells clear 75% rectangularity). Floor set safely \
              below that.",
    },
    // -- p129_common_areas_at_the_heart (P129). --
    CascadeContract {
        generator: "p129_common_areas_at_the_heart",
        generator_pattern: 129,
        opinion: "p129_common_areas_at_the_heart",
        opinion_pattern: 129,
        check: CascadeCheck::MinValue(0.8),
        why: "Same-number self-pair: p129_common_areas_at_the_heart (generator) is the only real \
              producer of InteriorCell.is_common; p129_common_areas_at_the_heart (opinion) checks \
              whether the real marked cell sits on the actual connectivity path. Measured on the \
              real fixture: Value 1.000 (35/35 real buildings). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p129_common_areas_at_the_heart",
        generator_pattern: 129,
        opinion: "p128_indoor_sunlight",
        opinion_pattern: 128,
        check: CascadeCheck::MinValue(0.05),
        why: "data/apl-pattern-graph.json: P129's own smaller-patterns list includes Indoor \
              Sunlight (128). p129_common_areas_at_the_heart is the real generator that marks \
              which cell is common, which this opinion checks for a real south-facing window. A \
              real, honestly LOW score (structural, per this project's own earlier investigation \
              -- see p128's own module doc) -- measured on the real fixture: Value 0.171 (6/35 \
              real buildings). Floor set safely below that, not claiming a fix that wasn't made.",
    },
    // -- p130_entrance_room (P130). --
    CascadeContract {
        generator: "p130_entrance_room",
        generator_pattern: 130,
        opinion: "p130_entrance_room",
        opinion_pattern: 130,
        check: CascadeCheck::MinValue(0.8),
        why: "Same-number self-pair: p130_entrance_room (generator) is the only real producer of \
              InteriorCell.kind == 'entrance'; p130_entrance_room (opinion) checks that every real \
              multi-cell building has exactly one, at the shallowest depth. Measured on the real \
              fixture: Value 1.000 (35/35 real buildings). Floor set safely below that.",
    },
    // -- p131_the_flow_through_rooms (P131). --
    CascadeContract {
        generator: "p131_the_flow_through_rooms",
        generator_pattern: 131,
        opinion: "p131_the_flow_through_rooms",
        opinion_pattern: 131,
        check: CascadeCheck::MinValue(0.01),
        why: "Same-number self-pair: p131_the_flow_through_rooms (generator) is the only real \
              producer of InteriorCell.connects_to; p131_the_flow_through_rooms (opinion) checks \
              for a real closed loop (no dead-end cells). A real, honestly LOW score on this real \
              fixture -- measured: Value 0.029 (1/35 real buildings achieve a real loop). Floor \
              set safely below that.",
    },
    // -- p133_staircase_as_a_stage (P133), in addition to its existing
    // p195_staircase_volume contract above. --
    CascadeContract {
        generator: "p133_staircase_as_a_stage",
        generator_pattern: 133,
        opinion: "p133_staircase_as_a_stage",
        opinion_pattern: 133,
        check: CascadeCheck::MinValue(0.7),
        why: "Same-number self-pair: p133_staircase_as_a_stage (generator) is the only real \
              producer of InteriorCell.kind == 'stair'; p133_staircase_as_a_stage (opinion) \
              checks that every real eligible multi-story building has one carved. Measured on \
              the real fixture: Value 0.971 (34/35 real eligible buildings). Floor set safely \
              below that.",
    },
    // -- p221_natural_doors_and_windows (P221). --
    CascadeContract {
        generator: "p221_natural_doors_and_windows",
        generator_pattern: 221,
        opinion: "p221_natural_doors_and_windows",
        opinion_pattern: 221,
        check: CascadeCheck::MinValue(0.8),
        why: "Same-number self-pair: p221_natural_doors_and_windows (generator) is the only real \
              producer of Building.openings; p221_natural_doors_and_windows (opinion) checks that \
              every real building has at least one opening and one door. Measured on the real \
              fixture: Value 1.000 (35/35 real buildings). Floor set safely below that.",
    },
    CascadeContract {
        generator: "p221_natural_doors_and_windows",
        generator_pattern: 221,
        opinion: "p164_street_windows",
        opinion_pattern: 164,
        check: CascadeCheck::MinValue(0.8),
        why: "data/apl-pattern-graph.json: P221's own smaller-patterns list includes Street \
              Windows (164). p221_natural_doors_and_windows is the only real producer of window \
              Openings. Measured on the real fixture: Value 1.000 (13/13 real buildings adjacent \
              to a Local/Pedestrian street have a real ground-floor window within 15m). Floor set \
              safely below that.",
    },
    CascadeContract {
        generator: "p221_natural_doors_and_windows",
        generator_pattern: 221,
        opinion: "p192_windows_overlooking_life",
        opinion_pattern: 192,
        check: CascadeCheck::MinValue(0.4),
        why: "data/apl-pattern-graph.json: P221's own smaller-patterns list includes Windows \
              Overlooking Life (192). p221_natural_doors_and_windows is the only real producer of \
              window Openings. Measured on the real fixture: Value 0.742 (1638 real ground-floor \
              windows checked; 74% have real street/open-space activity within 25m). Floor set \
              safely below that.",
    },
];
