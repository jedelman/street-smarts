extends Control

## Reusable settings overlay -- instanced from code (`preload(...).new()`,
## no .tscn) by BOTH main_menu.gd and pause_menu.gd. The in-game case is
## why this is an overlay rather than its own scene reached via
## change_scene_to_file(): swapping the whole scene tree while inside
## Pattern Lab would tear down NeighborhoodNode3D and lose whatever
## real operators have already run and haven't been saved yet.
##
## All child controls built in code, same convention pattern_lab.gd and
## minimap.gd already established for this codebase's dynamically-built
## panels -- no .tscn to keep in sync by hand. Only two real sliders
## exist because only two real, wired settings exist (see GameState.gd's
## own doc) -- no placeholder "graphics quality" or "sound" controls for
## systems this engine doesn't have.

signal closed

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_STOP

	var bg := ColorRect.new()
	bg.color = Color(0.05, 0.05, 0.07, 0.94)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(bg)

	var center := CenterContainer.new()
	center.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(center)

	var box := VBoxContainer.new()
	box.custom_minimum_size = Vector2(420, 0)
	box.add_theme_constant_override("separation", 16)
	center.add_child(box)

	var title := Label.new()
	title.text = "Settings"
	title.add_theme_font_size_override("font_size", 28)
	box.add_child(title)

	_add_slider(box, "Look sensitivity", 0.25, 2.5, GameState.look_sensitivity, GameState.set_look_sensitivity)
	_add_slider(box, "Walk speed", 0.25, 2.5, GameState.walk_speed_scale, GameState.set_walk_speed_scale)

	var back := Button.new()
	back.text = "Back"
	back.custom_minimum_size = Vector2(0, 44)
	back.pressed.connect(func(): closed.emit())
	box.add_child(back)

func _add_slider(parent: VBoxContainer, label_text: String, min_v: float, max_v: float, current: float, on_change: Callable) -> void:
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 12)
	parent.add_child(row)

	var label := Label.new()
	label.text = label_text
	label.custom_minimum_size = Vector2(160, 0)
	row.add_child(label)

	var slider := HSlider.new()
	slider.min_value = min_v
	slider.max_value = max_v
	slider.step = 0.05
	slider.value = current
	slider.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	row.add_child(slider)

	var value_label := Label.new()
	value_label.custom_minimum_size = Vector2(50, 0)
	value_label.text = "%.2fx" % current
	slider.value_changed.connect(func(v):
		value_label.text = "%.2fx" % v
		on_change.call(v)
	)
	row.add_child(value_label)
