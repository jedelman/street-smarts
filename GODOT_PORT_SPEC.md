# GODOT_PORT_SPEC.md — Rust + Godot 4 Spatial Engine Integration

**Status:** Implementation Initialized on branch `antigravity/godot-rust-port`.  
**Architecture Target:** Unified Rust GDExtension (`gdext` crate) + Godot 4.3 Forward+/Mobile rendering engine.

---

## 1. Overview & Architectural Goals

The current rendering pipeline (`tools/vibe-render/render.py`) relies on an out-of-band Python sidecar utilizing CadQuery / OpenCASCADE for B-Rep 3D extrusion, Matplotlib for 2D line-art floorplans, and offline glTF (`.glb`) exports.

This port transitions `street-smarts` into a **unified, interactive 60FPS spatial engine**:

1. **Pure Rust Core Preservation**: `street-smarts-core`, `street-smarts-opinions`, `street-smarts-conflict`, and `street-smarts-patterns` remain 100% engine-agnostic crates without engine or GUI dependencies.
2. **Native GDExtension Bindings (`street-smarts-godot`)**: A dedicated GDExtension crate exposes native Godot nodes (`NeighborhoodNode3D`, `OpinionEvaluatorNode`) directly into Godot 4.
3. **Procedural Building Mesh & Opening Punching**: Replaces Python OpenCASCADE script runs with direct procedural 3D mesh building in Rust using Godot's `SurfaceTool` and `CSGCombiner3D`.
4. **Interactive Opinion Chorus & Disagreement Overlays**: Enables real-time evaluation of Alexander's 15 properties, Salingaros geometric ratios ($2\text{--}5\times$), and ownership patterns as users edit or steer neighborhood form.
5. **Zero-Cloud Client Execution**: Godot 4 GDExtension builds run natively on Desktop (Windows, macOS, Linux) and target HTML5/WebAssembly for zero-cloud browser execution.

---

## 2. Directory & Workspace Structure

```
street-smarts/
├── Cargo.toml                      (Workspace root: includes crates/street-smarts-godot)
├── crates/
│   ├── street-smarts-core/         (NIR schema, geometry primitives, opinion protocols)
│   ├── street-smarts-opinions/     (Levels of Scale, Strong Centers, Ownership Pattern)
│   ├── street-smarts-conflict/     (Disagreement detection & human prompts)
│   ├── street-smarts-patterns/     (Procedural pattern operators: P95, P127, P221)
│   └── street-smarts-godot/        [NEW] Native GDExtension Crate
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs              (GDExtension entrypoint & Godot nodes)
├── godot/                          [NEW] Godot 4 Engine Project
│   ├── project.godot               (Godot project settings)
│   ├── street_smarts.gdextension   (GDExtension platform library mapping)
│   ├── bin/                        (Staged compiled .dll / .so / .dylib binaries)
│   └── scenes/
│       └── Main.tscn               (3D Viewport scene & Opinion UI Panel)
└── scripts/
    ├── build-godot.ps1             (Windows build automation)
    └── build-godot.sh               (Linux / macOS build automation)
```

---

## 3. Node API Reference

### `NeighborhoodNode3D` (Subclass of `Node3D`)
- `load_nir_json(json_str: String) -> bool`: Parses an NIR JSON representation into Rust memory.
- `get_building_count() -> i32`: Returns the active building footprint count.
- `evaluate_opinions() -> Dictionary`: Evaluates the full opinion chorus and returns summary metrics (`geometric_headline`, `activist_headline`, `geometric_mean`, `question_count`).
- `rebuild_3d_mesh() -> bool`: Triggers procedural 3D mesh reconstruction for building massings, streets, and Salingaros centers.

### `OpinionEvaluatorNode` (Subclass of `Node`)
- `evaluate_nir_json(json_str: String) -> Dictionary`: Standalone evaluation function that takes raw NIR JSON and returns a full structured `DisagreementReport` with human prompts.

---

## 4. Building & Running

### 1. Build the GDExtension Library
```bash
# Windows (PowerShell)
.\scripts\build-godot.ps1 -Configuration release

# Linux / macOS
./scripts/build-godot.sh release
```

### 2. Launch Godot Project
Open the `godot/` directory inside Godot 4.3+ or run:
```bash
godot --path godot/
```

---

## 5. Phase Roadmap

- [x] **Phase 1: Workspace & GDExtension Skeleton**: Crate setup, `.gdextension` manifest, `project.godot`, and base nodes.
- [ ] **Phase 2: Procedural SurfaceTool Extrusion**: Building footprint extrusion, street mesh generation, and wall thickness.
- [ ] **Phase 3: Real-Time Openings & Intimacy Shaders**: Procedural P221 door/window CSG punches and floorplan intimacy gradient visualizers.
- [ ] **Phase 4: Interactive Provocation Viewport**: User interaction controls for live pattern steering and real-time disagreement prompts.
