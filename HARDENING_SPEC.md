# Hardening spec: predicates, bundle discipline, capability typing, visual regression, synthetic fixtures, chorus calibration

**Status:** proposal, not yet implemented.
**Author:** Claude (for Jason), 2026-07-17.
**Relationship to other docs:** `PATTERN_LANGUAGE_SIMULATION.md` and `PRIMITIVES_SPEC.md` are architecture proposals — new data structures and interfaces. The six items here are different in kind: operational and test-discipline hardening against the *existing* system. None of them require the ECS/DAG/scoped-type/pass-manager/steering-loop work from `PRIMITIVES_SPEC.md` to ship — each stands alone against the codebase as it is today, and two of them (§1/§5, §3) get more valuable once that other work lands, but aren't blocked on it. Pick any subset, in any order.

Every claim below was checked against the actual repo — `.github/workflows/deploy.yml`, `.github/workflows/vibe-render.yml`, `tools/vibe-render/render.py`, `crates/street-smarts-web/Cargo.toml`, `crates/street-smarts-core/src/geometry.rs`, `crates/street-smarts-patterns/src/planar.rs`, `crates/street-smarts-opinions/src/lib.rs` — not inferred from the READMEs alone.

---

## If you only do one thing: investigate `wasm-opt = false`

Found while grounding §2, but it deserves to be surfaced on its own, ahead of everything else in this document. `crates/street-smarts-web/Cargo.toml`:

```toml
[package.metadata.wasm-pack.profile.release]
wasm-opt = false
```

No comment nearby explaining why. `deploy.yml`'s "Bundle size" step runs right after the WASM build and only *echoes* the raw and gzip byte counts — it doesn't compare them to anything, and whatever it's reporting today is the size of an **unoptimized** release binary, because `wasm-opt` — the actual size/speed optimizer in the `wasm-pack` release pipeline — is explicitly turned off. `SPEC.md` §7 lists "Bundle size feasibility... Should fit but unconfirmed" as an open unknown; that unknown has been getting measured against a number that's larger than it needs to be.

This might be turned off for a real reason — a specific `wasm-opt` version has had known miscompilation bugs against certain `wasm-bindgen` output shapes in the past, and if that's what happened here, re-enabling it blind would be a mistake, not a fix. But if it was turned off once for a reason that's since stopped applying (or was never validated), turning it back on (`-Oz` for size, matching the project's stated size sensitivity, or `-O3` if runtime speed in the generator loop matters more) is very likely the single highest-value, lowest-design-cost action in this entire document — a compiler flag, not a new subsystem. Worth a dedicated PR on its own, with before/after gzip numbers in the description, before touching anything else here.

---

## 1. Robust geometric predicates

### 1.1 Motivation

`crates/street-smarts-core/src/geometry.rs` has no `EPSILON`, tolerance, or robust-arithmetic anywhere — `shoelace`, `ring_abs_area`, and the centroid/area/perimeter math are all plain `f64` operations. `crates/street-smarts-patterns/src/planar.rs`'s ear-clipping triangulator (line 373: "Guard against infinite loops on degenerate input") is explicitly aware that self-intersecting and collinear-chain inputs happen — it defends against the *symptom* (an infinite loop) but the underlying orientation/point-classification tests it and other planar operations use are still naive floating-point comparisons, not the adaptive-precision predicates (Shewchuk-style) that computational geometry libraries generally use once real polygon boolean operations are in play. This is the textbook setup for a bug class where two code paths disagree about the same geometric fact near a boundary — a point classified as inside a ring by one test and outside by another, purely from rounding, not from the actual input being ambiguous.

Circumstantial evidence this class of bug already exists, just silently: `pipeline.rs`'s per-block loop tolerates a block failing P61 or P95 by skipping it rather than aborting the run ("a block that fails P61 or P95 (e.g. too small to be worthwhile) is skipped rather than aborting the whole run"). Some of those skips are legitimately "too small to be worthwhile." Some fraction may be a predicate quietly misclassifying a near-degenerate block boundary. There's currently no way to tell the two apart, because nothing distinguishes "genuinely too small" from "geometry op produced garbage" at the skip site.

### 1.2 Design — two tiers, escalate only if needed

**Tier 1 (do this first): centralize, don't rewrite the math yet.** Every ad hoc orientation/cross-product comparison scattered across `planar.rs` and `geometry.rs` routes through one shared function:

```rust
// crates/street-smarts-core/src/predicates.rs

pub enum Orientation { CounterClockwise, Clockwise, Collinear }

/// Meaningfully smaller than the smallest real feature size already in use
/// in this codebase's own constants (P95/P108's 0.1m construction-joint
/// `pad_inset_m`, the 0.3m `STREET_THICKNESS_M` / 0.15m `PLAZA_THICKNESS_M`
/// used by vibe-render) — 1mm, not an arbitrarily chosen small number.
pub const EPSILON_M: f64 = 0.001;

pub fn orient2d(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Orientation { ... }
pub fn point_in_ring(p: (f64, f64), ring: &[(f64, f64)]) -> bool { ... }
pub fn segments_intersect(a0: (f64, f64), a1: (f64, f64), b0: (f64, f64), b1: (f64, f64)) -> bool { ... }
```

This alone fixes the *inconsistency* bug class (two call sites disagreeing with each other) even before touching precision — one function, one tolerance, used everywhere, instead of N independent reimplementations that may each round slightly differently.

**Tier 2 (escalate only if Tier 1's tests still show inconsistency): adaptive-precision arithmetic**, either hand-rolled Shewchuk-style adaptive predicates or the `robust` crate (a small, pure-Rust, dependency-light port — worth evaluating against the project's general low-dependency preference before adding it, per Tier 1 potentially already being sufficient at street-smarts' scale: projected, meter-precision, author-controlled parcel geometry, not unbounded-precision CAD input). Don't build this speculatively — build Tier 1, instrument the skip-tolerance in the pipeline (§1.3), and only reach for Tier 2 if real data shows fixed-epsilon tolerance genuinely isn't enough.

### 1.3 Instrumentation — make the existing tolerance visible

Add a debug counter (or a structured log line) at every "skip a block that failed" site in `pipeline.rs`, tagging *why* it failed (guard-clause reason string already likely exists in the `Result<_, String>` error — surface it, don't swallow it). This turns "how often is this masking a real bug" from a guess into a measured rate, and is the natural companion to §5 below — synthetic fixtures are what would actually exercise this path at a rate the single real Eastside Commons fixture doesn't.

### 1.4 Milestone

**Ships when:** every orientation/containment/intersection test in `planar.rs` and `geometry.rs` routes through `predicates.rs`, the per-block skip-tolerance in `pipeline.rs` logs its rejection reason instead of silently dropping the block, and a targeted regression test (a deliberately near-degenerate synthetic parcel — see §5 — that used to produce inconsistent classification across two call sites) now produces the same answer everywhere.

---

## 2. WASM bundle-size budget in CI

### 2.1 Motivation

Direct continuation of the finding above. `deploy.yml`'s existing "Bundle size" step:

```yaml
- name: Bundle size
  run: |
    echo "WASM raw   : $(stat -c%s public/pkg/street_smarts_web_bg.wasm) bytes"
    echo "WASM gzip  : $(gzip -c public/pkg/street_smarts_web_bg.wasm | wc -c) bytes"
```

measures and reports, but nothing fails the build if the number regresses. `SPEC.md` §3.5 states "< 5 MB compressed" as a hard constraint and §7 lists actually measuring it as an open unknown — this closes that unknown with an enforced number instead of a periodically-eyeballed log line.

### 2.2 Design

- A checked-in budget: `.bundle-budget` (or a field in an existing config file) holding a single gzip-byte-count ceiling. Seed it from the *current* measured size (once §"if you only do one thing" is resolved and `wasm-opt` is confirmed on-or-legitimately-off) plus a small headroom margin, not an arbitrary round number.
- `deploy.yml`'s step becomes a gate: compute gzip size, compare to budget, fail the job (with the actual vs. budgeted numbers in the failure message) if exceeded.
- Per-crate/per-symbol attribution on failure (or on every PR, as a non-blocking informational step): `twiggy top` / `twiggy monomorphizations` against the compiled `.wasm`, so a size regression is diagnosable ("P37's new dependency added 40KB from monomorphized generics") instead of just "the number went up, go find out why yourself." Post as a PR comment or upload alongside the existing vibe-render artifacts.
- Ratchet, don't just cap: once the budget's been comfortably under for a while, lower it — otherwise the budget only ever prevents catastrophic regressions, not slow bloat.

### 2.3 Risks

- `twiggy`'s output can be noisy with generic-heavy Rust code (this project uses `Parameters`/`PatternOperator` generics extensively) — expect to tune what counts as "attributable" rather than trusting the raw tool output uncritically.
- A budget that's too tight becomes a nuisance every contributor routes around (`--no-verify`-style workarounds); seed it generously first, tighten over time based on real data, not a guess.

### 2.4 Milestone

**Ships when:** `deploy.yml` fails a deliberately bloated test PR (e.g. one that vendors an unnecessary dependency) with a clear budget-exceeded message, and passes normally otherwise with the current real bundle size comfortably under budget.

---

## 3. Capability-typed opinions

### 3.1 Motivation

`SPEC.md` §6.3: "No API keys required. No third-party telemetry. VLM hooks are optional add-ons, never blockers... This is a political requirement, not an engineering preference." Checked against the actual code: `crates/street-smarts-web/Cargo.toml` has zero network-capable dependencies today, and `crates/street-smarts-opinions/src/lib.rs`'s own doc comment confirms the VLM family is "deferred to a later version" — nothing currently violates the constraint. This spec is explicitly preventive: it's about making sure the constraint survives v0.2's VLM hook (`SPEC.md` §5.2 milestone) landing, rather than trusting every future contributor to remember an unenforced policy while wiring up an Anthropic/OpenAI client.

### 3.2 Design

```rust
// crates/street-smarts-core/src/opinion.rs — addition to the existing Opinion trait

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Network,   // calls an external API
    ApiKey,    // requires a credential
}

pub trait Opinion {
    // ...existing methods unchanged...

    /// What this opinion needs to function. Empty by default — every
    /// existing v0.1 opinion satisfies this with zero code changes.
    fn capabilities(&self) -> &'static [Capability] { &[] }
}
```

**The real enforcement mechanism is a Cargo feature boundary, not the trait method alone.** When the VLM family is built, it should live behind a `vlm` feature that `street-smarts-web`'s default build never enables — at that point the VLM code is *physically absent* from the compiled activist-facing WASM artifact, which is a stronger guarantee than any runtime check. `Capability::Network` on the trait is the documentation and the check for contexts where a feature boundary alone isn't legible enough to audit at a glance — e.g. a future native tool (`wholeness-lab`) that intentionally links VLM opinions *and* browser-safe ones side by side and needs to programmatically filter which outputs are safe to bake into an activist-facing export.

- CI addition: `cargo build -p street-smarts-web --no-default-features` (the actual activist bundle build) should be asserted, structurally, to never link a crate that declares a `Capability::Network` opinion — either by the feature boundary making it a non-issue by construction, or, belt-and-suspenders, a test that calls `all_opinions_v01()` (or its future equivalent) in that exact build configuration and asserts every returned opinion's `capabilities()` is empty.

### 3.3 Risks

- Low implementation risk (additive trait method, default value, zero migration cost for the 7 existing opinions) — the real risk is process, not code: this only protects the constraint if it's built *before* the VLM feature, not retrofitted after someone's already wired up an API client without it. Worth doing now, ahead of `SPEC.md` §5.2's VLM milestone, specifically so the constraint is established before the code that could violate it exists.

### 3.4 Milestone

**Ships when:** `Opinion::capabilities()` exists with its default, a `vlm` Cargo feature boundary is drawn in `street-smarts-opinions`'s `Cargo.toml` (even before any VLM opinion is implemented inside it — an empty module behind the feature flag is enough to establish the wall), and a CI step confirms `street-smarts-web`'s default build doesn't enable that feature.

---

## 4. Golden visual-regression testing via vibe-render

### 4.1 Motivation

`.github/workflows/vibe-render.yml` already runs on every PR and uploads renders as artifacts, with its own header comment stating the purpose plainly: "so a reviewer can eyeball scale/density/fragmentation before merging without running cadquery/OpenCascade locally." That's real infrastructure, already wired to `deploy.yml`'s exact list of expected output files (`clean_baseline_isometric.png`, `barrio_mallcore_isometric.png`, `mallcore_seeding_stratified_isometric.png`, `mallcore_seeding_fieldguided_isometric.png`, plus `.glb` and `.svg` outputs) — but comparison against those renders is 100% human-eyeball, every time, on every PR. There's no automated signal that a render changed at all; a reviewer has to remember to look, and has to hold the previous version in their head (or open the last PR's artifact) to know if something's different.

### 4.2 Design

**Two tiers, matching §1's cost discipline:**

- **Tier 1 (cheap, runs on every PR):** perceptual hash (pHash or dHash) of each of the existing named PNG outputs, compared against a checked-in reference hash — a short hex string per scenario, not a stored image, so this doesn't bloat the git repo. Catches gross regressions (a pattern silently stopped producing buildings, a color/scale/orientation regression) with a near-zero storage and runtime cost.
- **Tier 2 (on-demand, or nightly against `main`):** full pixel diff against the last-approved baseline image, retrieved from the most recent successful `main`-branch workflow artifact (not committed to git history — avoids the repo-bloat problem full image storage would create) for cases where a hash mismatch needs visual confirmation of *what* changed.

**Re-baselining as a first-class, scripted action**, not manual file surgery: `scripts/update-vibe-baseline.sh` regenerates and commits the reference hashes when a change *intentionally* alters a render — the same discipline `cargo insta` or Jest snapshot testing uses. Without this, a perceptual-diff gate rots into something contributors route around rather than trust.

**Prerequisite worth checking before building the rest: is `render.py`'s output actually deterministic?** cadquery/OpenCascade boolean operations (`punch_openings`, per the script's own doc comment) can have platform- or version-dependent floating-point behavior in some geometry kernels. A quick verification (render the same fixture twice, hash both, confirm equality — ideally on both a local machine and the actual CI runner image) should happen before investing in the comparison infrastructure, or the gate will be chasing false positives from day one.

### 4.3 The honest limit — restated deliberately, matching the other two docs' house style

This catches "the render changed in a way nobody flagged as intentional." It does not, and cannot, check whether the result is *good* — whether it has the quality without a name `PATTERN_LANGUAGE_SIMULATION.md` §4.5 already flagged as outside any automated check's reach. A pattern that regresses from "alive" to "technically unchanged but uninspired" produces an identical perceptual hash. This is a mechanism-changed detector, not a QWAN detector, and should never be described as the latter in any report this project produces.

### 4.4 Milestone

**Ships when:** determinism is confirmed, perceptual-hash comparison runs in `vibe-render.yml` against the four scenarios `deploy.yml` already names, a deliberately-broken test PR (one that visibly changes a render) fails the check, and the re-baseline script has been exercised at least once on a real, intentional rendering change without manual file editing.

---

## 5. Procedural / synthetic fixture generation

### 5.1 Motivation

`data/eastside-baseline.json` and `data/eastside-proposal.json` are the only two real fixtures in the repo — one real site shape. Every existing pattern test (`tests/p37_house_cluster.rs`, `tests/p61_small_public_squares.rs`, etc.) varies the RNG seed against this one shape. `PATTERN_LANGUAGE_SIMULATION.md` §4.4 already proposed generalizing the seed-variance fuzzing into a shared harness — but seed variance alone can't find a bug that's triggered by *shape*, not randomness: a concave near-self-intersecting boundary, a sliver parcel with almost no developable area, a site an order of magnitude larger or smaller than Eastside Commons. Those are exactly the inputs §1's predicate work is worried about, and the single real fixture may simply never contain one.

### 5.2 Design

A small, deterministic, seeded generator — `crates/street-smarts-patterns/tests/common/synthetic_fixtures.rs`, or its own thin crate if it grows — producing valid-but-varied parcel fabrics along explicit, named axes:

```rust
pub struct FixtureAxes {
    pub aspect_ratio: f64,      // 1.0 (square) .. 20.0 (sliver)
    pub concavity: f64,         // 0.0 (convex) .. 1.0 (star-shaped, near-self-intersecting)
    pub area_m2: f64,           // tiny lot .. multi-hundred-acre site
    pub existing_building_density: f64,
    pub vertex_count: usize,    // simple rectangle .. highly irregular boundary
}
pub fn generate(axes: &FixtureAxes, seed: u64) -> Neighborhood { ... }
```

Every generated fixture is validated by a minimal physical-plausibility checker (closed rings, non-self-intersecting *by construction* even at the extreme end of the `concavity` axis, positive area) before being handed to a pattern operator — so a downstream test failure is attributable to the operator under test, not to the generator having produced nonsense.

**This directly feeds `PATTERN_LANGUAGE_SIMULATION.md` §4.4's `assert_pattern_invariant` harness** with the second axis it was designed to accept (fixture × seed) but had no real non-trivial fixture source for beyond the one real site. And it's the practical mechanism that would actually exercise §1's predicate work — pairing §1 and §5 is deliberate: one fixes a bug class, the other is the test that would have caught it in the first place.

### 5.3 Milestone

**Ships when:** the generator produces valid fixtures across all five named axes, the existing pattern test suite runs against real *and* synthetic fixtures in CI, and — the actual proof this was worth building, not just a coverage-percentage vanity metric — at least one real bug surfaces from a synthetic fixture that the single real Eastside Commons shape never would have exercised.

---

## 6. Chorus calibration / VLM-drift detection

### 6.1 Motivation

`SPEC.md` §7 names this risk explicitly and proposes a partial mitigation already: "Frontier models drift. Pin versions. Log every score with its model version. Accept that VLM opinions in 2026 won't match VLM opinions in 2028." What's missing is the actual check that would catch drift happening, rather than just accepting it will happen and hoping the version-pinning log is enough to explain it after the fact.

### 6.2 Design

- A scheduled job (not per-PR — calling a real VLM API repeatedly per commit is slow and, per §3, exactly the kind of thing that should never be in the activist-facing critical path) that re-runs the full opinion chorus against the fixed sanity-floor reference sites `SPEC.md` §3.7/Appendix B already names (Siena, Trastevere, the TBD non-European site), and appends the resulting opinion vector to a small time series.
- Two concrete signals this catches: (a) **absolute drift** — a frontier-model version bump silently shifting a VLM opinion's score on an *unchanged* input, visible as a step change in the time series; (b) **correlation drift** — if a geometric detector and a VLM opinion on the same axis have historically tracked each other on the sanity-floor sites and suddenly diverge, that's worth a human look before either output is trusted on live coalition-facing data, independent of whether either one individually "moved."
- Requires one small, currently-missing structural piece: `OpinionOutput` doesn't carry a model-version field today (`method_summary`/`details` are free text). Add `model_version: Option<String>`, populated for VLM opinions specifically, so (a) is queryable rather than something a human has to reconstruct from log timestamps.
- Explicitly a monitoring/alerting concern, not a CI gate — a real, meaningful VLM improvement will also show up as "drift" in this signal, and auto-rejecting it would be a mistake. Triage by a human, always.

### 6.3 Honest scoping

This is the least actionable item in this document today, and it should be presented that way rather than padded to look equally ready-to-build as the other five: **no VLM opinion exists yet** (confirmed — `street-smarts-opinions/src/lib.rs` defers the whole family). The only genuinely actionable piece right now is reserving `model_version: Option<String>` on `OpinionOutput` — cheap, zero-risk, and avoids a schema retrofit later. The rest of this spec is a design to build the VLM feature *against* from day one, not a standalone deliverable to schedule this quarter.

### 6.4 Milestone

**Ships when (partial, now):** `OpinionOutput::model_version` field exists, unused, ready. **Ships when (full, post-VLM):** the scheduled calibration job runs against the sanity-floor sites, the time series is queryable, and a deliberately-simulated version bump (swap in a different pinned model temporarily) produces a visible, correctly-attributed signal in the log.

---

## 7. How to pick

These six are close to independent of each other — unlike `PRIMITIVES_SPEC.md`'s five, there's no real dependency chain forcing an order. Groupings worth knowing about, if sequencing matters to you:

- **Do first, essentially free:** investigate `wasm-opt = false` (see top of this document) — not even really "one of the six," just the most immediately actionable finding here.
- **A natural pair:** §1 (predicates) and §5 (synthetic fixtures) — §5 is what would actually surface the bug class §1 is worried about; doing one without the other leaves either an unexercised fix or an untriaged signal.
- **Best done ahead of a feature landing, not after:** §3 (capability typing) — cheap now, meaningfully harder to retrofit once a VLM client already exists somewhere in the dependency graph.
- **Needs a quick prerequisite check before the real build:** §4 (visual regression) — confirm `render.py`'s output is actually deterministic first, or the gate chases false positives from day one.
- **Not independently schedulable yet:** §6 (chorus calibration) — reserve the one cheap field now (`model_version`), defer the rest until the VLM feature it's calibrating actually exists.
- **Standalone, do whenever CI velocity or a size-conscious release makes it relevant:** §2 (bundle budget) — the most mechanical, lowest-judgment item on the list once `wasm-opt` is sorted out.
