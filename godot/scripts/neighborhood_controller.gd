extends Node

@onready var neighborhood_node: Node3D = $"../NeighborhoodNode3D"
@onready var camera: Camera3D = $"../Camera3D"
@onready var minimap: Control = $"../UI/Minimap"
@onready var pattern_lab: Control = get_node_or_null("../UI/PatternLab")

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

## False (default): unchanged behavior -- run_pattern_pipeline() runs the
## real corrected pipeline end to end, same as always. True: skip it
## entirely and leave the raw baseline's own parcels on screen (real
## ground_features::parcel_polygon rendering, see that function's own
## doc), for the PatternLab panel to step through by hand, one real
## operator at a time, instead. See PatternLab's own doc for why this
## exists: only P95 Building Complex/P37 House Cluster accept a specific
## real target today, so this is honestly "watch the pipeline unfold
## stage by stage" more than "edit one chosen building," at least until
## more real operators grow their own per-target scope.
@export var manual_pattern_stepping: bool = false

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

            if needs_pipeline and not manual_pattern_stepping:
                # Runs synchronously on the main thread -- on a real 35-building
                # site this is a real, noticeable pause (desktop CPU: ~5s just
                # for the pipeline, plus meshing on top). A loading indicator /
                # background-thread run is the honest next step here, not
                # attempted yet.
                print("[StreetSmarts] Running pattern-language pipeline on parcel '%s'..." % REAL_PARCEL_ID)
                neighborhood_node.run_pattern_pipeline(REAL_PARCEL_ID, REAL_PIPELINE_SEED)
            elif manual_pattern_stepping:
                print("[StreetSmarts] Manual pattern stepping: loaded the raw baseline only -- use PatternLab to apply real operators one at a time.")

            if cluster_anchor_building_id != "":
                print("[StreetSmarts] Restricting to a %d-building cluster around '%s' for fast iteration..." % [cluster_size, cluster_anchor_building_id])
                neighborhood_node.restrict_to_cluster(cluster_anchor_building_id, cluster_size)

            neighborhood_node.rebuild_3d_mesh()
            camera.collider = neighborhood_node
            _frame_generated_massing()
            _setup_minimap()
            _setup_pattern_lab()
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
    # No real buildings yet (manual_pattern_stepping's early stages, before
    # any building-producing operator has run) -- frame the raw parcel
    # fabric instead of leaving the camera at whatever default it had, so
    # there's something real to look at from the very first frame.
    if not have_any:
        for child in neighborhood_node.get_children():
            if child is MeshInstance3D and child.name.begins_with("GeneratedParcel_"):
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

## Wires PatternLab to the real neighborhood node and refreshes the minimap
## + camera framing after every real pattern step, so newly-generated
## buildings/parcels show up immediately without a manual reload.
func _setup_pattern_lab() -> void:
    if pattern_lab == null:
        return
    pattern_lab.neighborhood_node = neighborhood_node
    # Deferred: Controller's own _ready() (this function) can run before
    # PatternLab's sibling _ready() has built its own child controls
    # (Godot's per-frame _ready() order across UI's children isn't
    # guaranteed to have finished by the time Controller, an earlier
    # sibling of UI under Main, runs its own) -- calling refresh_patterns
    # directly here hit exactly that: _pattern_list was still null.
    pattern_lab.call_deferred("refresh_patterns")
    if not pattern_lab.pattern_applied.is_connected(_on_pattern_applied):
        pattern_lab.pattern_applied.connect(_on_pattern_applied)

func _on_pattern_applied() -> void:
    _frame_generated_massing()
    _setup_minimap()

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
