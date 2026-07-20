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
];
