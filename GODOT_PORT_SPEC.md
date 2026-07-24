# GODOT_PORT_SPEC.md — Alexandrian Spatial SDF & Godot 4 Integration

**Status:** Implementation Initialized on branch `antigravity/godot-rust-port`.  
**Architecture Target:** Unified Rust GDExtension (`gdext` crate) + Godot 4 (WebGL2 / Compatibility & Forward+ Export).

---

## 1. Overview & Architectural Philosophy

This specification unifies the `street-smarts` Christopher Alexander provocation engine with an **Alexandrian Spatial SDF Engine** running in **Godot 4 via Rust WebAssembly / GDExtension**.

Instead of static CAD CSG scripts (`render.py`) or heavy polygon modeling, the engine models architectural design as a system of **3D spatial forces and constraints via Signed Distance Fields (SDFs)**.

### Target Client Platform
* **Primary Environment:** Mobile & Desktop Web Browsers (Android / iOS / Desktop WebGL2) and Native Desktop Executables.
* **Viewport & Touch UI:** Godot 4 (WebGL2 / Compatibility Export).
* **Logic & Math Engine:** Rust compiled to WebAssembly (`wasm32-unknown-unknown` + SIMD) / GDExtension (`gdext`).
* **Geometry Pipeline:** 3D Implicit SDFs + Dual Contouring / Surface Nets + `manifold-rust` CSG.

---

## 2. Technical Stack & Geometry Pipeline

```
[ Client Platform: Android Mobile / Desktop Web Browser ]
├── Viewport & Touch UI: Godot 4 (WebGL2 / Compatibility Export)
├── Logic & Math Engine: Rust compiled to WASM (wasm32-unknown-unknown + SIMD) / GDExtension
└── Geometry Pipeline: 3D Implicit SDFs + Dual Contouring / Manifold CSG
```

### 1. 3D SDF Formulations & Alexandrian Operators
Spatial forces and apertures are evaluated natively in Rust (`street-smarts-core::sdf`):
* **Union:** $\min(A, B)$
* **Intersection:** $\max(A, B)$
* **Difference (Aperture Cuts P221):** $\max(A, -B)$
* **Smooth Union ($\text{smin}$):** Organic architectural transitions (arches, fillets, wall-to-ceiling transitions).

### 2. Texturing & Material Pipeline (Triplanar Mapping)
To avoid costly CPU/WASM 2D UV unwrapping on dynamic SDF cuts:
* Apply Godot’s `StandardMaterial3D` with **Triplanar Mapping** enabled (`uv1_triplanar = true`).
* Materials (brick, plaster, wood, concrete) automatically align seamlessly across dynamic procedural cuts without UV degradation.

---

## 3. Site Scale Strategy (91-Acre / Multi-Scale)

To evaluate thousands of apertures (`P221`) across a 91-acre site (e.g. Eastside Commons in Norfolk) on mobile ARM chips without memory or thermal starvation:

1. **Hierarchical Spatial Indexing (BVH / AABB):**
   * Wrap pattern evaluators in Bounding Volume Hierarchies (BVH) and `AABB3D` bounds.
   * Instantly prune distant aperture/window SDF computations when query points fall outside a local structure's bounding box.
2. **Static Neighborhood Context vs. Active Parcel Isolation:**
   * **Neighborhood Hot-Loading:** Surrounding context buildings and GIS/CAD terrain load as static, low-poly Godot `ArrayMesh` buffers **once**.
   * **Active Parcel Isolation:** Dynamic SDF math, Alexander pattern evaluation, and Dual Contouring surface extraction occur **only within the bounding box of the active parcel being remodeled**.

---

## 4. Directory & Workspace Structure

```
street-smarts/
├── Cargo.toml                      (Workspace root: includes crates/street-smarts-godot)
├── crates/
│   ├── street-smarts-core/         (NIR schema, geometry primitives, 3D SDF primitives & BVH)
│   ├── street-smarts-opinions/     (Levels of Scale, Strong Centers, Ownership Pattern)
│   ├── street-smarts-conflict/     (Disagreement detection & human prompts)
│   ├── street-smarts-patterns/     (Procedural pattern operators: P95, P127, P221)
│   └── street-smarts-godot/        [NEW] Native GDExtension Crate
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs              (GDExtension entrypoint & Godot nodes)
├── godot/                          [NEW] Godot 4 Engine Project
│   ├── project.godot               (Godot WebGL2/Compatibility & Forward+ project settings)
│   ├── street_smarts.gdextension   (GDExtension platform library mapping)
│   ├── bin/                        (Staged compiled .dll / .so / .dylib binaries)
│   └── scenes/
│       └── Main.tscn               (3D Viewport scene & Opinion UI Panel)
└── scripts/
    ├── build-godot.ps1             (Windows build automation)
    └── build-godot.sh               (Linux / macOS build automation)
```

---

## 5. Node API Reference

### `NeighborhoodNode3D` (Subclass of `Node3D`)
- `load_nir_json(json_str: String) -> bool`: Parses an NIR JSON representation into Rust memory.
- `get_building_count() -> i32`: Returns the active building footprint count.
- `evaluate_opinions() -> Dictionary`: Evaluates the full opinion chorus and returns summary metrics (`geometric_headline`, `activist_headline`, `geometric_mean`, `question_count`).
- `rebuild_3d_mesh() -> bool`: Triggers procedural 3D mesh reconstruction for building massings, streets, and Salingaros centers.

### `OpinionEvaluatorNode` (Subclass of `Node`)
- `evaluate_nir_json(json_str: String) -> Dictionary`: Standalone evaluation function that takes raw NIR JSON and returns a full structured `DisagreementReport` with human prompts.

---

## 6. Development & Performance Guidelines

* **Language Mapping:**
  * Implement spatial math, BVH trees, 3D SDF primitives, and Dual Contouring mesh extraction in **Rust**.
  * Implement UI interaction, gesture handling, and scene composition in **GDScript** or **`godot-rust` GDExtension**.
* **Performance Constraints:**
  * Debounce/throttle UI touch interactions (run mesh recalculation cycles at $30\text{--}60\text{ms}$ intervals).
  * Do **not** generate WebGL2 compute shaders (`RenderingDevice`) due to mobile web driver limits; execute spatial grids via WASM on the mobile CPU using SIMD.

---

## 7. Phase Roadmap

- [x] **Phase 1: Workspace & GDExtension Skeleton**: Crate setup, `.gdextension` manifest, `project.godot`, base nodes, and `street-smarts-core::sdf` 3D primitives.
- [ ] **Phase 2: Dual Contouring & Procedural Mesh Extrusion**: Dynamic SDF surface nets for active parcels, triplanar material integration, and static context hot-loading.
- [ ] **Phase 3: Real-Time Openings & Intimacy Shaders**: Procedural P221 door/window CSG punches and floorplan intimacy gradient visualizers.
- [ ] **Phase 4: Interactive Mobile Viewport**: Touch/orbit gesture controls for live pattern steering and real-time disagreement prompts.
