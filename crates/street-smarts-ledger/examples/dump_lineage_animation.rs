//! Renders a real pipeline run as an animated SVG in the neighborhood's
//! own coordinate space (real parcel/pad/building/street geometry, not
//! the abstract commit-graph layout `dump_lineage_graph` draws) -- the
//! site unfolding step by step, through the REAL, FULL corrected
//! pipeline: P37 blocks -> PathNetwork's streets -> P29's density tiers
//! -> per-block P61 squares/P95 pads -> P108's merges -> P96's story
//! counts -> P107's real building massing -> P127/P130/P129/P131's
//! interior partitioning -> P221's openings -> P133's stair cores.
//!
//! Every polygon/polyline is real geometry from the actual `Neighborhood`
//! snapshot at each commit -- not a schematic. Uses SMIL `<animate>` on
//! each entity's opacity (born at the step that created it, faded out at
//! the step that replaced it, if any), so the file is a single self-
//! contained `.svg` with no JS. SMIL only runs when the SVG is the
//! top-level document or inlined directly in an HTML page -- `<img
//! src=...>` won't animate it.
//!
//! # What this can and can't show
//!
//! Every real commit in the corrected pipeline (`pipeline.rs`'s own
//! `run_corrected_pipeline_with_p37_traced`, which this mirrors exactly --
//! same operators, same targets, same per-block P61 area-budget split,
//! same seed derivation) gets a real step and a real label. But not every
//! step changes something this renderer draws: P29 (density-tier tagging),
//! P96 (story-count tagging), and P127/P130/P129/P131/P221/P133 (interior
//! cells, entrance/common-area tags, connections, window/door openings,
//! stair cores) all mutate a building/parcel's own FIELDS in place --
//! same id, same footprint -- rather than adding or removing a top-level
//! entity. This renderer only draws each entity's OUTER footprint
//! (building interior partition lines and door/window openings aren't
//! rendered at all, a real limitation, not a bug), so those steps
//! legitimately produce no new visible shape -- the step counter and
//! label still advance, honestly reflecting that the commit really ran,
//! even though there's nothing new to fade in.
//!
//! Usage:
//!   cargo run -p street-smarts-ledger --release --example dump_lineage_animation -- \
//!       <fixture.json> <parcel_id> <seed> <out.svg> [seconds_per_step]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_core::Scope;
use street_smarts_ledger::{HistoryStore, InMemoryHistoryStore, NeighborhoodId};
use street_smarts_patterns::p107_wings_of_light::{P107Params, P107WingsOfLight};
use street_smarts_patterns::p108_connected_buildings::{P108ConnectedBuildings, P108Params};
use street_smarts_patterns::p127_intimacy_gradient::{P127IntimacyGradient, P127Params};
use street_smarts_patterns::p129_common_areas_at_the_heart::{P129CommonAreasAtTheHeart, P129Params};
use street_smarts_patterns::p130_entrance_room::{P130EntranceRoom, P130Params};
use street_smarts_patterns::p131_the_flow_through_rooms::{P131Params, P131TheFlowThroughRooms};
use street_smarts_patterns::p133_staircase_as_a_stage::{P133Params, P133StaircaseAsAStage};
use street_smarts_patterns::p221_natural_doors_and_windows::{P221NaturalDoorsAndWindows, P221Params};
use street_smarts_patterns::p29_density_rings::{P29DensityRings, P29Params};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p61_small_public_squares::{P61Params, P61SmallPublicSquares};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::p96_number_of_stories::{P96NumberOfStories, P96Params};
use street_smarts_patterns::path_network::{PathNetwork, PathNetworkParams};
use street_smarts_patterns::pipeline::allocate_squares_by_area;
use street_smarts_patterns::{DynOperator, Parameters};

/// An entity's real geometry -- a filled area (parcel/open-space/building,
/// with real holes for a P107 courtyard building) or a stroked line (a
/// street's real centerline, bulges included).
enum Geometry {
    Area { outer: Vec<LngLat>, holes: Vec<Vec<LngLat>> },
    Line(Vec<LngLat>),
}

struct Entity {
    geometry: Geometry,
    color: &'static str,
    /// 0 = present in the baseline fixture; N = introduced by the Nth
    /// commit in `steps`.
    born_step: usize,
    /// The commit that replaced this entity, if any -- `None` means it
    /// survives to the final frame.
    removed_step: Option<usize>,
}

/// Colors by the entity's REAL type/fields wherever one reliably
/// distinguishes it (`Parcel.use_category`, `OpenSpace.kind`,
/// `Street.classification`) -- id-substring matching only for the one
/// case with no other real signal: P95's courtyard `OpenSpace` and P61's
/// real public squares share the exact same `OpenSpaceKind::Plaza`, so
/// P95's own `"{parcel}_P95_courtyard_p{n}"` naming convention (an id
/// format that operator itself constructs and owns, not a guess) is the
/// only way to tell them apart. Same for P108's `"p108_merged_{n}"` --
/// its own real synthetic id convention, not inferred.
fn entity_color(n: &Neighborhood, id: &str) -> &'static str {
    if id.starts_with("p108_merged_") {
        return "#f5a623"; // merged pad (P108)
    }
    if id.contains("_P95_courtyard_p") {
        return "#4fb3a9"; // P95 courtyard (real OpenSpaceKind::Plaza, P95's own naming)
    }
    if n.buildings.iter().any(|b| b.id == id) {
        return "#a0522d"; // real building massing (P107)
    }
    if let Some(s) = n.streets.iter().find(|s| s.id == id) {
        return match s.classification.as_deref() {
            Some("local") => "#2c3e50",
            _ => "#16a085", // pedestrian, or unclassified
        };
    }
    if let Some(o) = n.open_space.iter().find(|o| o.id == id) {
        return match o.kind {
            OpenSpaceKind::Common => "#6ab04c", // P37's informal common land
            OpenSpaceKind::Plaza => "#f1c40f",  // P61's real public square
            _ => "#95a5a6",
        };
    }
    if let Some(p) = n.parcels.iter().find(|p| p.id == id) {
        return match p.use_category.as_deref() {
            Some("house_cluster_block") => "#8064a2", // P37 block
            Some("p95_building_pad") => "#5b9bd5",    // P95 pad, unmerged
            _ => "#c0392b",                           // raw pre-P37 parcel
        };
    }
    "#c0392b"
}

fn geometry_of(n: &Neighborhood, id: &str) -> Geometry {
    if let Some(s) = n.streets.iter().find(|s| s.id == id) {
        return Geometry::Line(s.centerline.clone());
    }
    if let Some(b) = n.buildings.iter().find(|b| b.id == id) {
        let part = b.polygon.parts_view().into_iter().next().unwrap_or_default_part();
        return Geometry::Area { outer: part.outer, holes: part.holes };
    }
    if let Some(o) = n.open_space.iter().find(|o| o.id == id) {
        let part = o.polygon.parts_view().into_iter().next().unwrap_or_default_part();
        return Geometry::Area { outer: part.outer, holes: part.holes };
    }
    if let Some(p) = n.parcels.iter().find(|p| p.id == id) {
        let part = p.polygon.parts_view().into_iter().next().unwrap_or_default_part();
        return Geometry::Area { outer: part.outer, holes: part.holes };
    }
    panic!("entity {id} not found in snapshot");
}

/// Local convenience: `parts_view()`'s first part, or an empty part for a
/// degenerate (empty-ring) polygon rather than panicking on `.unwrap()`.
trait FirstPartOrEmpty {
    fn unwrap_or_default_part(self) -> street_smarts_core::geometry::PolygonPart;
}
impl FirstPartOrEmpty for Option<street_smarts_core::geometry::PolygonPart> {
    fn unwrap_or_default_part(self) -> street_smarts_core::geometry::PolygonPart {
        self.unwrap_or(street_smarts_core::geometry::PolygonPart { outer: vec![], holes: vec![] })
    }
}

fn entity_ids(n: &Neighborhood) -> BTreeSet<String> {
    n.parcels.iter().map(|p| p.id.clone())
        .chain(n.open_space.iter().map(|o| o.id.clone()))
        .chain(n.buildings.iter().map(|b| b.id.clone()))
        .chain(n.streets.iter().map(|s| s.id.clone()))
        .collect()
}

struct Step {
    operator_name: String,
    target: String,
    seed: u64,
}

/// Diffs the just-materialized snapshot against `prev_ids`, records any
/// newly-appeared entity (born at this step) and any entity that
/// disappeared (removed at this step -- e.g. a P95 pad P108 just merged
/// away), then pushes the step itself. Kept as a plain function taking
/// `store`/`entities`/etc. explicitly, not a capturing closure, so it can
/// be called freely between `&mut store` borrows elsewhere in `main`.
fn record_step(
    store: &InMemoryHistoryStore,
    id: NeighborhoodId,
    commit_op: String,
    commit_target: String,
    commit_seed: u64,
    prev_ids: &mut BTreeSet<String>,
    entities: &mut BTreeMap<String, Entity>,
    steps: &mut Vec<Step>,
) {
    let snapshot = store.materialize(&id).expect("just-computed commit must materialize");
    let now_ids = entity_ids(&snapshot);
    let step_idx = steps.len() + 1;
    for added in now_ids.difference(prev_ids) {
        entities.insert(added.clone(), Entity {
            geometry: geometry_of(&snapshot, added),
            color: entity_color(&snapshot, added),
            born_step: step_idx,
            removed_step: None,
        });
    }
    for removed in prev_ids.difference(&now_ids) {
        if let Some(e) = entities.get_mut(removed) {
            e.removed_step = Some(step_idx);
        }
    }
    *prev_ids = now_ids;
    steps.push(Step { operator_name: commit_op, target: commit_target, seed: commit_seed });
}

/// Tries `op` at `target`/`params`/`seed`; on success, records the commit
/// as a real step and advances `cur`. On failure (a real, expected skip --
/// e.g. a block too small for P61/P95, or an operator with nothing left
/// to do), leaves everything untouched, exactly like `pipeline.rs`'s own
/// `if let Ok(...)` tolerance.
#[allow(clippy::too_many_arguments)]
fn try_run_and_record(
    store: &mut InMemoryHistoryStore,
    op: &dyn DynOperator,
    target: &str,
    params: &serde_json::Value,
    seed: u64,
    cur: &mut NeighborhoodId,
    prev_ids: &mut BTreeSet<String>,
    entities: &mut BTreeMap<String, Entity>,
    steps: &mut Vec<Step>,
) -> bool {
    match store.get_or_compute(*cur, op, target, params, seed, "v1") {
        Ok(next) => {
            record_step(store, next, op.name().to_string(), target.to_string(), seed, prev_ids, entities, steps);
            *cur = next;
            true
        }
        Err(_) => false,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        eprintln!("usage: dump_lineage_animation <fixture.json> <parcel_id> <seed> <out.svg> [seconds_per_step]");
        std::process::exit(1);
    }
    let fixture_path = &args[1];
    let parcel_id = &args[2];
    let seed: u64 = args[3].parse().expect("seed must be a u64");
    let out_path = &args[4];
    let seconds_per_step: f64 = args.get(5).map(|s| s.parse().expect("seconds_per_step must be a number")).unwrap_or(1.1);

    let raw = std::fs::read_to_string(fixture_path)
        .unwrap_or_else(|e| panic!("couldn't read {fixture_path}: {e}"));
    let baseline: Neighborhood = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("couldn't parse {fixture_path}: {e}"));

    let mut store = InMemoryHistoryStore::new();
    let root_id = store.insert_root(&baseline);

    let mut entities: BTreeMap<String, Entity> = BTreeMap::new();
    let mut steps: Vec<Step> = Vec::new();
    // The diff loop needs the FULL baseline id set to correctly detect
    // what P37 adds/removes, but only `parcel_id` itself is worth drawing
    // at step 0 -- a real fixture like eastside-baseline.json carries
    // hundreds of unrelated neighboring lots this run never touches;
    // rendering all of them would bury the one parcel actually being
    // redeveloped in a sea of static, irrelevant context.
    let mut prev_ids = entity_ids(&baseline);
    for id in prev_ids.iter().filter(|id| id.as_str() == parcel_id.as_str()) {
        entities.insert(id.clone(), Entity {
            geometry: geometry_of(&baseline, id),
            color: entity_color(&baseline, id),
            born_step: 0,
            removed_step: None,
        });
    }

    let mut cur = root_id;

    // The exact same 14-stage sequence, targets, and skip-tolerance as
    // `pipeline.rs`'s own `run_corrected_pipeline_with_p37_traced` -- see
    // that function's doc comment for the full real rationale behind this
    // order. Every commit here is a REAL `get_or_compute` call, not a
    // narrated guess.
    try_run_and_record(&mut store, &P37HouseCluster, parcel_id, &P37Params::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);

    try_run_and_record(&mut store, &PathNetwork, "*", &PathNetworkParams::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);

    try_run_and_record(&mut store, &P29DensityRings, "*", &P29Params::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);

    // Site-scale square budget split across blocks by area -- identical
    // computation to `pipeline.rs`'s own, reusing its real `pub fn` rather
    // than a second, independently-maintained copy.
    let after_p29 = store.materialize(&cur).unwrap();
    let block_ids: Vec<String> = after_p29.select_ids(&Scope::Block);
    eprintln!("{} block(s)", block_ids.len());
    let block_areas: Vec<f64> = block_ids.iter()
        .map(|id| after_p29.parcels.iter().find(|p| &p.id == id).map(|p| p.polygon.area_m2()).unwrap_or(0.0))
        .collect();
    let total_squares = P61Params::defaults().max_squares.round().max(1.0) as usize;
    let square_counts = allocate_squares_by_area(&block_areas, total_squares);

    for (i, block_id) in block_ids.iter().enumerate() {
        let block_seed = seed + i as u64 + 1;
        let n_squares = square_counts[i];
        if n_squares > 0 {
            // `P61SmallPublicSquares::apply` with no existing Plaza on this
            // block falls through to the same `place_new_squares_n` logic
            // `pipeline.rs` calls directly, given the SAME target square
            // count via `max_squares` -- see p61_small_public_squares.rs's
            // own `place_new_squares` thin wrapper. Going through the
            // typed `Params`/`DynOperator` interface here (instead of that
            // private free function) is what lets this run through
            // `get_or_compute` at all.
            let p61_params = P61Params { max_squares: n_squares as f64, ..P61Params::defaults() };
            try_run_and_record(&mut store, &P61SmallPublicSquares, block_id, &p61_params.as_map(), block_seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);
        }
        try_run_and_record(&mut store, &P95BuildingComplex, block_id, &P95Params::defaults().as_map(), block_seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);
    }

    try_run_and_record(&mut store, &P108ConnectedBuildings, "*", &P108Params::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);
    try_run_and_record(&mut store, &P96NumberOfStories, "*", &P96Params::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);
    try_run_and_record(&mut store, &P107WingsOfLight, "*", &P107Params::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);
    try_run_and_record(&mut store, &P127IntimacyGradient, "*", &P127Params::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);
    try_run_and_record(&mut store, &P130EntranceRoom, "*", &P130Params::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);
    try_run_and_record(&mut store, &P129CommonAreasAtTheHeart, "*", &P129Params::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);
    try_run_and_record(&mut store, &P131TheFlowThroughRooms, "*", &P131Params::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);
    try_run_and_record(&mut store, &P221NaturalDoorsAndWindows, "*", &P221Params::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);
    // AFTER P221, not right after P131 -- Building.floors isn't set until
    // P221 derives it from real height. See pipeline.rs's own step 14 doc.
    try_run_and_record(&mut store, &P133StaircaseAsAStage, "*", &P133Params::defaults().as_map(), seed, &mut cur, &mut prev_ids, &mut entities, &mut steps);

    eprintln!("{} commit(s), {} entities total", steps.len(), entities.len());

    let svg = render_animated_svg(&entities, &steps, seconds_per_step);
    if let Some(parent) = std::path::Path::new(out_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out_path, svg).unwrap_or_else(|e| panic!("couldn't write {out_path}: {e}"));
    println!("{out_path}: {} step(s), {} entities, {:.1}s loop", steps.len(), entities.len(), steps.len() as f64 * seconds_per_step / 0.9);
}

/// Fraction of the loop spent stepping through the pipeline; the rest is a
/// hold on the finished frame before it loops, so a viewer actually gets
/// to look at the end state instead of it flashing straight into a reset.
const CONTENT_FRAC: f64 = 0.9;
const FADE_FRAC: f64 = 0.012;
const PX_PER_METER: f64 = 0.9;
const MARGIN_PX: f64 = 30.0;

fn render_animated_svg(entities: &BTreeMap<String, Entity>, steps: &[Step], seconds_per_step: f64) -> String {
    let n = steps.len().max(1);
    let total_dur = n as f64 * seconds_per_step / CONTENT_FRAC;

    // Local-meter projection, single fixed frame for the whole animation
    // (nothing here ever moves; entities only appear/disappear) -- same
    // flat-earth approximation p29_density_rings and friends already use
    // elsewhere in this codebase, valid at single-parcel scale.
    let all_pts: Vec<&LngLat> = entities.values().flat_map(|e| match &e.geometry {
        Geometry::Area { outer, .. } => outer.iter(),
        Geometry::Line(pts) => pts.iter(),
    }).collect();
    let lat0 = all_pts.iter().map(|p| p.lat).sum::<f64>() / all_pts.len().max(1) as f64;
    let lng_min = all_pts.iter().map(|p| p.lng).fold(f64::INFINITY, f64::min);
    let lat_max = all_pts.iter().map(|p| p.lat).fold(f64::NEG_INFINITY, f64::max);
    let m_per_deg_lat = 110_540.0;
    let m_per_deg_lng = 111_320.0 * lat0.to_radians().cos();
    let project = |p: &LngLat| -> (f64, f64) {
        let x_m = (p.lng - lng_min) * m_per_deg_lng;
        let y_m = (lat_max - p.lat) * m_per_deg_lat; // north-up: increasing lat -> decreasing svg y
        (x_m * PX_PER_METER + MARGIN_PX, y_m * PX_PER_METER + MARGIN_PX + 40.0) // +40 header clearance
    };

    let max_x = all_pts.iter().map(|p| project(p).0).fold(0.0, f64::max);
    let max_y = all_pts.iter().map(|p| project(p).1).fold(0.0, f64::max);
    let width = max_x + MARGIN_PX;
    let height = max_y + MARGIN_PX + 20.0; // extra clearance for the 2-row legend

    let mut body = String::new();
    for (id, e) in entities {
        let born_t = (e.born_step as f64 / n as f64) * CONTENT_FRAC;
        let removed_t = e.removed_step.map(|r| (r as f64 / n as f64) * CONTENT_FRAC);
        let anim = opacity_animate(born_t, removed_t, FADE_FRAC);
        let static_opacity = if anim.is_none() { 1.0 } else { 0.0 };
        let anim_tag = anim.map(|(kt, v)| format!(
            r##"<animate attributeName="opacity" dur="{total_dur}s" repeatCount="indefinite" keyTimes="{kt}" values="{v}" calcMode="linear"/>"##
        )).unwrap_or_default();

        match &e.geometry {
            Geometry::Area { outer, holes } => {
                let d = area_path_d(outer, holes, &project);
                let _ = writeln!(
                    body,
                    r##"<path d="{d}" fill="{color}" fill-rule="evenodd" fill-opacity="0.75" stroke="{color}" stroke-width="1" opacity="{op}">{anim_tag}<title>{id}</title></path>"##,
                    d = d, color = e.color, op = static_opacity,
                );
            }
            Geometry::Line(pts) => {
                let d = line_path_d(pts, &project);
                let _ = writeln!(
                    body,
                    r##"<path d="{d}" fill="none" stroke="{color}" stroke-width="2.5" stroke-linecap="round" opacity="{op}">{anim_tag}<title>{id}</title></path>"##,
                    d = d, color = e.color, op = static_opacity,
                );
            }
        }
    }

    let mut labels = String::new();
    for (i, step) in steps.iter().enumerate() {
        let idx = i + 1;
        let start_t = (i as f64 / n as f64) * CONTENT_FRAC;
        // Each label is on for exactly its own step's window; the LAST
        // label holds through the end-of-loop pause too, so the viewer
        // gets to read the final step's caption while looking at the
        // finished frame, not just a blank one.
        let (kt, v) = if idx == n {
            (format!("0;{start_t};1"), "0;1;1".to_string())
        } else {
            let end_t = (idx as f64 / n as f64) * CONTENT_FRAC;
            (format!("0;{start_t};{end_t};1"), "0;1;0;0".to_string())
        };
        let _ = writeln!(
            labels,
            r##"<text x="12" y="24" font-family="monospace" font-size="15" fill="#111111" opacity="0"><animate attributeName="opacity" dur="{total_dur}s" repeatCount="indefinite" keyTimes="{kt}" values="{v}" calcMode="discrete"/>step {idx}/{n}: {op} target={target} seed={seed}</text>"##,
            op = xml_escape(&step.operator_name), target = xml_escape(&step.target), seed = step.seed,
        );
    }

    // Two-row legend -- ten real entity kinds now (was five), too many for
    // one row at a readable font size.
    let legend_y1 = height - 32.0;
    let legend_y2 = height - 16.0;
    let row1 = [
        ("#c0392b", "raw parcel"), ("#8064a2", "block (P37)"), ("#2c3e50", "street, local (P52)"),
        ("#16a085", "street, pedestrian (P52/P61)"), ("#6ab04c", "common land (P37)"),
    ];
    let row2 = [
        ("#f1c40f", "square (P61)"), ("#5b9bd5", "pad, unmerged (P95)"), ("#4fb3a9", "courtyard (P95)"),
        ("#f5a623", "merged pad (P108)"), ("#a0522d", "building (P107)"),
    ];
    let mut legend = String::new();
    let mut x = 12.0;
    for (color, label) in row1 {
        let _ = writeln!(legend, r##"<rect x="{x}" y="{y}" width="10" height="10" fill="{color}"/><text x="{tx}" y="{ty}">{label}</text>"##,
            y = legend_y1 - 9.0, tx = x + 14.0, ty = legend_y1);
        x += 14.0 + label.len() as f64 * 5.6 + 14.0;
    }
    x = 12.0;
    for (color, label) in row2 {
        let _ = writeln!(legend, r##"<rect x="{x}" y="{y}" width="10" height="10" fill="{color}"/><text x="{tx}" y="{ty}">{label}</text>"##,
            y = legend_y2 - 9.0, tx = x + 14.0, ty = legend_y2);
        x += 14.0 + label.len() as f64 * 5.6 + 14.0;
    }

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
<rect x="0" y="0" width="{width}" height="{height}" fill="#ffffff"/>
<g font-family="monospace" font-size="10">
{legend}
</g>
{labels}
{body}
</svg>
"##,
    )
}

fn area_path_d(outer: &[LngLat], holes: &[Vec<LngLat>], project: &impl Fn(&LngLat) -> (f64, f64)) -> String {
    let mut d = ring_path_d(outer, project);
    for h in holes {
        d.push(' ');
        d.push_str(&ring_path_d(h, project));
    }
    d
}

fn ring_path_d(ring: &[LngLat], project: &impl Fn(&LngLat) -> (f64, f64)) -> String {
    let mut d = String::new();
    for (i, p) in ring.iter().enumerate() {
        let (x, y) = project(p);
        if i == 0 {
            let _ = write!(d, "M {x:.1} {y:.1} ");
        } else {
            let _ = write!(d, "L {x:.1} {y:.1} ");
        }
    }
    d.push('Z');
    d
}

fn line_path_d(pts: &[LngLat], project: &impl Fn(&LngLat) -> (f64, f64)) -> String {
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        let (x, y) = project(p);
        if i == 0 {
            let _ = write!(d, "M {x:.1} {y:.1} ");
        } else {
            let _ = write!(d, "L {x:.1} {y:.1} ");
        }
    }
    d
}

/// Builds a `(keyTimes, values)` pair for an entity's opacity: invisible
/// before `born_t`, a short crossfade in, visible until `removed_t` (if
/// any, with a crossfade out), invisible after. `None` means "just leave
/// it statically visible" -- born at the very start and never removed.
fn opacity_animate(born_t: f64, removed_t: Option<f64>, fade: f64) -> Option<(String, String)> {
    if born_t <= 0.0 && removed_t.is_none() {
        return None;
    }
    match removed_t {
        Some(r) => {
            let f = fade.min((r - born_t).max(0.0) / 3.0).max(0.0);
            if born_t <= 0.0 {
                Some((format!("0;{};{};1", (r - f).max(0.0), r), "1;1;0;0".to_string()))
            } else {
                Some((
                    format!("0;{born_t};{};{};{r};1", born_t + f, (r - f).max(born_t + f)),
                    "0;0;1;1;0;0".to_string(),
                ))
            }
        }
        None => Some((format!("0;{born_t};{};1", born_t + fade), "0;0;1;1".to_string())),
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
