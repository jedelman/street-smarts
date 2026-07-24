//! # street-smarts-godot
//!
//! Godot 4 GDExtension bindings for `street-smarts`.
//!
//! Exposes the NIR schema, procedural pattern operators, 3D mesh building,
//! and opinion chorus / disagreement report engine directly to Godot as native nodes.

use godot::classes::mesh::PrimitiveType;
use godot::classes::{ArrayMesh, MeshInstance3D, StandardMaterial3D, SurfaceTool};
use godot::prelude::*;
use street_smarts_conflict::build_report;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::Mesh as SsMesh;
use street_smarts_opinions::registry::evaluate_all;

pub mod building_mesh;
pub mod ground_features;
use building_mesh::BuildingSolid;

/// Builds a Godot `MeshInstance3D` from an extracted `Mesh`, with flat
/// per-triangle face normals (see `rebuild_3d_mesh`'s own doc for why:
/// Naive Surface Nets shares one vertex per grid cell across every face
/// orientation it touches, which blends normals at sharp corners into a
/// wrong-looking soft gradient -- confirmed against a real device
/// screenshot). Returns `None` for an empty mesh or a SurfaceTool commit
/// failure; the caller decides whether that's worth a warning.
fn mesh_to_instance(mesh: &SsMesh, name: String, material: Option<&Gd<StandardMaterial3D>>) -> Option<Gd<MeshInstance3D>> {
    if mesh.triangles.is_empty() {
        return None;
    }
    let mut surface_tool = SurfaceTool::new_gd();
    surface_tool.begin(PrimitiveType::TRIANGLES);
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
        for &idx in tri {
            let p = mesh.positions[idx as usize];
            surface_tool.set_normal(face_normal);
            surface_tool.add_vertex(Vector3::new(p.x as real, p.y as real, p.z as real));
        }
    }
    if let Some(mat) = material {
        surface_tool.set_material(mat);
    }
    let array_mesh = surface_tool.commit()?;
    let mut mesh_instance = MeshInstance3D::new_alloc();
    mesh_instance.set_name(&name);
    mesh_instance.set_mesh(&array_mesh);
    Some(mesh_instance)
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
}

#[godot_api]
impl INode3D for NeighborhoodNode3D {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            neighborhood_json: String::new(),
            building_count: 0,
            mean_wholeness_score: 0.0,
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
        let mut open_space_material = StandardMaterial3D::new_gd();
        open_space_material.set_albedo(Color::from_rgb(0.35, 0.55, 0.32));
        open_space_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);
        let mut street_material = StandardMaterial3D::new_gd();
        street_material.set_albedo(Color::from_rgb(0.27, 0.27, 0.29));
        street_material.set_cull_mode(godot::classes::base_material_3d::CullMode::DISABLED);

        let rebuild_start = std::time::Instant::now();
        let mut meshed = 0i32;
        let mut total_tris = 0usize;
        let mut skipped_no_height = 0i32;
        for building in &nir.buildings {
            let Some(solid) = BuildingSolid::from_building(building, &origin) else {
                skipped_no_height += 1;
                continue;
            };
            // Adaptive, not a fixed 0.3m for every building: a real
            // P108-merged block can be 10x a typical building's footprint
            // diagonal, and Surface Nets cost is cubic in that -- see
            // `suggested_voxel_size`'s own doc for measured numbers.
            let mesh = solid.to_mesh(solid.suggested_voxel_size());
            total_tris += mesh.triangles.len();
            let Some(mesh_instance) = mesh_to_instance(&mesh, format!("GeneratedMassing_{}", building.id), None) else {
                continue;
            };
            self.base_mut().add_child(&mesh_instance);
            meshed += 1;
        }

        let mut open_space_meshed = 0i32;
        for open_space in &nir.open_space {
            let Some(pad) = ground_features::open_space_polygon(open_space, &origin) else {
                continue;
            };
            let mesh = pad.to_mesh();
            total_tris += mesh.triangles.len();
            let name = format!("GeneratedOpenSpace_{}", open_space.id);
            let Some(mesh_instance) = mesh_to_instance(&mesh, name, Some(&open_space_material)) else {
                continue;
            };
            self.base_mut().add_child(&mesh_instance);
            open_space_meshed += 1;
        }

        let mut street_meshed = 0i32;
        for street in &nir.streets {
            for (seg_idx, pad) in ground_features::street_ribbon_segments(street, &origin).into_iter().enumerate() {
                let mesh = pad.to_mesh();
                total_tris += mesh.triangles.len();
                let name = format!("GeneratedStreet_{}_seg{}", street.id, seg_idx);
                let Some(mesh_instance) = mesh_to_instance(&mesh, name, Some(&street_material)) else {
                    continue;
                };
                self.base_mut().add_child(&mesh_instance);
                street_meshed += 1;
            }
        }

        godot_print!(
            "Rebuilt scene: {} of {} buildings (Surface Nets), {} of {} open spaces, {} street segments (ear-clipping) -- {} tris total in {:?} ({} buildings skipped: no height_m assigned).",
            meshed,
            nir.buildings.len(),
            open_space_meshed,
            nir.open_space.len(),
            street_meshed,
            total_tris,
            rebuild_start.elapsed(),
            skipped_no_height
        );
        meshed > 0 || open_space_meshed > 0 || street_meshed > 0
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
