//! # street-smarts-godot
//!
//! Godot 4 GDExtension bindings for `street-smarts`.
//!
//! Exposes the NIR schema, procedural pattern operators, 3D mesh building,
//! and opinion chorus / disagreement report engine directly to Godot as native nodes.

// `deny`, not `forbid`: gdext's own `#[gdextension]` entry-point macro
// requires exactly one `unsafe impl ExtensionLibrary`, which `forbid` can't
// be locally overridden for. Everywhere else in this crate, unsafe code is
// still rejected.
#![deny(unsafe_code)]

use godot::classes::mesh::PrimitiveType;
use godot::classes::{ArrayMesh, MeshInstance3D, SurfaceTool};
use godot::prelude::*;
use street_smarts_conflict::build_report;
use street_smarts_core::geometry::LngLat;
use street_smarts_core::nir::Neighborhood;
use street_smarts_opinions::registry::evaluate_all;

mod building_mesh;
use building_mesh::BuildingSolid;

struct StreetSmartsExtension;

// gdext's own entry-point macro requires this `unsafe impl` -- it's the one
// spot the crate-wide `deny(unsafe_code)` above can't honestly apply to,
// since the trait itself is `unsafe` in the `godot` crate we depend on, not
// unsafe code this crate wrote. A module-level `allow` is used (rather than
// one directly on the impl) because `#[gdextension]` doesn't re-emit outer
// attributes placed on the item it expands.
#[allow(unsafe_code)]
mod extension_entry {
    use super::StreetSmartsExtension;
    use godot::prelude::{gdextension, ExtensionLibrary};

    #[gdextension]
    unsafe impl ExtensionLibrary for StreetSmartsExtension {}
}

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

    /// Rebuilds the procedural 3D building massing via constructive SDF +
    /// Surface Nets extraction (`building_mesh::BuildingSolid`), replacing
    /// any previously generated massing children. Streets, plazas, and
    /// Salingaros scale/center indicators are not yet built here -- only
    /// building massing (Phase 2 continuation).
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
                .filter(|c| c.get_name().to_string().starts_with("GeneratedMassing_"))
                .collect()
        };
        for mut child in stale {
            child.queue_free();
        }

        let mut meshed = 0i32;
        let mut skipped_no_height = 0i32;
        for building in &nir.buildings {
            let Some(solid) = BuildingSolid::from_building(building, &origin) else {
                skipped_no_height += 1;
                continue;
            };
            let mesh = solid.to_mesh(0.3);
            if mesh.triangles.is_empty() {
                continue;
            }

            let mut surface_tool = SurfaceTool::new_gd();
            surface_tool.begin(PrimitiveType::TRIANGLES);
            for tri in &mesh.triangles {
                for &idx in tri {
                    let p = mesh.positions[idx as usize];
                    let n = mesh.normals[idx as usize];
                    surface_tool.set_normal(Vector3::new(n.x as real, n.y as real, n.z as real));
                    surface_tool.add_vertex(Vector3::new(p.x as real, p.y as real, p.z as real));
                }
            }
            let Some(array_mesh) = surface_tool.commit() else {
                godot_warn!("SurfaceTool::commit() returned no mesh for building {}", building.id);
                continue;
            };

            let mut mesh_instance = MeshInstance3D::new_alloc();
            mesh_instance.set_name(&format!("GeneratedMassing_{}", building.id));
            mesh_instance.set_mesh(&array_mesh);
            self.base_mut().add_child(&mesh_instance);
            meshed += 1;
        }

        godot_print!(
            "Rebuilt 3D massing for {} of {} buildings via Surface Nets extraction ({} skipped: no height_m assigned).",
            meshed,
            nir.buildings.len(),
            skipped_no_height
        );
        meshed > 0
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
