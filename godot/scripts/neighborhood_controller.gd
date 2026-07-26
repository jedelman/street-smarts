extends Node

@onready var neighborhood_node: Node3D = $"../NeighborhoodNode3D"
@onready var camera: Camera3D = $"../Camera3D"
@onready var minimap: Control = $"../UI/Minimap"

## Real 97.7-acre Military Circle site, the same parcel spec
## scripts/vibe-render.sh's clean_baseline scenario develops.
const REAL_PARCEL_ID := "MILITARY_CIRCLE_ASSEMBLED"
const REAL_PIPELINE_SEED := 42

## Empty (default) -> the full real 35-building site, same as before.
## Set to a real building id (e.g. from scenes/ClusterTest.tscn) to
## restrict to that building's own nearest `cluster_size` real neighbors
## instead -- a fast integration-test fixture for iterating on a single
## feature without waiting on a full-site rebuild. See
## NeighborhoodNode3D::restrict_to_cluster's own doc.
@export var cluster_anchor_building_id: String = ""
@export var cluster_size: int = 9

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

            if cluster_anchor_building_id != "":
                print("[StreetSmarts] Restricting to a %d-building cluster around '%s' for fast iteration..." % [cluster_size, cluster_anchor_building_id])
                neighborhood_node.restrict_to_cluster(cluster_anchor_building_id, cluster_size)

            neighborhood_node.rebuild_3d_mesh()
            camera.collider = neighborhood_node
            _frame_generated_massing()
            _setup_minimap()
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

## Feeds the minimap the real building footprint polygons/ids (see
## NeighborhoodNode3D::get_building_footprints) and wires its
## waypoint_selected signal to _walk_to_building -- the minimap's own
## tappable markers replace the old waypoint dropdown entirely.
func _setup_minimap() -> void:
    if minimap == null or not neighborhood_node.has_method("get_building_footprints"):
        return
    minimap.camera = camera
    minimap.set_buildings(neighborhood_node.get_building_footprints(), neighborhood_node.get_building_ids())
    if not minimap.waypoint_selected.is_connected(_walk_to_building):
        minimap.waypoint_selected.connect(_walk_to_building)

## Real generated id (e.g. "p108_merged_9_building") -> walk mode, standing
## just outside it. Empty string is the "Site Overview" convention the old
## waypoint dropdown's own null-metadata entry used -- back to orbit mode,
## reframed on the whole real site.
func _walk_to_building(building_id: String) -> void:
    if building_id == "":
        camera.set_mode_orbit()
        _frame_generated_massing()
        return
    if not camera.has_method("set_mode_walk"):
        return

    var target_child: MeshInstance3D = null
    var overall_center := Vector3.ZERO
    var have_overall := false
    var building_count := 0
    for child in neighborhood_node.get_children():
        if child is MeshInstance3D and child.name.begins_with("GeneratedMassing_"):
            overall_center += child.get_aabb().get_center()
            have_overall = true
            building_count += 1
            if child.name == "GeneratedMassing_" + building_id:
                target_child = child
    if target_child == null:
        return
    if have_overall:
        overall_center /= building_count

    var mesh_aabb: AABB = target_child.get_aabb()
    var building_center: Vector3 = mesh_aabb.get_center()
    var building_size: Vector3 = mesh_aabb.size

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
