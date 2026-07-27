extends Control

## In-game pause overlay -- the one way back to the main menu (or to
## Settings, or to a real save) once you've actually started a game.
## Instanced from code by neighborhood_controller.gd (no .tscn), same
## convention as settings_panel.gd, which this reuses verbatim for its own
## "Settings" button rather than duplicating slider code.
##
## A small always-visible toggle button, upper-left (the minimap owns
## upper-right) opens/closes a centered overlay. The Android back
## button/gesture (NOTIFICATION_WM_GO_BACK_REQUEST) does the same thing,
## so a player never has to hunt for the toggle to get unstuck from Pattern
## Lab or a walk-mode view.
##
## Deliberately doesn't know HOW to save or quit -- that's
## neighborhood_controller.gd's job (it owns the real neighborhood_node/
## camera this would need to read). This only emits `save_requested`/
## `quit_to_menu_requested` and shows the controller's own confirmation via
## `show_saved_confirmation()`.

signal save_requested
signal quit_to_menu_requested

var _overlay: Control = null
var _menu_center: CenterContainer = null
var _settings_overlay: Control = null
var _status_label: Label = null

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_TOP_LEFT)
	custom_minimum_size = Vector2(56, 56)
	mouse_filter = Control.MOUSE_FILTER_IGNORE

	var toggle_button := Button.new()
	toggle_button.text = "≡"  # "≡" -- a plain hamburger glyph, no icon asset needed
	toggle_button.custom_minimum_size = Vector2(44, 44)
	toggle_button.position = Vector2(12, 12)
	toggle_button.mouse_filter = Control.MOUSE_FILTER_STOP
	toggle_button.pressed.connect(toggle)
	add_child(toggle_button)

func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_GO_BACK_REQUEST:
		toggle()

func toggle() -> void:
	if _overlay != null:
		_close_overlay()
	else:
		_open_overlay()

func is_open() -> bool:
	return _overlay != null

## Called by neighborhood_controller.gd right after a real save write
## succeeds -- kept separate from save_requested's own emit so the
## controller (which knows whether the write actually worked) decides
## when the confirmation text appears, not this overlay guessing.
func show_saved_confirmation() -> void:
	if _status_label != null:
		_status_label.text = "Saved."

func _open_overlay() -> void:
	_overlay = Control.new()
	_overlay.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_overlay.mouse_filter = Control.MOUSE_FILTER_STOP
	add_child(_overlay)

	var bg := ColorRect.new()
	bg.color = Color(0.05, 0.05, 0.07, 0.85)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_overlay.add_child(bg)

	_menu_center = CenterContainer.new()
	_menu_center.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_overlay.add_child(_menu_center)

	var box := VBoxContainer.new()
	box.custom_minimum_size = Vector2(320, 0)
	box.add_theme_constant_override("separation", 12)
	_menu_center.add_child(box)

	var title := Label.new()
	title.text = "Paused"
	title.add_theme_font_size_override("font_size", 26)
	title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	box.add_child(title)

	_add_button(box, "Resume", toggle)
	_add_button(box, "Save Game", func(): save_requested.emit())
	_add_button(box, "Settings", _on_settings_pressed)
	_add_button(box, "Quit to Main Menu", func(): quit_to_menu_requested.emit())

	_status_label = Label.new()
	_status_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_status_label.modulate = Color(0.6, 1.0, 0.6)
	box.add_child(_status_label)

func _close_overlay() -> void:
	if _overlay != null:
		_overlay.queue_free()
		_overlay = null
		_menu_center = null
		_settings_overlay = null
		_status_label = null

func _add_button(parent: VBoxContainer, text: String, on_pressed: Callable) -> void:
	var button := Button.new()
	button.text = text
	button.custom_minimum_size = Vector2(0, 44)
	button.pressed.connect(on_pressed)
	parent.add_child(button)

func _on_settings_pressed() -> void:
	if _settings_overlay != null or _overlay == null:
		return
	# Hide the pause buttons rather than stacking Settings on top of them
	# uncovered -- both are full-rect CenterContainers under the same
	# `_overlay`, so without this they render on top of each other.
	if _menu_center != null:
		_menu_center.visible = false
	_settings_overlay = preload("res://scripts/settings_panel.gd").new()
	_overlay.add_child(_settings_overlay)
	_settings_overlay.closed.connect(func():
		_settings_overlay.queue_free()
		_settings_overlay = null
		if _menu_center != null:
			_menu_center.visible = true
	)
