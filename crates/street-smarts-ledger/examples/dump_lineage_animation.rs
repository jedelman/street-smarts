//! Renders a real pipeline run as an animated SVG in the neighborhood's
//! own coordinate space (real parcel/pad/building/street geometry, not
//! the abstract commit-graph layout `dump_lineage_graph` draws) -- the
//! site unfolding step by step, through the REAL, FULL corrected
//! pipeline (`street_smarts_ledger::run_corrected_pipeline_via_ledger` --
//! see that function's own doc for why this and `examples/dump_pipeline.rs`
//! both build on it instead of each independently re-deriving the same
//! 14-stage sequence): P37 blocks -> PathNetwork's streets -> P29's
//! density tiers -> per-block P61 squares/P95 pads -> P108's merges ->
//! P96's story counts -> P107's real building massing -> P127/P130/P129/
//! P131's interior partitioning -> P221's openings -> P133's stair cores.
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
//! Every real commit in the corrected pipeline gets a real step and a
//! real label. But not every step changes something this renderer draws:
//! P29 (density-tier tagging), P96 (story-count tagging), and P127/P130/
//! P129/P131/P221/P133 (interior cells, entrance/common-area tags,
//! connections, window/door openings, stair cores) all mutate a
//! building/parcel's own FIELDS in place -- same id, same footprint --
//! rather than adding or removing a top-level entity. This renderer only
//! draws each entity's OUTER footprint (building interior partition
//! lines and door/window openings aren't rendered at all, a real
//! limitation, not a bug), so those steps legitimately produce no new
//! visible shape -- the step counter and label still advance, honestly
//! reflecting that the commit really ran, even though there's nothing
//! new to fade in.
//!
//! One operator does NOT fit that "same id, same footprint" pattern:
//! `p107_wings_of_light` (and `building_shape.rs`) replace every pad's
//! `use_category` (pad -> `PadRole::PadWithBuilding`) via
//! `replaced_parcel_ids`/`new_parcels`, but REUSE the pad's own id for
//! the replacement instead of minting a new one -- a real, deliberate
//! choice (so a building's source pad stays a legible, same-id record),
//! but it means simple id-set add/remove diffing can't see that a real
//! content swap happened. Without special-casing it, the ORIGINAL pad
//! shape/color sits at full opacity forever, under the real building
//! fading in at the same step -- worst for P108's large merged/courtyard
//! pads, where the real building is a thin ring around a hole and the
//! solid stale pad underneath swallows almost all of it visually
//! (confirmed by rendering a real frame before this fix: the whole site
//! read as solid P108-orange, with only slivers of real P107-brown
//! building visible at the edges). Handled below by tracking each parcel
//! id's own `use_category` across steps and fading an id out the moment
//! its role changes, even though its id never left `entity_ids()`'s set
//! -- detected off the raw field, not off `entity_color()`'s output,
//! since P108's own `id.starts_with("p108_merged_")` shortcut in that
//! function returns the same orange regardless of category and would
//! miss exactly this case. No replacement entity is reinserted under the
//! same id -- the real replacement is always a separate new id (the
//! building itself) already covered by the ordinary add path.
//!
//! Usage:
//!   cargo run -p street-smarts-ledger --release --example dump_lineage_animation -- \
//!       <fixture.json> <parcel_id> <seed> <out.svg> [seconds_per_step]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::{Neighborhood, OpenSpaceKind};
use street_smarts_ledger::{run_corrected_pipeline_via_ledger, Commit, HistoryStore, InMemoryHistoryStore};

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
/// `Street.classification`) -- id-substring matching only for cases with
/// no other real signal: P95's/P107's courtyard `OpenSpace`s and P61's
/// real public squares share the exact same `OpenSpaceKind::Plaza`, so
/// their own id-naming conventions (formats those operators construct
/// and own, not a guess -- `"{parcel}_P95_courtyard_p{n}"`,
/// `"{parcel}_P107_courtyard"`) are the only way to tell them apart; same
/// for P108's `"p108_merged_{n}"` parcel ids.
///
/// The real-type checks (`n.buildings`/`n.streets`/`n.open_space` lookups)
/// run FIRST, before any id-substring check, and the id-substring checks
/// are scoped inside their own branch (only applied once we already know
/// `id` names an OpenSpace, or a Parcel) rather than at the top level --
/// P107 and P108 both build their own new ids by literally appending a
/// suffix to their SOURCE pad's id (`{pad_id}_building`,
/// `{pad_id}_P107_courtyard`), so a top-level `id.starts_with
/// ("p108_merged_")` check (this function's own earlier form) would ALSO
/// match a merged pad's own derived building/courtyard ids and miscolor
/// them as "merged pad" orange instead of their real color -- confirmed
/// by rendering a real frame: every P108-merged building/courtyard on
/// site still rendered orange even after the pad's own stale-shape bug
/// (see this module's own doc) was fixed, because THIS was masking it.
fn entity_color(n: &Neighborhood, id: &str) -> &'static str {
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
        if id.contains("_P95_courtyard_p") || id.ends_with("_P107_courtyard") {
            return "#4fb3a9"; // building courtyard (P95 or P107), real OpenSpaceKind::Plaza either way
        }
        return match o.kind {
            OpenSpaceKind::Common => "#6ab04c", // P37's informal common land
            OpenSpaceKind::Plaza => "#f1c40f",  // P61's real public square
            _ => "#95a5a6",
        };
    }
    if let Some(p) = n.parcels.iter().find(|p| p.id == id) {
        if id.starts_with("p108_merged_") {
            return "#f5a623"; // merged pad (P108) -- safe here: only reached once `id` is confirmed to name a real Parcel, not a derived building/courtyard id that merely shares the prefix
        }
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
        let part = first_part_or_empty(b.polygon.parts_view());
        return Geometry::Area { outer: part.outer, holes: part.holes };
    }
    if let Some(o) = n.open_space.iter().find(|o| o.id == id) {
        let part = first_part_or_empty(o.polygon.parts_view());
        return Geometry::Area { outer: part.outer, holes: part.holes };
    }
    if let Some(p) = n.parcels.iter().find(|p| p.id == id) {
        let part = first_part_or_empty(p.polygon.parts_view());
        return Geometry::Area { outer: part.outer, holes: part.holes };
    }
    panic!("entity {id} not found in snapshot");
}

fn first_part_or_empty(parts: Vec<street_smarts_core::geometry::PolygonPart>) -> street_smarts_core::geometry::PolygonPart {
    parts.into_iter().next().unwrap_or(street_smarts_core::geometry::PolygonPart { outer: vec![], holes: vec![] })
}

/// A parcel's own `use_category`, if `id` names a parcel -- the raw field
/// `p107_wings_of_light`'s in-place-replacement (see this module's own
/// doc) is detected off, since `entity_color()` can shortcut past it for
/// `p108_merged_` ids.
fn parcel_use_category<'a>(n: &'a Neighborhood, id: &str) -> Option<&'a str> {
    n.parcels.iter().find(|p| p.id == id).and_then(|p| p.use_category.as_deref())
}

fn entity_ids(n: &Neighborhood) -> BTreeSet<String> {
    n.parcels.iter().map(|p| p.id.clone())
        .chain(n.open_space.iter().map(|o| o.id.clone()))
        .chain(n.buildings.iter().map(|b| b.id.clone()))
        .chain(n.streets.iter().map(|s| s.id.clone()))
        .collect()
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

    // The single real pipeline run -- same function `dump_pipeline.rs`
    // uses for the final-state JSON, so there's exactly one computation
    // of "what does this pipeline produce," not two that could drift.
    let (_final_id, commits) = run_corrected_pipeline_via_ledger(&mut store, root_id, parcel_id, seed);
    eprintln!("{} real commit(s)", commits.len());

    let mut entities: BTreeMap<String, Entity> = BTreeMap::new();
    // Each parcel id's own `use_category` as of the last step we looked --
    // the signal the in-place-replacement check below diffs on.
    let mut role: BTreeMap<String, Option<String>> = BTreeMap::new();

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
        role.insert(id.clone(), parcel_use_category(&baseline, id).map(str::to_string));
    }

    for (step_idx, commit) in commits.iter().enumerate() {
        let step_idx = step_idx + 1; // 1-based, matching the label text
        let snapshot = store.materialize(&commit.id).expect("recorded commit must materialize");
        let now_ids = entity_ids(&snapshot);
        for added in now_ids.difference(&prev_ids) {
            entities.insert(added.clone(), Entity {
                geometry: geometry_of(&snapshot, added),
                color: entity_color(&snapshot, added),
                born_step: step_idx,
                removed_step: None,
            });
            role.insert(added.clone(), parcel_use_category(&snapshot, added).map(str::to_string));
        }
        for removed in prev_ids.difference(&now_ids) {
            if let Some(e) = entities.get_mut(removed) {
                e.removed_step = Some(step_idx);
            }
            role.remove(removed);
        }
        // Ids that survive this commit unchanged in the id-set sense might
        // still have been swapped out in place -- p107_wings_of_light's
        // (and building_shape.rs's) pad -> PadRole::PadWithBuilding
        // recategorization, same id, is the real case (see module doc).
        // Fade the pad here rather than reinsert a "reborn" entity under
        // it: the real replacement is always a SEPARATE new entity (the
        // building itself, a distinct id) born at this same step via the
        // `added` loop above, so fading without reinserting is what
        // actually lets that real building read clearly instead of being
        // covered by another same-shaped overlay. Detected off the raw
        // field, not `entity_color()`, since that function's
        // `p108_merged_` id-prefix shortcut would mask the change for
        // exactly the pads where it matters most.
        for id in now_ids.intersection(&prev_ids) {
            let new_role = parcel_use_category(&snapshot, id).map(str::to_string);
            if role.get(id).cloned().unwrap_or(None) != new_role {
                if let Some(e) = entities.get_mut(id) {
                    e.removed_step = Some(step_idx);
                }
                role.insert(id.clone(), new_role);
            }
        }
        prev_ids = now_ids;
    }

    eprintln!("{} entities total", entities.len());

    let svg = render_animated_svg(&entities, &commits, seconds_per_step);
    if let Some(parent) = std::path::Path::new(out_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out_path, svg).unwrap_or_else(|e| panic!("couldn't write {out_path}: {e}"));
    println!("{out_path}: {} step(s), {} entities, {:.1}s loop", commits.len(), entities.len(), commits.len() as f64 * seconds_per_step / 0.9);
}

/// Fraction of the loop spent stepping through the pipeline; the rest is a
/// hold on the finished frame before it loops, so a viewer actually gets
/// to look at the end state instead of it flashing straight into a reset.
const CONTENT_FRAC: f64 = 0.9;
const FADE_FRAC: f64 = 0.012;
const PX_PER_METER: f64 = 0.9;
const MARGIN_PX: f64 = 30.0;

fn render_animated_svg(entities: &BTreeMap<String, Entity>, commits: &[Commit], seconds_per_step: f64) -> String {
    let n = commits.len().max(1);
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
    for (i, commit) in commits.iter().enumerate() {
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
            op = xml_escape(&commit.operator_name), target = xml_escape(&commit.target), seed = commit.seed,
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
        ("#f1c40f", "square (P61)"), ("#5b9bd5", "pad, unmerged (P95)"), ("#4fb3a9", "courtyard (P95/P107)"),
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
