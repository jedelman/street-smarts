//! Registry: list available operators and dispatch by name.

use crate::block_grouping::BlockGrouping;
use crate::building_shape::BuildingShape;
use crate::p29_density_rings::P29DensityRings;
use crate::p37_house_cluster::P37HouseCluster;
use crate::p95_building_complex::P95BuildingComplex;
use crate::p96_number_of_stories::P96NumberOfStories;
use crate::p107_wings_of_light::P107WingsOfLight;
use crate::p108_connected_buildings::P108ConnectedBuildings;
use crate::p127_intimacy_gradient::P127IntimacyGradient;
use crate::p129_common_areas_at_the_heart::P129CommonAreasAtTheHeart;
use crate::p130_entrance_room::P130EntranceRoom;
use crate::p131_the_flow_through_rooms::P131TheFlowThroughRooms;
use crate::p133_staircase_as_a_stage::P133StaircaseAsAStage;
use crate::p221_natural_doors_and_windows::P221NaturalDoorsAndWindows;
use crate::p61_small_public_squares::P61SmallPublicSquares;
use crate::path_network::PathNetwork;
use crate::parameters::ParamSpec;
use crate::subdivision::{DynOperator, Subdivision};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::SourceCitation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorInfo {
    pub name: String,
    pub description: String,
    pub source: SourceCitation,
    pub parameter_schema: Vec<ParamSpec>,
    pub default_params: JsonValue,
}

/// Returns metadata for all v0.1 operators (UI uses this to populate the picker
/// AND render parameter sliders).
pub fn available_operators() -> Vec<OperatorInfo> {
    let ops = all_operators_v01();
    ops.iter()
        .map(|op| OperatorInfo {
            name: op.name().to_string(),
            description: op.description().to_string(),
            source: op.source(),
            parameter_schema: op.parameter_schema(),
            default_params: op.default_params_json(),
        })
        .collect()
}

/// Construct boxed instances of all operators.
///
/// **Order matters, and it's Alexander's own pattern numbering**: larger,
/// more-fixed patterns first, smaller ones nested inside what came before.
/// The corrected pipeline (see `tests/corrected_pipeline.rs` for the real,
/// tested sequence, and the web UI's "Run full pipeline" button for the
/// same orchestration client-side) is:
///   1. P37 House Cluster (#37) -- carve the raw parcel into human-scaled
///      BLOCK_n sub-parcels. Runs ONCE, site-scale.
///   2. PathNetwork / P52 (#52) -- connect the blocks to each other. Runs
///      ONCE, site-scale, on the BLOCK_n parcels P37 produced.
///   3. P29 Density Rings (#29) -- tags each BLOCK_n with a density tier
///      and target story count from its own distance to the site's density
///      center. Runs ONCE, site-scale. (Numbered 29 in Alexander's own
///      sequence, well before House Cluster -- runs here instead because
///      this schema has nothing to tag until real blocks exist; see the
///      module's own doc comment for why.)
///   4. P61 (#61) -- a total of `max_squares` (default 4, Alexander's own
///      "a few") small squares spread across the SITE's blocks by area, not
///      `max_squares` repeated on every block (see p61's "v0.6" module doc
///      for why that was wrong). Most blocks get zero squares. Then within
///      EACH block: P95 (#95, reworked to build pads around whatever P61
///      placed on that block, if anything) -- pads inherit their source
///      block's density tier/target from P29.
///   5. P108 Connected Buildings (#108) -- merges pads separated by nothing
///      but P95's construction-joint-sized `pad_inset_m` (default 0.1m)
///      into one continuous party-wall footprint. Runs ONCE, site-scale,
///      BEFORE P96/P107 (Alexander numbers it after Wings of Light, but
///      daylight-depth shaping needs to see the real, final connected
///      footprint -- see the module's own doc comment for why).
///   6. P96 Number of Stories (#96) -- turns each pad's inherited tier
///      target into a real per-pad story count, capping ordinary buildings
///      at `max_ordinary_stories` (P21 Four-Story Limit) and allowing a
///      very few, widely-spaced exceptions where a tier's target exceeds
///      it. Runs ONCE, site-scale, over every `p95_building_pad`.
///   7. P107 (#107) -- daylight-depth building shape. Runs ONCE, site-scale,
///      after every block has its pads, since it already filters by
///      `use_category == "p95_building_pad"` across the whole neighborhood.
///      Reads each pad's `target_stories` (P96's assignment) to compute
///      real height, falling back to its own flat `assumed_height_m` for
///      any pad P96 didn't touch.
///   8. P127 Intimacy Gradient (#127) -- partitions each building's ground
///      floor into a depth-ordered sequence of cells (public wall/entrance
///      bay -> deepest point). Runs ONCE, site-scale, on `nbhd.buildings`
///      (the second operator in this crate to target buildings directly,
///      after P221 -- except this one now runs BEFORE P221, since
///      canonical numbering (107 < 127 < 129 < 131 < 221) needs no
///      reordering here, unlike P29/P108 above). See the module's own doc
///      comment for the sourced sequence (Alexander's own transition text
///      lists 127 -> 128 -> 129 -> 130 -> 131 -> 132 -> 133 explicitly).
///   9. P130 Entrance Room (#130) -- tags the cell P127 built at depth 0.0
///      as `kind: "entrance"`. Alexander's own cited sequence (127 -> 128
///      -> 129 -> 130 -> 131...) actually puts this AFTER P129, but this
///      operator never touches cell geometry or count, so nothing about
///      P129's center-of-gravity computation depends on run order here --
///      kept next to P127 (whose output it reads) instead of splitting a
///      tightly-coupled pair across P129. See the module's own doc for why
///      this is a label only, not a resize (P130's primary-source text
///      couldn't be reverified this session -- patternlanguage.com 404s
///      for this pattern).
///   10. P129 Common Areas at the Heart (#129) -- marks which of P127's
///      cells sits nearest the plan's center of gravity. Runs ONCE,
///      site-scale, right after P130.
///   11. P131 The Flow Through Rooms (#131) -- connects P127's cells into a
///      chain (solid buildings) or a closed loop (courtyard buildings get
///      one for free from their ring shape; solid buildings get a
///      loop-closing passage cell only when short and wide enough, per
///      Pattern 132's own cited length threshold -- folded into this
///      operator rather than a separate P132 one, see its module doc).
///   12. P221 (#221) -- real window/door openings. Runs ONCE, site-scale,
///      after every building exists. Reads each building's real height
///      (-> floor count) and footprint (-> wall segments) to place
///      openings; no randomness. Also the FIRST operator to populate
///      `Building.floors` itself (derived from real height) -- see the
///      struct field's own doc comment. See the module's own doc comment
///      for the pattern-graph reasoning (P107 -> P159 -> P221) and the
///      `street-smarts-opinions::pattern::p159_light_on_two_sides` opinion
///      that scores the result.
///   13. P133 Staircase as a Stage (#133) -- carves a real stair-core strip
///      out of the common-area cell of every multi-story building. Runs
///      AFTER P221, not right after P131 where Alexander's own numbering
///      would put it -- this operator's own multi-story filter reads
///      `Building.floors`, which stays `None` until P221 sets it (see
///      step 12 just above); running P133 earlier left every building
///      unmatched and the resulting error silently swallowed by the
///      pipeline's own `if let Ok(...)`, confirmed against real fixture
///      data. See the module's own doc for the full story, plus the
///      union_pieces bug a first, rejected implementation hit and the
///      clip_half_plane-based strip technique (borrowed from P131's own
///      passage cell) that replaced it -- and for why this pattern's
///      primary-source text also couldn't be reverified this session.
/// `crate::pipeline::run_corrected_pipeline` runs all thirteen steps end to end
/// for callers that just want the final neighborhood (used by
/// `examples/dump_pipeline.rs` and by `tests/corrected_pipeline.rs`'s
/// per-stage assertions, which reimplement the loop locally to check
/// intermediate counts). The web UI's "Run full pipeline" button
/// reimplements the same sequence client-side in JS, since it needs to
/// update the map after each stage rather than only at the end.
/// BlockGrouping and the older BuildingShape stub are kept for backward
/// compatibility with pipelines/tests that ran the OLD order (P95 first,
/// grouping its pads into blocks afterward) -- not used by the corrected
/// pipeline, which doesn't need to re-derive blocks after the fact because
/// P37 already provides them up front.
pub fn all_operators_v01() -> Vec<Box<dyn DynOperator>> {
    vec![
        Box::new(P37HouseCluster),
        Box::new(P95BuildingComplex),
        Box::new(BlockGrouping),
        Box::new(PathNetwork),
        Box::new(P107WingsOfLight),
        Box::new(BuildingShape),
        Box::new(P61SmallPublicSquares),
        Box::new(P29DensityRings),
        Box::new(P96NumberOfStories),
        Box::new(P108ConnectedBuildings),
        Box::new(P127IntimacyGradient),
        Box::new(P130EntranceRoom),
        Box::new(P129CommonAreasAtTheHeart),
        Box::new(P131TheFlowThroughRooms),
        Box::new(P221NaturalDoorsAndWindows),
        Box::new(P133StaircaseAsAStage),
    ]
}

/// Run a named operator on a parcel. `params_json` is a JSON object (named)
/// or array (vector form) of parameter values; pass `Value::Null` for defaults.
pub fn run_operator(
    nbhd: &Neighborhood,
    operator_name: &str,
    parcel_id: &str,
    params_json: &JsonValue,
    seed: u64,
) -> Result<Subdivision, String> {
    let ops = all_operators_v01();
    let op = ops
        .iter()
        .find(|o| o.name() == operator_name)
        .ok_or_else(|| format!("unknown operator: {operator_name}"))?;
    op.apply_json(nbhd, parcel_id, params_json, seed)
}
