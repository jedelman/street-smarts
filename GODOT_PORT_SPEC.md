# GODOT_PORT_SPEC.md — Alexandrian Spatial SDF & Godot 4 Integration

**Status:** Phase 2 (building massing extraction) implemented and tested; see §8 for exactly what is and isn't verified.  
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
│           ├── lib.rs              (GDExtension entrypoint & Godot nodes)
│           └── building_mesh.rs    (Building -> SDF -> Mesh: footprint extrusion, P221 punches)
│   street-smarts-core/src/surface_nets.rs  (generic SDF -> triangle mesh extractor, used by building_mesh)
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
- `rebuild_3d_mesh() -> bool`: For every building with a `height_m`, builds a real constructive-SDF solid (footprint extrusion + P221 opening punches, `building_mesh::BuildingSolid`), extracts it via Surface Nets, and attaches it as a `MeshInstance3D` child (`GeneratedMassing_<building.id>`), replacing any from the previous call. Streets, plazas, and Salingaros scale/center indicators are **not yet built** — massing only. Returns `true` iff at least one building produced geometry.

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
- [~] **Phase 2: Surface Extraction & Procedural Mesh Building** (partial — see §8):
  - [x] Generic SDF → triangle mesh extraction (`street-smarts-core::surface_nets`, Naive Surface Nets, not Dual Contouring — see §8 for why).
  - [x] Real building massing from NIR data: footprint extrusion to `height_m`, ground-floor `Opening`s punched as real P221 aperture cuts (`street-smarts-godot::building_mesh`).
  - [x] Wired into `NeighborhoodNode3D::rebuild_3d_mesh()` — builds and attaches real `ArrayMesh` geometry via `SurfaceTool`, replacing the earlier no-op stub.
  - [ ] Triplanar material integration (`uv1_triplanar`).
  - [ ] Static neighborhood context hot-loading vs. active-parcel-only recompute (§3's 91-acre strategy — every call currently rebuilds every building).
  - [ ] `wall_thickness_m` as a real hollow shell (punches currently notch a solid mass, no interior cavity behind them).
  - [ ] `roof` / `roof_segments` / `canopies` / `wall_niches` geometry.
- [ ] **Phase 3: Real-Time Openings & Intimacy Shaders**: Live re-punching as a user edits (today's punches are correct but rebuilt from scratch, not incrementally), and floorplan intimacy gradient visualizers.
- [ ] **Phase 4: Interactive Mobile Viewport**: Touch/orbit gesture controls for live pattern steering and real-time disagreement prompts.

---

## 8. What's Actually Verified (and What Isn't)

This environment has the Rust toolchain and network access to crates.io, but **no Godot editor and no display** — nothing here has been visually confirmed inside Godot. Everything below was checked the only way available: `cargo test`/`cargo build` against the real `gdext` 0.2.4 crate.

**Verified:**
- `street-smarts-core::surface_nets` (Naive Surface Nets): extracted meshes for a sphere and a box match their analytic volumes within 5% (via the divergence theorem, `Mesh::signed_volume`), and every extracted vertex sits within one grid cell of the true SDF zero-surface. This is what proves the triangle winding-correction logic (§ code comments) actually produces a closed, consistently outward-wound mesh, not just "some triangles."
- `street-smarts-godot::building_mesh`: a synthetic rectangular building's extracted massing matches its analytic box volume within 5%; a real `Opening` placed via its actual `ring_index`/`t`/`width_m`/`sill_height_m`/`head_height_m` fields punches a hole exactly there (SDF flips sign at the opening's real position, stays solid elsewhere on the same wall) and measurably reduces the mesh's enclosed volume.
- `cargo build -p street-smarts-godot` produces a linked `libstreet_smarts_godot.so` against the real generated `SurfaceTool`/`ArrayMesh`/`MeshInstance3D` bindings (method signatures were read directly from `gdext`'s own codegen output, not guessed).
- The original Phase 1 code on `antigravity/godot-rust-port` did **not** actually compile against current `main`: `street-smarts-opinions::registry::evaluate_all`'s signature had changed (2 call sites), one `Variant`/`AsArg` mismatch, and `#![forbid(unsafe_code)]` directly conflicted with gdext's own required `unsafe impl ExtensionLibrary`. All fixed as part of this pass.

**Not verified (needs an actual Godot editor):**
- That the generated `MeshInstance3D` actually renders — winding order is corrected analytically per-quad (see `emit_quad` in `surface_nets.rs`) against the SDF gradient direction, and back-face culling has deliberately been left at Godot's default rather than force-disabled, so an error here would show as missing/inverted faces, not a crash.
- Frame rate/thermal behavior of rebuilding a whole neighborhood's massing per `rebuild_3d_mesh()` call — no active-parcel isolation yet (§3), so this does not yet scale to the 91-acre case the spec's Site Scale Strategy targets.
- Touch/orbit input, WebGL2 export size and load time, and the COOP/COEP header requirement for a threaded web export (all still open per the original assessment of this branch).
