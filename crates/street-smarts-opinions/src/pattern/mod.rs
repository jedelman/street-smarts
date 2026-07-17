//! Pattern-presence opinions — one per Alexander pattern with a v0.1 operator.
//! Unlike the geometric chorus, these look directly for the thing the
//! pattern claims to produce, rather than proxying through general
//! wholeness properties.

pub mod p21_four_story_limit;
pub mod p29_density_rings;
pub mod p61_small_public_squares;
pub mod p95_building_complex;
pub mod p106_positive_outdoor_space;
pub mod p108_connected_buildings;
pub mod p127_intimacy_gradient;
pub mod p128_indoor_sunlight;
pub mod p129_common_areas_at_the_heart;
pub mod p131_the_flow_through_rooms;
pub mod p159_light_on_two_sides;
pub mod p221_natural_doors_and_windows;

pub use p21_four_story_limit::P21FourStoryLimit;
pub use p29_density_rings::P29DensityRings;
pub use p61_small_public_squares::P61SmallPublicSquares;
pub use p95_building_complex::P95BuildingComplexOpinion;
pub use p106_positive_outdoor_space::P106PositiveOutdoorSpace;
pub use p108_connected_buildings::P108ConnectedBuildings;
pub use p127_intimacy_gradient::P127IntimacyGradient;
pub use p128_indoor_sunlight::P128IndoorSunlight;
pub use p129_common_areas_at_the_heart::P129CommonAreasAtTheHeart;
pub use p131_the_flow_through_rooms::P131TheFlowThroughRooms;
pub use p159_light_on_two_sides::P159LightOnTwoSides;
pub use p221_natural_doors_and_windows::P221NaturalDoorsAndWindows;
