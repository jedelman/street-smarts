extends Node

@onready var neighborhood_node: Node3D = $"../NeighborhoodNode3D"
@onready var camera: Camera3D = $"../Camera3D"
@onready var geometric_label: Label = $"../UI/Panel/VBox/GeometricChorusLabel"
@onready var activist_label: Label = $"../UI/Panel/VBox/ActivistChorusLabel"
@onready var prompt_label: Label = $"../UI/Panel/VBox/HumanPromptLabel"

## Real 97.7-acre Military Circle site, the same parcel spec
## scripts/vibe-render.sh's clean_baseline scenario develops.
const REAL_PARCEL_ID := "MILITARY_CIRCLE_ASSEMBLED"
const REAL_PIPELINE_SEED := 42

func _ready():
    # Real Eastside Commons parcel data first: eastside-baseline.json is
    # parcels-only on disk (no generator has ever run against it before
    # now), so run_pattern_pipeline() runs the actual Alexander
    # pattern-language pipeline (P96/P107/P221/etc, the SAME
    # street_smarts_patterns::pipeline::run_corrected_pipeline the
    # production gallery's offline renders come from) right here, on
    # whatever device this is running on, to populate real buildings
    # before rebuild_3d_mesh() has anything to mesh. Falls back to the
    # synthetic demo fixture only if the real one isn't staged.
    var real_path = "res://eastside-baseline.json"
    if not FileAccess.file_exists(real_path):
        real_path = "res://data/eastside-baseline.json"

    var path = real_path
    var needs_pipeline = true
    if not FileAccess.file_exists(path):
        path = "res://demo-massing.json"
        needs_pipeline = false

    var file = FileAccess.open(path, FileAccess.READ)
    if file:
        var json_str = file.get_as_text()
        if neighborhood_node.has_method("load_nir_json") and neighborhood_node.load_nir_json(json_str):
            print("[StreetSmarts] Loaded NIR fixture into Godot 4 spatial engine!")

            if needs_pipeline:
                # Runs synchronously on the main thread -- on a real 35-building
                # site this is a real, noticeable pause (desktop CPU: ~5s just
                # for the pipeline, plus meshing on top). A loading indicator /
                # background-thread run is the honest next step here, not
                # attempted yet.
                print("[StreetSmarts] Running pattern-language pipeline on parcel '%s'..." % REAL_PARCEL_ID)
                neighborhood_node.run_pattern_pipeline(REAL_PARCEL_ID, REAL_PIPELINE_SEED)

            neighborhood_node.rebuild_3d_mesh()
            _frame_generated_massing()

            var metrics = neighborhood_node.evaluate_opinions()
            if metrics.has("geometric_headline"):
                geometric_label.text = "Geometric Chorus: " + str(metrics["geometric_headline"])
            if metrics.has("activist_headline"):
                activist_label.text = "Activist Chorus: " + str(metrics["activist_headline"])
            if metrics.has("question_count"):
                prompt_label.text = "Human Disagreement Prompts (" + str(metrics["question_count"]) + " surfaced)"
    else:
        print("[StreetSmarts] NIR fixture file ready to be bound at runtime.")

## Points the orbit camera at the real combined bounding box of whatever
## rebuild_3d_mesh() actually generated, instead of trusting orbit_camera.gd's
## fixed default (tuned for the small synthetic demo near local origin).
## Every "GeneratedMassing_*" MeshInstance3D sits at an identity transform
## directly under neighborhood_node (rebuild_3d_mesh() never repositions
## them -- vertex positions are already in the shared local-meter frame),
## so get_aabb()'s local-space result IS the world-space bounds here; no
## transform math needed on top.
func _frame_generated_massing() -> void:
    if not camera.has_method("frame_bounds"):
        return
    var combined: AABB
    var have_any := false
    for child in neighborhood_node.get_children():
        if child is MeshInstance3D and child.name.begins_with("GeneratedMassing_"):
            var mesh_aabb: AABB = child.get_aabb()
            if not have_any:
                combined = mesh_aabb
                have_any = true
            else:
                combined = combined.merge(mesh_aabb)
    if have_any:
        camera.frame_bounds(combined.get_center(), combined.size.length() * 0.5)
