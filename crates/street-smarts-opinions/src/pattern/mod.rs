//! Pattern-presence opinions — one per Alexander pattern with a v0.1 operator.
//! Unlike the geometric chorus, these look directly for the thing the
//! pattern claims to produce, rather than proxying through general
//! wholeness properties.

pub mod p21_four_story_limit;
pub mod p95_building_complex;
pub mod p106_positive_outdoor_space;
pub mod p127_intimacy_gradient;
pub mod p128_indoor_sunlight;
pub mod p129_common_areas_at_the_heart;
pub mod p131_the_flow_through_rooms;
pub mod p159_light_on_two_sides;

pub use p21_four_story_limit::P21FourStoryLimit;
pub use p95_building_complex::P95BuildingComplexOpinion;
pub use p106_positive_outdoor_space::P106PositiveOutdoorSpace;
pub use p127_intimacy_gradient::P127IntimacyGradient;
pub use p128_indoor_sunlight::P128IndoorSunlight;
pub use p129_common_areas_at_the_heart::P129CommonAreasAtTheHeart;
pub use p131_the_flow_through_rooms::P131TheFlowThroughRooms;
pub use p159_light_on_two_sides::P159LightOnTwoSides;
