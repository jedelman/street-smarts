//! Renders the commit + entity-provenance lineage graph for a real pipeline
//! slice as a single SVG, so a `block_membership` run can be inspected
//! visually instead of only queried programmatically.
//!
//! Two layers overlaid, per Jason's request ("surface the trace as a
//! graph"):
//! - **Commit clusters** (light background boxes, one per `get_or_compute`
//!   call, left to right in chain order) -- the operator/target/seed that
//!   produced each step, exactly what `Commit` records.
//! - **Entity nodes** inside each cluster, one per parcel/open-space id that
//!   commit introduced (diffed against the previous snapshot's id set, not
//!   just `entity_provenance`'s keys -- this also catches entities a step
//!   creates without recording provenance, like P37's own `BLOCK_n` blocks,
//!   so they still appear as real graph nodes even though nothing points
//!   INTO them).
//! - **Provenance edges** between entity nodes, drawn from `Commit::
//!   entity_provenance` -- exactly the data `block_membership` walks. A
//!   `p108_merged_N` node with two incoming edges from two different
//!   `BLOCK_*` columns IS a real cross-block merge, visible directly,
//!   without querying `block_membership` at all.
//!
//! Only runs P37 -> per-block P95 -> P108 -- the three operators this
//! crate's `get_or_compute`/`entity_provenance` wiring actually covers
//! today (see `components.rs`'s own note on why the full 14-step
//! production pipeline isn't wired through `HistoryStore` yet).
//!
//! Usage:
//!   cargo run -p street-smarts-ledger --release --example dump_lineage_graph -- \
//!       <fixture.json> <parcel_id> <seed> <out.svg>
//!
//! Matches `scripts/vibe-render.sh`'s "clean_baseline" scenario by default:
//!   cargo run -p street-smarts-ledger --release --example dump_lineage_graph -- \
//!       data/eastside-baseline.json 00001129 1 /tmp/lineage.svg

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::Scope;
use street_smarts_ledger::{Commit, HistoryStore, InMemoryHistoryStore, NeighborhoodId};
use street_smarts_patterns::p108_connected_buildings::{P108ConnectedBuildings, P108Params};
use street_smarts_patterns::p37_house_cluster::{P37HouseCluster, P37Params};
use street_smarts_patterns::p95_building_complex::{P95BuildingComplex, P95Params};
use street_smarts_patterns::Parameters;

/// One column of the graph: the commit that produced it, plus every entity
/// id that appeared for the first time in that commit's resulting snapshot
/// (a superset of `commit.entity_provenance`'s keys -- see module doc).
struct GraphStep {
    commit: Commit,
    new_entity_ids: Vec<String>,
}

fn entity_ids(n: &Neighborhood) -> BTreeSet<String> {
    n.parcels.iter().map(|p| p.id.clone())
        .chain(n.open_space.iter().map(|o| o.id.clone()))
        .chain(n.buildings.iter().map(|b| b.id.clone()))
        .collect()
}

/// Coarse entity kind, purely for node color -- inferred from id shape
/// since that's the only signal available uniformly across P37/P95/P108
/// output (there's no single typed "EntityKind" field on `Parcel`/
/// `OpenSpace` this could read instead).
fn entity_kind(id: &str) -> (&'static str, &'static str) {
    if id.starts_with("p108_merged_") {
        ("#f5a623", "merged")
    } else if id.contains("_P95_courtyard_p") {
        ("#4fb3a9", "courtyard")
    } else if id.contains("_P95_cell_") {
        ("#5b9bd5", "pad")
    } else if id.starts_with("BLOCK_") {
        ("#8064a2", "block")
    } else {
        ("#999999", "other")
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!("usage: dump_lineage_graph <fixture.json> <parcel_id> <seed> <out.svg>");
        std::process::exit(1);
    }
    let fixture_path = &args[1];
    let parcel_id = &args[2];
    let seed: u64 = args[3].parse().expect("seed must be a u64");
    let out_path = &args[4];

    let raw = std::fs::read_to_string(fixture_path)
        .unwrap_or_else(|e| panic!("couldn't read {fixture_path}: {e}"));
    let baseline: Neighborhood = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("couldn't parse {fixture_path}: {e}"));

    let mut store = InMemoryHistoryStore::new();
    let mut prev_ids = entity_ids(&baseline);
    let root_id = store.insert_root(&baseline);
    let mut steps: Vec<GraphStep> = Vec::new();

    let record_step = |store: &InMemoryHistoryStore, id: NeighborhoodId, prev_ids: &mut BTreeSet<String>, steps: &mut Vec<GraphStep>| {
        let snapshot = store.materialize(&id).expect("just-computed commit must materialize");
        let commit = store.commit(&id).expect("just-computed commit must be recorded");
        let now_ids = entity_ids(&snapshot);
        let mut new_entity_ids: Vec<String> = now_ids.difference(prev_ids).cloned().collect();
        new_entity_ids.sort();
        *prev_ids = now_ids;
        steps.push(GraphStep { commit, new_entity_ids });
    };

    let sub37 = store
        .get_or_compute(root_id, &P37HouseCluster, parcel_id, &P37Params::defaults().as_map(), seed, "v1")
        .unwrap_or_else(|e| panic!("P37 failed on {parcel_id}: {e}"));
    record_step(&store, sub37, &mut prev_ids, &mut steps);
    let mut cur: NeighborhoodId = sub37;

    let after_p37 = store.materialize(&cur).unwrap();
    let block_ids = after_p37.select_ids(&Scope::Block);
    eprintln!("P37: {} block(s)", block_ids.len());

    for (i, block_id) in block_ids.iter().enumerate() {
        let block_seed = seed + i as u64 + 1;
        match store.get_or_compute(cur, &P95BuildingComplex, block_id, &P95Params::defaults().as_map(), block_seed, "v1") {
            Ok(next) => {
                record_step(&store, next, &mut prev_ids, &mut steps);
                cur = next;
            }
            Err(e) => eprintln!("P95 skipped {block_id}: {e}"),
        }
    }

    match store.get_or_compute(cur, &P108ConnectedBuildings, "*", &P108Params::defaults().as_map(), seed, "v1") {
        Ok(next) => record_step(&store, next, &mut prev_ids, &mut steps),
        Err(e) => eprintln!("P108 skipped: {e}"),
    }

    eprintln!(
        "{} commit(s), {} entities total",
        steps.len(),
        steps.iter().map(|s| s.new_entity_ids.len()).sum::<usize>()
    );

    let svg = render_svg(&baseline, &steps);
    if let Some(parent) = std::path::Path::new(out_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(out_path, svg).unwrap_or_else(|e| panic!("couldn't write {out_path}: {e}"));
    println!("{out_path}: {} commit column(s)", steps.len() + 1);
}

const COL_WIDTH: f64 = 230.0;
const COL_GAP: f64 = 70.0;
const ROW_HEIGHT: f64 = 24.0;
const HEADER_HEIGHT: f64 = 76.0;
const TOP_MARGIN: f64 = 20.0;
const NODE_HEIGHT: f64 = 18.0;

fn render_svg(baseline: &Neighborhood, steps: &[GraphStep]) -> String {
    // node id -> (column_index, center_x, center_y)
    let mut pos: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    let n_cols = steps.len() + 1; // +1 for the root column
    let max_rows = steps.iter().map(|s| s.new_entity_ids.len()).max().unwrap_or(0).max(1);
    let height = HEADER_HEIGHT + TOP_MARGIN + max_rows as f64 * ROW_HEIGHT + 40.0;
    let width = n_cols as f64 * (COL_WIDTH + COL_GAP) + COL_GAP;

    let mut body = String::new();

    // Column 0: root.
    let root_x = COL_GAP;
    let _ = write!(
        body,
        r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="8" fill="#eeeeee" stroke="#bbbbbb"/>
<text x="{tx}" y="{ty}" font-family="monospace" font-size="12" font-weight="bold">root</text>
<text x="{tx}" y="{ty2}" font-family="monospace" font-size="10">{label}</text>
"##,
        x = root_x, y = TOP_MARGIN, w = COL_WIDTH, h = height - TOP_MARGIN - 20.0,
        tx = root_x + 10.0, ty = TOP_MARGIN + 20.0, ty2 = TOP_MARGIN + 38.0,
        label = xml_escape(&baseline.metadata.label),
    );

    for (col_idx, step) in steps.iter().enumerate() {
        let col_x = COL_GAP + (col_idx as f64 + 1.0) * (COL_WIDTH + COL_GAP);
        let short_id = &step.commit.id.to_hex()[..8];
        let _ = write!(
            body,
            r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="8" fill="#f7f7f7" stroke="#bbbbbb"/>
<text x="{tx}" y="{ty1}" font-family="monospace" font-size="12" font-weight="bold">{op}</text>
<text x="{tx}" y="{ty2}" font-family="monospace" font-size="10">target={target}</text>
<text x="{tx}" y="{ty3}" font-family="monospace" font-size="10">seed={seed} · {short_id}</text>
"##,
            x = col_x, y = TOP_MARGIN, w = COL_WIDTH, h = height - TOP_MARGIN - 20.0,
            tx = col_x + 10.0, ty1 = TOP_MARGIN + 18.0, ty2 = TOP_MARGIN + 34.0, ty3 = TOP_MARGIN + 50.0,
            op = xml_escape(&step.commit.operator_name),
            target = xml_escape(&step.commit.target),
            seed = step.commit.seed,
        );

        for (row_idx, id) in step.new_entity_ids.iter().enumerate() {
            let node_y = TOP_MARGIN + HEADER_HEIGHT + row_idx as f64 * ROW_HEIGHT;
            let node_x = col_x + 10.0;
            let (color, kind) = entity_kind(id);
            let cx = node_x + (COL_WIDTH - 20.0) / 2.0;
            let cy = node_y + NODE_HEIGHT / 2.0;
            pos.insert(id.clone(), (cx, cy));
            let _ = write!(
                body,
                r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="4" fill="{color}" fill-opacity="0.85"/>
<text x="{tx}" y="{ty}" font-family="monospace" font-size="9" fill="#111111">{label}</text>
"##,
                x = node_x, y = node_y, w = COL_WIDTH - 20.0, h = NODE_HEIGHT,
                color = color,
                tx = node_x + 4.0, ty = node_y + NODE_HEIGHT - 5.0,
                label = xml_escape(&truncate_middle(id, 26)),
            );
            let _ = kind; // used only via color today; kept named for the legend below
        }
    }

    // Provenance edges -- drawn after all nodes are positioned, so a merged
    // pad's edges back to an earlier P95 column can be resolved regardless
    // of column order.
    let mut edges = String::new();
    for step in steps {
        for (derived, sources) in &step.commit.entity_provenance {
            let Some(&(dx, dy)) = pos.get(derived) else { continue };
            for source in sources {
                let Some(&(sx, sy)) = pos.get(source) else { continue };
                let midx = (sx + dx) / 2.0;
                let _ = writeln!(
                    edges,
                    r##"<path d="M {sx} {sy} C {midx} {sy}, {midx} {dy}, {dx} {dy}" fill="none" stroke="#444444" stroke-width="1.2" marker-end="url(#arrow)" opacity="0.7"/>"##
                );
            }
        }
    }

    let legend = r##"<g font-family="monospace" font-size="10">
<rect x="20" y="8" width="10" height="10" fill="#8064a2"/><text x="34" y="17">block (P37)</text>
<rect x="140" y="8" width="10" height="10" fill="#5b9bd5"/><text x="154" y="17">pad (P95)</text>
<rect x="250" y="8" width="10" height="10" fill="#4fb3a9"/><text x="264" y="17">courtyard (P95)</text>
<rect x="400" y="8" width="10" height="10" fill="#f5a623"/><text x="414" y="17">merged pad (P108)</text>
</g>"##;

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
<defs>
<marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="3" orient="auto" markerUnits="strokeWidth">
<path d="M0,0 L0,6 L7,3 z" fill="#444444"/>
</marker>
</defs>
<rect x="0" y="0" width="{width}" height="{height}" fill="#ffffff"/>
{legend}
<g transform="translate(0,10)">
{edges}
{body}
</g>
</svg>
"##,
        width = width, height = height + 20.0,
    )
}

fn truncate_middle(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    let half = (max - 1) / 2;
    format!("{}…{}", &s[..half], &s[s.len() - half..])
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
