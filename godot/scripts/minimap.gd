extends Control

## A real top-down map drawn from the same real building footprint polygons
## `resolve_move`/`find_path` route around (see NeighborhoodNode3D::
## get_building_footprints) -- not an approximation from mesh AABBs.
##
## Two states on one Control:
## - Small (default): fixed-size corner widget, upper right, auto-fit to
##   the whole real site, showing the player's own real position/heading
##   as a marker. Any tap expands it.
## - Expanded: fills the screen. Real building markers become tappable --
##   tapping one emits `waypoint_selected(id)` (neighborhood_controller.gd
##   does the actual walk-there, same as the waypoint dropdown it
##   replaces) and collapses back to small. One-finger drag pans,
##   two-finger pinch zooms (same touch-tracking pattern orbit_camera.gd
##   already uses for its own pinch handling). A short tap that doesn't
##   land on a building marker just closes the map -- no dedicated close
##   button, tap-outside-to-dismiss is the whole affordance.
##
## Replaces the old UI/Panel (title + opinion-chorus labels + waypoint
## dropdown) entirely, per the explicit design call to lead with real
## spatial navigation instead of a dropdown list of raw generator ids.
##
## A third state, `_picking`, layers on top of expanded mode for the
## object-selector (see pattern_lab.gd's "Pick on Map" button): entered
## via `start_picking()`, a tap resolves through the SAME real hit-test
## `waypoint_selected` already used for buildings (nearest marker within
## `MARKER_HIT_RADIUS_PX`), falling back to `NeighborhoodNode3D::
## pick_zone_at` (a real point-in-polygon test against both buildings and
## raw parcels) for a tap that misses every marker -- a picking tap on a
## parcel/block has no marker to hit, unlike a building. Emits
## `zone_selected(kind, id)` instead of walking there, and a miss (empty
## ground, off the real site) stays in picking mode rather than closing
## the map, so a mis-tap doesn't force re-opening the picker.

signal waypoint_selected(building_id: String)
signal zone_selected(kind: String, id: String)

## Set by neighborhood_controller.gd, same as `camera` -- only needed for
## the picking-mode parcel fallback (`pick_zone_at`); building hits never
## need it, they're already resolved from `_centroids`/`_building_ids`.
var neighborhood_node: Node = null

const SMALL_SIZE := Vector2(160, 160)
const SMALL_MARGIN := 16.0
const MARKER_RADIUS_PX := 6.0
const MARKER_HIT_RADIUS_PX := 22.0
const PLAYER_MARKER_RADIUS_PX := 5.0
## A touch/click that moves less than this between press and release counts
## as a tap; more counts as a pan drag. Same convention orbit_camera.gd
## uses for its own tap-vs-drag distinction.
const TAP_MAX_DRAG_PX := 12.0
const MIN_ZOOM := 0.5
const MAX_ZOOM := 8.0

var camera: Camera3D = null

var _footprints: Array = []          # Array[PackedVector2Array], real (x,z) rings
var _building_ids: PackedStringArray = PackedStringArray()
var _centroids: Array[Vector2] = []  # parallel to _building_ids
var _site_center: Vector2 = Vector2.ZERO
var _site_half_span: float = 50.0    # meters; guards against a degenerate/empty site

var _expanded: bool = false
var _pan: Vector2 = Vector2.ZERO     # pixels, expanded mode only
var _zoom: float = 1.0               # expanded mode only

var _touch_points: Dictionary = {}
var _touch_start_pos: Dictionary = {}
var _last_pinch_span: float = 0.0

var _picking: bool = false

func _ready() -> void:
	mouse_filter = Control.MOUSE_FILTER_STOP
	_apply_layout()

## Enters picking mode -- expands the map (if not already) and switches
## the next tap over to `zone_selected` instead of walk-there. Idempotent.
func start_picking() -> void:
	_picking = true
	if not _expanded:
		_set_expanded(true)
	queue_redraw()

## Leaves picking mode without making a selection -- called by
## pattern_lab.gd when its own "Pick on Map" toggle is turned back off by
## hand, so a half-finished pick doesn't linger.
func cancel_picking() -> void:
	_picking = false
	queue_redraw()

## Called once after a rebuild -- real building footprints (site-local
## meters, ground plane x/z) and their real ids, same order. Recomputes
## the real site bounds this map auto-fits to.
func set_buildings(footprints: Array, ids: PackedStringArray) -> void:
	_footprints = footprints
	_building_ids = ids
	_centroids.clear()

	var min_x := INF
	var max_x := -INF
	var min_z := INF
	var max_z := -INF
	var have_any := false
	for ring in _footprints:
		var sum := Vector2.ZERO
		var n: int = ring.size()
		if n == 0:
			_centroids.append(Vector2.ZERO)
			continue
		for p in ring:
			sum += p
			min_x = min(min_x, p.x)
			max_x = max(max_x, p.x)
			min_z = min(min_z, p.y)
			max_z = max(max_z, p.y)
			have_any = true
		_centroids.append(sum / n)

	if have_any:
		_site_center = Vector2((min_x + max_x) * 0.5, (min_z + max_z) * 0.5)
		_site_half_span = max(max((max_x - min_x) * 0.5, (max_z - min_z) * 0.5), 5.0)
	queue_redraw()

func _process(_delta: float) -> void:
	# The player marker moves continuously in walk mode; redraw every
	# frame is cheap here (a few dozen polygons, immediate-mode 2D draw).
	if camera != null:
		queue_redraw()

func _set_expanded(value: bool) -> void:
	if _expanded == value:
		return
	_expanded = value
	_pan = Vector2.ZERO
	_zoom = 1.0
	_apply_layout()
	queue_redraw()

func _apply_layout() -> void:
	if _expanded:
		set_anchors_and_offsets_preset(PRESET_FULL_RECT)
	else:
		anchor_left = 1.0
		anchor_right = 1.0
		anchor_top = 0.0
		anchor_bottom = 0.0
		offset_left = -(SMALL_SIZE.x + SMALL_MARGIN)
		offset_right = -SMALL_MARGIN
		offset_top = SMALL_MARGIN
		offset_bottom = SMALL_MARGIN + SMALL_SIZE.y

## Real world (x, z) -> this control's own local pixel space. North (+z)
## points up on the map, the usual map convention -- screen Y grows down,
## so it's subtracted rather than added.
func _world_to_screen(world_xz: Vector2) -> Vector2:
	var base_scale: float = (min(size.x, size.y) * 0.42) / _site_half_span
	var scale: float = base_scale * (_zoom if _expanded else 1.0)
	var dx := world_xz.x - _site_center.x
	var dz := world_xz.y - _site_center.y
	var pan: Vector2 = _pan if _expanded else Vector2.ZERO
	return Vector2(
		size.x * 0.5 + dx * scale + pan.x,
		size.y * 0.5 - dz * scale + pan.y
	)

## Inverse of `_world_to_screen` -- picking mode's only real use for it,
## turning a tap that missed every building marker into the real world
## (x, z) `pick_zone_at` needs to test against raw parcels.
func _screen_to_world(screen_pos: Vector2) -> Vector2:
	var base_scale: float = (min(size.x, size.y) * 0.42) / _site_half_span
	var scale: float = base_scale * (_zoom if _expanded else 1.0)
	var pan: Vector2 = _pan if _expanded else Vector2.ZERO
	var dx := (screen_pos.x - size.x * 0.5 - pan.x) / scale
	var dz := -(screen_pos.y - size.y * 0.5 - pan.y) / scale
	return Vector2(_site_center.x + dx, _site_center.y + dz)

func _draw() -> void:
	draw_rect(Rect2(Vector2.ZERO, size), Color(0.08, 0.08, 0.09, 0.88 if _expanded else 0.75), true)
	draw_rect(Rect2(Vector2.ZERO, size), Color(1, 1, 1, 0.25), false, 1.5)

	for ring in _footprints:
		if ring.size() < 3:
			continue
		var screen_pts := PackedVector2Array()
		for p in ring:
			screen_pts.append(_world_to_screen(p))
		draw_colored_polygon(screen_pts, Color(0.62, 0.60, 0.54, 0.9))

	if _expanded:
		for i in range(_centroids.size()):
			var p := _world_to_screen(_centroids[i])
			draw_circle(p, MARKER_RADIUS_PX, Color(1.0, 0.05, 0.9, 0.95) if not _picking else Color(0.15, 0.85, 1.0, 0.95))
		var hint := "tap a building or block to select" if _picking else "tap a building to walk there · tap elsewhere to close"
		draw_string(ThemeDB.fallback_font, Vector2(12, 22), hint,
			HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color(1, 1, 1, 0.85))

	if camera != null:
		var player_xz: Vector2
		var has_heading := false
		var heading := 0.0
		# camera is the real orbit_camera.gd-scripted Camera3D (same
		# trust convention `collider.resolve_move()`/`find_path()` already
		# use elsewhere) -- duck-typed access to its own Mode enum/fields.
		if camera.mode == camera.Mode.WALK:
			player_xz = Vector2(camera.walk_position.x, camera.walk_position.z)
			heading = camera.walk_yaw
			has_heading = true
		else:
			player_xz = Vector2(camera.target.x, camera.target.z)
		var p := _world_to_screen(player_xz)
		draw_circle(p, PLAYER_MARKER_RADIUS_PX, Color(0.2, 0.6, 1.0, 1.0))
		if has_heading:
			# Same yaw convention _look_direction() uses (0 faces +Z,
			# positive yaw sweeps toward +X) -- screen Y is flipped
			# (north-up), so the Z component negates to match.
			var dir := Vector2(sin(heading), -cos(heading))
			draw_line(p, p + dir * (PLAYER_MARKER_RADIUS_PX * 3.0), Color(0.2, 0.6, 1.0, 1.0), 2.0)

func _gui_input(event: InputEvent) -> void:
	if event is InputEventScreenTouch:
		if event.pressed:
			_touch_points[event.index] = event.position
			_touch_start_pos[event.index] = event.position
		else:
			var was_sole_touch: bool = _touch_points.size() == 1 and _touch_points.has(event.index)
			if was_sole_touch:
				var start: Vector2 = _touch_start_pos.get(event.index, event.position)
				if start.distance_to(event.position) <= TAP_MAX_DRAG_PX:
					_handle_tap(event.position)
			_touch_points.erase(event.index)
			_touch_start_pos.erase(event.index)
		if _touch_points.size() != 2:
			_last_pinch_span = 0.0
		accept_event()
	elif event is InputEventScreenDrag:
		_touch_points[event.index] = event.position
		if _expanded:
			if _touch_points.size() == 1:
				_pan += event.relative
				queue_redraw()
			elif _touch_points.size() == 2:
				_handle_pinch()
		accept_event()
	elif event is InputEventMouseButton and event.button_index == MOUSE_BUTTON_LEFT:
		if event.pressed:
			_touch_start_pos[-1] = event.position
		else:
			var start: Vector2 = _touch_start_pos.get(-1, event.position)
			if start.distance_to(event.position) <= TAP_MAX_DRAG_PX:
				_handle_tap(event.position)
		accept_event()

func _handle_tap(pos: Vector2) -> void:
	if not _expanded:
		_set_expanded(true)
		return
	var nearest_idx := -1
	var nearest_dist := MARKER_HIT_RADIUS_PX
	for i in range(_centroids.size()):
		var d: float = _world_to_screen(_centroids[i]).distance_to(pos)
		if d < nearest_dist:
			nearest_dist = d
			nearest_idx = i

	if _picking:
		if nearest_idx >= 0:
			_picking = false
			zone_selected.emit("building", _building_ids[nearest_idx])
			_set_expanded(false)
			return
		var world := _screen_to_world(pos)
		if neighborhood_node != null and neighborhood_node.has_method("pick_zone_at"):
			var result: Dictionary = neighborhood_node.pick_zone_at(world.x, world.y)
			if String(result.get("kind", "none")) != "none":
				_picking = false
				zone_selected.emit(result["kind"], result["id"])
				_set_expanded(false)
				return
		# A real miss (empty ground, off-site) -- stay in picking mode
		# rather than closing the map, so a mis-tap doesn't force
		# re-opening the picker from pattern_lab.gd's own toggle.
		return

	# A real building hit walks there; tapping empty map space is the
	# close gesture AND doubles as "back to overview" (empty string,
	# same "no building" convention the old waypoint dropdown's own
	# null-metadata "Site Overview" entry used) -- no separate button
	# needed to get back out of walk mode.
	waypoint_selected.emit(_building_ids[nearest_idx] if nearest_idx >= 0 else "")
	_set_expanded(false)

func _handle_pinch() -> void:
	var indices := _touch_points.keys()
	if indices.size() < 2:
		return
	var a: Vector2 = _touch_points[indices[0]]
	var b: Vector2 = _touch_points[indices[1]]
	var current_span := a.distance_to(b)
	if _last_pinch_span > 0.0 and current_span > 0.0:
		_zoom = clamp(_zoom * (current_span / _last_pinch_span), MIN_ZOOM, MAX_ZOOM)
		queue_redraw()
	_last_pinch_span = current_span
