extends Control

## Interactive pattern-application panel -- the alternative to
## neighborhood_controller.gd's one-shot run_pattern_pipeline() call.
## Lists every real operator from NeighborhoodNode3D::get_available_patterns
## (the SAME registry the web client's own pipeline-stepper UI already
## drives -- see that method's own doc), lets a person tune its real
## parameters with real min/max bounds, and applies exactly one step at a
## time via NeighborhoodNode3D::apply_pattern. Each Apply is a real,
## inspectable Subdivision with its own headline/step narration, not a
## black-box whole-pipeline run.
##
## Real, honest limitation surfaced here, not hidden: `target` defaults to
## "*" (whole site) because that's the ONLY scope the large majority of
## real operators accept today (confirmed against every real call site in
## pipeline.rs) -- P95 Building Complex (per real block) and P37 House
## Cluster (the site's own top-level parcel) are the two real exceptions
## that take a specific id. Typing an id `apply_pattern` doesn't accept
## surfaces that operator's own real error, not a silent no-op.
##
## All child controls are built here in code rather than in the .tscn --
## same choice minimap.gd made for its own custom drawing, extended here
## to ordinary Control nodes instead: one plain Control in the scene with
## this script attached, no hand-authored nested node tree to keep in
## sync by hand.

signal pattern_applied

## Emitted when the "Pick on Map/World" button is toggled -- true entering
## picking mode, false leaving it (including a self-cancel via the same
## button). neighborhood_controller.gd owns forwarding this to both
## minimap.gd's and orbit_camera.gd's own `start_picking`/`cancel_picking`,
## since PatternLab has no reference to either -- it only knows about the
## real operator/apply_pattern side of NeighborhoodNode3D, not the
## camera/minimap UI wiring, and there's no reason to give it one just for
## this.
signal pick_toggled(active: bool)

@export var neighborhood_node: Node = null

const PANEL_HEIGHT := 340.0

var _patterns: Array = []
var _selected_index: int = -1
var _param_sliders: Dictionary = {}  # param name (String) -> HSlider
var _picking: bool = false

var _pattern_list: ItemList
var _description_label: RichTextLabel
var _params_box: VBoxContainer
var _target_field: LineEdit
var _pick_button: Button
var _seed_field: SpinBox
var _apply_button: Button
var _status_label: RichTextLabel

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_WIDE)
	offset_top = -PANEL_HEIGHT
	mouse_filter = Control.MOUSE_FILTER_STOP

	var bg := ColorRect.new()
	bg.color = Color(0.07, 0.07, 0.09, 0.92)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(bg)

	var margin := MarginContainer.new()
	margin.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	margin.add_theme_constant_override("margin_left", 12)
	margin.add_theme_constant_override("margin_right", 12)
	margin.add_theme_constant_override("margin_top", 8)
	margin.add_theme_constant_override("margin_bottom", 8)
	add_child(margin)

	var root_h := HBoxContainer.new()
	root_h.add_theme_constant_override("separation", 12)
	margin.add_child(root_h)

	# Left column: pattern list.
	_pattern_list = ItemList.new()
	_pattern_list.custom_minimum_size = Vector2(220, 0)
	_pattern_list.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_pattern_list.item_selected.connect(_on_pattern_selected)
	root_h.add_child(_pattern_list)

	# Right column: description, params, target/seed, apply, status.
	var right_v := VBoxContainer.new()
	right_v.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	right_v.add_theme_constant_override("separation", 6)
	root_h.add_child(right_v)

	_description_label = RichTextLabel.new()
	_description_label.custom_minimum_size = Vector2(0, 48)
	_description_label.bbcode_enabled = true
	_description_label.fit_content = true
	right_v.add_child(_description_label)

	var params_scroll := ScrollContainer.new()
	params_scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	right_v.add_child(params_scroll)
	_params_box = VBoxContainer.new()
	_params_box.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	params_scroll.add_child(_params_box)

	var target_row := HBoxContainer.new()
	target_row.add_theme_constant_override("separation", 8)
	right_v.add_child(target_row)

	var target_label := Label.new()
	target_label.text = "target:"
	target_row.add_child(target_label)

	_target_field = LineEdit.new()
	_target_field.text = "*"
	_target_field.custom_minimum_size = Vector2(140, 0)
	_target_field.tooltip_text = "\"*\" for whole-site (most patterns). A real BLOCK_n parcel id for P95 Building Complex."
	target_row.add_child(_target_field)

	_pick_button = Button.new()
	_pick_button.text = "Pick on Map/World"
	_pick_button.tooltip_text = "Tap a building or block on the minimap, or walk up and tap it in 3D, to fill target."
	_pick_button.toggle_mode = true
	_pick_button.toggled.connect(_on_pick_button_toggled)
	target_row.add_child(_pick_button)

	var seed_label := Label.new()
	seed_label.text = "seed:"
	target_row.add_child(seed_label)

	_seed_field = SpinBox.new()
	_seed_field.min_value = 0
	_seed_field.max_value = 999999
	_seed_field.value = 42
	target_row.add_child(_seed_field)

	_apply_button = Button.new()
	_apply_button.text = "Apply Pattern"
	_apply_button.pressed.connect(_on_apply_pressed)
	target_row.add_child(_apply_button)

	_status_label = RichTextLabel.new()
	_status_label.bbcode_enabled = true
	_status_label.custom_minimum_size = Vector2(0, 90)
	_status_label.size_flags_vertical = Control.SIZE_EXPAND_FILL
	right_v.add_child(_status_label)

	refresh_patterns()

## Re-fetches the real operator list from Rust -- call once at startup
## (neighborhood_controller.gd does, after the raw baseline loads).
func refresh_patterns() -> void:
	if neighborhood_node == null or not neighborhood_node.has_method("get_available_patterns"):
		return
	_patterns = neighborhood_node.get_available_patterns()
	_pattern_list.clear()
	for p in _patterns:
		_pattern_list.add_item(p["name"])
	if _patterns.size() > 0:
		_pattern_list.select(0)
		_on_pattern_selected(0)

func _on_pattern_selected(index: int) -> void:
	_selected_index = index
	if index < 0 or index >= _patterns.size():
		return
	var p: Dictionary = _patterns[index]
	_description_label.text = "%s\n[i][%s][/i]" % [p["description"], p["source_display"]]
	_rebuild_param_sliders(p["params"])

func _rebuild_param_sliders(params: Array) -> void:
	for child in _params_box.get_children():
		child.queue_free()
	_param_sliders.clear()
	for param in params:
		var row := HBoxContainer.new()
		row.add_theme_constant_override("separation", 8)

		var unit: String = param.get("unit", "")
		var label := Label.new()
		label.text = "%s%s" % [param["name"], (" (%s)" % unit) if unit != "" else ""]
		label.custom_minimum_size = Vector2(170, 0)
		label.tooltip_text = param.get("description", "")
		row.add_child(label)

		var is_integer: bool = param.get("integer", false)
		var slider := HSlider.new()
		slider.min_value = param["min"]
		slider.max_value = param["max"]
		slider.step = 1.0 if is_integer else max((param["max"] - param["min"]) / 200.0, 0.001)
		slider.value = param["default"]
		slider.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		row.add_child(slider)

		var value_label := Label.new()
		value_label.custom_minimum_size = Vector2(56, 0)
		value_label.text = _format_param_value(param["default"], is_integer)
		slider.value_changed.connect(func(v): value_label.text = _format_param_value(v, is_integer))
		row.add_child(value_label)

		_params_box.add_child(row)
		_param_sliders[String(param["name"])] = slider

func _format_param_value(v: float, is_integer: bool) -> String:
	return str(int(round(v))) if is_integer else "%.2f" % v

func _on_pick_button_toggled(active: bool) -> void:
	_picking = active
	_pick_button.text = "Cancel Pick" if active else "Pick on Map/World"
	pick_toggled.emit(active)

## Called by neighborhood_controller.gd when either the minimap or the
## 3D walkabout view resolves a real pick -- fills `target` with the real
## id and drops the button back out of its toggled "picking" state (the
## picking session that just produced this id is over, on both the
## minimap/camera side, which neighborhood_controller.gd cancels itself,
## and this button's own visual state).
func set_target(id: String) -> void:
	_target_field.text = id
	_picking = false
	# set_pressed_no_signal, not the `button_pressed` property directly --
	# that setter re-emits `toggled`, which would loop back into
	# _on_pick_button_toggled(false) and re-emit pick_toggled(false) a
	# second time (harmless since cancel_picking is idempotent, but not
	# worth the redundant round trip).
	_pick_button.set_pressed_no_signal(false)
	_pick_button.text = "Pick on Map/World"

func _on_apply_pressed() -> void:
	if _selected_index < 0 or _selected_index >= _patterns.size() or neighborhood_node == null:
		return
	var p: Dictionary = _patterns[_selected_index]

	var params := {}
	for param_name in _param_sliders:
		params[param_name] = _param_sliders[param_name].value
	var params_json := JSON.stringify(params)

	var target := _target_field.text.strip_edges()
	if target == "":
		target = "*"
	var seed_value := int(_seed_field.value)

	var result: Dictionary = neighborhood_node.apply_pattern(p["name"], target, params_json, seed_value)
	if result.get("success", false):
		var msg := "[color=lightgreen]%s[/color]\n" % String(result.get("headline", "Applied.")).xml_escape()
		msg += "parcels+%d buildings+%d open_space+%d streets+%d\n" % [
			result.get("new_parcels", 0), result.get("new_buildings", 0),
			result.get("new_open_space", 0), result.get("new_streets", 0)
		]
		for step in result.get("steps", []):
			msg += "  - %s\n" % String(step).xml_escape()
		_status_label.text = msg
		pattern_applied.emit()
	else:
		_status_label.text = "[color=salmon]%s[/color]" % String(result.get("error", "Unknown error")).xml_escape()
