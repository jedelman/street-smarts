extends Node

@onready var neighborhood_node: Node3D = $"../NeighborhoodNode3D"
@onready var camera: Camera3D = $"../Camera3D"
@onready var minimap: Control = $"../UI/Minimap"
@onready var pattern_lab: Control = get_node_or_null("../UI/PatternLab")

var pause_menu: Control = null

## Real 97.7-acre Military Circle site, the same parcel spec
## scripts/vibe-render.sh's clean_baseline scenario develops.
const REAL_PARCEL_ID := "MILITARY_CIRCLE_ASSEMBLED"
const REAL_PIPELINE_SEED := 42

## This scene's own real path -- set per-scene (Main.tscn / PatternLab.tscn)
## in the editor. Used two ways: tagging a save so MainMenu's "Continue"
## reloads the SAME scene the save came from (not Node's own
## scene_file_path, which is only ever set on a packed scene's root node --
## Controller is a child of it, not the root), and matching a save against
## this scene on load so a Site Tour save doesn't get restored into
## Pattern Lab or vice versa.
@export var scene_path: String = "res://scenes/Main.tscn"

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
    GameState.apply_settings_to_camera(camera)
    _setup_pause_menu()

    # MainMenu's "Continue" queues real save data before changing scene --
    # see GameState.gd's own doc. Only honored if it's actually for THIS
    # scene (a Site Tour save reloaded into Pattern Lab, or vice versa,
    # would restore the wrong controller's expectations entirely).
    var pending := GameState.consume_pending_save()
    if not pending.is_empty() and String(pending.get("scene_path", "")) == scene_path:
        _restore_from_save(pending)
        return

    _load_fresh_baseline()

## The original (pre-save/continue) boot path: real Eastside Commons
## parcel data first: eastside-baseline.json is parcels-only on disk (no
## generator has ever run against it before now), so run_pattern_pipeline()
## runs the actual Alexander pattern-language pipeline (P96/P107/P221/etc,
## the SAME street_smarts_patterns::pipeline::run_corrected_pipeline the
## production gallery's offline renders come from) right here, on
## whatever device this is running on, to populate real buildings before
## rebuild_3d_mesh() has anything to mesh. Falls back to the synthetic
## demo fixture only if the real one isn't staged. Also the fallback when
## a queued save turns out to be missing/corrupt (see _restore_from_save).
func _load_fresh_baseline() -> void:
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

## Restores a real, previously-saved neighborhood (possibly already
## pattern-edited in Pattern Lab) instead of the raw baseline, plus the
## real camera state it was saved with. Falls back to _load_fresh_baseline()
## if the save's own JSON no longer parses -- a stale/corrupt save
## shouldn't strand the player on a blank scene.
func _restore_from_save(data: Dictionary) -> void:
    var json_str := String(data.get("neighborhood_json", ""))
    if json_str == "" or not neighborhood_node.has_method("load_nir_json") or not neighborhood_node.load_nir_json(json_str):
        print("[StreetSmarts] Saved neighborhood JSON missing or no longer parses -- falling back to a fresh baseline.")
        _load_fresh_baseline()
        return

    print("[StreetSmarts] Restored a saved neighborhood (%d buildings)." % neighborhood_node.get_building_count())
    neighborhood_node.rebuild_3d_mesh()
    camera.collider = neighborhood_node
    _restore_camera_from_save(data)
    _setup_minimap()
    _setup_pattern_lab()

func _restore_camera_from_save(data: Dictionary) -> void:
    if String(data.get("camera_mode", "orbit")) == "walk" and camera.has_method("set_mode_walk"):
        var wp: Array = data.get("walk_position", [0.0, 0.0, 0.0])
        camera.set_mode_walk(Vector3(wp[0], wp[1], wp[2]), float(data.get("walk_yaw", 0.0)))
    else:
        var t: Array = data.get("target", [camera.target.x, camera.target.y, camera.target.z])
        camera.target = Vector3(t[0], t[1], t[2])
        camera.distance = float(data.get("distance", camera.distance))
        camera.yaw = float(data.get("yaw", camera.yaw))
        camera.pitch = float(data.get("pitch", camera.pitch))
        camera.set_mode_orbit()

## Instances the pause overlay (see pause_menu.gd's own doc for why it's
## the one way back to the main menu once a game scene has started) and
## wires its two real actions -- this controller is the only thing that
## knows how to build real save data or where "the main menu" is.
func _setup_pause_menu() -> void:
    pause_menu = preload("res://scripts/pause_menu.gd").new()
    get_node("../UI").add_child(pause_menu)
    pause_menu.save_requested.connect(_on_save_requested)
    pause_menu.quit_to_menu_requested.connect(_on_quit_to_menu_requested)

func _on_save_requested() -> void:
    GameState.write_save(_build_save_data())
    if pause_menu != null:
        pause_menu.show_saved_confirmation()

func _on_quit_to_menu_requested() -> void:
    get_tree().change_scene_to_file("res://scenes/MainMenu.tscn")

## Everything MainMenu's "Continue" needs to put a player back exactly
## where they left off: which scene, the real (possibly pattern-edited)
## neighborhood, and where the camera/walker was standing.
func _build_save_data() -> Dictionary:
    var data := {
        "scene_path": scene_path,
        "neighborhood_json": neighborhood_node.get_neighborhood_json(),
    }
    if camera.mode == camera.Mode.WALK:
        data["camera_mode"] = "walk"
        data["walk_position"] = [camera.walk_position.x, camera.walk_position.y, camera.walk_position.z]
        data["walk_yaw"] = camera.walk_yaw
    else:
        data["camera_mode"] = "orbit"
        data["target"] = [camera.target.x, camera.target.y, camera.target.z]
        data["distance"] = camera.distance
        data["yaw"] = camera.yaw
        data["pitch"] = camera.pitch
    return data

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
    minimap.neighborhood_node = neighborhood_node
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
    if not pattern_lab.pick_toggled.is_connected(_on_pick_toggled):
        pattern_lab.pick_toggled.connect(_on_pick_toggled)
    if minimap != null and not minimap.zone_selected.is_connected(_on_zone_picked):
        minimap.zone_selected.connect(_on_zone_picked)
    if camera.has_signal("zone_picked") and not camera.zone_picked.is_connected(_on_zone_picked):
        camera.zone_picked.connect(_on_zone_picked)

func _on_pattern_applied() -> void:
    _frame_generated_massing()
    _setup_minimap()

## The object selector's real single entry/exit point: turns PatternLab's
## own "Pick on Map/World" toggle into `start_picking`/`cancel_picking`
## calls on BOTH the minimap and the 3D walkabout camera at once, since a
## person picking a target could reasonably do either without switching
## anything else first.
func _on_pick_toggled(active: bool) -> void:
    if active:
        if minimap != null:
            minimap.start_picking()
        if camera.has_method("start_picking"):
            camera.start_picking()
    else:
        if minimap != null:
            minimap.cancel_picking()
        if camera.has_method("cancel_picking"):
            camera.cancel_picking()

## Fires from either real picking source (minimap.zone_selected or
## orbit_camera's own zone_picked) -- cancels picking on BOTH regardless
## of which one actually produced the hit (the other is still sitting in
## its own picking state and has no way to know the session is over
## otherwise), then hands the real id to PatternLab's target field.
func _on_zone_picked(_kind: String, id: String) -> void:
    if minimap != null:
        minimap.cancel_picking()
    if camera.has_method("cancel_picking"):
        camera.cancel_picking()
    if pattern_lab != null:
        pattern_lab.set_target(id)

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
