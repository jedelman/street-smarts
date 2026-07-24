extends Control

## Virtual movement joystick for walk mode. Touch-down INSIDE this
## control's own screen rect claims that touch index and keeps responding
## to its drag/release even once the finger moves outside the visual
## circle (clamped there instead) -- deliberately built on raw _input(),
## not Control's own _gui_input()/mouse_filter dispatch, whose exact
## touch-capture-outside-bounds behavior isn't something this environment
## can verify without a real screen. _input() fires before
## orbit_camera.gd's _unhandled_input(), and set_input_as_handled() stops
## a claimed touch from reaching it -- so a finger on the joystick never
## also drags the look direction.

@export var radius: float = 70.0
@export var dead_zone: float = 0.15

## Normalized movement vector: x = strafe (-1 left .. 1 right), y = forward
## push magnitude as (-1 back .. 1 forward) once negated by the reader --
## see orbit_camera.gd's _walk_step() for the exact sign convention this
## feeds into. Zero when untouched or within the dead zone.
var output: Vector2 = Vector2.ZERO

var _active_index: int = -1
var _nub_offset: Vector2 = Vector2.ZERO

func _ready() -> void:
	mouse_filter = Control.MOUSE_FILTER_IGNORE

func _input(event: InputEvent) -> void:
	if event is InputEventScreenTouch:
		if event.pressed:
			if _active_index == -1 and get_global_rect().has_point(event.position):
				_active_index = event.index
				_update_nub(event.position)
				get_viewport().set_input_as_handled()
		elif event.index == _active_index:
			_active_index = -1
			_nub_offset = Vector2.ZERO
			output = Vector2.ZERO
			queue_redraw()
			get_viewport().set_input_as_handled()
	elif event is InputEventScreenDrag and event.index == _active_index:
		_update_nub(event.position)
		get_viewport().set_input_as_handled()

func _update_nub(global_touch_pos: Vector2) -> void:
	var center := size * 0.5
	var local_pos := global_touch_pos - global_position
	var offset := local_pos - center
	if offset.length() > radius:
		offset = offset.normalized() * radius
	_nub_offset = offset

	var magnitude := offset.length() / radius
	if magnitude < dead_zone:
		output = Vector2.ZERO
	else:
		var eased := (magnitude - dead_zone) / (1.0 - dead_zone)
		output = offset.normalized() * eased
	queue_redraw()

func _draw() -> void:
	var center := size * 0.5
	draw_circle(center, radius, Color(1.0, 1.0, 1.0, 0.12))
	draw_arc(center, radius, 0.0, TAU, 32, Color(1.0, 1.0, 1.0, 0.35), 2.0)
	draw_circle(center + _nub_offset, radius * 0.4, Color(1.0, 1.0, 1.0, 0.55))
