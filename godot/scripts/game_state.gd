extends Node

## App-wide state that outlives any one scene -- the thing an autoload
## singleton exists for. Two real, separate jobs:
##
## 1. Settings: the handful of real per-player preferences this engine
##    actually has today (look sensitivity, walk speed) -- not a
##    fabricated "graphics quality" tier with nothing behind it. Persisted
##    to `user://settings.cfg`, loaded once at startup, and applied to
##    orbit_camera.gd's own exported fields by whichever game scene boots
##    (see `apply_settings_to_camera`) -- the camera owns the actual base
##    values (`orbit_camera.gd`'s `BASE_*` consts), this just holds the
##    scale factor a person chose.
##
## 2. Save/continue: MainMenu's "Continue" button needs real state to
##    resume into -- the currently-loaded neighborhood (which, in Pattern
##    Lab, may already differ from the raw baseline after real operators
##    have run) plus where the camera/player was. `neighborhood_
##    controller.gd` is the only thing that knows how to build or restore
##    that state for its own scene; this autoload is deliberately dumb
##    file I/O plus the one piece of cross-scene handoff
##    (`queue_continue`/`consume_pending_save`) a scene-change can't carry
##    on its own.

const SAVE_PATH := "user://savegame.json"
const SETTINGS_PATH := "user://settings.cfg"

var look_sensitivity: float = 1.0
var walk_speed_scale: float = 1.0

## Set by MainMenu's "Continue" button right before change_scene_to_file,
## since Godot has no other built-in way to hand data to the next scene.
## The destination scene's own controller calls consume_pending_save() in
## its _ready() and restores from it instead of loading the raw baseline.
var _pending_save: Dictionary = {}

func _ready() -> void:
	_load_settings()

func has_save() -> bool:
	return FileAccess.file_exists(SAVE_PATH)

## `data` is whatever the calling controller's own get_save_data() built --
## this function doesn't know or care about its shape beyond adding the
## real wall-clock timestamp.
func write_save(data: Dictionary) -> void:
	var to_write := data.duplicate()
	to_write["saved_at"] = Time.get_datetime_string_from_system()
	var file := FileAccess.open(SAVE_PATH, FileAccess.WRITE)
	if file == null:
		push_warning("GameState: could not open %s for writing" % SAVE_PATH)
		return
	file.store_string(JSON.stringify(to_write))

func read_save() -> Dictionary:
	if not FileAccess.file_exists(SAVE_PATH):
		return {}
	var file := FileAccess.open(SAVE_PATH, FileAccess.READ)
	if file == null:
		return {}
	var parsed = JSON.parse_string(file.get_as_text())
	return parsed if parsed is Dictionary else {}

func clear_save() -> void:
	if FileAccess.file_exists(SAVE_PATH):
		DirAccess.remove_absolute(SAVE_PATH)

func queue_continue(data: Dictionary) -> void:
	_pending_save = data

## Consumes (clears) the pending save -- called at most once per scene
## load, by the scene's own controller. Returns {} for an ordinary "New
## Game" boot with nothing queued.
func consume_pending_save() -> Dictionary:
	var data := _pending_save
	_pending_save = {}
	return data

## Pushes the current settings onto a real orbit_camera.gd instance --
## duck-typed the same way neighborhood_controller.gd already treats
## `camera` elsewhere, since GameState (autoload, loads before any game
## scene exists) can't statically reference a scene-local class.
func apply_settings_to_camera(camera: Node) -> void:
	if camera != null and camera.has_method("apply_settings"):
		camera.apply_settings(look_sensitivity, walk_speed_scale)

func set_look_sensitivity(value: float) -> void:
	look_sensitivity = value
	_save_settings()

func set_walk_speed_scale(value: float) -> void:
	walk_speed_scale = value
	_save_settings()

func _save_settings() -> void:
	var cfg := ConfigFile.new()
	cfg.set_value("settings", "look_sensitivity", look_sensitivity)
	cfg.set_value("settings", "walk_speed_scale", walk_speed_scale)
	cfg.save(SETTINGS_PATH)

func _load_settings() -> void:
	var cfg := ConfigFile.new()
	if cfg.load(SETTINGS_PATH) == OK:
		look_sensitivity = cfg.get_value("settings", "look_sensitivity", 1.0)
		walk_speed_scale = cfg.get_value("settings", "walk_speed_scale", 1.0)
