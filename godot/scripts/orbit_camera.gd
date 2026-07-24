extends Camera3D

## Touch-first orbit camera. One-finger drag orbits around `target`;
## two-finger pinch zooms. Mouse-drag + scroll wheel mirror the same
## behavior for desktop testing, routed through the same yaw/pitch/distance
## state as the touch path, so there's one source of truth for the camera
## transform regardless of input device.

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

var _touch_points: Dictionary = {}  # touch index -> Vector2 position
var _last_pinch_span: float = 0.0
var _mouse_dragging: bool = false

func _ready() -> void:
	_update_transform()

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
			_orbit(event.relative)
		elif _touch_points.size() == 2:
			_handle_pinch()
	elif event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_LEFT:
			_mouse_dragging = event.pressed
		elif event.button_index == MOUSE_BUTTON_WHEEL_UP and event.pressed:
			_zoom_by_ratio(1.0 - mouse_zoom_step)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN and event.pressed:
			_zoom_by_ratio(1.0 + mouse_zoom_step)
	elif event is InputEventMouseMotion and _mouse_dragging:
		_orbit(event.relative * (mouse_orbit_sensitivity / orbit_sensitivity))

func _orbit(relative: Vector2) -> void:
	yaw -= relative.x * orbit_sensitivity
	pitch = clamp(pitch + relative.y * orbit_sensitivity, min_pitch, max_pitch)
	_update_transform()

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
	var offset := Vector3(
		distance * cos(pitch) * sin(yaw),
		distance * sin(pitch),
		distance * cos(pitch) * cos(yaw)
	)
	position = target + offset
	look_at(target, Vector3.UP)

## Recenters the orbit around `bounds_center`, at a distance that fits a
## `bounds_radius`-sized sphere inside the vertical FOV (with a margin).
## `target`/`distance`'s own @export defaults were tuned for the small
## synthetic demo fixture clustered near local (0,0,0) -- the real
## pattern-pipeline output projects buildings from the full site bbox's
## own center, so a real building cluster can sit hundreds of meters away
## from that origin and/or span a much larger area. Called by
## neighborhood_controller.gd right after rebuild_3d_mesh(), from the
## actual generated massing's own AABB, so the camera frames whatever was
## really built instead of trusting a fixed guess.
func frame_bounds(bounds_center: Vector3, bounds_radius: float) -> void:
	target = bounds_center
	if bounds_radius <= 0.0:
		_update_transform()
		return
	var half_fov := deg_to_rad(fov) * 0.5
	distance = clamp((bounds_radius / sin(half_fov)) * 1.3, min_distance, max_distance)
	_update_transform()
