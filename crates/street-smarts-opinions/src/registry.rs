//! Registry of v0.1 opinions and a helper to evaluate them all.

use crate::activist::OwnershipPattern;
use crate::geometric::{LevelsOfScale, StrongCenters};
use crate::pattern::{
    P102FamilyOfEntrances, P105SouthFacingOutdoors, P108ConnectedBuildings, P110MainEntrance,
    P114HierarchyOfOpenSpace, P115CourtyardsWhichLive, P121PathShape, P130EntranceRoom,
    P133StaircaseAsAStage, P160BuildingEdge, P165OpeningToTheStreet, P21FourStoryLimit,
    P221NaturalDoorsAndWindows, P29DensityRings, P30ActivityNodes, P37HouseCluster,
    P49LoopedLocalRoads, P50TJunctions, P60AccessibleGreen, P61SmallPublicSquares, P67CommonLand,
    P95BuildingComplexOpinion, P99MainBuilding, P106PositiveOutdoorSpace, P127IntimacyGradient,
    P128IndoorSunlight, P129CommonAreasAtTheHeart, P131TheFlowThroughRooms, P159LightOnTwoSides,
};
use serde::{Deserialize, Serialize};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::opinion::{Opinion, OpinionFamily, OpinionOutput, OpinionRef};

/// One evaluated opinion, ready for the conflict engine and the renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatedOpinion {
    pub opinion: OpinionRef,
    pub output: OpinionOutput,
}

/// Build the v0.1 opinion roster as boxed trait objects.
pub fn all_opinions_v01() -> Vec<Box<dyn Opinion>> {
    vec![
        Box::new(LevelsOfScale),
        Box::new(StrongCenters),
        Box::new(OwnershipPattern),
        Box::new(P95BuildingComplexOpinion),
        Box::new(P106PositiveOutdoorSpace),
        Box::new(P21FourStoryLimit),
        Box::new(P127IntimacyGradient),
        Box::new(P128IndoorSunlight),
        Box::new(P129CommonAreasAtTheHeart),
        Box::new(P131TheFlowThroughRooms),
        Box::new(P159LightOnTwoSides),
        Box::new(P29DensityRings),
        Box::new(P61SmallPublicSquares),
        Box::new(P108ConnectedBuildings),
        Box::new(P221NaturalDoorsAndWindows),
        Box::new(P37HouseCluster),
        Box::new(P130EntranceRoom),
        Box::new(P133StaircaseAsAStage),
        Box::new(P67CommonLand),
        Box::new(P114HierarchyOfOpenSpace),
        Box::new(P115CourtyardsWhichLive),
        Box::new(P165OpeningToTheStreet),
        Box::new(P102FamilyOfEntrances),
        Box::new(P110MainEntrance),
        Box::new(P30ActivityNodes),
        Box::new(P49LoopedLocalRoads),
        Box::new(P50TJunctions),
        Box::new(P60AccessibleGreen),
        Box::new(P99MainBuilding),
        Box::new(P105SouthFacingOutdoors),
        Box::new(P121PathShape),
        Box::new(P160BuildingEdge),
    ]
}

/// Run all v0.1 opinions against a neighborhood and collect results.
pub fn evaluate_all(n: &Neighborhood) -> Vec<EvaluatedOpinion> {
    all_opinions_v01()
        .into_iter()
        .map(|op| {
            let opinion_ref = Opinion::as_ref(op.as_ref());
            let output = op.evaluate(n);
            EvaluatedOpinion {
                opinion: opinion_ref,
                output,
            }
        })
        .collect()
}

/// Group evaluated opinions by family — used by the conflict engine to
/// keep the geometric chorus separate from the activist guards.
pub fn group_by_family(
    evaluated: &[EvaluatedOpinion],
) -> std::collections::HashMap<OpinionFamily, Vec<&EvaluatedOpinion>> {
    let mut groups: std::collections::HashMap<OpinionFamily, Vec<&EvaluatedOpinion>> =
        std::collections::HashMap::new();
    for ev in evaluated {
        groups.entry(ev.opinion.family).or_default().push(ev);
    }
    groups
}
