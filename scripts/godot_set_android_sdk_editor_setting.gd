# One-time setup for headless/CI Android export.
#
# `export/android/android_sdk_path` is a global EDITOR setting, not a
# project setting -- it isn't in export_presets.cfg and can't be set via
# an environment variable read at export time (ANDROID_HOME is NOT picked
# up automatically). It lives in editor_settings-<major>.<minor>.tres under
# EditorPaths::get_config_dir() (`~/.config/godot/` on Linux), which only
# gets created by the editor itself -- so a fresh CI machine needs this run
# once before `--export-debug`/`--export-release` will work.
#
# Usage (reads ANDROID_HOME or ANDROID_SDK_ROOT from the environment):
#   ANDROID_HOME=/path/to/android-sdk godot4 --headless --editor \
#       --path godot -s ../scripts/godot_set_android_sdk_editor_setting.gd --quit-after 1
#
# Must run with --editor (not plain --headless) -- EditorScript requires an
# editor context, unlike a plain SceneTree/MainLoop script passed to -s.
@tool
extends EditorScript

func _run():
	var sdk_path = OS.get_environment("ANDROID_HOME")
	if sdk_path.is_empty():
		sdk_path = OS.get_environment("ANDROID_SDK_ROOT")
	if sdk_path.is_empty():
		push_error("Set ANDROID_HOME or ANDROID_SDK_ROOT before running this script.")
		return

	var settings = EditorInterface.get_editor_settings()
	settings.set_setting("export/android/android_sdk_path", sdk_path)
	print("export/android/android_sdk_path set to: ", settings.get_setting("export/android/android_sdk_path"))
