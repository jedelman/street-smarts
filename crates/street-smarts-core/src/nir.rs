//! Neighborhood Intermediate Representation.
//!
//! Single canonical schema all adapters produce and all opinions consume.
//! Decouples scoring from input format.

use crate::geometry::Polygon;
use crate::provenance::ProvenanceTag;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// The top-level neighborhood document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Neighborhood {
    pub id: String,
    pub bbox_wgs84: [f64; 4], // [min_lng, min_lat, max_lng, max_lat]
    #[serde(default)]
    pub parcels: Vec<Parcel>,
    #[serde(default)]
    pub buildings: Vec<Building>,
    #[serde(default)]
    pub streets: Vec<Street>,
    #[serde(default)]
    pub open_space: Vec<OpenSpace>,
    #[serde(default)]
    pub boundaries: Vec<Boundary>,
    #[serde(default)]
    pub activity_nodes: Vec<ActivityNode>,
    /// Real, sampleable potentials over raw site geometry -- see
    /// `PatternField`'s own doc. Empty until a field-producing operator
    /// (P29 Density Rings) has run; every existing fixture round-trips
    /// unchanged (defaults to `vec![]`).
    #[serde(default)]
    pub pattern_fields: Vec<PatternField>,
    pub metadata: NeighborhoodMeta,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeighborhoodMeta {
    pub source: String,
    pub fetched_at: String,
    pub license: String,
    #[serde(default)]
    pub layer_provenance: HashMap<String, ProvenanceTag>,
    /// Human-readable note about what this neighborhood represents
    /// (e.g. "Eastside Commons, current parcel fabric, May 2026").
    pub label: String,
}

/// A parcel = a tax-assessor unit of land. May or may not have a building.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parcel {
    pub id: String,
    pub polygon: Polygon,
    #[serde(default)]
    pub area_acres: f64,
    /// Free-form use category (e.g. "residential", "commercial", "vacant", "civic").
    #[serde(default)]
    pub use_category: Option<String>,
    /// Optional ownership tag — public, private, CLT, cooperative, etc.
    /// This is what feeds the activist `ownership` opinion.
    #[serde(default)]
    pub ownership: Option<Ownership>,
    /// EC-specific: is this part of the Eastside Development Authority (EDA) project?
    #[serde(default)]
    pub is_eda: bool,
    /// Spec code if part of a designed proposal (e.g. "CLT_NORTH", "MAIN_ST_LARGE").
    #[serde(default)]
    pub spec: Option<String>,
    /// Density-ring tier ("core" / "mid" / "edge") set by P29 Density Rings
    /// -- how far this parcel sits from the site's density center. `None`
    /// if P29 hasn't run.
    #[serde(default)]
    pub density_tier: Option<String>,
    /// Target story count. P29 sets it at block scale (a goal for that
    /// block as a whole); P96 Number of Stories overwrites it per-pad with
    /// the actual assigned value once building pads exist. `None` means
    /// no pattern has assigned a target yet -- downstream operators (P107)
    /// fall back to their own flat default.
    #[serde(default)]
    pub target_stories: Option<f64>,
}

/// One real, sampleable potential over raw site geometry -- see
/// `PATTERN_ORDERING_AUDIT.md` (repo root) for why this exists: a larger,
/// prior Alexander pattern (P29 Density Rings) describes a general
/// disposition across the whole site, not yet a value attached to any
/// specific parcel or block. Storing that disposition here, instead of
/// forcing the producing operator to wait until something smaller has
/// already been individuated (a block, a pad) purely so it has somewhere
/// to stamp a tag, is what lets a pattern like P29 run at its own real,
/// canonical pipeline position -- see `p29_density_rings`'s own module
/// doc for the concrete before/after.
///
/// One variant per producing pattern, not a generic "field of floats" --
/// each field's own real sampling math differs (see `DensityField`'s own
/// doc) and lives with its producing operator in `street-smarts-patterns`
/// (`p29_density_rings::sample_density_field`), not here; this schema only
/// carries the data, same split every other operator-specific NIR type
/// already uses (e.g. `RoofForm` is data, `p116_cascade_of_roofs` is the
/// math that produces and reads it). `Intimacy` is the second variant this
/// doc comment used to anticipate as a future extension -- see its own
/// doc and `PATTERN_ORDERING_AUDIT.md` §4.5/§6e for the real generator gap
/// it closes.
///
/// Named `PatternField`, not `Field` -- `street_smarts_patterns::field`
/// already defines an unrelated `Field` (a rasterized pressure grid for
/// Voronoi seed placement, ported from eastside-commons' own field
/// solver). Same metaphor, a different real thing; kept namespaced apart
/// on purpose so `use`-ing both in the same file (P37 needs to) is never
/// ambiguous.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PatternField {
    Density(DensityField),
    Intimacy(IntimacyField),
}

/// P29 Density Rings' own real data: site density should step down with
/// distance from a real center, not be flat everywhere. Pure data -- the
/// real sampling logic (`sample_density_field`) lives with the operator
/// that produces this, in `street-smarts-patterns::p29_density_rings`, not
/// here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DensityField {
    /// Real center of the density gradient, already shifted by the
    /// producing operator's own `eccentricity_frac` at construction time
    /// -- see `p29_density_rings`'s own module doc for why an eccentric,
    /// off-center peak (P28 Eccentric Nucleus) is the real target, not a
    /// dead-center one.
    pub center: crate::geometry::LngLat,
    /// Real distance (meters) at which the gradient reaches its edge
    /// value and stops falling further -- a sample beyond this clamps to
    /// `edge_target_stories`, it is not extrapolated past it.
    pub radius_m: f64,
    pub core_target_stories: f64,
    pub edge_target_stories: f64,
    /// Number of concentric density bands -- carried on the field itself
    /// (not re-passed as a separate parameter at sample time) so every
    /// consumer buckets into the exact same rings P29 itself was
    /// configured with.
    pub n_rings: u32,
}

/// `p127_intimacy_gradient`'s own real "public-to-private" gradient over
/// ONE specific building's own footprint -- pure data; the real sampling
/// logic (`sample_intimacy_field`) lives with the operator that produces
/// this, in `street-smarts-patterns::p127_intimacy_gradient`, not here.
/// Unlike `DensityField` (one field for the whole site), this is inherently
/// per-building -- `Neighborhood.pattern_fields` already accommodates many
/// entries, so one `Intimacy` field is attached per building P127
/// partitions.
///
/// **Solid buildings only.** A solid building's depth is a real linear
/// gradient along one axis (see `axis_x`/`axis_y` below) -- a closed form
/// cheap enough to store as a few scalars and resample anywhere on the
/// footprint, exactly the property that made `DensityField` work as a
/// primitive. A courtyard building's depth is angular arc-length position
/// around its own ring, which isn't reducible to a few scalars without
/// also carrying the ring geometry itself (already on `Building.polygon`,
/// redundant to duplicate here) -- courtyard buildings don't get an
/// `Intimacy` field; consumers fall back to reading `InteriorCell.depth`
/// directly for those, exactly as if this field didn't exist. See
/// `PATTERN_ORDERING_AUDIT.md` §4.5/§6e for the real scope call this is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntimacyField {
    /// Which building this gradient belongs to -- unlike `DensityField`,
    /// there is one of these per building, not one for the whole site.
    pub building_id: String,
    /// The building footprint's own centroid, in real lng/lat -- the same
    /// origin `p127_intimacy_gradient`'s own local-metre conversions use
    /// for this building, and what `axis_x`/`axis_y`/`s_min`/`s_max` below
    /// are measured relative to.
    pub origin: crate::geometry::LngLat,
    /// Real unit vector, in local metres (east-west, north-south) from
    /// `origin`, pointing from the deepest point TOWARD the public realm
    /// -- `p127_intimacy_gradient`'s own cardinal-snapped depth axis.
    pub axis_x: f64,
    pub axis_y: f64,
    /// The real range of `(point - origin) . axis` this building's own
    /// footprint spans -- `s_max` is where the axis projection is
    /// greatest, i.e. nearest the public realm (depth 0.0); `s_min` is
    /// farthest from it (depth 1.0). A sample outside this range clamps,
    /// the same convention `DensityField`'s own radius clamp uses.
    pub s_min: f64,
    pub s_max: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Ownership {
    Public,
    Private,
    Clt,        // community land trust
    Cooperative,
    Commons,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Building {
    pub id: String,
    pub polygon: Polygon,
    #[serde(default)]
    pub height_m: Option<f64>,
    #[serde(default)]
    pub typology: Option<String>, // e.g. "tower", "rowhouse", "single-family"
    #[serde(default)]
    pub year_built: Option<i32>,
    #[serde(default)]
    pub parcel_id: Option<String>,
    /// Story count derived from `height_m`. `None` until an operator that
    /// knows the building's floor-to-floor height (e.g.
    /// `p221_natural_doors_and_windows`) has run.
    #[serde(default)]
    pub floors: Option<u32>,
    /// Window/door openings cut into this building's exterior walls.
    /// Empty until a window/door-placing operator has run -- every
    /// existing fixture round-trips unchanged (defaults to `vec![]`).
    #[serde(default)]
    pub openings: Vec<Opening>,
    /// The building's interior, partitioned into cells by
    /// `p127_intimacy_gradient` (depth), `p129_common_areas_at_the_heart`
    /// (which cell is common), and `p131_the_flow_through_rooms`
    /// (adjacency). Empty until those operators have run. Ground-floor
    /// only in this version -- see `InteriorCell.floor`.
    #[serde(default)]
    pub interior_cells: Vec<InteriorCell>,
    /// Real exterior-wall depth in meters, applied uniformly around this
    /// building's own footprint. `None` until an operator that assigns
    /// real wall depth (`p197_thick_walls`) has run -- every existing
    /// fixture round-trips unchanged. Before this field existed, a
    /// `Building`'s outer ring and its `Opening`s modeled a zero-depth
    /// membrane (see `render.py`'s own documented caveat, and
    /// `p197_thick_walls`'s own opinion module doc for the real gap this
    /// closes).
    #[serde(default)]
    pub wall_thickness_m: Option<f64>,
    /// Real roof form -- Alexander's P117 Sheltering Roof ("slope the
    /// roof... make its entire surface visible, and bring the eaves down
    /// low") and P162 North Face ("make the north face of the building a
    /// cascade which slopes down to the ground, so the sun... strikes the
    /// ground immediately beside the building"). `None` until an operator
    /// that assigns a real roof (`p117_sheltering_roof` generator) has
    /// run -- every existing fixture round-trips unchanged. Before this
    /// field existed, every building was a flat-topped extrusion (see
    /// `render.py`'s own documented "no roof forms" caveat).
    #[serde(default)]
    pub roof: Option<RoofForm>,
    /// Real per-wing roof segments -- Alexander's P116 Cascade of Roofs
    /// ("if the building is complex, and has several different roofs...
    /// connect the roofs so that they step down, following the social
    /// hierarchy of the spaces they cover"). Each segment covers its own
    /// real sub-polygon of `polygon` with its own `RoofForm`, so a lower
    /// wing can carry a lower ridge than a taller one -- the actual
    /// "cascade" -- instead of the single whole-building `roof` above.
    /// Empty until a real P116 generator exists to partition the
    /// footprint: no such generator is implemented yet, and there is
    /// nothing to key segments to today -- `p107_wings_of_light`
    /// explicitly does NOT produce discrete wing entities (see that
    /// generator's own module doc for why true branching wings were
    /// deferred). `roof` above remains the whole-building fallback when
    /// this is empty; every existing fixture round-trips unchanged.
    #[serde(default)]
    pub roof_segments: Vec<RoofSegment>,
    /// Real covered walkways/porches attached along this building's own
    /// exterior wall -- Alexander's P119 Arcades ("all busy public
    /// paths... need a roofed arcade... to give pedestrians shelter") and
    /// P166 Gallery Surround (the same covered-walkway idea, wrapped
    /// further around a building's own perimeter). Empty until a real
    /// generator exists -- `p221_natural_doors_and_windows` already
    /// computes which wall ring edges face the street, the natural real
    /// input a P119/P166 generator would need, but no such generator is
    /// implemented yet.
    #[serde(default)]
    pub canopies: Vec<Canopy>,
    /// Real local wall thickenings -- bay windows, built-in seats/shelves
    /// deep enough to sit in -- along specific spans of this building's
    /// own exterior wall. Alexander's P160 Building Edge ("the edge...
    /// must be a place, must have volume... deep enough to contain
    /// seats, bookshelves, bay windows"): a uniform `wall_thickness_m`
    /// (P197 Thick Walls, above) gives every wall real depth, but P160's
    /// own claim is about LOCAL extra depth at specific spots, not a
    /// thicker uniform wall everywhere -- this is additive to, not a
    /// replacement for, `wall_thickness_m`. Empty until a real P160
    /// generator exists.
    #[serde(default)]
    pub wall_niches: Vec<WallNiche>,
}

/// The real geometric family a roof belongs to. Only `Shed` is generated
/// today (see `p117_sheltering_roof.rs`'s own module doc for why a shed
/// roof, specifically low on the true-north side, is the one form that
/// honestly satisfies BOTH P117's "sloped, eaves down low" claim and
/// P162's specifically NORTH-facing "cascade... to the ground" claim at
/// once) -- `Gable`/`Hip`/`Flat` are real, named variants reserved for
/// later work (P116 Cascade of Roofs' own per-wing segments, P118 Roof
/// Garden's flat section), not fabricated options nothing ever produces.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoofShape {
    Flat,
    Shed,
    Gable,
    Hip,
}

/// A real, dimensioned roof over a building's own footprint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoofForm {
    pub shape: RoofShape,
    /// Real height (meters, above the site datum P107/P96 already use for
    /// `Building.height_m`) of the roof's own highest edge.
    pub ridge_height_m: f64,
    /// Real height of the roof's own lowest edge -- Alexander's P117
    /// literal figure (6'0"-6'6" / 1.8288-1.9812m) at its low side.
    pub eave_height_m: f64,
    /// Real compass bearing (0 = true north, 90 = east, degrees) the roof
    /// slopes DOWN toward -- i.e. the direction of the low (eave) edge
    /// from the high (ridge) edge. `p117_sheltering_roof` always assigns
    /// `0.0` (true north), for P162's own literal "north face" claim --
    /// this pipeline's real lng/lat makes that an exact bearing, not an
    /// approximation.
    pub slope_azimuth_deg: f64,
    /// Whether this roof plane is real, intentional occupiable space --
    /// Alexander's P118 Roof Garden's own core claim (only meaningful
    /// when `shape == RoofShape::Flat`; a sloped roof isn't occupiable by
    /// this pipeline's own reasoning). `false` for every roof
    /// `p117_sheltering_roof` assigns today (always `Shed`) -- purely
    /// additive, no existing fixture changes meaning. A real access point
    /// (stair/hatch up to the roof) is a separate, still-unbuilt concept
    /// this field doesn't attempt to solve.
    #[serde(default)]
    pub occupiable: bool,
}

/// One real roof plane over part of a building's own footprint -- see
/// `Building.roof_segments`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoofSegment {
    /// The real sub-polygon of the building's own footprint this segment
    /// covers. Expected (not validated here -- this schema doesn't own
    /// that invariant, a future P116 generator does) to lie within the
    /// parent `Building.polygon`'s outer ring.
    pub footprint: Polygon,
    pub form: RoofForm,
}

/// The Alexander pattern a `Canopy` instance realizes -- both patterns
/// share the same real geometry (a thin roof projecting from a wall over
/// a walkway), differing only in intent/coverage, so one struct serves
/// both rather than two near-identical ones.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanopyKind {
    /// P119 Arcades -- shelter along one busy public-facing wall span.
    Arcade,
    /// P166 Gallery Surround -- the same covered walkway, wrapped further
    /// around the building's own perimeter.
    Gallery,
}

/// A real covered walkway projecting from one wall of a building's own
/// footprint -- see `Building.canopies`. Spans a real RANGE of one wall
/// edge (`t_start..=t_end`), unlike `Opening`'s single-point `t`: a
/// window is a point feature, an arcade is a real linear span a person
/// walks the length of.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Canopy {
    pub kind: CanopyKind,
    /// Same real ring-edge indexing as `Opening.ring_index`.
    pub ring_index: usize,
    /// Same convention as `Opening.on_hole`.
    #[serde(default)]
    pub on_hole: bool,
    /// Start of the covered span along that wall edge, in `0.0..=1.0`.
    pub t_start: f64,
    /// End of the covered span, in `0.0..=1.0`. Expected `>= t_start`
    /// (not validated here; a future generator owns that invariant).
    pub t_end: f64,
    /// How far the canopy projects outward from the wall face, in meters.
    pub depth_m: f64,
    /// Real clearance height under the canopy, meters above this floor's
    /// own base elevation (same convention as `Opening.sill_height_m`).
    pub height_m: f64,
    /// 0 = ground floor, same convention as `Opening.floor`. Distinguishes
    /// P119 Arcades (a real ground-level, street-facing claim) from P166
    /// Gallery Surround's own explicit "at every story" claim -- without a
    /// real floor number, "every story" can't honestly be checked as
    /// anything more than "at least one canopy exists somewhere."
    #[serde(default)]
    pub floor: u32,
}

/// One real local bulge in an exterior wall's own depth -- see
/// `Building.wall_niches`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WallNiche {
    /// Same real ring-edge indexing as `Opening.ring_index`.
    pub ring_index: usize,
    #[serde(default)]
    pub on_hole: bool,
    pub t_start: f64,
    pub t_end: f64,
    /// Extra depth beyond `Building.wall_thickness_m` at this span, in
    /// meters -- the real bay/niche bump-out, not a replacement for the
    /// building's own uniform exterior thickness.
    pub extra_depth_m: f64,
}

/// One cell of a building's interior partition. Deliberately FORM-only --
/// a position in the public/private gradient and a set of connections, not
/// a room type. Nothing here ever says "bedroom" or "kitchen": Alexander's
/// own pattern language describes spatial relationships, not prescribed
/// uses, and this project doesn't assume a program this pipeline has no
/// way to know.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteriorCell {
    pub id: String,
    /// The cell's own 2D footprint.
    pub polygon: Polygon,
    /// Position in the public-to-private gradient: 0.0 sits at the
    /// public-facing wall, 1.0 is the deepest point in the building (a
    /// solid building's far band, or the courtyard-ring point diametrically
    /// opposite the entrance). Set by `p127_intimacy_gradient`.
    pub depth: f64,
    /// True for the one cell `p129_common_areas_at_the_heart` identifies as
    /// nearest the whole footprint's center of gravity.
    #[serde(default)]
    pub is_common: bool,
    /// Free-form, same convention as `Parcel.use_category` /
    /// `Building.typology`: "room" for a normal gradient band/bay, or
    /// "passage" for a P131/P132 loop-closing passage cell.
    #[serde(default)]
    pub kind: String,
    /// Other `InteriorCell` ids this one connects to via an internal
    /// doorway. Set by `p131_the_flow_through_rooms`; empty until it runs.
    #[serde(default)]
    pub connects_to: Vec<String>,
    /// 0 = ground floor. Always 0 in this version -- there's no modeled
    /// way to reach an upper floor (no staircase/vertical circulation
    /// pattern implemented yet), so partitioning one would be fiction.
    #[serde(default)]
    pub floor: u32,
}

/// A window or door opening on one of a `Building`'s exterior walls.
/// Positioned relative to the building's own footprint ring, not in
/// absolute coordinates, so it stays correct if the footprint is
/// reprojected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Opening {
    pub kind: OpeningKind,
    /// Index `i` of the wall edge this opening sits on: the segment from
    /// `ring[i]` to `ring[i+1]` (wrapping), where `ring` is the building
    /// polygon's outer ring, or its (single) hole ring when `on_hole` is
    /// true.
    pub ring_index: usize,
    /// `false` = outer (street/yard-facing) ring. `true` = the courtyard
    /// hole ring of a P107 courtyard-typology building.
    #[serde(default)]
    pub on_hole: bool,
    /// Position of the opening's center along that wall edge, in `0.0..=1.0`.
    pub t: f64,
    pub width_m: f64,
    /// Sill height in meters above this floor's own base elevation (not
    /// above ground) -- add `floor * floor_to_floor_m` to get absolute Z.
    pub sill_height_m: f64,
    pub head_height_m: f64,
    /// 0 = ground floor.
    pub floor: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpeningKind {
    Window,
    Door,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Street {
    pub id: String,
    pub centerline: Vec<crate::geometry::LngLat>,
    #[serde(default)]
    pub classification: Option<String>, // "arterial", "local", "alley", "pedestrian"
    #[serde(default)]
    pub row_width_m: Option<f64>,
    /// Free-form, same convention as `Parcel.use_category` / `Building.typology`:
    /// "grass_pavers" (Alexander's P51 Green Streets -- grass with paving
    /// stones set in for wheels) or "asphalt". `None` means unset (older
    /// fixtures, or a generator that hasn't decided yet) -- not the same
    /// as "asphalt".
    #[serde(default)]
    pub surface: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenSpace {
    pub id: String,
    pub polygon: Polygon,
    pub kind: OpenSpaceKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenSpaceKind {
    Park,
    Plaza,
    Vacant,
    Sponge,    // stormwater / ecological
    Parking,
    Other,
    /// Land a pattern operator carved out of something else but explicitly
    /// declined to resolve -- distinct from `Vacant` (an assessor's
    /// judgment about existing land) or `Other` (a shrug). This is real
    /// geometry, not a bare number in a trace string: P61's capped-off
    /// candidate squares and dropped slivers land here, and a future P106
    /// (Positive Outdoor Space) check should scan for exactly this kind
    /// rather than trusting operators to have resolved everything.
    Undecided,
    /// Informal shared land within ONE house cluster (P37) -- what the
    /// cluster's households actually face onto and identify with. Distinct
    /// from `Plaza` (P61's intentionally placed, publicly-scaled small
    /// square, or P95's designed interconnecting courtyard): this is
    /// smaller, informal, and belongs to a single cluster rather than the
    /// neighborhood at large.
    Common,
    /// A small, partly enclosed real bump projecting from an adjacent
    /// building's own footprint at a point where it borders a real
    /// `Plaza` -- Alexander's P124 Activity Pockets ("jut forward into
    /// the open space," his own literal words). Distinct from `Plaza`
    /// itself (the square's own open area) and from `Common` (informal
    /// cluster-scale shared land): this is deliberately small, attached
    /// to one specific building, and real geometry added to that
    /// building's own mass, not an independently placed patch.
    Pocket,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Boundary {
    pub id: String,
    pub centerline: Vec<crate::geometry::LngLat>,
    pub kind: BoundaryKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryKind {
    Natural,           // river, ridge, coast
    Infrastructural,   // highway, rail, levee
    Jurisdictional,    // city limit, zoning edge
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityNode {
    pub id: String,
    pub location: crate::geometry::LngLat,
    pub kind: ActivityKind,
    /// Anticipated visits/day or similar intensity hint, if known.
    #[serde(default)]
    pub intensity: Option<f64>,
    #[serde(default)]
    pub label: Option<String>,
    /// Open-ended fit scores, one entry per real activity dimension a
    /// real signal exists for (e.g. "commerce", "gathering", "transit",
    /// "play"), each in `[0.0, 1.0]`. Empty means no fit signal has been
    /// assessed yet -- NOT zero fit for everything. Deliberately not a
    /// fixed enum/classification the way `kind` is: a real place rarely
    /// fits exactly one activity, and this lets a generator or a future
    /// input source express that without committing to a single label.
    #[serde(default)]
    pub activity_fit: BTreeMap<String, f64>,
    /// How public (`1.0`) vs. intimate/private (`0.0`) this node's own
    /// setting reads -- independent of `Street.classification`, so it
    /// doesn't wait on a real "arterial" street existing anywhere in the
    /// generated network. `None` means no real signal exists yet, not a
    /// fabricated midpoint -- see `p126_something_roughly_in_the_middle.rs`
    /// and `p61_small_public_squares.rs`'s own module docs for what
    /// currently sets (or deliberately leaves unset) this field.
    #[serde(default)]
    pub publicness: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Commerce,
    Civic,
    Transit,
    School,
    Worship,
    Health,
    /// A real threshold marker where a major path crosses a real site
    /// `Boundary` -- P53 Main Gateways' own real proxy for "a great
    /// gateway," not a generic activity concentration the way every other
    /// variant here is. Fits `ActivityNode`'s own documented scope (see
    /// `Subdivision.new_activity_nodes`'s doc: "a real focal point --
    /// fountain, marker, concentration of use") without inventing a
    /// separate schema type for what's still fundamentally a real point
    /// with real human significance.
    Gateway,
    Other,
}

#[cfg(test)]
mod activity_node_tests {
    use super::*;
    use crate::geometry::LngLat;

    #[test]
    fn old_json_with_no_activity_fit_or_publicness_deserializes_to_honest_defaults() {
        let old_json = r#"{
            "id": "A1",
            "location": {"lng": 0.0, "lat": 0.0},
            "kind": "civic"
        }"#;
        let node: ActivityNode = serde_json::from_str(old_json).expect("older ActivityNode JSON should still deserialize");
        assert!(node.activity_fit.is_empty(), "no fit signal in the source JSON should mean an empty map, not a fabricated one");
        assert_eq!(node.publicness, None, "no publicness signal in the source JSON should mean None, not a fabricated midpoint");
    }

    #[test]
    fn activity_fit_and_publicness_round_trip_through_json() {
        let mut fit = BTreeMap::new();
        fit.insert("commerce".to_string(), 0.7);
        fit.insert("gathering".to_string(), 0.4);
        let node = ActivityNode {
            id: "A1".into(),
            location: LngLat::new(0.0, 0.0),
            kind: ActivityKind::Civic,
            intensity: None,
            label: None,
            activity_fit: fit.clone(),
            publicness: Some(0.8),
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: ActivityNode = serde_json::from_str(&json).unwrap();
        assert_eq!(back.activity_fit, fit);
        assert_eq!(back.publicness, Some(0.8));
    }
}

#[cfg(test)]
mod building_wall_thickness_tests {
    use super::*;
    use crate::geometry::{LngLat, Polygon};

    #[test]
    fn old_json_with_no_wall_thickness_deserializes_to_honest_none() {
        let old_json = r#"{
            "id": "B1",
            "polygon": {"outer": [
                {"lng": 0.0, "lat": 0.0}, {"lng": 0.0001, "lat": 0.0}, {"lng": 0.0001, "lat": 0.0001}
            ]}
        }"#;
        let b: Building = serde_json::from_str(old_json).expect("older Building JSON should still deserialize");
        assert_eq!(b.wall_thickness_m, None, "no thickness signal in the source JSON should mean None, not a fabricated default");
        assert!(b.roof_segments.is_empty(), "no roof-segment signal should mean empty, not a fabricated segment");
        assert!(b.canopies.is_empty(), "no canopy signal should mean empty, not a fabricated canopy");
        assert!(b.wall_niches.is_empty(), "no niche signal should mean empty, not a fabricated niche");
    }

    #[test]
    fn wall_thickness_m_round_trips_through_json() {
        let b = Building {
            id: "B1".into(),
            polygon: Polygon::from_ring(vec![
                LngLat::new(0.0, 0.0), LngLat::new(0.0001, 0.0),
                LngLat::new(0.0001, 0.0001), LngLat::new(0.0, 0.0001), LngLat::new(0.0, 0.0),
            ]),
            height_m: None, typology: None, year_built: None, parcel_id: None,
            floors: None, openings: vec![], interior_cells: vec![],
            wall_thickness_m: Some(0.3),
            roof: None,
            roof_segments: vec![],
            canopies: vec![],
            wall_niches: vec![],
        };
        let json = serde_json::to_string(&b).unwrap();
        let back: Building = serde_json::from_str(&json).unwrap();
        assert_eq!(back.wall_thickness_m, Some(0.3));
    }
}

#[cfg(test)]
mod roof_segment_canopy_niche_tests {
    use super::*;
    use crate::geometry::{LngLat, Polygon};

    fn square_polygon(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon {
        Polygon::from_ring(vec![
            LngLat::new(x0, y0), LngLat::new(x1, y0),
            LngLat::new(x1, y1), LngLat::new(x0, y1), LngLat::new(x0, y0),
        ])
    }

    fn building(id: &str) -> Building {
        Building {
            id: id.into(),
            polygon: square_polygon(0.0, 0.0, 0.0002, 0.0001),
            height_m: None, typology: None, year_built: None, parcel_id: None,
            floors: None, openings: vec![], interior_cells: vec![],
            wall_thickness_m: None, roof: None,
            roof_segments: vec![], canopies: vec![], wall_niches: vec![],
        }
    }

    #[test]
    fn old_roof_form_json_with_no_occupiable_deserializes_to_honest_false() {
        let old_json = r#"{
            "shape": "flat", "ridge_height_m": 6.0, "eave_height_m": 6.0, "slope_azimuth_deg": 0.0
        }"#;
        let r: RoofForm = serde_json::from_str(old_json).expect("older RoofForm JSON should still deserialize");
        assert!(!r.occupiable, "no occupiable signal in the source JSON should mean false, not a fabricated true");
    }

    #[test]
    fn roof_segments_canopies_and_wall_niches_round_trip_through_json() {
        let mut b = building("B1");
        b.roof_segments.push(RoofSegment {
            footprint: square_polygon(0.0, 0.0, 0.0001, 0.0001),
            form: RoofForm {
                shape: RoofShape::Flat, ridge_height_m: 6.0, eave_height_m: 6.0,
                slope_azimuth_deg: 0.0, occupiable: true,
            },
        });
        b.canopies.push(Canopy {
            kind: CanopyKind::Arcade, ring_index: 0, on_hole: false,
            t_start: 0.1, t_end: 0.6, depth_m: 1.5, height_m: 2.4, floor: 0,
        });
        b.wall_niches.push(WallNiche {
            ring_index: 1, on_hole: false, t_start: 0.2, t_end: 0.4, extra_depth_m: 0.4,
        });

        let json = serde_json::to_string(&b).unwrap();
        let back: Building = serde_json::from_str(&json).unwrap();

        assert_eq!(back.roof_segments.len(), 1);
        assert_eq!(back.roof_segments[0].form.shape, RoofShape::Flat);
        assert!(back.roof_segments[0].form.occupiable);
        assert_eq!(back.canopies.len(), 1);
        assert_eq!(back.canopies[0].kind, CanopyKind::Arcade);
        assert_eq!(back.canopies[0].t_end, 0.6);
        assert_eq!(back.wall_niches.len(), 1);
        assert_eq!(back.wall_niches[0].extra_depth_m, 0.4);
    }
}

#[cfg(test)]
mod pattern_field_tests {
    use super::*;
    use crate::geometry::LngLat;

    fn nbhd_json_without_pattern_fields() -> &'static str {
        r#"{
            "id": "test", "bbox_wgs84": [0.0, 0.0, 0.01, 0.01],
            "metadata": {"source": "s", "fetched_at": "t", "license": "l", "label": "old fixture"}
        }"#
    }

    #[test]
    fn old_neighborhood_json_with_no_pattern_fields_deserializes_to_empty_vec() {
        let n: Neighborhood = serde_json::from_str(nbhd_json_without_pattern_fields())
            .expect("pre-field Neighborhood JSON should still deserialize");
        assert!(n.pattern_fields.is_empty());
    }

    #[test]
    fn density_field_round_trips_through_json_with_its_kind_tag() {
        let field = PatternField::Density(DensityField {
            center: LngLat::new(-76.1, 36.8),
            radius_m: 300.0,
            core_target_stories: 6.0,
            edge_target_stories: 2.0,
            n_rings: 3,
        });
        let json = serde_json::to_string(&field).unwrap();
        assert!(json.contains("\"kind\":\"Density\""), "expected an internally-tagged 'kind' discriminator, got: {json}");
        let back: PatternField = serde_json::from_str(&json).unwrap();
        match back {
            PatternField::Density(d) => {
                assert_eq!(d.n_rings, 3);
                assert!((d.radius_m - 300.0).abs() < 1e-9);
            }
            PatternField::Intimacy(_) => panic!("expected Density, got Intimacy"),
        }
    }

    #[test]
    fn intimacy_field_round_trips_through_json_with_its_kind_tag() {
        let field = PatternField::Intimacy(IntimacyField {
            building_id: "B1".into(),
            origin: LngLat::new(-76.1, 36.8),
            axis_x: 1.0,
            axis_y: 0.0,
            s_min: -5.0,
            s_max: 5.0,
        });
        let json = serde_json::to_string(&field).unwrap();
        assert!(json.contains("\"kind\":\"Intimacy\""), "expected an internally-tagged 'kind' discriminator, got: {json}");
        let back: PatternField = serde_json::from_str(&json).unwrap();
        match back {
            PatternField::Intimacy(f) => {
                assert_eq!(f.building_id, "B1");
                assert!((f.s_max - f.s_min - 10.0).abs() < 1e-9);
            }
            PatternField::Density(_) => panic!("expected Intimacy, got Density"),
        }
    }
}
