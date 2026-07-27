extends Control

## The app's real boot scene (project.godot's run/main_scene) -- Godot 4
## default behavior previously dropped straight into the full real-site
## scene with no way back out, no settings, and no way to resume a
## Pattern Lab session after quitting. See GameState.gd's own doc for the
## save/continue contract this reads and writes.
##
## All child controls built in code, same convention pattern_lab.gd/
## minimap.gd already established -- no .tscn node tree to keep in sync
## by hand; MainMenu.tscn itself is just this script on a bare Control.

const SITE_TOUR_SCENE := "res://scenes/Main.tscn"
const PATTERN_LAB_SCENE := "res://scenes/PatternLab.tscn"

var _continue_button: Button
var _settings_overlay: Control = null

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)

	var bg := ColorRect.new()
	bg.color = Color(0.08, 0.09, 0.10, 1.0)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(bg)

	var center := CenterContainer.new()
	center.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(center)

	var box := VBoxContainer.new()
	box.custom_minimum_size = Vector2(360, 0)
	box.add_theme_constant_override("separation", 14)
	center.add_child(box)

	var title := Label.new()
	title.text = "Street Smarts"
	title.add_theme_font_size_override("font_size", 36)
	title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	box.add_child(title)

	var subtitle := Label.new()
	subtitle.text = "A Christopher Alexander pattern-language site explorer"
	subtitle.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	subtitle.modulate = Color(1, 1, 1, 0.7)
	box.add_child(subtitle)

	box.add_child(HSeparator.new())

	_continue_button = _add_button(box, "Continue", _on_continue_pressed)
	_continue_button.disabled = not GameState.has_save()

	_add_button(box, "Site Tour (new)", func(): _start_new(SITE_TOUR_SCENE))
	_add_button(box, "Pattern Lab (new)", func(): _start_new(PATTERN_LAB_SCENE))
	_add_button(box, "Settings", _on_settings_pressed)
	_add_button(box, "Quit", func(): get_tree().quit())

func _add_button(parent: VBoxContainer, text: String, on_pressed: Callable) -> Button:
	var button := Button.new()
	button.text = text
	button.custom_minimum_size = Vector2(0, 44)
	button.pressed.connect(on_pressed)
	parent.add_child(button)
	return button

func _on_continue_pressed() -> void:
	var data := GameState.read_save()
	if data.is_empty():
		return
	GameState.queue_continue(data)
	var scene_path := String(data.get("scene_path", SITE_TOUR_SCENE))
	get_tree().change_scene_to_file(scene_path)

func _start_new(scene_path: String) -> void:
	# Guards a real, if rare, failure path: if a prior Continue press's own
	# change_scene_to_file call failed (bad/missing scene), this same
	# MainMenu instance is still alive with _pending_save still set --
	# without this, pressing New Game right after would resume that stale
	# save instead of starting fresh.
	GameState.queue_continue({})
	get_tree().change_scene_to_file(scene_path)

func _on_settings_pressed() -> void:
	if _settings_overlay != null:
		return
	_settings_overlay = preload("res://scripts/settings_panel.gd").new()
	add_child(_settings_overlay)
	_settings_overlay.closed.connect(func():
		_settings_overlay.queue_free()
		_settings_overlay = null
	)
