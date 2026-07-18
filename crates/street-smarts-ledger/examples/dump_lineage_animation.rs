//! Renders a real pipeline run as an animated SVG in the neighborhood's
//! own coordinate space (real parcel/pad footprints, not the abstract
//! commit-graph layout `dump_lineage_graph` draws) -- the site unfolding
//! step by step: the raw parcel gives way to P37's blocks, each block's
//! P95 pads/courtyard fade in as that block's commit runs, and P108
//! merges fade the source pads out as the merged footprint fades in.
//!
//! Every polygon is real geometry from the actual `Neighborhood` snapshot
//! at each commit -- not a schematic. Uses SMIL `<animate>` on each
//! entity's opacity (born at the step that created it, faded out at the
//! step that replaced it, if any), so the file is a single self-contained
//! `.svg` with no JS. SMIL only runs when the SVG is the top-level
//! document or inlined directly in an HTML page -- `<img src=...>` won't
//! animate it.
//!
//! Usage:
//!   cargo run -p street-smarts-ledger --release --example dump_lineage_animation -- \
//!       <fixture.json> <parcel_id> <seed> <out.svg> [seconds_per_step]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::Scope;
use street_smarts_ledger::{HistoryStore, InMemoryHistoryStore, NeighborhoodId};
use street_smarts_patterns::p108_connected_buildings::{P108ConnectedBuildings, P108Params};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::Parameters;

struct Entity {
    ring: Vec<LngLat>,
    color: &'static str,
    /// 0 = present in the baseline fixture; N = introduced by the Nth
    /// commit in `steps`.
    born_step: usize,
    /// The commit that replaced this entity, if any -- `None` means it
    /// survives to the final frame.
    removed_step: Option<usize>,
}

fn entity_kind_color(id: &str) -> &'static str {
    if id.starts_with("p108_merged_") {
        "#f5a623"
    } else if id.contains("_P95_courtyard_p") {
        "#4fb3a9"
    } else if id.contains("_P95_cell_") {
        "#5b9bd5"
    } else if id.starts_with("BLOCK_") {
        "#8064a2"
    } else {
        "#c0392b" // the raw pre-P37 parcel
    }
}

fn entity_ids(n: &Neighborhood) -> BTreeSet<String> {
    n.parcels.iter().map(|p| p.id.clone())
        .chain(n.open_space.iter().map(|o| o.id.clone()))
        .collect()
}

fn ring_of(n: &Neighborhood, id: &str) -> Vec<LngLat> {
    n.parcels.iter().find(|p| p.id == id).map(|p| p.polygon.outer.clone())
        .or_else(|| n.open_space.iter().find(|o| o.id == id).map(|o| o.polygon.outer.clone()))
        .unwrap_or_else(|| panic!("entity {id} not found in snapshot"))
}

struct Step {
    operator_name: String,
    target: String,
    seed: u64,
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
            ring: ring_of(&baseline, id),
            color: entity_kind_color(id),
            born_step: 0,
            removed_step: None,
        });
    }

    let record_step = |store: &InMemoryHistoryStore, id: NeighborhoodId, commit_op: String, commit_target: String, commit_seed: u64,
                            prev_ids: &mut BTreeSet<String>, entities: &mut BTreeMap<String, Entity>, steps: &mut Vec<Step>| {
        let snapshot = store.materialize(&id).expect("just-computed commit must materialize");
        let now_ids = entity_ids(&snapshot);
        let step_idx = steps.len() + 1;
        for added in now_ids.difference(prev_ids) {
            entities.insert(added.clone(), Entity {
                ring: ring_of(&snapshot, added),
                color: entity_kind_color(added),
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
    };

    let sub37 = store
        .get_or_compute(root_id, &P37HouseCluster, parcel_id, &P37Params::defaults().as_map(), seed, "v1")
        .unwrap_or_else(|e| panic!("P37 failed on {parcel_id}: {e}"));
    record_step(&store, sub37, "p37_house_cluster".into(), parcel_id.to_string(), seed, &mut prev_ids, &mut entities, &mut steps);
    let mut cur = sub37;

    let after_p37 = store.materialize(&cur).unwrap();
    let block_ids = after_p37.select_ids(&Scope::Block);
    eprintln!("P37: {} block(s)", block_ids.len());

    for (i, block_id) in block_ids.iter().enumerate() {
        let block_seed = seed + i as u64 + 1;
        match store.get_or_compute(cur, &P95BuildingComplex, block_id, &P95Params::defaults().as_map(), block_seed, "v1") {
            Ok(next) => {
                record_step(&store, next, "p95_building_complex".into(), block_id.clone(), block_seed, &mut prev_ids, &mut entities, &mut steps);
                cur = next;
            }
            Err(e) => eprintln!("P95 skipped {block_id}: {e}"),
        }
    }

    match store.get_or_compute(cur, &P108ConnectedBuildings, "*", &P108Params::defaults().as_map(), seed, "v1") {
        Ok(next) => record_step(&store, next, "p108_connected_buildings".into(), "*".into(), seed, &mut prev_ids, &mut entities, &mut steps),
        Err(e) => eprintln!("P108 skipped: {e}"),
    }

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
    let all_pts: Vec<&LngLat> = entities.values().flat_map(|e| e.ring.iter()).collect();
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
    let height = max_y + MARGIN_PX;

    let mut body = String::new();
    for (id, e) in entities {
        let born_t = (e.born_step as f64 / n as f64) * CONTENT_FRAC;
        let removed_t = e.removed_step.map(|r| (r as f64 / n as f64) * CONTENT_FRAC);
        let d = path_d(&e.ring, &project);
        let anim = opacity_animate(born_t, removed_t, FADE_FRAC);
        let static_opacity = if anim.is_none() { 1.0 } else { 0.0 };
        let _ = writeln!(
            body,
            r##"<path d="{d}" fill="{color}" fill-opacity="0.75" stroke="{color}" stroke-width="1" opacity="{op}">{anim_tag}<title>{id}</title></path>"##,
            d = d, color = e.color, op = static_opacity,
            anim_tag = anim.map(|(kt, v)| format!(
                r##"<animate attributeName="opacity" dur="{total_dur}s" repeatCount="indefinite" keyTimes="{kt}" values="{v}" calcMode="linear"/>"##
            )).unwrap_or_default(),
        );
        let _ = id;
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

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
<rect x="0" y="0" width="{width}" height="{height}" fill="#ffffff"/>
<g font-family="monospace" font-size="10">
<rect x="{lx0}" y="{ly}" width="10" height="10" fill="#c0392b"/><text x="{lx1}" y="{lty}">raw parcel</text>
<rect x="{lx2}" y="{ly}" width="10" height="10" fill="#8064a2"/><text x="{lx3}" y="{lty}">block (P37)</text>
<rect x="{lx4}" y="{ly}" width="10" height="10" fill="#5b9bd5"/><text x="{lx5}" y="{lty}">pad (P95)</text>
<rect x="{lx6}" y="{ly}" width="10" height="10" fill="#4fb3a9"/><text x="{lx7}" y="{lty}">courtyard (P95)</text>
<rect x="{lx8}" y="{ly}" width="10" height="10" fill="#f5a623"/><text x="{lx9}" y="{lty}">merged pad (P108)</text>
</g>
{labels}
{body}
</svg>
"##,
        width = width, height = height,
        lx0 = width - 640.0, ly = height - 18.0, lty = height - 9.0,
        lx1 = width - 626.0, lx2 = width - 520.0, lx3 = width - 506.0,
        lx4 = width - 400.0, lx5 = width - 386.0, lx6 = width - 300.0,
        lx7 = width - 286.0, lx8 = width - 160.0, lx9 = width - 146.0,
    )
}

fn path_d(ring: &[LngLat], project: &impl Fn(&LngLat) -> (f64, f64)) -> String {
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
