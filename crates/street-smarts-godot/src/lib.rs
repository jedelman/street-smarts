//! # street-smarts-godot
//!
//! Godot 4 GDExtension bindings for `street-smarts`.
//!
//! Exposes the NIR schema, procedural pattern operators, 3D mesh building,
//! and opinion chorus / disagreement report engine directly to Godot as native nodes.

use godot::classes::mesh::PrimitiveType;
use godot::classes::mesh::ArrayType;
use godot::classes::{ArrayMesh, MeshInstance3D, StandardMaterial3D};
use godot::prelude::*;
use street_smarts_conflict::build_report;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::Mesh as SsMesh;
use street_smarts_opinions::registry::evaluate_all;

pub mod building_mesh;
pub mod cluster;
pub mod ground_features;
pub mod pathfinding;
use building_mesh::{canopy_mesh, BuildingSolid, FootprintCollider};

/// A face counts as "roof" (rather than wall) once its normal points at
/// least this far upward. `0.5` is `cos(60°)`, deliberately far from both
/// real cases this needs to separate: a wall's normal.y is ~0 (vertical),
/// while even the shallowest real shed roof this pipeline generates (2m
/// rise over a 100m+ run, per p117_sheltering_roof's own doc) has
/// normal.y > 0.999 -- there's no real geometry anywhere near this
/// boundary to misclassify.
const ROOF_NORMAL_Y_THRESHOLD: f32 = 0.5;

/// Builds a Godot `MeshInstance3D` from an extracted `Mesh`, with flat
/// per-triangle face normals (see `rebuild_3d_mesh`'s own doc for why:
/// Naive Surface Nets shares one vertex per grid cell across every face
/// orientation it touches, which blends normals at sharp corners into a
/// wrong-looking soft gradient -- confirmed against a real device
/// screenshot). Returns `None` for an empty mesh or a SurfaceTool commit
/// failure; the caller decides whether that's worth a warning.
///
/// When `roof_material` is `Some`, upward-facing triangles (see
/// `ROOF_NORMAL_Y_THRESHOLD`) are split into a second mesh surface and
/// given that material instead of `material`. This exists because a real
/// shed roof's slope (2m rise over a 100m+ building) is nearly invisible
/// from the ground -- Alexander's own "sheltering roof" is a subtle,
/// architecturally real pitch, not a dramatic one, so exaggerating the
/// geometry itself would misrepresent the actual pattern data. Coloring
/// the roof PLANE distinctly (independent of how steep it visually is)
/// gets the "I can tell this building has a roof" legibility the geometry
/// alone doesn't provide, without touching `roof_rise_m` or anything else
/// that opinions/pattern data downstream actually reads. `None` keeps the
/// exact single-surface behavior this function always had (open space and
/// street ribbons don't have a roof to distinguish).
///
/// The roof surface also gets a per-vertex height-contour tint (see
/// `roof_contour_tint`) driving `vertex_color_use_as_albedo` on the roof
/// materials (set once in `rebuild_3d_mesh`): real ridge-height variation
/// across a roof -- a P116 cascade's real per-wing step, or a plain shed's
/// own slope -- gets contrast-stretched to `roof_height_range`, so even a
/// genuinely tiny real step (a real P116 cascade on a merged block
/// measured at under 5cm between neighboring segments -- real, but
/// invisible as geometry at any normal viewing distance) still shows up as
/// a visible gradient. Same "shade the real data, don't fabricate a bigger
/// version of it" choice `ROOF_NORMAL_Y_THRESHOLD`'s own split already made
/// for the shed slope itself. `roof_height_range` is the real
/// `(eave_height_m, ridge_height_m)` from the building's own roof data --
/// deliberately NOT read back from the extracted mesh's own vertex Y
/// values (see `roof_contour_tint`'s own doc for why that was tried first
/// and abandoned: Surface Nets residual sliver triangles at sharp
/// composite corners pollute the range badly enough to erase the real
/// signal).
fn mesh_to_instance(
    mesh: &SsMesh,
    name: String,
    material: Option<&Gd<StandardMaterial3D>>,
    roof_material: Option<&Gd<StandardMaterial3D>>,
    roof_height_range: Option<(f32, f32)>,
) -> Option<Gd<MeshInstance3D>> {
    if mesh.triangles.is_empty() {
        return None;
    }
    // Built as flat vertex/normal arrays and uploaded via
    // add_surface_from_arrays, rather than driving SurfaceTool with
    // per-vertex add_vertex/set_normal calls. At real-site scale that was
    // ~9.8M individual FFI calls per rebuild (3 per triangle, x2 for
    // normals) plus SurfaceTool's own internal vertex dedup/index build,
    // and it was the dominant SERIAL cost left after Surface Nets
    // extraction itself was parallelized -- the one part that has to run
    // on the main thread, so it sets the floor on rebuild time.
    //
    // Deliberately unindexed (3 unique vertices per triangle): the flat
    // per-triangle face normals below mean adjacent triangles genuinely
    // don't share vertex attributes, so there is nothing to dedup. See
    // rebuild_3d_mesh's own doc for why the normals are flat.
    let vertex_count = mesh.triangles.len() * 3;
    let mut wall_positions: Vec<Vector3> = Vec::with_capacity(vertex_count);
    let mut wall_normals: Vec<Vector3> = Vec::with_capacity(vertex_count);
    let mut roof_positions: Vec<Vector3> = Vec::new();
    let mut roof_normals: Vec<Vector3> = Vec::new();
    for tri in &mesh.triangles {
        let p0 = mesh.positions[tri[0] as usize];
        let p1 = mesh.positions[tri[1] as usize];
        let p2 = mesh.positions[tri[2] as usize];
        let e1 = (p1.x - p0.x, p1.y - p0.y, p1.z - p0.z);
        let e2 = (p2.x - p0.x, p2.y - p0.y, p2.z - p0.z);
        let (mut fx, mut fy, mut fz) = (
            e1.1 * e2.2 - e1.2 * e2.1,
            e1.2 * e2.0 - e1.0 * e2.2,
            e1.0 * e2.1 - e1.1 * e2.0,
        );
        let len = (fx * fx + fy * fy + fz * fz).sqrt();
        if len > 1e-9 {
            fx /= len;
            fy /= len;
            fz /= len;
        }
        let face_normal = Vector3::new(fx as real, fy as real, fz as real);
        let (positions, normals) = if roof_material.is_some() && face_normal.y > ROOF_NORMAL_Y_THRESHOLD {
            (&mut roof_positions, &mut roof_normals)
        } else {
            (&mut wall_positions, &mut wall_normals)
        };
        for &idx in tri {
            let p = mesh.positions[idx as usize];
            positions.push(Vector3::new(p.x as real, p.y as real, p.z as real));
            normals.push(face_normal);
        }
    }

    let mut array_mesh = ArrayMesh::new_gd();
    add_surface(&mut array_mesh, &wall_positions, &wall_normals, None, material);
    if let Some(roof_mat) = roof_material {
        let roof_colors = roof_contour_tint(&roof_positions, roof_height_range);
        add_surface(&mut array_mesh, &roof_positions, &roof_normals, Some(&roof_colors), Some(roof_mat));
    }
    if array_mesh.get_surface_count() == 0 {
        return None;
    }

    let mut mesh_instance = MeshInstance3D::new_alloc();
    mesh_instance.set_name(&name);
    mesh_instance.set_mesh(&array_mesh);
    Some(mesh_instance)
}

/// A real per-vertex albedo multiplier from each roof vertex's own height:
/// full brightness (`Color(1,1,1)`, the material's own true color) at
/// `height_range`'s own ridge, darkening toward `MIN_TINT` at its eave --
/// contrast-stretched to the building's own real `(eave_height_m,
/// ridge_height_m)`, not a fixed absolute scale, so a real cascade step of
/// any size (a dramatic P118 flat-vs-shed swap or a few centimeters
/// between two P116 segments on a merged block) still produces a visible
/// gradient. Brighter-is-higher deliberately matches Alexander's own P116
/// framing ("the largest and highest roofs over the most significant
/// areas") -- the tint is reading real significance, not just decorating.
///
/// Deliberately reads `height_range` from the building's own real roof
/// data (the caller's `RoofForm.eave_height_m`/`ridge_height_m`), NOT back
/// from `positions`' own extracted Y values -- that was the first attempt,
/// and it doesn't work: Surface Nets can emit a handful of degenerate
/// near-vertical sliver triangles at a building's sharp composite corners
/// (a known, already-accepted residual -- see `rebuild_3d_mesh`'s own doc
/// on inverted-winding triangles and why CULL_DISABLED exists), whose
/// accidental near-upward normal gets them classified as roof even though
/// their real vertices sit near the wall base. Confirmed against the real
/// Military Circle site: several buildings' naive min Y came back as
/// exactly 0.0 (ground level) against a real ~16m ridge, even though the
/// roof's OWN real height variation is under 1.5m end to end -- even a
/// 98th-percentile clip still left some buildings with a badly polluted
/// range (a few had MORE than 2% of their roof vertices as ground-level
/// outliers). Going straight to the source data sidesteps the extraction
/// artifact entirely instead of trying to statistically filter around it.
/// `None` (a flat-topped box with no real `roof` field at all) gets
/// uniform full brightness -- there's no real roof data to shade by.
fn roof_contour_tint(positions: &[Vector3], height_range: Option<(f32, f32)>) -> Vec<Color> {
    const MIN_TINT: f32 = 0.4;
    let full_bright = || vec![Color::from_rgb(1.0, 1.0, 1.0); positions.len()];
    let Some((lo, hi)) = height_range else {
        return full_bright();
    };
    let span = hi - lo;
    if span <= 1e-4 {
        return full_bright();
    }
    positions
        .iter()
        .map(|p| {
            let t = ((p.y - lo) / span).clamp(0.0, 1.0);
            let shade = MIN_TINT + (1.0 - MIN_TINT) * t;
            Color::from_rgb(shade, shade, shade)
        })
        .collect()
}

/// Appends one surface to `array_mesh` from flat position/normal (and
/// optionally per-vertex color) arrays. No-op on an empty pair (a building
/// with no roof-facing triangles, e.g. a degenerate sliver, shouldn't get
/// an empty second surface).
fn add_surface(
    array_mesh: &mut Gd<ArrayMesh>,
    positions: &[Vector3],
    normals: &[Vector3],
    colors: Option<&[Color]>,
    material: Option<&Gd<StandardMaterial3D>>,
) {
    if positions.is_empty() {
        return;
    }
    let mut arrays = VariantArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    arrays.set(ArrayType::VERTEX.ord() as usize, &PackedVector3Array::from(positions).to_variant());
    arrays.set(ArrayType::NORMAL.ord() as usize, &PackedVector3Array::from(normals).to_variant());
    if let Some(colors) = colors {
        arrays.set(ArrayType::COLOR.ord() as usize, &PackedColorArray::from(colors).to_variant());
    }
    let surface_idx = array_mesh.get_surface_count();
    array_mesh.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
    if let Some(mat) = material {
        array_mesh.surface_set_material(surface_idx, mat);
    }
}

struct StreetSmartsExtension;

#[gdextension]
unsafe impl ExtensionLibrary for StreetSmartsExtension {}

/// Godot 3D Node representing a Christopher Alexander Neighborhood.
///
/// Builds 3D building massing, punched doors/windows (P221), streets, plazas,
/// and Salingaros scale/center indicators directly in Godot's 3D viewport.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct NeighborhoodNode3D {
    base: Base<Node3D>,
    neighborhood_json: String,
    building_count: i32,
    mean_wholeness_score: f64,
    /// Rebuilt alongside the massing; see `resolve_move`.
    colliders: Vec<FootprintCollider>,
    /// Rebuilt alongside `colliders`; see `find_path`. `None` before the
    /// first rebuild, or if the site has no real buildings to route
    /// around at all.
    nav_grid: Option<pathfinding::NavGrid>,
}

#[godot_api]
impl INode3D for NeighborhoodNode3D {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            neighborhood_json: String::new(),
            building_count: 0,
            mean_wholeness_score: 0.0,
            colliders: Vec::new(),
            nav_grid: None,
        }
    }

    fn ready(&mut self) {
        godot_print!("StreetSmarts NeighborhoodNode3D ready.");
    }
}

#[godot_api]
impl NeighborhoodNode3D {
    /// Load a Neighborhood Intermediate Representation (NIR) JSON string.
    #[func]
    pub fn load_nir_json(&mut self, json_str: GString) -> bool {
        let rust_str = json_str.to_string();
        match serde_json::from_str::<Neighborhood>(&rust_str) {
            Ok(nir) => {
                self.building_count = nir.buildings.len() as i32;
                self.neighborhood_json = rust_str;
                godot_print!("Successfully loaded NIR neighborhood with {} buildings.", self.building_count);
                true
            }
            Err(err) => {
                godot_error!("Failed to parse NIR JSON: {}", err);
                false
            }
        }
    }

    /// Returns the building count of the loaded neighborhood.
    #[func]
    pub fn get_building_count(&self) -> i32 {
        self.building_count
    }

    /// The currently-loaded neighborhood's own real NIR JSON, exactly as
    /// `run_pattern_pipeline`/`apply_pattern` last left it -- for real
    /// save/continue (see GameState.gd and neighborhood_controller.gd's
    /// `_build_save_data`), which needs to persist whatever a Pattern Lab
    /// session has already built, not just the raw baseline `load_nir_json`
    /// started from.
    #[func]
    pub fn get_neighborhood_json(&self) -> GString {
        GString::from(&self.neighborhood_json)
    }

    /// Runs the real Alexander pattern-language pipeline (the same
    /// `street_smarts_patterns::pipeline::run_corrected_pipeline` the
    /// production web build's static gallery renders are generated from
    /// offline, via `scripts/vibe-render.sh` -> `examples/dump_pipeline.rs`)
    /// against the currently-loaded neighborhood, replacing it with the
    /// result. Turns a parcels-only NIR (what every checked-in
    /// `data/eastside-*.json` fixture actually is -- no generator has
    /// populated `buildings[]` in them) into one with real building
    /// massing, openings, and roofs, entirely on-device: no Python, no
    /// offline step, no network call. `parcel_id` selects which parcel's
    /// `spec` to develop (e.g. `"MILITARY_CIRCLE_ASSEMBLED"`, the real
    /// 97.7-acre Military Circle site); `seed` drives every
    /// pseudo-random choice the pipeline makes (P37 seeding, etc.) --
    /// same seed, same parcel, same output, every time.
    #[func]
    pub fn run_pattern_pipeline(&mut self, parcel_id: GString, seed: i64) -> bool {
        if self.neighborhood_json.is_empty() {
            godot_warn!("Cannot run pattern pipeline: No NIR JSON loaded.");
            return false;
        }
        let baseline: Neighborhood = match serde_json::from_str(&self.neighborhood_json) {
            Ok(n) => n,
            Err(err) => {
                godot_error!("Cannot run pattern pipeline: NIR JSON no longer parses: {}", err);
                return false;
            }
        };

        let parcel_id_str = parcel_id.to_string();
        let start = std::time::Instant::now();
        let result = street_smarts_patterns::pipeline::run_corrected_pipeline(&baseline, &parcel_id_str, seed as u64);
        let elapsed = start.elapsed();

        self.building_count = result.buildings.len() as i32;
        self.neighborhood_json = match serde_json::to_string(&result) {
            Ok(s) => s,
            Err(err) => {
                godot_error!("Pattern pipeline produced a neighborhood that failed to re-serialize: {}", err);
                return false;
            }
        };

        godot_print!(
            "Ran pattern pipeline on parcel '{}' (seed {}) in {:?}: {} parcels, {} buildings, {} streets, {} open_space.",
            parcel_id_str, seed, elapsed,
            result.parcels.len(), result.buildings.len(), result.streets.len(), result.open_space.len()
        );
        true
    }

    /// Restricts the currently-loaded neighborhood down to `anchor_building_id`
    /// plus its `count - 1` real nearest other buildings, replacing it in
    /// place -- a fast, still-real (every building came out of the actual
    /// pipeline run, nothing synthetic) integration-test fixture. See
    /// `cluster::nearest_building_cluster`'s own doc for why this exists:
    /// the full real site is too slow to iterate on (a full offscreen
    /// render pass took minutes and had to be killed), and a small real
    /// cluster is the fast middle layer between `building_mesh.rs`'s
    /// no-rendering unit tests and a full on-device site walkthrough.
    /// Call after `run_pattern_pipeline()` (there must be real buildings to
    /// restrict to) and before `rebuild_3d_mesh()`. Returns `false` (no
    /// change made) if `anchor_building_id` doesn't name a real building in
    /// the currently-loaded neighborhood, or nothing is loaded yet.
    #[func]
    pub fn restrict_to_cluster(&mut self, anchor_building_id: GString, count: i32) -> bool {
        if self.neighborhood_json.is_empty() {
            godot_warn!("Cannot restrict to cluster: no neighborhood loaded.");
            return false;
        }
        let nir: Neighborhood = match serde_json::from_str(&self.neighborhood_json) {
            Ok(n) => n,
            Err(err) => {
                godot_error!("Cannot restrict to cluster: neighborhood JSON no longer parses: {}", err);
                return false;
            }
        };
        let anchor_id_str = anchor_building_id.to_string();
        let Some(cluster) = cluster::nearest_building_cluster(&nir, &anchor_id_str, count.max(0) as usize) else {
            godot_error!(
                "Cannot restrict to cluster: '{}' is not a real building id in the currently-loaded neighborhood (or count was 0).",
                anchor_id_str
            );
            return false;
        };

        let cluster_ids: Vec<String> = cluster.buildings.iter().map(|b| b.id.clone()).collect();
        self.building_count = cluster.buildings.len() as i32;
        self.neighborhood_json = match serde_json::to_string(&cluster) {
            Ok(s) => s,
            Err(err) => {
                godot_error!("Cluster-restricted neighborhood failed to re-serialize: {}", err);
                return false;
            }
        };
        godot_print!(
            "Restricted to a {}-building cluster around '{}': {:?}",
            cluster_ids.len(), anchor_id_str, cluster_ids
        );
        true
    }

    /// Real A* route around every real building footprint on the site,
    /// from `from` to `to` (Y ignored; the route is a ground-level
    /// polyline, ordinary walk-mode height gets added on top by the
    /// caller). Returns an empty array if nothing's been built yet, or if
    /// `from`/`to` is genuinely unreachable (e.g. sealed inside a solid
    /// block with no real opening) -- the caller falls back to the old
    /// straight-line walk in that case, the same honest "no collider, no
    /// smarts" precedent `resolve_move` already sets for an empty site.
    /// See `pathfinding.rs`'s own doc for why this is a hand-rolled grid
    /// over the same real footprint SDF `resolve_move` already trusts,
    /// not Godot's own navigation subsystem.
    #[func]
    pub fn find_path(&self, from: Vector3, to: Vector3) -> PackedVector3Array {
        let Some(grid) = &self.nav_grid else {
            return PackedVector3Array::new();
        };
        let Some(path) = grid.find_path(&self.colliders, (from.x as f64, from.z as f64), (to.x as f64, to.z as f64)) else {
            return PackedVector3Array::new();
        };
        let points: Vec<Vector3> = path.iter().map(|&(x, z)| Vector3::new(x as real, 0.0, z as real)).collect();
        PackedVector3Array::from(points.as_slice())
    }

    /// Real building footprint outlines (site-local meters, ground plane
    /// x/z), for a 2D minimap to draw real building SHAPES instead of
    /// approximating them from mesh AABBs. One polygon per real building
    /// with an assigned height (the same set `resolve_move`/`find_path`
    /// already route around), outer ring only -- a minimap silhouette
    /// doesn't need a courtyard's own hole the way collision does. Paired
    /// 1:1, same order, with `get_building_ids()`.
    #[func]
    pub fn get_building_footprints(&self) -> Array<PackedVector2Array> {
        let mut out = Array::new();
        for c in &self.colliders {
            let ring: Vec<Vector2> = c.outer_points().iter().map(|p| Vector2::new(p.x as real, p.y as real)).collect();
            out.push(&PackedVector2Array::from(ring.as_slice()));
        }
        out
    }

    /// Real building ids, same order as `get_building_footprints()`.
    #[func]
    pub fn get_building_ids(&self) -> PackedStringArray {
        let ids: Vec<GString> = self.colliders.iter().map(|c| GString::from(c.id())).collect();
        PackedStringArray::from(ids.as_slice())
    }

    /// Resolves a tapped ground point (site-local meters, x/z -- the same
    /// frame `find_path`/`resolve_move` already use) to the real building
    /// or parcel it lands inside. The object-selector's one shared
    /// entry point: the minimap (tapping a marker or the map itself) and
    /// the 3D "walkabout" view (a ground-plane raycast, same technique
    /// `orbit_camera.gd`'s tap-to-walk already uses) both resolve a real
    /// id through here instead of each re-implementing their own
    /// point-in-polygon logic, so PatternLab's `target` field always gets
    /// a real id whichever way it was picked.
    ///
    /// Buildings are checked first via `FootprintCollider::distance()`
    /// (the same real SDF `resolve_move`/`find_path` already trust) --
    /// cheap, and there's no real overlap to arbitrate: a parcel is
    /// removed from `Neighborhood.parcels` the moment a building replaces
    /// it (see `ground_features::parcel_polygon`'s own doc), so a point
    /// can never legitimately land inside both a live building and a live
    /// parcel at once. Parcels are checked second by re-parsing
    /// `neighborhood_json` fresh (they aren't cached anywhere -- unlike
    /// `colliders`, `rebuild_3d_mesh()` only ever touches them
    /// transiently for the raw-parcel rendering pass) and testing the
    /// same real outer ring `parcel_polygon` renders, via
    /// `street_smarts_patterns::planar::point_in_polygon`. Returns
    /// `{"kind": "none", "id": ""}` for a miss, or nothing's loaded yet.
    #[func]
    pub fn pick_zone_at(&self, x: f32, z: f32) -> Dictionary {
        let mut result = Dictionary::new();
        for c in &self.colliders {
            if c.distance(x as f64, z as f64) < 0.0 {
                result.insert("kind", "building");
                result.insert("id", c.id());
                return result;
            }
        }

        if !self.neighborhood_json.is_empty() {
            if let Ok(nir) = serde_json::from_str::<Neighborhood>(&self.neighborhood_json) {
                // Same shared local-meter origin rebuild_3d_mesh() uses --
                // see that function's own doc for why (neighborhood bbox
                // center).
                let origin = LngLat::new(
                    (nir.bbox_wgs84[0] + nir.bbox_wgs84[2]) / 2.0,
                    (nir.bbox_wgs84[1] + nir.bbox_wgs84[3]) / 2.0,
                );
                let pt = street_smarts_patterns::planar::Pt2::new(x as f64, z as f64);
                for parcel in &nir.parcels {
                    let ring = street_smarts_patterns::planar::ring_to_local(&parcel.polygon.outer, &origin);
                    if ring.len() >= 3 && street_smarts_patterns::planar::point_in_polygon(pt, &ring) {
                        result.insert("kind", "parcel");
                        result.insert("id", parcel.id.as_str());
                        return result;
                    }
                }
            }
        }

        result.insert("kind", "none");
        result.insert("id", "");
        result
    }

    /// Resolves a walk-mode move against real building footprints, so a
    /// walker stops at walls instead of ghosting through them. Returns the
    /// position actually reachable this step: `to` when it's clear, a
    /// slide along the wall when the move is oblique to it, or `from` when
    /// it's head-on.
    ///
    /// Uses the real footprint polygons (`FootprintCollider`), not a
    /// physics body over the generated mesh -- see that type's own doc for
    /// the reasoning. Courtyards are correctly walkable: a hole ring is
    /// subtracted from the footprint, so the courtyard interior reads as
    /// outside the solid, same as the street does.
    #[func]
    pub fn resolve_move(&self, from: Vector3, to: Vector3, body_radius: f32) -> Vector3 {
        if self.colliders.is_empty() {
            return to;
        }
        let radius = body_radius.max(0.0) as f64;

        let clearance = |x: f64, z: f64| -> f64 {
            self.colliders
                .iter()
                .map(|c| c.distance(x, z))
                .fold(f64::MAX, f64::min)
        };

        let (tx, tz) = (to.x as f64, to.z as f64);
        if clearance(tx, tz) >= radius {
            return to;
        }

        // Blocked. Slide: the footprint SDF's gradient at the blocked point
        // is the outward wall normal, so removing the component of the move
        // that points into the wall leaves the along-wall component.
        let (fx, fz) = (from.x as f64, from.z as f64);
        let eps = 0.05;
        let gx = clearance(tx + eps, tz) - clearance(tx - eps, tz);
        let gz = clearance(tx, tz + eps) - clearance(tx, tz - eps);
        let glen = (gx * gx + gz * gz).sqrt();
        if glen > 1e-9 {
            let (nx, nz) = (gx / glen, gz / glen);
            let (mx, mz) = (tx - fx, tz - fz);
            let into_wall = mx * nx + mz * nz;
            let (sx, sz) = (mx - nx * into_wall, mz - nz * into_wall);
            let slid_x = fx + sx;
            let slid_z = fz + sz;
            if clearance(slid_x, slid_z) >= radius {
                return Vector3::new(slid_x as real, to.y, slid_z as real);
            }
        }
        from
    }

    /// Evaluates opinions on the loaded neighborhood and returns a summary.
    #[func]
    pub fn evaluate_opinions(&mut self) -> Dictionary {
        let mut dict = Dictionary::new();
        if self.neighborhood_json.is_empty() {
            dict.insert("error", "No NIR JSON loaded");
            return dict;
        }

        if let Ok(nir) = serde_json::from_str::<Neighborhood>(&self.neighborhood_json) {
            let evaluated = evaluate_all(&nir);
            let report = build_report(evaluated);

            dict.insert("geometric_headline", report.geometric_summary.headline.as_str());
            dict.insert("activist_headline", report.activist_summary.headline.as_str());
            dict.insert("question_count", report.questions_for_humans.len() as i32);
            dict.insert("abstention_count", report.abstentions.len() as i32);

            if let Some(m) = report.geometric_summary.mean_value {
                self.mean_wholeness_score = m;
                dict.insert("geometric_mean", m);
            }
        }

        dict
    }

    /// Real metadata for every pattern operator this pipeline knows about
    /// -- name, description, source citation, and full parameter schema
    /// (min/max/default/unit) -- so a UI can build a real pattern picker
    /// and per-operator parameter sliders instead of hardcoding a list.
    /// Backed by `street_smarts_patterns::registry::available_operators()`
    /// -- the SAME registry the web client's own "Run full pipeline"
    /// stepper already drives (see that module's own doc); this exposes
    /// that existing interface to Godot for the first time, it isn't a
    /// new one.
    #[func]
    pub fn get_available_patterns(&self) -> Array<Variant> {
        let mut out = Array::<Variant>::new();
        for op in street_smarts_patterns::registry::available_operators() {
            let mut dict = Dictionary::new();
            dict.insert("name", op.name.as_str());
            dict.insert("description", op.description.as_str());
            dict.insert("source_display", op.source.display.as_str());
            dict.insert("source_url", op.source.url.clone().unwrap_or_default().as_str());
            dict.insert("default_params_json", op.default_params.to_string().as_str());

            let mut params = Array::<Variant>::new();
            for p in &op.parameter_schema {
                let mut pd = Dictionary::new();
                pd.insert("name", p.name.as_str());
                pd.insert("description", p.description.as_str());
                pd.insert("min", p.min);
                pd.insert("max", p.max);
                pd.insert("default", p.default);
                pd.insert("unit", p.unit.clone().unwrap_or_default().as_str());
                pd.insert("integer", p.integer);
                params.push(&pd.to_variant());
            }
            dict.insert("params", params);
            out.push(&dict.to_variant());
        }
        out
    }

    /// Applies exactly one named real pattern operator to the currently
    /// loaded neighborhood and merges the result in place -- the
    /// interactive alternative to `run_pattern_pipeline`'s one-shot whole-
    /// pipeline run, for a UI that wants to step through the pattern
    /// language one real choice at a time instead of only seeing the
    /// finished end state.
    ///
    /// `parcel_id` is the real target this operator runs on: `"*"` for
    /// the large majority of operators, which only ever run whole-site
    /// today (confirmed against every real call site in `pipeline.rs`),
    /// or a specific block/parcel id for the few real exceptions (P95
    /// Building Complex runs per real block; P37 House Cluster takes the
    /// site's own top-level parcel id). An operator that doesn't accept
    /// the given target returns a real error from its own `apply()`
    /// (e.g. "only supports parcel_id \"*\""), surfaced here honestly,
    /// not silently ignored or guessed around.
    ///
    /// `params_json` is a JSON object matching the operator's own
    /// parameter schema (see `get_available_patterns`); pass `"{}"` to
    /// use every real default. Rebuilds the 3D scene automatically on
    /// success, so applying one pattern is immediately visible before
    /// choosing the next -- the actual point of exposing this at all.
    #[func]
    pub fn apply_pattern(&mut self, operator_name: GString, parcel_id: GString, params_json: GString, seed: i64) -> Dictionary {
        let mut result = Dictionary::new();
        if self.neighborhood_json.is_empty() {
            result.insert("success", false);
            result.insert("error", "No NIR JSON loaded.");
            return result;
        }
        let nbhd: Neighborhood = match serde_json::from_str(&self.neighborhood_json) {
            Ok(n) => n,
            Err(err) => {
                result.insert("success", false);
                result.insert("error", format!("Neighborhood JSON no longer parses: {err}").as_str());
                return result;
            }
        };
        let params_str = params_json.to_string();
        let params_value: serde_json::Value = if params_str.trim().is_empty() {
            serde_json::json!({})
        } else {
            match serde_json::from_str(&params_str) {
                Ok(v) => v,
                Err(err) => {
                    result.insert("success", false);
                    result.insert("error", format!("Invalid params JSON: {err}").as_str());
                    return result;
                }
            }
        };

        let op_name = operator_name.to_string();
        let target = parcel_id.to_string();
        let sub = match street_smarts_patterns::registry::run_operator(&nbhd, &op_name, &target, &params_value, seed as u64) {
            Ok(sub) => sub,
            Err(err) => {
                result.insert("success", false);
                result.insert("error", err.as_str());
                return result;
            }
        };

        let updated = street_smarts_patterns::subdivision::apply_subdivision(&nbhd, &sub);
        self.building_count = updated.buildings.len() as i32;
        self.neighborhood_json = match serde_json::to_string(&updated) {
            Ok(s) => s,
            Err(err) => {
                result.insert("success", false);
                result.insert("error", format!("Applied pattern but failed to re-serialize: {err}").as_str());
                return result;
            }
        };

        result.insert("success", true);
        result.insert("headline", sub.trace.headline.as_str());
        let mut steps = Array::<Variant>::new();
        for step in &sub.trace.steps {
            steps.push(&step.as_str().to_variant());
        }
        result.insert("steps", steps);
        result.insert("new_parcels", sub.new_parcels.len() as i32);
        result.insert("new_buildings", sub.new_buildings.len() as i32);
        result.insert("new_open_space", sub.new_open_space.len() as i32);
        result.insert("new_streets", sub.new_streets.len() as i32);

        self.rebuild_3d_mesh();
        result
    }

    /// Rebuilds the procedural 3D scene via constructive SDF + Surface
    /// Nets extraction: building massing (`building_mesh::BuildingSolid`,
    /// punched doors/windows), street ribbons and plazas/commons
    /// (`ground_features::FlatPolygon`), replacing any previously generated
    /// children of each kind. Roofs (sloped, not flat), interior
    /// partition walls, canopies, and wall niches are real data the
    /// pipeline assigns that this still doesn't consume -- see this
    /// method's own git history for exactly what's covered as of a given
    /// commit, since that list only shrinks over time. Salingaros scale/
    /// center indicators aren't built here at all yet.
    #[func]
    pub fn rebuild_3d_mesh(&mut self) -> bool {
        if self.neighborhood_json.is_empty() {
            godot_warn!("Cannot rebuild mesh: No NIR JSON loaded.");
            return false;
        }
        let nir: Neighborhood = match serde_json::from_str(&self.neighborhood_json) {
            Ok(n) => n,
            Err(err) => {
                godot_error!("Cannot rebuild mesh: NIR JSON no longer parses: {}", err);
                return false;
            }
        };

        // Shared local-meter origin (neighborhood bbox center) so every
        // building's massing stays correctly positioned relative to the
        // others, not just internally consistent with itself.
        let origin = LngLat::new(
            (nir.bbox_wgs84[0] + nir.bbox_wgs84[2]) / 2.0,
            (nir.bbox_wgs84[1] + nir.bbox_wgs84[3]) / 2.0,
        );

        let stale: Vec<Gd<Node>> = {
            let base = self.base();
            (0..base.get_child_count())
                .filter_map(|i| base.get_child(i))
                .filter(|c| {
                    let name = c.get_name().to_string();
                    name.starts_with("GeneratedMassing_")
                        || name.starts_with("GeneratedOpenSpace_")
                        || name.starts_with("GeneratedStreet_")
                        || name.starts_with("GeneratedCanopy_")
                        || name.starts_with("GeneratedParcel_")
                        || name.starts_with("GeneratedActivityNode_")
                })
                .collect()
        };
        for mut child in stale {
            child.queue_free();
        }

        // CULL_DISABLED, not the default CULL_BACK: ground_features.rs
        // emits a single winding per polygon (not a duplicate reversed-
        // winding copy -- that was the earlier approach, dropped because
        // two coincident triangles at identical positions is exactly the
        // setup for z-fighting between the correctly-lit one and its
        // backwards-normal twin), so these need both faces visible from a
        // single triangle instead.
        // Buildings are closed watertight solids (verified: zero boundary
        // edges across all 35 real buildings), so back faces are hidden by
        // front faces via the depth test anyway and disabling culling
        // costs only fill rate. What it BUYS is that the handful of
        // triangles Surface Nets still emits with inverted winding at
        // sharp composite corners (178 of 3.26M site-wide after the voxel
        // cap change, down from 1091) render as ordinary surface instead
        // of being culled into see-through holes in a wall. A safety net
        // for a known-residual, not a substitute for correct winding.
        let mut building_material = StandardMaterial3D::new_gd();
        building_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);

        // Roof surfaces get their own materials, split out by face normal
        // in mesh_to_instance (see ROOF_NORMAL_Y_THRESHOLD's own doc) --
        // a real shed roof's slope is only ~1 degree over a real building's
        // own footprint, invisible on its own, so color is what actually
        // makes "this building has a roof" legible. Shed roofs (the
        // overwhelming majority -- see p117_sheltering_roof) get a warm
        // terracotta/clay-tile tone; the tallest buildings' flat, occupiable
        // P118 garden roofs get a distinct green, since they're a real,
        // different thing (a walkable garden plane, not a shed).
        //
        // ALBEDO_FROM_VERTEX_COLOR: multiplies this base color by
        // mesh_to_instance's own per-vertex roof_contour_tint -- a real
        // P116 cascade step can be under 5cm on a merged block (confirmed
        // against the real Military Circle site: 66 segments spanning a
        // 1.3m total ridge-height range), completely invisible as geometry
        // at any normal viewing distance. The tint shades by real height
        // instead of exaggerating it, same "color, not geometry" choice
        // this whole roof-material split already made for the shed slope.
        let mut shed_roof_material = StandardMaterial3D::new_gd();
        shed_roof_material.set_albedo(Color::from_rgb(0.62, 0.30, 0.20));
        shed_roof_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        shed_roof_material.set_flag(godot::classes::base_material_3d::Flags::ALBEDO_FROM_VERTEX_COLOR, true);
        let mut flat_garden_roof_material = StandardMaterial3D::new_gd();
        flat_garden_roof_material.set_albedo(Color::from_rgb(0.30, 0.52, 0.24));
        flat_garden_roof_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        flat_garden_roof_material.set_flag(godot::classes::base_material_3d::Flags::ALBEDO_FROM_VERTEX_COLOR, true);

        // P119 Arcades canopies: a warm wood/awning tone, distinct from
        // both roof colors and the plain wall material -- see
        // canopy_mesh's own doc for why these are separate flat quads
        // rather than part of the main building solid.
        let mut canopy_material = StandardMaterial3D::new_gd();
        canopy_material.set_albedo(Color::from_rgb(0.45, 0.33, 0.20));
        canopy_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);

        // OpenSpaceKind materials -- real data on the Military Circle site
        // is a mix of Plaza (P61's own intentional public square, a
        // hardscape claim) and Common (P37's informal, soft cluster-scale
        // shared land); every other kind gets the same plaza-like default
        // rather than inventing a color for a kind that's never actually
        // been produced yet. Previously ALL open space rendered as one
        // uniform green regardless of kind -- a plaza reading as grass is
        // exactly the kind of "reads as warehouse" flattening this pass is
        // fixing.
        let mut plaza_material = StandardMaterial3D::new_gd();
        plaza_material.set_albedo(Color::from_rgb(0.62, 0.60, 0.54));
        plaza_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        let mut common_material = StandardMaterial3D::new_gd();
        common_material.set_albedo(Color::from_rgb(0.42, 0.58, 0.34));
        common_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);

        // Raw, not-yet-consumed Parcels -- the pre-building pad/block
        // fabric an interactive pattern step (apply_pattern) hasn't built
        // on yet. A flat, neutral tan distinct from every other ground
        // material, since this is genuinely "nothing has happened here
        // yet," not a plaza or a lawn.
        let mut parcel_material = StandardMaterial3D::new_gd();
        parcel_material.set_albedo(Color::from_rgb(0.72, 0.68, 0.55));
        parcel_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);

        // Street materials by real classification -- `path_network.rs`
        // already computes Arterial (asphalt) vs. Local/Pedestrian (grass
        // pavers), but every segment used to render with one uniform grey
        // regardless. Pedestrian gets its own tone even though it shares
        // Local's exact surface value today, so a foot path already reads
        // differently on screen ahead of also getting its own real
        // (narrower) width.
        let mut arterial_material = StandardMaterial3D::new_gd();
        arterial_material.set_albedo(Color::from_rgb(0.27, 0.27, 0.29));
        arterial_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        let mut local_street_material = StandardMaterial3D::new_gd();
        local_street_material.set_albedo(Color::from_rgb(0.45, 0.50, 0.34));
        local_street_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        let mut pedestrian_material = StandardMaterial3D::new_gd();
        pedestrian_material.set_albedo(Color::from_rgb(0.58, 0.52, 0.40));
        pedestrian_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);

        // ActivityNode beacons, one color per real ActivityKind -- real
        // data P61 (plaza centroids) and P124 (activity pockets) have
        // populated since those operators first shipped, but never once
        // rendered until now (see ground_features::activity_node_marker's
        // own doc). Every real ActivityKind variant gets a distinct color
        // even though only Civic is produced by any real generator today
        // (same "cover the whole enum honestly" choice OpenSpaceKind's own
        // material match above already makes for kinds nothing produces
        // yet), so a future generator's Commerce/Transit/etc. node doesn't
        // silently fall back to some other kind's color.
        use street_smarts_core::nir::ActivityKind;
        let mut activity_commerce_material = StandardMaterial3D::new_gd();
        activity_commerce_material.set_albedo(Color::from_rgb(0.85, 0.55, 0.15));
        activity_commerce_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        let mut activity_civic_material = StandardMaterial3D::new_gd();
        activity_civic_material.set_albedo(Color::from_rgb(0.25, 0.45, 0.75));
        activity_civic_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        let mut activity_transit_material = StandardMaterial3D::new_gd();
        activity_transit_material.set_albedo(Color::from_rgb(0.85, 0.75, 0.15));
        activity_transit_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        let mut activity_school_material = StandardMaterial3D::new_gd();
        activity_school_material.set_albedo(Color::from_rgb(0.55, 0.35, 0.65));
        activity_school_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        let mut activity_worship_material = StandardMaterial3D::new_gd();
        activity_worship_material.set_albedo(Color::from_rgb(0.85, 0.82, 0.70));
        activity_worship_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        let mut activity_health_material = StandardMaterial3D::new_gd();
        activity_health_material.set_albedo(Color::from_rgb(0.80, 0.25, 0.30));
        activity_health_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        let mut activity_other_material = StandardMaterial3D::new_gd();
        activity_other_material.set_albedo(Color::from_rgb(0.55, 0.55, 0.55));
        activity_other_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);

        let rebuild_start = std::time::Instant::now();
        let mut meshed = 0i32;
        let mut total_tris = 0usize;
        let mut skipped_no_height = 0i32;

        // Surface Nets extraction is by far the dominant cost of a rebuild
        // and is pure Rust with no Godot calls in it, so it runs across
        // every available core; only the SurfaceTool/scene work below has
        // to stay on the main thread. Work is handed out one building at a
        // time via an atomic cursor rather than pre-sliced into equal
        // chunks, because per-building cost varies by ~30x on the real
        // site (a P108-merged block vs. a small P95 cell) and static
        // chunking would leave most threads idle behind the one slow one.
        let prepared: Vec<(&street_smarts_core::nir::Building, BuildingSolid)> = nir
            .buildings
            .iter()
            .filter_map(|b| BuildingSolid::from_building(b, &origin).map(|s| (b, s)))
            .collect();
        skipped_no_height += (nir.buildings.len() - prepared.len()) as i32;

        let thread_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(prepared.len().max(1));
        let cursor = std::sync::atomic::AtomicUsize::new(0);
        let mut indexed_meshes: Vec<(usize, SsMesh)> = Vec::with_capacity(prepared.len());
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(thread_count);
            for _ in 0..thread_count {
                let cursor = &cursor;
                let prepared = &prepared;
                handles.push(scope.spawn(move || {
                    let mut out: Vec<(usize, SsMesh)> = Vec::new();
                    loop {
                        let idx = cursor.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if idx >= prepared.len() {
                            break;
                        }
                        let solid = &prepared[idx].1;
                        // Adaptive voxel size, not a fixed one: a real
                        // P108-merged block can be 10x a typical building's
                        // footprint diagonal, and Surface Nets cost is cubic
                        // in that -- see `suggested_voxel_size`'s own doc.
                        out.push((idx, solid.to_mesh(solid.suggested_voxel_size())));
                    }
                    out
                }));
            }
            for handle in handles {
                if let Ok(chunk) = handle.join() {
                    indexed_meshes.extend(chunk);
                }
            }
        });
        // Restore deterministic order: threads finish out of order, and
        // scene child order should not depend on how work happened to be
        // scheduled.
        indexed_meshes.sort_by_key(|(idx, _)| *idx);

        // Walk-mode collision comes from the real footprint polygons, not
        // from the generated triangles -- see FootprintCollider's own doc
        // for why a physics body over the extracted mesh is the wrong tool
        // for a ground-plane walker.
        self.colliders = nir
            .buildings
            .iter()
            .filter_map(|b| FootprintCollider::from_building(b, &origin))
            .collect();

        // Real A* pathfinding grid over the same colliders -- see
        // pathfinding.rs's own doc for why a grid over this real SDF
        // rather than Godot's own navigation subsystem. `None` on an
        // empty site: nothing to route around, and no real bounds to size
        // a grid from.
        self.nav_grid = if self.colliders.is_empty() {
            None
        } else {
            let mut min_x = f64::MAX;
            let mut min_z = f64::MAX;
            let mut max_x = f64::MIN;
            let mut max_z = f64::MIN;
            for c in &self.colliders {
                let (cx0, cz0, cx1, cz1) = c.bounds();
                min_x = min_x.min(cx0);
                min_z = min_z.min(cz0);
                max_x = max_x.max(cx1);
                max_z = max_z.max(cz1);
            }
            Some(pathfinding::NavGrid::build(&self.colliders, (min_x, min_z, max_x, max_z)))
        };

        let mut parcel_meshed = 0i32;
        for parcel in &nir.parcels {
            let Some(pad) = ground_features::parcel_polygon(parcel, &origin) else {
                continue;
            };
            let mesh = pad.to_mesh();
            total_tris += mesh.triangles.len();
            let name = format!("GeneratedParcel_{}", parcel.id);
            let Some(mesh_instance) = mesh_to_instance(&mesh, name, Some(&parcel_material), None, None) else {
                continue;
            };
            self.base_mut().add_child(&mesh_instance);
            parcel_meshed += 1;
        }

        for (idx, mesh) in &indexed_meshes {
            let building = prepared[*idx].0;
            total_tris += mesh.triangles.len();
            let roof_material = match building.roof.as_ref().map(|r| r.shape) {
                Some(street_smarts_core::nir::RoofShape::Flat) => &flat_garden_roof_material,
                _ => &shed_roof_material,
            };
            let roof_height_range = building.roof.as_ref().map(|r| (r.eave_height_m as f32, r.ridge_height_m as f32));
            let Some(mesh_instance) = mesh_to_instance(mesh, format!("GeneratedMassing_{}", building.id), Some(&building_material), Some(roof_material), roof_height_range) else {
                continue;
            };
            self.base_mut().add_child(&mesh_instance);
            meshed += 1;

            if let Some(canopy) = canopy_mesh(building, &origin) {
                total_tris += canopy.triangles.len();
                if let Some(canopy_instance) = mesh_to_instance(&canopy, format!("GeneratedCanopy_{}", building.id), Some(&canopy_material), None, None) {
                    self.base_mut().add_child(&canopy_instance);
                }
            }
        }

        let mut open_space_meshed = 0i32;
        for open_space in &nir.open_space {
            let Some(pad) = ground_features::open_space_polygon(open_space, &origin) else {
                continue;
            };
            let mesh = pad.to_mesh();
            total_tris += mesh.triangles.len();
            let name = format!("GeneratedOpenSpace_{}", open_space.id);
            // Hardscape-family kinds (an intentional public square, or a
            // real bay attached to one) read as plaza; soft/informal kinds
            // read as common. Nothing in the real Military Circle site
            // produces Park/Vacant/Sponge/Parking/Other/Undecided today,
            // so this default is a reasonable, not a verified, choice.
            use street_smarts_core::nir::OpenSpaceKind;
            let open_space_material = match open_space.kind {
                OpenSpaceKind::Plaza | OpenSpaceKind::Pocket => &plaza_material,
                OpenSpaceKind::Common | OpenSpaceKind::Park | OpenSpaceKind::Sponge => &common_material,
                OpenSpaceKind::Vacant | OpenSpaceKind::Parking | OpenSpaceKind::Other | OpenSpaceKind::Undecided => &plaza_material,
            };
            let Some(mesh_instance) = mesh_to_instance(&mesh, name, Some(open_space_material), None, None) else {
                continue;
            };
            self.base_mut().add_child(&mesh_instance);
            open_space_meshed += 1;
        }

        let mut street_meshed = 0i32;
        for street in &nir.streets {
            // Real classification from path_network.rs's MST + loop-budget
            // split; unclassified (older fixtures) keeps the original
            // uniform grey rather than guessing.
            let street_material = match street.classification.as_deref() {
                Some("pedestrian") => &pedestrian_material,
                Some("local") => &local_street_material,
                _ => &arterial_material,
            };
            for (seg_idx, pad) in ground_features::street_ribbon_segments(street, &origin).into_iter().enumerate() {
                let mesh = pad.to_mesh();
                total_tris += mesh.triangles.len();
                let name = format!("GeneratedStreet_{}_seg{}", street.id, seg_idx);
                let Some(mesh_instance) = mesh_to_instance(&mesh, name, Some(street_material), None, None) else {
                    continue;
                };
                self.base_mut().add_child(&mesh_instance);
                street_meshed += 1;
            }
        }

        let mut activity_node_meshed = 0i32;
        for node in &nir.activity_nodes {
            let mesh = ground_features::activity_node_marker(node, &origin);
            total_tris += mesh.triangles.len();
            let name = format!("GeneratedActivityNode_{}", node.id);
            let material = match node.kind {
                ActivityKind::Commerce => &activity_commerce_material,
                ActivityKind::Civic => &activity_civic_material,
                ActivityKind::Transit => &activity_transit_material,
                ActivityKind::School => &activity_school_material,
                ActivityKind::Worship => &activity_worship_material,
                ActivityKind::Health => &activity_health_material,
                ActivityKind::Other => &activity_other_material,
            };
            let Some(mesh_instance) = mesh_to_instance(&mesh, name, Some(material), None, None) else {
                continue;
            };
            self.base_mut().add_child(&mesh_instance);
            activity_node_meshed += 1;
        }

        godot_print!(
            "Rebuilt scene: {} of {} buildings (Surface Nets), {} of {} open spaces, {} street segments (ear-clipping), {} raw parcels, {} activity nodes -- {} tris total in {:?} ({} buildings skipped: no height_m assigned).",
            meshed,
            nir.buildings.len(),
            open_space_meshed,
            nir.open_space.len(),
            street_meshed,
            parcel_meshed,
            activity_node_meshed,
            total_tris,
            rebuild_start.elapsed(),
            skipped_no_height
        );
        meshed > 0 || open_space_meshed > 0 || street_meshed > 0 || parcel_meshed > 0 || activity_node_meshed > 0
    }
}

/// Godot Node providing the Opinion Chorus & Conflict Engine API.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct OpinionEvaluatorNode {
    base: Base<Node>,
}

#[godot_api]
impl INode for OpinionEvaluatorNode {
    fn init(base: Base<Node>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl OpinionEvaluatorNode {
    /// Evaluates a raw NIR JSON string and returns a full DisagreementReport as a Godot Dictionary.
    #[func]
    pub fn evaluate_nir_json(&self, json_str: GString) -> Dictionary {
        let mut result = Dictionary::new();
        let s = json_str.to_string();

        match serde_json::from_str::<Neighborhood>(&s) {
            Ok(nir) => {
                let evaluated = evaluate_all(&nir);
                let report = build_report(evaluated);

                result.insert("geometric_headline", report.geometric_summary.headline.as_str());
                result.insert("activist_headline", report.activist_summary.headline.as_str());

                let mut questions_arr = Array::<Variant>::new();
                for q in &report.questions_for_humans {
                    let mut q_dict = Dictionary::new();
                    q_dict.insert("question", q.question.as_str());
                    q_dict.insert("why_it_matters", q.why_it_matters.as_str());
                    questions_arr.push(&q_dict.to_variant());
                }
                result.insert("questions", questions_arr);
            }
            Err(e) => {
                result.insert("error", format!("Invalid NIR JSON: {}", e).as_str());
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A perfectly flat roof (no real `roof` field at all -- `height_range`
    /// is `None`) gets uniform full brightness, not an arbitrary shade --
    /// there's no real height data to contrast-stretch against.
    #[test]
    fn no_height_range_is_uniform_full_brightness() {
        let positions = vec![Vector3::new(0.0, 3.0, 0.0), Vector3::new(1.0, 9.0, 1.0)];
        let colors = roof_contour_tint(&positions, None);
        assert_eq!(colors.len(), 2);
        for c in colors {
            assert_eq!((c.r, c.g, c.b), (1.0, 1.0, 1.0));
        }
    }

    /// A degenerate range (eave == ridge, e.g. a data glitch) doesn't
    /// divide by ~zero into a blown-out or NaN color -- same uniform
    /// full-brightness fallback as no real range at all.
    #[test]
    fn degenerate_zero_span_range_is_uniform_full_brightness() {
        let positions = vec![Vector3::new(0.0, 5.0, 0.0)];
        let colors = roof_contour_tint(&positions, Some((5.0, 5.0)));
        assert_eq!((colors[0].r, colors[0].g, colors[0].b), (1.0, 1.0, 1.0));
    }

    /// The real, load-bearing case: a genuine (even tiny) eave/ridge span
    /// produces a strictly brighter color at the ridge than at the eave --
    /// this is the whole fix for the real P116 cascade this function
    /// exists for (confirmed against the real Military Circle site: a
    /// merged block with 66 roof segments spanning just 1.3m of real
    /// ridge-height variation, invisible as geometry, needed this to be
    /// visible at all).
    #[test]
    fn a_real_height_span_shades_the_eave_darker_than_the_ridge() {
        let positions = vec![
            Vector3::new(0.0, 14.2, 0.0), // at the real eave
            Vector3::new(0.0, 15.2, 0.0), // halfway up the real cascade
            Vector3::new(0.0, 16.2, 0.0), // at the real ridge
        ];
        let colors = roof_contour_tint(&positions, Some((14.2, 16.2)));
        assert!(colors[0].r < colors[1].r, "eave should be darker than the midpoint");
        assert!(colors[1].r < colors[2].r, "midpoint should be darker than the ridge");
        assert_eq!((colors[2].r, colors[2].g, colors[2].b), (1.0, 1.0, 1.0), "the real ridge itself should read at full brightness");
    }

    /// A vertex outside the real `[eave, ridge]` range (e.g. a Surface Nets
    /// residual sliver triangle sitting near the wall base, see this
    /// function's own doc for why height_range comes from the building's
    /// real roof data rather than back from extracted geometry) gets
    /// clamped to the darkest real shade, not extrapolated into a
    /// nonsensical negative-brightness color.
    #[test]
    fn a_vertex_below_the_real_eave_clamps_instead_of_extrapolating() {
        let positions = vec![Vector3::new(0.0, 0.0, 0.0)];
        let colors = roof_contour_tint(&positions, Some((14.2, 16.2)));
        let c = colors[0];
        assert!(c.r > 0.0 && c.r < 1.0, "expected the clamped MIN_TINT shade, got {}", c.r);
    }
}
