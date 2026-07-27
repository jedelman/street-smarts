extends Camera3D

## Two camera modes sharing one Camera3D node:
## - ORBIT (default): one-finger drag orbits around `target`, two-finger
##   pinch zooms. Mouse-drag + scroll wheel mirror it for desktop testing.
## - WALK: fixed eye height above the ground plane (walk_height_m).
##   Tap-to-move (Diablo-style): a short tap raycasts from the camera
##   through the tapped screen point onto the y=0 ground plane and walks
##   there, auto-facing the direction of travel. The route itself comes
##   from NeighborhoodNode3D::find_path -- a real A* search around every
##   real building footprint (see pathfinding.rs), not a straight line:
##   tapping a point behind a building now routes around it instead of
##   walking into its near wall. `resolve_move`'s per-step wall collision
##   + sliding still runs on every leg regardless, as a real-time safety
##   net under the planned route (grid-cell snapping means a leg can graze
##   a few cm closer to a wall than intended). No collider wired up, or no
##   real route exists (e.g. a fully sealed interior), falls back to the
##   original straight-line walk rather than refusing to move at all. A
##   one-finger DRAG still looks around (unchanged) and cancels any
##   in-progress walk, so look always wins over a stale destination
##   instead of fighting it. Replaces an earlier virtual-joystick control
##   scheme (touch_joystick.gd, since removed) that coupled movement and
##   look in a way that felt wrong on a real device.
##
## Both modes build their look direction from the SAME (pitch, yaw) ->
## direction formula (see _look_direction()), which orbit's own
## _update_transform() already uses and which is confirmed correct against
## a real device (buildings visibly, correctly framed on screen) --
## walk mode's _apply_walk_transform() reuses it via look_at() rather than
## setting Node3D.rotation directly, deliberately avoiding a second,
## separately-signed convention this environment has no display to check.

## Object-selector "walkabout" picking (see NeighborhoodNode3D::pick_zone_at
## and pattern_lab.gd's "Pick on Map/World" button): while `picking_mode` is
## on, a walk-mode tap that would normally plan a route instead raycasts
## the SAME ground point (`_try_set_walk_target`'s own technique, reused
## verbatim) and resolves it through `pick_zone_at` on `collider`, emitting
## `zone_picked(kind, id)` instead of walking there. Gated to WALK mode's
## existing tap handling, same as tap-to-walk itself -- picking from the
## 3D view only makes sense once you're standing in it, not while orbiting
## the whole site from above (the minimap already covers that overview
## case). A miss (ground point outside every building/parcel, or looking
## at the sky) stays in picking mode rather than silently doing nothing
## forever, mirroring minimap.gd's own "stay open on a miss" behavior.
signal zone_picked(kind: String, id: String)
var picking_mode: bool = false

func start_picking() -> void:
	picking_mode = true

func cancel_picking() -> void:
	picking_mode = false

enum Mode { ORBIT, WALK }

## GameState's own real "look sensitivity"/"walk speed" settings (see
## apply_settings() below) scale these base values rather than duplicating
## them as separate constants over there -- this file owns what "1.0x"
## means, GameState just owns the chosen scale factor.
const BASE_ORBIT_SENSITIVITY := 0.006
const BASE_MOUSE_ORBIT_SENSITIVITY := 0.008
const BASE_WALK_LOOK_SENSITIVITY := 0.006
const BASE_WALK_SPEED_MPS := 9.0
const BASE_WALK_MAX_SPEED_MPS := 75.0

@export var target: Vector3 = Vector3(0.0, 5.0, 0.0)
@export var distance: float = 72.0
@export var min_distance: float = 8.0
# High enough to fit the real Military Circle site (real buildings measured
# spanning 600m+) via frame_bounds(), not just the small synthetic demo.
@export var max_distance: float = 2000.0
@export var yaw: float = 0.0
@export var pitch: float = deg_to_rad(33.7)
@export var min_pitch: float = deg_to_rad(10.0)
@export var max_pitch: float = deg_to_rad(80.0)
@export var orbit_sensitivity: float = BASE_ORBIT_SENSITIVITY
@export var mouse_orbit_sensitivity: float = BASE_MOUSE_ORBIT_SENSITIVITY
@export var mouse_zoom_step: float = 0.1

@export var walk_height_m: float = 1.7
## Speed for a tap-to-move trip is derived from its own distance (see
## _try_set_walk_target()), not this flat value -- a short adjustment
## across a plaza and a 300m cross-site trip shouldn't take the same
## per-meter time. This is the walking pace for a short hop; distance
## scales it up toward walk_max_speed_mps for longer trips.
@export var walk_speed_mps: float = BASE_WALK_SPEED_MPS
@export var walk_max_speed_mps: float = BASE_WALK_MAX_SPEED_MPS
## Roughly how long a trip should take regardless of its distance --
## speed = clamp(distance / walk_target_trip_seconds, walk_speed_mps,
## walk_max_speed_mps). Fixed per trip at tap time, not recomputed as the
## remaining distance shrinks -- a speed that keeps dropping as you
## approach would make the last few meters crawl instead of arrive.
@export var walk_target_trip_seconds: float = 4.0
@export var walk_look_sensitivity: float = BASE_WALK_LOOK_SENSITIVITY
@export var walk_min_pitch: float = deg_to_rad(-80.0)
@export var walk_max_pitch: float = deg_to_rad(80.0)
## A touch that moves less than this many pixels between press and release
## counts as a tap (walk there); more than this counts as a look-drag.
@export var tap_max_drag_px: float = 16.0
@export var walk_arrive_epsilon_m: float = 0.3
## Half-width of the walker used for wall collision. Set by
## neighborhood_controller.gd along with `collider`.
@export var body_radius_m: float = 0.35

## NeighborhoodNode3D, which owns the real building footprints and resolves
## a move against them (see its resolve_move). Left null -> no collision,
## exactly the old walk-through-walls behaviour, so walk mode still works
## if the node isn't wired up.
var collider: Node = null

var mode: Mode = Mode.ORBIT
var walk_position: Vector3 = Vector3.ZERO
var walk_yaw: float = 0.0
var walk_pitch: float = 0.0

## The active route: real waypoints from NeighborhoodNode3D::find_path
## (or a single-element fallback straight to the tapped point -- see
## _try_set_walk_target), walked in order starting at _walk_path_index.
## Empty means "no walk in progress," replacing the old _has_walk_target
## bool now that a walk is a sequence of legs, not one target.
var _walk_path: PackedVector3Array = PackedVector3Array()
var _walk_path_index: int = 0
var _current_walk_speed_mps: float = 3.0

var _touch_points: Dictionary = {}     # touch index -> current Vector2 position
var _touch_start_pos: Dictionary = {}  # touch index -> Vector2 position at press
var _last_pinch_span: float = 0.0
var _mouse_dragging: bool = false

func _ready() -> void:
	_update_transform()

func _process(delta: float) -> void:
	if mode == Mode.WALK:
		_walk_step(delta)

## Switches to orbit mode without changing where it's orbiting -- callers
## that want a fresh framing (e.g. "Site Overview") call frame_bounds()
## themselves right after, same as neighborhood_controller.gd does.
func set_mode_orbit() -> void:
	mode = Mode.ORBIT
	_clear_walk_path()
	_update_transform()

## Switches to walk mode, spawning at `start_position` (ground level --
## walk_height_m is added on top) facing `facing_yaw` (same yaw convention
## as _look_direction(): 0 faces +Z, increasing yaw turns toward +X).
func set_mode_walk(start_position: Vector3, facing_yaw: float) -> void:
	mode = Mode.WALK
	walk_position = start_position
	walk_yaw = facing_yaw
	walk_pitch = 0.0
	_clear_walk_path()
	_apply_walk_transform()

## Called by neighborhood_controller.gd right after this camera is ready,
## with GameState's own real, persisted settings -- see GameState.gd's own
## doc for why the base values live here instead of being duplicated
## there. `look_scale`/`speed_scale` are plain multipliers (1.0 = the
## BASE_* defaults above), not absolute values, so a settings slider never
## has to know this file's own numbers.
func apply_settings(look_scale: float, speed_scale: float) -> void:
	orbit_sensitivity = BASE_ORBIT_SENSITIVITY * look_scale
	mouse_orbit_sensitivity = BASE_MOUSE_ORBIT_SENSITIVITY * look_scale
	walk_look_sensitivity = BASE_WALK_LOOK_SENSITIVITY * look_scale
	walk_speed_mps = BASE_WALK_SPEED_MPS * speed_scale
	walk_max_speed_mps = BASE_WALK_MAX_SPEED_MPS * speed_scale

func frame_bounds(bounds_center: Vector3, bounds_radius: float) -> void:
	target = bounds_center
	if bounds_radius <= 0.0:
		_update_transform()
		return
	var half_fov := deg_to_rad(fov) * 0.5
	distance = clamp((bounds_radius / sin(half_fov)) * 1.3, min_distance, max_distance)
	_update_transform()

func _clear_walk_path() -> void:
	_walk_path = PackedVector3Array()
	_walk_path_index = 0

func _walk_step(delta: float) -> void:
	if _walk_path_index < _walk_path.size():
		var leg_target: Vector3 = _walk_path[_walk_path_index]
		var to_target := leg_target - walk_position
		to_target.y = 0.0
		var dist := to_target.length()
		if dist < walk_arrive_epsilon_m:
			_walk_path_index += 1
		else:
			var step: float = min(_current_walk_speed_mps * delta, dist)
			var dir := to_target / dist
			var desired := walk_position + dir * step
			var resolved := desired
			if collider != null and collider.has_method("resolve_move"):
				resolved = collider.resolve_move(walk_position, desired, body_radius_m)
			# Fully blocked (slide failed too): drop the WHOLE remaining
			# route, not just this leg -- a planned route that's still
			# blocked here is stale (real geometry changed underneath it,
			# or the grid-cell snap put a leg closer to a wall than the
			# plan intended), and grinding through the rest of it leg by
			# leg would just repeat the same failure.
			if resolved.distance_to(walk_position) < step * 0.05:
				_clear_walk_path()
			else:
				walk_yaw = atan2(dir.x, dir.z)
			walk_position = resolved
	_apply_walk_transform()

## Raycasts from the camera through `screen_pos` onto the y=0 ground
## plane, and if it hits one in front of the camera, plans a real route
## there via NeighborhoodNode3D::find_path (around every real building
## footprint) and walks it leg by leg, at a speed derived from the trip's
## own straight-line distance (walk_speed_mps for a short hop, scaling up
## toward walk_max_speed_mps so a long cross-site trip doesn't take
## minutes at a fixed walking pace -- the ROUTE can be longer than that
## straight-line distance when it has to detour, but the pace is set from
## the direct distance, not the detour's own length). Targets the flat
## ground plane specifically, not whatever building geometry is under the
## tap -- so tapping a rooftop walks to the ground point beneath it.
func _try_set_walk_target(screen_pos: Vector2) -> void:
	var ray_origin := project_ray_origin(screen_pos)
	var ray_dir := project_ray_normal(screen_pos)
	if absf(ray_dir.y) < 0.0001:
		return  # looking parallel to the ground: no sane intersection
	var t := -ray_origin.y / ray_dir.y
	if t <= 0.0:
		return  # ground plane is behind the camera
	var destination: Vector3 = ray_origin + ray_dir * t

	_clear_walk_path()
	if collider != null and collider.has_method("find_path"):
		var route: PackedVector3Array = collider.find_path(walk_position, destination)
		if route.size() > 0:
			_walk_path = route
	if _walk_path.is_empty():
		# No collider wired up, or no real route exists (e.g. the
		# destination is fully sealed off) -- the same honest
		# straight-line fallback tap-to-walk always had, rather than
		# silently refusing to move at all.
		_walk_path = PackedVector3Array([destination])

	var horizontal := destination - walk_position
	horizontal.y = 0.0
	var trip_distance := horizontal.length()
	_current_walk_speed_mps = clamp(trip_distance / walk_target_trip_seconds, walk_speed_mps, walk_max_speed_mps)

## Same ground-plane raycast as `_try_set_walk_target`, but resolves the
## hit through `pick_zone_at` instead of planning a route -- see
## `picking_mode`'s own doc for why this is a separate function rather
## than a branch bolted onto the walk-target one (they diverge completely
## after finding `destination`; sharing that little math isn't worth the
## indirection).
func _try_pick_zone(screen_pos: Vector2) -> void:
	var ray_origin := project_ray_origin(screen_pos)
	var ray_dir := project_ray_normal(screen_pos)
	if absf(ray_dir.y) < 0.0001:
		return
	var t := -ray_origin.y / ray_dir.y
	if t <= 0.0:
		return
	var ground_point: Vector3 = ray_origin + ray_dir * t

	if collider == null or not collider.has_method("pick_zone_at"):
		return
	var result: Dictionary = collider.pick_zone_at(ground_point.x, ground_point.z)
	if String(result.get("kind", "none")) != "none":
		picking_mode = false
		zone_picked.emit(result["kind"], result["id"])

func _apply_walk_transform() -> void:
	position = walk_position + Vector3(0.0, walk_height_m, 0.0)
	look_at(position + _look_direction(walk_yaw, walk_pitch), Vector3.UP)

## Same convention _update_transform()'s orbit offset already uses (yaw=0
## faces +Z, positive yaw sweeps toward +X) -- shared so a `yaw` computed
## for one mode (e.g. a waypoint's facing direction, computed by
## neighborhood_controller.gd) means the same thing in the other.
func _look_direction(a_yaw: float, a_pitch: float) -> Vector3:
	return Vector3(cos(a_pitch) * sin(a_yaw), sin(a_pitch), cos(a_pitch) * cos(a_yaw))

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventScreenTouch:
		if event.pressed:
			_touch_points[event.index] = event.position
			_touch_start_pos[event.index] = event.position
		else:
			var was_sole_touch: bool = _touch_points.size() == 1 and _touch_points.has(event.index)
			if was_sole_touch and mode == Mode.WALK:
				var start: Vector2 = _touch_start_pos.get(event.index, event.position)
				if start.distance_to(event.position) <= tap_max_drag_px:
					if picking_mode:
						_try_pick_zone(event.position)
					else:
						_try_set_walk_target(event.position)
			_touch_points.erase(event.index)
			_touch_start_pos.erase(event.index)
		if _touch_points.size() != 2:
			_last_pinch_span = 0.0
	elif event is InputEventScreenDrag:
		_touch_points[event.index] = event.position
		if _touch_points.size() == 1:
			_look_or_orbit(event.relative)
		elif _touch_points.size() == 2 and mode == Mode.ORBIT:
			_handle_pinch()
	elif event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_LEFT:
			_mouse_dragging = event.pressed
		elif event.button_index == MOUSE_BUTTON_WHEEL_UP and event.pressed and mode == Mode.ORBIT:
			_zoom_by_ratio(1.0 - mouse_zoom_step)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN and event.pressed and mode == Mode.ORBIT:
			_zoom_by_ratio(1.0 + mouse_zoom_step)
	elif event is InputEventMouseMotion and _mouse_dragging:
		_look_or_orbit(event.relative * (mouse_orbit_sensitivity / orbit_sensitivity))

func _look_or_orbit(relative: Vector2) -> void:
	if mode == Mode.ORBIT:
		_orbit(relative)
	else:
		_look(relative)

func _orbit(relative: Vector2) -> void:
	yaw -= relative.x * orbit_sensitivity
	pitch = clamp(pitch + relative.y * orbit_sensitivity, min_pitch, max_pitch)
	_update_transform()

func _look(relative: Vector2) -> void:
	# A manual look always wins over a stale tap-to-move route, rather
	# than the two fighting over walk_yaw every frame.
	_clear_walk_path()
	walk_yaw -= relative.x * walk_look_sensitivity
	walk_pitch = clamp(walk_pitch - relative.y * walk_look_sensitivity, walk_min_pitch, walk_max_pitch)
	_apply_walk_transform()

func _handle_pinch() -> void:
	var indices := _touch_points.keys()
	if indices.size() < 2:
		return
	var a: Vector2 = _touch_points[indices[0]]
	var b: Vector2 = _touch_points[indices[1]]
	var current_span := a.distance_to(b)
	if _last_pinch_span > 0.0 and current_span > 0.0:
		_zoom_by_ratio(_last_pinch_span / current_span)
	_last_pinch_span = current_span

func _zoom_by_ratio(ratio: float) -> void:
	distance = clamp(distance * ratio, min_distance, max_distance)
	_update_transform()

func _update_transform() -> void:
	# Matches the original, confirmed-working formula exactly:
	# offset = distance * (cos(pitch)*sin(yaw), sin(pitch), cos(pitch)*cos(yaw)),
	# position = target + offset. _look_direction(yaw, pitch) IS that same
	# offset direction (verified by inspection, not just by construction --
	# this is the one formula in this file with an actual on-device check
	# behind it, so getting its sign right here matters more than anywhere
	# else in this rewrite).
	position = target + _look_direction(yaw, pitch) * distance
	look_at(target, Vector3.UP)
