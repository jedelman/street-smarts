extends Node

@onready var neighborhood_node: Node3D = $"../NeighborhoodNode3D"
@onready var geometric_label: Label = $"../UI/Panel/VBox/GeometricChorusLabel"
@onready var activist_label: Label = $"../UI/Panel/VBox/ActivistChorusLabel"
@onready var prompt_label: Label = $"../UI/Panel/VBox/HumanPromptLabel"

func _ready():
    # Prefer the synthetic demo fixture: it's the only one with populated
    # Building.height_m/openings today (the real eastside-*.json fixtures
    # are parcel-only -- no generator has run to populate buildings yet),
    # so it's the only one rebuild_3d_mesh() can actually put geometry on
    # screen for. Falls back to the real site fixture (opinion chorus text,
    # no visible massing) if the demo file isn't staged.
    var path = "res://demo-massing.json"
    if not FileAccess.file_exists(path):
        path = "res://eastside-baseline.json"
    if not FileAccess.file_exists(path):
        path = "res://data/eastside-baseline.json"
    
    var file = FileAccess.open(path, FileAccess.READ)
    if file:
        var json_str = file.get_as_text()
        if neighborhood_node.has_method("load_nir_json") and neighborhood_node.load_nir_json(json_str):
            print("[StreetSmarts] Loaded NIR baseline fixture into Godot 4 spatial engine!")
            neighborhood_node.rebuild_3d_mesh()
            
            var metrics = neighborhood_node.evaluate_opinions()
            if metrics.has("geometric_headline"):
                geometric_label.text = "Geometric Chorus: " + str(metrics["geometric_headline"])
            if metrics.has("activist_headline"):
                activist_label.text = "Activist Chorus: " + str(metrics["activist_headline"])
            if metrics.has("question_count"):
                prompt_label.text = "Human Disagreement Prompts (" + str(metrics["question_count"]) + " surfaced)"
    else:
        print("[StreetSmarts] NIR fixture file ready to be bound at runtime.")
