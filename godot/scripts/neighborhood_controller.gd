extends Node

@onready var neighborhood_node: Node3D = $"../NeighborhoodNode3D"
@onready var camera: Camera3D = $"../Camera3D"
@onready var geometric_label: Label = $"../UI/Panel/VBox/GeometricChorusLabel"
@onready var activist_label: Label = $"../UI/Panel/VBox/ActivistChorusLabel"
@onready var prompt_label: Label = $"../UI/Panel/VBox/HumanPromptLabel"
@onready var waypoint_dropdown: OptionButton = $"../UI/Panel/VBox/WaypointDropdown"

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
            camera.collider = neighborhood_node
            _frame_generated_massing()
            _populate_waypoints()

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

## Fills the dropdown with "Site Overview" (index 0, back to orbit mode)
## plus one entry per real generated building, labeled with its real
## generated id (e.g. "p108 merged 9 building") -- not a curated or
## invented human name, since there isn't one to give it; the pattern
## pipeline doesn't produce anything more readable than that. Selecting a
## building entry switches to walk mode, standing just outside it.
func _populate_waypoints() -> void:
    if camera == null or not camera.has_method("set_mode_walk"):
        return
    if waypoint_dropdown.item_selected.is_connected(_on_waypoint_selected):
        waypoint_dropdown.item_selected.disconnect(_on_waypoint_selected)
    waypoint_dropdown.clear()
    waypoint_dropdown.add_item("Site Overview")
    waypoint_dropdown.set_item_metadata(0, null)

    var overall_center := Vector3.ZERO
    var have_overall := false
    var entries := []
    for child in neighborhood_node.get_children():
        if child is MeshInstance3D and child.name.begins_with("GeneratedMassing_"):
            var mesh_aabb: AABB = child.get_aabb()
            var center := mesh_aabb.get_center()
            entries.append({
                "label": child.name.trim_prefix("GeneratedMassing_").replace("_", " "),
                "center": center,
                "size": mesh_aabb.size,
            })
            overall_center += center
            have_overall = true
    if have_overall:
        overall_center /= entries.size()

    for entry in entries:
        var idx := waypoint_dropdown.item_count
        waypoint_dropdown.add_item(entry["label"])
        entry["overall_center"] = overall_center
        waypoint_dropdown.set_item_metadata(idx, entry)

    waypoint_dropdown.item_selected.connect(_on_waypoint_selected)

func _on_waypoint_selected(index: int) -> void:
    var meta = waypoint_dropdown.get_item_metadata(index)
    if meta == null:
        camera.set_mode_orbit()
        _frame_generated_massing()
        return

    var building_center: Vector3 = meta["center"]
    var building_size: Vector3 = meta["size"]
    var overall_center: Vector3 = meta["overall_center"]

    # Stand between the site's overall center and this building, facing
    # inward -- a real, deterministic vantage point computed from the
    # actual generated geometry, not a fabricated "front door" (openings
    # aren't exposed to GDScript today, only the finished mesh AABB is).
    var away := building_center - overall_center
    away.y = 0.0
    if away.length() < 0.5:
        away = Vector3(0.0, 0.0, 1.0)
    away = away.normalized()

    var standoff: float = max(building_size.x, building_size.z) * 0.6 + 6.0
    var spawn := building_center + away * standoff
    spawn.y = 0.0

    var to_building := building_center - spawn
    var facing_yaw := atan2(to_building.x, to_building.z)
    camera.set_mode_walk(spawn, facing_yaw)
