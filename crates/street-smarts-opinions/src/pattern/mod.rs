//! Pattern-presence opinions — one per Alexander pattern with a v0.1 operator.
//! Unlike the geometric chorus, these look directly for the thing the
//! pattern claims to produce, rather than proxying through general
//! wholeness properties.

pub mod p21_four_story_limit;
pub mod p29_density_rings;
pub mod p37_house_cluster;
pub mod p61_small_public_squares;
pub mod p95_building_complex;
pub mod p106_positive_outdoor_space;
pub mod p108_connected_buildings;
pub mod p127_intimacy_gradient;
pub mod p128_indoor_sunlight;
pub mod p129_common_areas_at_the_heart;
pub mod p130_entrance_room;
pub mod p131_the_flow_through_rooms;
pub mod p133_staircase_as_a_stage;
pub mod p102_family_of_entrances;
pub mod p110_main_entrance;
pub mod p30_activity_nodes;
pub mod p114_hierarchy_of_open_space;
pub mod p115_courtyards_which_live;
pub mod p159_light_on_two_sides;
pub mod p165_opening_to_the_street;
pub mod p221_natural_doors_and_windows;
pub mod p67_common_land;

pub use p21_four_story_limit::P21FourStoryLimit;
pub use p29_density_rings::P29DensityRings;
pub use p37_house_cluster::P37HouseCluster;
pub use p61_small_public_squares::P61SmallPublicSquares;
pub use p95_building_complex::P95BuildingComplexOpinion;
pub use p106_positive_outdoor_space::P106PositiveOutdoorSpace;
pub use p108_connected_buildings::P108ConnectedBuildings;
pub use p127_intimacy_gradient::P127IntimacyGradient;
pub use p128_indoor_sunlight::P128IndoorSunlight;
pub use p129_common_areas_at_the_heart::P129CommonAreasAtTheHeart;
pub use p130_entrance_room::P130EntranceRoom;
pub use p131_the_flow_through_rooms::P131TheFlowThroughRooms;
pub use p133_staircase_as_a_stage::P133StaircaseAsAStage;
pub use p102_family_of_entrances::P102FamilyOfEntrances;
pub use p110_main_entrance::P110MainEntrance;
pub use p30_activity_nodes::P30ActivityNodes;
pub use p114_hierarchy_of_open_space::P114HierarchyOfOpenSpace;
pub use p115_courtyards_which_live::P115CourtyardsWhichLive;
pub use p159_light_on_two_sides::P159LightOnTwoSides;
pub use p165_opening_to_the_street::P165OpeningToTheStreet;
pub use p221_natural_doors_and_windows::P221NaturalDoorsAndWindows;
pub use p67_common_land::P67CommonLand;
