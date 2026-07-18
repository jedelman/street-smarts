//! Pattern-presence opinions — one per Alexander pattern with a v0.1 operator.
//! Unlike the geometric chorus, these look directly for the thing the
//! pattern claims to produce, rather than proxying through general
//! wholeness properties.

pub mod p21_four_story_limit;
pub mod p22_nine_per_cent_parking;
pub mod p28_eccentric_nucleus;
pub mod p29_density_rings;
pub mod p36_degrees_of_publicness;
pub mod p38_row_houses;
pub mod p37_house_cluster;
pub mod p100_pedestrian_street;
pub mod p112_entrance_transition;
pub mod p120_paths_and_goals;
pub mod p122_building_fronts;
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
pub mod p159_light_on_two_sides;
pub mod p163_outdoor_room;
pub mod p192_windows_overlooking_life;
pub mod p221_natural_doors_and_windows;

pub use p21_four_story_limit::P21FourStoryLimit;
pub use p22_nine_per_cent_parking::P22NineParPercentParking;
pub use p28_eccentric_nucleus::P28EccentricNucleus;
pub use p29_density_rings::P29DensityRings;
pub use p36_degrees_of_publicness::P36DegreesOfPublicness;
pub use p38_row_houses::P38RowHouses;
pub use p37_house_cluster::P37HouseCluster;
pub use p100_pedestrian_street::P100PedestrianStreet;
pub use p112_entrance_transition::P112EntranceTransition;
pub use p120_paths_and_goals::P120PathsAndGoals;
pub use p122_building_fronts::P122BuildingFronts;
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
pub use p159_light_on_two_sides::P159LightOnTwoSides;
pub use p163_outdoor_room::P163OutdoorRoom;
pub use p192_windows_overlooking_life::P192WindowsOverlookingLife;
pub use p221_natural_doors_and_windows::P221NaturalDoorsAndWindows;
