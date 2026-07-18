//! Combines `dump_lineage_animation`'s real footprints and
//! `dump_lineage_graph`'s provenance edges into one view, georeferenced
//! over a real satellite basemap -- both "artifacts" the user asked to
//! see aligned to parcels, in the same coordinate space as the imagery.
//!
//! Unlike `dump_lineage_graph` (which lays entities out in commit-order
//! columns, no real coordinates), every node here sits at its own
//! footprint's real centroid, and provenance edges are real geographic
//! lines between those centroids drawn straight over the basemap.
//!
//! This tool does NOT fetch the basemap itself (no network dependency in
//! this crate) -- it only computes the exact bbox/pixel-size a caller
//! needs to fetch, and later embeds whatever image is handed back at that
//! same bbox/size, so the two steps stay pixel-aligned by construction
//! rather than by matching floats across a shell round-trip.
//!
//! Usage (two passes):
//!   1. cargo run -p street-smarts-ledger --release --example dump_lineage_map -- \
//!      <fixture.json> <parcel_id> <seed> <out.svg> --print-bbox
//!      -> prints "min_lng min_lat max_lng max_lat width_px height_px" and exits.
//!   2. Fetch a basemap image for exactly that bbox/size (e.g. an ArcGIS
//!      World Imagery `export` request with bboxSR=4326&imageSR=4326&
//!      bbox=<min_lng>,<min_lat>,<max_lng>,<max_lat>&size=<width_px>,<height_px>).
//!   3. cargo run -p street-smarts-ledger --release --example dump_lineage_map -- \
//!      <fixture.json> <parcel_id> <seed> <out.svg> --basemap <image.png> [seconds_per_step]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::Scope;
use street_smarts_ledger::{Commit, HistoryStore, InMemoryHistoryStore, NeighborhoodId};
use street_smarts_patterns::p108_connected_buildings::{P108ConnectedBuildings, P108Params};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::Parameters;

struct Entity {
    ring: Vec<LngLat>,
    color: &'static str,
    born_step: usize,
    removed_step: Option<usize>,
}

struct Step {
    operator_name: String,
    target: String,
    seed: u64,
}

fn entity_kind_color(id: &str) -> &'static str {
    if id.starts_with("p108_merged_") {
        "#f5a623"
    } else if id.contains("_P95_courtyard_p") {
        "#4fb3a9"
    } else if id.contains("_P95_cell_") {
        "#5b9bd5"
    } else if id.contains("_BLOCK_") {
        // Real block ids are "{parcel_id}_BLOCK_{n}" -- never a bare
        // "BLOCK_..." prefix, so a `starts_with` check here always misses.
        "#8064a2"
    } else {
        "#e74c3c" // the raw pre-P37 parcel -- brighter than dump_lineage_animation's
                   // #c0392b so it still reads clearly against varied satellite colors
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        eprintln!("usage: dump_lineage_map <fixture.json> <parcel_id> <seed> <out.svg> (--print-bbox | --basemap <image.png> [seconds_per_step])");
        std::process::exit(1);
    }
    let fixture_path = &args[1];
    let parcel_id = &args[2];
    let seed: u64 = args[3].parse().expect("seed must be a u64");
    let out_path = &args[4];
    let print_bbox_only = args.iter().any(|a| a == "--print-bbox");
    let basemap_path = args.iter().position(|a| a == "--basemap").map(|i| args[i + 1].clone());
    let seconds_per_step: f64 = args.iter().position(|a| a == "--basemap")
        .and_then(|i| args.get(i + 2))
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.1);

    let raw = std::fs::read_to_string(fixture_path)
        .unwrap_or_else(|e| panic!("couldn't read {fixture_path}: {e}"));
    let baseline: Neighborhood = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("couldn't parse {fixture_path}: {e}"));

    let mut store = InMemoryHistoryStore::new();
    let root_id = store.insert_root(&baseline);

    let mut entities: BTreeMap<String, Entity> = BTreeMap::new();
    let mut steps: Vec<Step> = Vec::new();
    let mut edges: Vec<(String, String)> = Vec::new(); // (derived_id, source_id)
    let mut prev_ids = entity_ids(&baseline);
    for id in prev_ids.iter().filter(|id| id.as_str() == parcel_id.as_str()) {
        entities.insert(id.clone(), Entity { ring: ring_of(&baseline, id), color: entity_kind_color(id), born_step: 0, removed_step: None });
    }

    let record_step = |store: &InMemoryHistoryStore, id: NeighborhoodId, op: String, target: String, seed: u64,
                        prev_ids: &mut BTreeSet<String>, entities: &mut BTreeMap<String, Entity>, steps: &mut Vec<Step>, edges: &mut Vec<(String, String)>| {
        let snapshot = store.materialize(&id).expect("just-computed commit must materialize");
        let commit: Commit = store.commit(&id).expect("just-computed commit must be recorded");
        let now_ids = entity_ids(&snapshot);
        let step_idx = steps.len() + 1;
        for added in now_ids.difference(prev_ids) {
            entities.insert(added.clone(), Entity { ring: ring_of(&snapshot, added), color: entity_kind_color(added), born_step: step_idx, removed_step: None });
        }
        for removed in prev_ids.difference(&now_ids) {
            if let Some(e) = entities.get_mut(removed) { e.removed_step = Some(step_idx); }
        }
        for (derived, sources) in &commit.entity_provenance {
            for s in sources {
                edges.push((derived.clone(), s.clone()));
            }
        }
        *prev_ids = now_ids;
        steps.push(Step { operator_name: op, target, seed });
    };

    let sub37 = store.get_or_compute(root_id, &P37HouseCluster, parcel_id, &P37Params::defaults().as_map(), seed, "v1")
        .unwrap_or_else(|e| panic!("P37 failed on {parcel_id}: {e}"));
    record_step(&store, sub37, "p37_house_cluster".into(), parcel_id.to_string(), seed, &mut prev_ids, &mut entities, &mut steps, &mut edges);
    let mut cur = sub37;

    let after_p37 = store.materialize(&cur).unwrap();
    let block_ids = after_p37.select_ids(&Scope::Block);
    eprintln!("P37: {} block(s)", block_ids.len());

    for (i, block_id) in block_ids.iter().enumerate() {
        let block_seed = seed + i as u64 + 1;
        match store.get_or_compute(cur, &P95BuildingComplex, block_id, &P95Params::defaults().as_map(), block_seed, "v1") {
            Ok(next) => {
                record_step(&store, next, "p95_building_complex".into(), block_id.clone(), block_seed, &mut prev_ids, &mut entities, &mut steps, &mut edges);
                cur = next;
            }
            Err(e) => eprintln!("P95 skipped {block_id}: {e}"),
        }
    }

    match store.get_or_compute(cur, &P108ConnectedBuildings, "*", &P108Params::defaults().as_map(), seed, "v1") {
        Ok(next) => record_step(&store, next, "p108_connected_buildings".into(), "*".into(), seed, &mut prev_ids, &mut entities, &mut steps, &mut edges),
        Err(e) => eprintln!("P108 skipped: {e}"),
    }

    eprintln!("{} commit(s), {} entities, {} provenance edge(s)", steps.len(), entities.len(), edges.len());

    let geo = GeoFrame::compute(&entities);

    if print_bbox_only {
        println!("{} {} {} {} {} {}", geo.min_lng, geo.min_lat, geo.max_lng, geo.max_lat, geo.width_px, geo.height_px);
        return;
    }

    let basemap_path = basemap_path.unwrap_or_else(|| {
        eprintln!("error: --basemap <image.png> is required unless --print-bbox is given");
        std::process::exit(1);
    });
    let image_bytes = std::fs::read(&basemap_path).unwrap_or_else(|e| panic!("couldn't read {basemap_path}: {e}"));
    let image_b64 = base64_encode(&image_bytes);
    let mime = if basemap_path.ends_with(".jpg") || basemap_path.ends_with(".jpeg") { "image/jpeg" } else { "image/png" };

    let svg = render_svg(&geo, &image_b64, mime, &entities, &steps, &edges, seconds_per_step);
    if let Some(parent) = std::path::Path::new(out_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out_path, svg).unwrap_or_else(|e| panic!("couldn't write {out_path}: {e}"));
    println!("{out_path}: {}x{}px, {:.1}s loop", geo.width_px, geo.height_px, steps.len() as f64 * seconds_per_step / CONTENT_FRAC);
}

/// A locally-undistorted linear lng/lat -> pixel map: per-axis scale
/// chosen from real ground distance (meters), not raw degrees, so a
/// requested-size basemap tile that fills this same pixel box looks
/// geometrically correct (a circle stays a circle) instead of stretched
/// by the ~20% real/degree ratio this latitude would otherwise introduce.
/// Because both the vector overlay and the basemap fetch use this exact
/// bbox and pixel size, alignment is exact by construction, not by
/// matching a projection formula after the fact.
struct GeoFrame {
    min_lng: f64,
    min_lat: f64,
    max_lng: f64,
    max_lat: f64,
    width_px: u32,
    height_px: u32,
}

const PADDING_FRAC: f64 = 0.18;
const TARGET_PX: f64 = 1300.0;

impl GeoFrame {
    fn compute(entities: &BTreeMap<String, Entity>) -> Self {
        let all_pts: Vec<&LngLat> = entities.values().flat_map(|e| e.ring.iter()).collect();
        let raw_min_lng = all_pts.iter().map(|p| p.lng).fold(f64::INFINITY, f64::min);
        let raw_max_lng = all_pts.iter().map(|p| p.lng).fold(f64::NEG_INFINITY, f64::max);
        let raw_min_lat = all_pts.iter().map(|p| p.lat).fold(f64::INFINITY, f64::min);
        let raw_max_lat = all_pts.iter().map(|p| p.lat).fold(f64::NEG_INFINITY, f64::max);
        let lat_mid = (raw_min_lat + raw_max_lat) / 2.0;
        let pad_lng = (raw_max_lng - raw_min_lng) * PADDING_FRAC;
        let pad_lat = (raw_max_lat - raw_min_lat) * PADDING_FRAC;
        let min_lng = raw_min_lng - pad_lng;
        let max_lng = raw_max_lng + pad_lng;
        let min_lat = raw_min_lat - pad_lat;
        let max_lat = raw_max_lat + pad_lat;

        let m_per_deg_lat = 110_540.0;
        let m_per_deg_lng = 111_320.0 * lat_mid.to_radians().cos();
        let x_m = (max_lng - min_lng) * m_per_deg_lng;
        let y_m = (max_lat - min_lat) * m_per_deg_lat;
        let (width_px, height_px) = if x_m >= y_m {
            (TARGET_PX, (TARGET_PX * y_m / x_m).round().max(1.0))
        } else {
            ((TARGET_PX * x_m / y_m).round().max(1.0), TARGET_PX)
        };
        Self { min_lng, min_lat, max_lng, max_lat, width_px: width_px as u32, height_px: height_px as u32 }
    }

    fn project(&self, p: &LngLat) -> (f64, f64) {
        let x = (p.lng - self.min_lng) / (self.max_lng - self.min_lng) * self.width_px as f64;
        let y = (self.max_lat - p.lat) / (self.max_lat - self.min_lat) * self.height_px as f64;
        (x, y)
    }
}

const CONTENT_FRAC: f64 = 0.9;
const FADE_FRAC: f64 = 0.012;

fn render_svg(geo: &GeoFrame, image_b64: &str, mime: &str, entities: &BTreeMap<String, Entity>, steps: &[Step], edges: &[(String, String)], seconds_per_step: f64) -> String {
    let n = steps.len().max(1);
    let total_dur = n as f64 * seconds_per_step / CONTENT_FRAC;
    let w = geo.width_px;
    let h = geo.height_px;

    let mut centroids: BTreeMap<&str, (f64, f64)> = BTreeMap::new();
    let mut footprints = String::new();
    for (id, e) in entities {
        let ring_px: Vec<(f64, f64)> = e.ring.iter().map(|p| geo.project(p)).collect();
        centroids.insert(id.as_str(), polygon_centroid(&ring_px));
        let d = path_d(&ring_px);
        let born_t = (e.born_step as f64 / n as f64) * CONTENT_FRAC;
        let removed_t = e.removed_step.map(|r| (r as f64 / n as f64) * CONTENT_FRAC);
        let anim = opacity_animate(born_t, removed_t, FADE_FRAC);
        let op = if anim.is_none() { 1.0 } else { 0.0 };
        let color = e.color;
        let _ = writeln!(
            footprints,
            r##"<path d="{d}" fill="{color}" fill-opacity="0.5" stroke="{color}" stroke-width="1.4" opacity="{op}">{anim_tag}<title>{id}</title></path>"##,
            anim_tag = anim.as_ref().map(|(kt, v)| format!(r##"<animate attributeName="opacity" dur="{total_dur}s" repeatCount="indefinite" keyTimes="{kt}" values="{v}" calcMode="linear"/>"##)).unwrap_or_default(),
        );
    }

    let mut nodes = String::new();
    let mut edge_lines = String::new();
    for (id, e) in entities {
        let (cx, cy) = centroids[id.as_str()];
        let born_t = (e.born_step as f64 / n as f64) * CONTENT_FRAC;
        let removed_t = e.removed_step.map(|r| (r as f64 / n as f64) * CONTENT_FRAC);
        let anim = opacity_animate(born_t, removed_t, FADE_FRAC);
        let op = if anim.is_none() { 1.0 } else { 0.0 };
        let _ = writeln!(
            nodes,
            r##"<circle cx="{cx:.1}" cy="{cy:.1}" r="4" fill="{color}" stroke="#111111" stroke-width="0.8" opacity="{op}">{anim_tag}</circle>"##,
            color = e.color,
            anim_tag = anim.as_ref().map(|(kt, v)| format!(r##"<animate attributeName="opacity" dur="{total_dur}s" repeatCount="indefinite" keyTimes="{kt}" values="{v}" calcMode="linear"/>"##)).unwrap_or_default(),
        );
    }
    for (derived, source) in edges {
        let (Some(&(dx, dy)), Some(&(sx, sy))) = (centroids.get(derived.as_str()), centroids.get(source.as_str())) else { continue };
        let derived_e = &entities[derived];
        let born_t = (derived_e.born_step as f64 / n as f64) * CONTENT_FRAC;
        let removed_t = derived_e.removed_step.map(|r| (r as f64 / n as f64) * CONTENT_FRAC);
        let anim = opacity_animate(born_t, removed_t, FADE_FRAC);
        let op = if anim.is_none() { 0.9 } else { 0.0 };
        let midx = (sx + dx) / 2.0;
        let midy = (sy + dy) / 2.0 - 12.0;
        let _ = writeln!(
            edge_lines,
            r##"<path d="M {sx:.1} {sy:.1} Q {midx:.1} {midy:.1} {dx:.1} {dy:.1}" fill="none" stroke="#ffffff" stroke-width="1.6" stroke-opacity="0.85" opacity="{op}">{anim_tag}</path>"##,
            anim_tag = anim.map(|(kt, v)| format!(r##"<animate attributeName="opacity" dur="{total_dur}s" repeatCount="indefinite" keyTimes="{kt}" values="{v}" calcMode="linear"/>"##)).unwrap_or_default(),
        );
    }

    let mut labels = String::new();
    for (i, step) in steps.iter().enumerate() {
        let idx = i + 1;
        let start_t = (i as f64 / n as f64) * CONTENT_FRAC;
        let (kt, v) = if idx == n {
            (format!("0;{start_t};1"), "0;1;1".to_string())
        } else {
            let end_t = (idx as f64 / n as f64) * CONTENT_FRAC;
            (format!("0;{start_t};{end_t};1"), "0;1;0;0".to_string())
        };
        let _ = writeln!(
            labels,
            r##"<g opacity="0"><animate attributeName="opacity" dur="{total_dur}s" repeatCount="indefinite" keyTimes="{kt}" values="{v}" calcMode="discrete"/><rect x="8" y="8" width="410" height="24" fill="#000000" fill-opacity="0.55" rx="3"/><text x="16" y="25" font-family="monospace" font-size="14" fill="#ffffff">step {idx}/{n}: {op} target={target} seed={seed}</text></g>"##,
            op = xml_escape(&step.operator_name), target = xml_escape(&step.target), seed = step.seed,
        );
    }

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
<image href="data:{mime};base64,{image_b64}" x="0" y="0" width="{w}" height="{h}" preserveAspectRatio="none"/>
{edge_lines}
{footprints}
{nodes}
{labels}
<g font-family="monospace" font-size="10" fill="#ffffff">
<text x="{w_minus_8}" y="{h_minus_8}" text-anchor="end" opacity="0.85">Imagery: Esri World Imagery (Maxar, Earthstar Geographics)</text>
</g>
</svg>
"##,
        w_minus_8 = w as i64 - 8, h_minus_8 = h as i64 - 8,
    )
}

/// Signed-area (Green's theorem) polygon centroid -- exact for the
/// already-projected pixel ring, falls back to a plain vertex average for
/// a degenerate (near-zero-area, e.g. collinear) ring.
fn polygon_centroid(ring: &[(f64, f64)]) -> (f64, f64) {
    let n = ring.len();
    if n == 0 { return (0.0, 0.0); }
    let mut a = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for i in 0..n {
        let (x0, y0) = ring[i];
        let (x1, y1) = ring[(i + 1) % n];
        let cross = x0 * y1 - x1 * y0;
        a += cross;
        cx += (x0 + x1) * cross;
        cy += (y0 + y1) * cross;
    }
    a *= 0.5;
    if a.abs() < 1e-6 {
        let sx: f64 = ring.iter().map(|p| p.0).sum();
        let sy: f64 = ring.iter().map(|p| p.1).sum();
        return (sx / n as f64, sy / n as f64);
    }
    (cx / (6.0 * a), cy / (6.0 * a))
}

fn path_d(ring_px: &[(f64, f64)]) -> String {
    let mut d = String::new();
    for (i, (x, y)) in ring_px.iter().enumerate() {
        if i == 0 { let _ = write!(d, "M {x:.1} {y:.1} "); } else { let _ = write!(d, "L {x:.1} {y:.1} "); }
    }
    d.push('Z');
    d
}

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
                Some((format!("0;{born_t};{};{};{r};1", born_t + f, (r - f).max(born_t + f)), "0;0;1;1;0;0".to_string()))
            }
        }
        None => Some((format!("0;{born_t};{};1", born_t + fade), "0;0;1;1".to_string())),
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[(n >> 18 & 0x3F) as usize] as char);
        out.push(CHARS[(n >> 12 & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { CHARS[(n >> 6 & 0x3F) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[(n & 0x3F) as usize] as char } else { '=' });
    }
    out
}
