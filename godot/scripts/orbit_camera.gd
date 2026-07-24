extends Camera3D

## Two camera modes sharing one Camera3D node:
## - ORBIT (default): one-finger drag orbits around `target`, two-finger
##   pinch zooms. Mouse-drag + scroll wheel mirror it for desktop testing.
## - WALK: fixed eye height above the ground plane (walk_height_m), a
##   virtual joystick (touch_joystick.gd) drives movement, one-finger drag
##   looks around instead of orbiting. No collision -- see this file's own
##   repo history / commit message for why that's a real, named gap, not
##   an oversight.
##
## Both modes build their look direction from the SAME (pitch, yaw) ->
## direction formula (see _look_direction()), which orbit's own
## _update_transform() already uses and which is confirmed correct against
## a real device (buildings visibly, correctly framed on screen) --
## walk mode's _apply_walk_transform() reuses it via look_at() rather than
## setting Node3D.rotation directly, deliberately avoiding a second,
## separately-signed convention this environment has no display to check.

enum Mode { ORBIT, WALK }

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
@export var orbit_sensitivity: float = 0.006
@export var mouse_orbit_sensitivity: float = 0.008
@export var mouse_zoom_step: float = 0.1

@export var walk_height_m: float = 1.7
@export var walk_speed_mps: float = 3.0
@export var walk_look_sensitivity: float = 0.006
@export var walk_min_pitch: float = deg_to_rad(-80.0)
@export var walk_max_pitch: float = deg_to_rad(80.0)
@export var joystick_path: NodePath

var mode: Mode = Mode.ORBIT
var walk_position: Vector3 = Vector3.ZERO
var walk_yaw: float = 0.0
var walk_pitch: float = 0.0

var _touch_points: Dictionary = {}  # touch index -> Vector2 position
var _last_pinch_span: float = 0.0
var _mouse_dragging: bool = false
var _joystick: Control = null

func _ready() -> void:
	if joystick_path != NodePath(""):
		_joystick = get_node_or_null(joystick_path)
	_update_transform()

func _process(delta: float) -> void:
	if mode == Mode.WALK:
		_walk_step(delta)

## Switches to orbit mode without changing where it's orbiting -- callers
## that want a fresh framing (e.g. "Site Overview") call frame_bounds()
## themselves right after, same as neighborhood_controller.gd does.
func set_mode_orbit() -> void:
	mode = Mode.ORBIT
	_update_transform()

## Switches to walk mode, spawning at `start_position` (ground level --
## walk_height_m is added on top) facing `facing_yaw` (same yaw convention
## as _look_direction(): 0 faces +Z, increasing yaw turns toward +X).
func set_mode_walk(start_position: Vector3, facing_yaw: float) -> void:
	mode = Mode.WALK
	walk_position = start_position
	walk_yaw = facing_yaw
	walk_pitch = 0.0
	_apply_walk_transform()

func frame_bounds(bounds_center: Vector3, bounds_radius: float) -> void:
	target = bounds_center
	if bounds_radius <= 0.0:
		_update_transform()
		return
	var half_fov := deg_to_rad(fov) * 0.5
	distance = clamp((bounds_radius / sin(half_fov)) * 1.3, min_distance, max_distance)
	_update_transform()

func _walk_step(delta: float) -> void:
	if _joystick == null:
		return
	var input: Vector2 = _joystick.output
	if input.length() > 0.0:
		var forward := _look_direction(walk_yaw, 0.0)
		var right := Vector3(forward.z, 0.0, -forward.x)
		# Joystick y is screen-space (drag UP = negative y in Godot's
		# 2D coordinate convention) -- negate so pushing the nub up means
		# "walk forward", not backward.
		var move := (forward * -input.y + right * input.x) * walk_speed_mps * delta
		walk_position += move
	_apply_walk_transform()

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
		else:
			_touch_points.erase(event.index)
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
