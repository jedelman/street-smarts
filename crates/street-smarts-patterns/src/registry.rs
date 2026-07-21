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
use crate::p124_activity_pockets::P124ActivityPockets;
use crate::p117_sheltering_roof::P117ShelteringRoof;
use crate::p118_roof_garden::P118RoofGarden;
use crate::p119_arcades::P119Arcades;
use crate::p133_staircase_as_a_stage::P133StaircaseAsAStage;
use crate::p160_building_edge::P160BuildingEdge;
use crate::p197_thick_walls::P197ThickWalls;
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
///
/// The full ordering rationale used to be duplicated here AND in
/// `pipeline.rs`'s module doc -- two files' worth of prose describing the
/// same 20-step sequence, exactly the "read instead of look up" cost
/// PATTERN_LANGUAGE_SIMULATION.md §3.4 named. It now lives in ONE place,
/// queryable instead of read-and-infer: `language_graph::LANGUAGE`, whose
/// `requires`/`why` fields are checked by `validate_order` against the
/// pipeline's own real, traced execution order (not a hand-copied guess --
/// see `language_graph.rs`'s own doc comment). For the always-in-sync,
/// generated version of this list:
///
/// ```text
/// cargo run -p street-smarts-patterns --example dump_pattern_language
/// ```
///
/// `pipeline.rs`'s module doc remains the one authoritative home for
/// implementation detail beyond bare ordering (specific numeric thresholds,
/// bug history, geometry techniques) -- see it and `tests/corrected_pipeline.rs`
/// for the real, tested sequence, and the web UI's "Run full pipeline"
/// button for the same orchestration client-side.
///
/// `crate::pipeline::run_corrected_pipeline` runs all twenty steps end to end
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
        Box::new(P124ActivityPockets),
        Box::new(P117ShelteringRoof),
        Box::new(P197ThickWalls),
        Box::new(P127IntimacyGradient),
        Box::new(P130EntranceRoom),
        Box::new(P129CommonAreasAtTheHeart),
        Box::new(P131TheFlowThroughRooms),
        Box::new(P221NaturalDoorsAndWindows),
        Box::new(P133StaircaseAsAStage),
        Box::new(P118RoofGarden),
        Box::new(P119Arcades),
        Box::new(P160BuildingEdge),
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
