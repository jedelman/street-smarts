# Deep primitives spec: ECS, content-addressed history, scoped types, pass manager, multi-objective search

**Status:** proposal, not yet implemented.
**Author:** Claude (for Jason), 2026-07-17.
**Relationship to other docs:** extends `PATTERN_LANGUAGE_SIMULATION.md`'s §3–4 (development speed / accuracy via pattern primitives) with five deeper substrate proposals that document flagged as bigger bets. Assumes that document's context. Numbered 1, 2, 4, 5, 7 to match the conversation that produced them — 3 (graph-rewrite critical-pair analysis) and 6 (conflict-of-laws vocabulary) are smaller, standalone ideas from that same conversation and don't need separate specs; they're one- or two-line changes once the numbered items below exist.

Every proposal here is grounded in the actual code as of this commit — struct names, field names, and file paths below were read from source, not guessed. Where a claim needed verifying (e.g. "everything is stringly-typed") I checked and corrected it against `crates/street-smarts-core/src/nir.rs`.

---

## 0. How these five relate

They are not five independent features. Three of them are the same underlying idea (a typed, queryable substrate for "what can read/write what") applied at three different layers, and the other two are consumers of that substrate:

```
                    ┌─────────────────────────┐
                    │  1. ECS substrate         │  ← the typed data model
                    │  (components replace       │
                    │   free-form string tags)   │
                    └────────────┬───────────────┘
                                 │ read/write component sets
                                 │ are now a fact the compiler
                                 │ knows about each system
                 ┌───────────────┼────────────────┐
                 ▼                                 ▼
      ┌─────────────────────┐          ┌─────────────────────────┐
      │ 4. Scoped types       │          │ 5. Pass manager            │
      │ (containment enforced │          │ (requires/writes/          │
      │  on read AND write)   │          │  preserves/invalidates)    │
      └───────────┬───────────┘          └────────────┬────────────┘
                  │                                    │ ordering + parallelism
                  │                                    │ correctness proof
                  └─────────────────┬──────────────────┘
                                    ▼
                    ┌─────────────────────────┐
                    │  2. Content-addressed      │  ← the immutable history
                    │  history (git-for-          │     substrate every
                    │  neighborhoods)              │     candidate lives in
                    └────────────┬───────────────┘
                                 │ cheap branching + memoized
                                 │ re-derivation
                                 ▼
                    ┌─────────────────────────┐
                    │  7. Multi-objective         │  ← the search algorithm
                    │  steering loop               │     that actually uses
                    │  (Pareto frontier + beam)    │     all of the above
                    └─────────────────────────┘
```

None of the five requires all the others to ship value on its own — §5 and §2 are usable today, standalone, against the existing `Vec<Parcel>`/string-tag data model. But §4 gets meaningfully cleaner once §1 exists, §5's `requires`/`writes` lists get derived instead of hand-declared once §1 exists, and §7 is close to pointless without §2 (repeated re-derivation of near-identical candidate states is the whole cost problem it's solving). Suggested build order is in §8.

---

## 1. Entity-Component-System substrate

### 1.1 Motivation

A precise look at `crates/street-smarts-core/src/nir.rs` shows the current data model is *already partly typed* — `OpenSpaceKind`, `Ownership`, `OpeningKind`, `BoundaryKind`, and `ActivityKind` are all real enums. The untyped part is narrower but still load-bearing: `Parcel.use_category: Option<String>`, `Parcel.spec: Option<String>`, `Parcel.density_tier: Option<String>`, `Building.typology: Option<String>`, `Street.classification: Option<String>`. These are exactly the fields patterns use to find their targets — `spec.as_deref().unwrap_or("").starts_with("BLOCK_")` (pipeline.rs), `use_category == "p95_building_pad"` (P107's filter). Every new cross-cutting concern a future pattern needs to tag (which cluster a parcel belongs to, which pass last touched it, a provisional vs. confirmed distinction) either overloads one of these five strings with a new convention, or adds a sixth field to `Parcel`/`Building` that most patterns will never populate.

The general version of this problem — many independent, loosely-coordinated systems that each need to attach their own typed data to a shared set of entities, without stepping on each other or bloating a single monolithic struct — is what ECS architectures solve. Applying it here is not a stretch: a `Parcel` already *is* a loose bag of optional, pattern-populated fields (`density_tier`, `target_stories`, `spec` were each added by a specific pattern operator, per their own doc comments in nir.rs). ECS just makes that bag open-ended and typed instead of closed and partly-stringly-typed.

### 1.2 Design

**Persistent, not mutable-in-place.** The existing architecture is built on immutable snapshots (`apply_subdivision` takes `&Neighborhood`, returns an owned new one) — an ECS built on `&mut World` in-place mutation would fight that. Use structurally-shared persistent maps instead (hand-rolled persistent `HashMap` via an Arc-based HAMT, or the `im` crate if a dependency is acceptable — see §1.5) so cloning a `World` to produce the "next" state is O(1) amortized, not O(n).

```rust
// crates/street-smarts-core/src/world.rs

pub type EntityId = String; // reuse existing Parcel/Building .id convention — no ID scheme migration

pub struct World {
    // The five existing NIR vectors, unchanged in shape, just indexed:
    parcels: PersistentMap<EntityId, Parcel>,
    buildings: PersistentMap<EntityId, Building>,
    streets: PersistentMap<EntityId, Street>,
    open_space: PersistentMap<EntityId, OpenSpace>,
    // NEW: typed sidecar components. Each component type gets its own map.
    components: TypeIndexedMap, // HashMap<TypeId, Box<dyn ErasedPersistentMap>>
}

pub trait Component: 'static + Clone + Send + Sync {}

impl World {
    pub fn get<C: Component>(&self, entity: &EntityId) -> Option<&C> { ... }
    pub fn set<C: Component>(&self, entity: &EntityId, value: C) -> World { ... } // returns new World
    pub fn query<C: Component>(&self) -> impl Iterator<Item = (&EntityId, &C)> { ... }
}
```

Example new components that today are `Option<String>` fields or don't exist yet: `BlockMembership { block_id: EntityId }`, `DensityTier(Tier)` (a real enum: `Core`/`Mid`/`Edge`, replacing the `density_tier: Option<String>` free-form field), `HouseClusterMembership { cluster_id: EntityId }`, `PadRole(PadRole)` (replacing the `use_category == "p95_building_pad"` string match).

**Systems.** A pattern operator becomes something that declares its component interest explicitly:

```rust
pub trait System {
    type Params: Parameters;
    fn reads(&self) -> &'static [TypeId];   // component types this system queries
    fn writes(&self) -> &'static [TypeId];  // component types this system's output touches
    fn run(&self, world: &World, target: &EntityId, params: &Self::Params, seed: u64)
        -> Result<World, String>;
}
```

This `reads`/`writes` pair is not new bookkeeping invented for its own sake — it is the exact input §5's pass manager needs (`requires`/`writes` in that spec), and it's the exact input a parallel scheduler needs to prove two systems don't conflict. Declaring it once, here, feeds both.

### 1.3 Migration path — do NOT touch the NIR wire format up front

The existing JSON fixtures (`data/eastside-baseline.json`, `data/eastside-proposal.json`) and every test that deserializes them are a hard constraint: this must not require a schema-breaking migration to get started. Three phases:

- **Phase A — adapter, zero schema change.** `World::from_neighborhood(&Neighborhood) -> World` and `World::to_neighborhood(&self) -> Neighborhood` convert both ways. `World` is a *view*, existing `Neighborhood`/`Parcel`/etc. stay canonical. Nothing downstream of the NIR schema (fixtures, fixture-consuming tests, the WASM/web boundary) changes at all in this phase.
- **Phase B — new components as sidecars, dual-write.** New concerns (`BlockMembership`, `DensityTier` as a real enum, `PadRole`) get written to `World`'s component maps *and* to their existing string-field shadow (`density_tier: Option<String>`, kept for serialization back-compat) inside `to_neighborhood`. New pattern code queries the typed component; nothing that reads the string field breaks.
- **Phase C — optional, later.** If Phase B proves out, deprecate the string fields, bump a schema version, and make components the only source of truth. Not committed to here — a decision to make once there's real experience with Phase B, not up front.

This mirrors the exact discipline `PATTERN_LANGUAGE_SIMULATION.md` argued for in the pattern operators themselves (unfold, don't rebuild-from-parts) — applied to the migration of the architecture that runs those operators.

### 1.4 Risks and non-goals

- **Bundle size / dependency surface.** SPEC.md's generator constraints (§3.5: "Bundle < 5 MB compressed") and this crate's `#![forbid(unsafe_code)]` argue for hand-rolling a minimal persistent map rather than pulling in `hecs`/`legion`/`bevy_ecs`. Those crates are excellent but are built for native game loops, not a size-constrained WASM deploy target, and most of their scheduler machinery (this proposal doesn't need archetype-based iteration performance at street-smarts' entity counts — hundreds to low thousands, not millions) would be dead weight. Recommend a from-scratch `PersistentMap<K, V>` (a simple Arc-based sharing scheme, not a full HAMT, is probably sufficient at this scale) over adopting an existing ECS crate.
- **Parallelism is not free in the deploy target.** `wasm32-unknown-unknown` without `SharedArrayBuffer` (which requires COOP/COEP headers Cloudflare Workers Assets would need to be configured to send) can't run systems concurrently in the browser. The scheduling benefit this unlocks (see §5) is real for the native/Python training loop (`wholeness-lab`) and for CI, but should not be pitched as a browser-side speed win without that infrastructure work being scoped separately.
- **This is the biggest bet of the five.** Every operator's signature changes. Recommend doing it after §5 ships against the existing string-tag model (so there's a working pass-manager to validate against before *and* after the migration, as a correctness check that the migration didn't silently change pipeline behavior).

### 1.5 Milestone

**Ships when:** `World::from_neighborhood`/`to_neighborhood` round-trip every existing fixture byte-for-byte (property test: `to_neighborhood(from_neighborhood(n)) == n` for both `data/*.json` fixtures), and one real pattern (recommend P29 Density Rings — it already introduces a tier concept that's crying out to be an enum, not a string) is ported to `System` in Phase B without changing its own test file's assertions.

---

## 2. Content-addressed history (git-for-neighborhoods)

### 2.1 Motivation

Look at `apply_subdivision` in `crates/street-smarts-patterns/src/subdivision.rs`:

```rust
out.id = format!(
    "{}__{}+{}",
    nbhd.id, sub.trace.operator_name, sub.trace.seed
);
```

This is already a hash chain — an `id` that encodes its own derivation from a parent plus the operation that produced it — just built from string concatenation instead of a real content hash, and with no actual store behind it (nothing lets you look up "give me the `Neighborhood` for this id" except recomputing the whole pipeline from scratch). Formalizing this closes three gaps that `SPEC.md` currently treats as three separate future features:

1. **§3.5's steering loop** needs to try multiple candidate next-steps from the same state and compare them — that's branching, and branching-with-cheap-comparison is what content-addressed history is *for*.
2. **§3.6's decision ledger** wants "what got picked/modified/rejected" to be an accurate record — a content hash is a stronger, tamper-evident version of the free-text `modification: Modification` field the current `LedgerEvent::modified_proposal` variant carries.
3. **Re-running the same fixture+seed repeatedly in tests and in the steering loop's shared-prefix exploration** is currently uncached, full recomputation every time.

### 2.2 Design

```rust
// crates/street-smarts-history/src/lib.rs  (new crate)

pub type NeighborhoodId = blake3::Hash; // 32 bytes, pure-Rust, wasm-friendly, no unsafe

pub struct Commit {
    pub id: NeighborhoodId,
    pub parent: Option<NeighborhoodId>,
    pub operator_name: String,
    pub params: serde_json::Value,
    pub seed: u64,
    /// Version tag of the code that produced this commit — see §2.4.
    pub algorithm_version: String,
}

pub trait HistoryStore {
    /// Content-address a full Neighborhood snapshot (canonical serialization, hashed).
    fn hash_neighborhood(n: &Neighborhood) -> NeighborhoodId;

    /// Look up a commit's metadata without materializing the full snapshot.
    fn commit(&self, id: NeighborhoodId) -> Option<Commit>;

    /// Materialize the actual Neighborhood for a commit (replays from the
    /// nearest cached ancestor snapshot + patch chain; see §2.3).
    fn materialize(&self, id: NeighborhoodId) -> Result<Neighborhood, String>;

    /// The memoization entry point: run `op` with `params`/`seed` on `parent`
    /// UNLESS that exact (parent, op, params, seed, algorithm_version) tuple
    /// has already been computed, in which case return the cached id.
    fn get_or_compute(
        &mut self,
        parent: NeighborhoodId,
        op: &dyn DynOperator,
        params: &serde_json::Value,
        seed: u64,
    ) -> Result<NeighborhoodId, String>;

    /// Reverse lookup: every commit whose parent is `id` — the "what were
    /// the alternatives from here" query the disagreement/coalition UI needs.
    fn children(&self, id: NeighborhoodId) -> Vec<NeighborhoodId>;
}
```

**Storage strategy: patches, not full snapshots, with cached materialization.** Storing a full `Neighborhood` per commit is simple but wasteful for a deep chain (the 11-step corrected pipeline already produces 11 intermediate states per run). Store the `Subdivision` (already exists as a struct — it *is* a patch) content-addressed by `(parent, operator_name, params_hash, seed)`, and keep an LRU cache of materialized `Neighborhood`s so `materialize()` doesn't replay from the root every call. This is the same keyframe-plus-delta idea video codecs and Git's own packfile format both use, at a much smaller scale here.

**Storage backend.** For `street-smarts-web`: IndexedDB, which `SPEC.md` §3.1 already commits to for adapter caching — this is the same storage mechanism, one more table. Native/training side (`wholeness-lab`): flat files or sqlite, whichever is already in use there.

### 2.3 What this does NOT solve: merging

Git's real complexity is merge — reconciling two branches that diverged and both changed something. That does not port over cleanly here: two `Subdivision`s from the same parent that both touch overlapping physical geometry (e.g. two candidate P61 square placements on the same block) do not have a generically well-defined "merge" the way two non-overlapping text diffs do. **Explicitly scope this proposal to not attempt automatic merge.** Branches from a common ancestor are permanently divergent alternatives to *compare*, never combined — which is not a limitation to work around, it's a match to `SPEC.md` §6.2's existing design stance ("a proposal where all the algorithms agree is less interesting... the conflict engine highlights disagreement"). The DAG gives you cheap comparison of siblings; it was never going to give you reconciliation, and it shouldn't try to.

### 2.4 Determinism and versioning

Two pitfalls specific to hashing geometric/floating-point state:

- **Cross-platform determinism.** Hash the canonical `serde_json` byte output (already deterministic per the existing derive setup — same struct, same field order, same f64 → JSON-number formatting), not raw memory layout. This should already be stable across the platforms this project targets (native + wasm32), but is worth an explicit test: hash the same fixture on both targets in CI, assert equal.
- **Algorithm-version drift.** If an operator's internal logic changes between commits (a bug fix to P37's clustering, say), the same `(parent, op, params, seed)` tuple now produces a *different* output than an old cached/ledger-recorded commit with that tuple — which is correct and desired (the old and new algorithm genuinely disagree), but silently comparing across that boundary would be misleading. `Commit.algorithm_version` (a crate version string or a git short-hash of the patterns crate, recorded at commit time) makes that boundary visible instead of silent; `get_or_compute` should treat cache entries from a different `algorithm_version` as misses, not hits.

### 2.5 Risks

- New crate, new dependency (`blake3` — small, pure-Rust, no `unsafe` in its public API surface relevant here, already a common WASM-compatible choice).
- The LRU materialization cache needs real sizing work once there's a real corpus of proposals — deferred to implementation, not a design blocker.
- This is a genuine architecture layer, not a drop-in — recommend building it under `street-smarts-ledger`'s currently-stubbed crate rather than as a fully separate one, since "the record of what was decided" (`SPEC.md` §3.6, "the only accurate part of the system") is precisely what a verifiable, hash-chained commit graph is for. That reframes `street-smarts-ledger`'s open implementation work as "build the history store, then the ledger's `LedgerEvent`s become references into it" rather than starting the ledger from nothing.

### 2.6 Milestone

**Ships when:** `run_corrected_pipeline_with_p37`'s 11 internal steps are recorded as a real commit chain (not just narrated in trace strings), `materialize()` reproduces byte-identical output to the current direct-call pipeline for the existing fixtures, and re-running the exact same `(baseline, parcel_id, seed)` twice is a cache hit on the second call, measurably faster.

---

## 4. Scoped types for operator containment

### 4.1 Motivation

`PATTERN_LANGUAGE_SIMULATION.md` §4.3 proposed a *runtime* property test: after running an operator, assert every ID in its returned `Subdivision.replaced_*_ids` was reachable from the scope it was invoked on. That test only catches what someone remembered to run. A stronger version makes the violation impossible to express in the first place — not by trusting convention (today, `apply(&self, nbhd: &Neighborhood, parcel_id: &str, ...)` hands every operator a reference to the *entire* neighborhood; nothing but the author's care stops an operator from reading or reasoning about entities far outside its stated target).

### 4.2 Design

Rust's borrow checker doesn't have a built-in notion of "logical partition of a HashMap," but a restricted-view wrapper gets most of the way there with ordinary, unexotic Rust — no unsafe, no nightly features:

```rust
pub struct ScopedView<'a, S: ScopeMarker> {
    parcels: Vec<&'a Parcel>,       // pre-filtered at construction time
    buildings: Vec<&'a Building>,
    open_space: Vec<&'a OpenSpace>,
    _scope: std::marker::PhantomData<S>,
}

impl Neighborhood {
    pub fn scoped<S: ScopeMarker>(&self, scope: Scope) -> ScopedView<'_, S> {
        // filters self.parcels/buildings/open_space by `scope` ONCE, here;
        // entities outside scope are never referenced by the returned view,
        // so operator code holding only a ScopedView literally cannot name them.
    }
}
```

**The wrinkle: read-scope and write-scope are not the same set, and conflating them breaks real patterns.** P29 Density Rings needs the *whole site's* geometry to compute a density center, even though it only writes a tag onto individual blocks (its own module doc, cited in `pipeline.rs`, is explicit about this). A `ScopedView` that only exposed one block would make P29 impossible to implement correctly. So the operator trait needs two declared scopes, not one:

```rust
pub trait ScopedOperator {
    type Params: Parameters;
    type ReadScope: ScopeMarker;   // broad — informs the decision
    type WriteScope: ScopeMarker;  // narrow — what the returned patch may touch

    fn apply(
        &self,
        read: &ScopedView<Self::ReadScope>,
        write_target: &ScopedView<Self::WriteScope>,
        params: &Self::Params,
        seed: u64,
    ) -> Result<Subdivision, String>;
}
```

**Reads are contained at compile time (can't name what isn't in the view); writes are validated at call time, generically.** `Subdivision` is still plain data — a bug could put a stray ID in `replaced_parcel_ids` that the `write_target` view never actually contained. But because `write_target` already computed the eligible-ID set to build the view, checking the returned `Subdivision` against it is a **generic** check written once (not a bespoke property test per operator, which is what `PATTERN_LANGUAGE_SIMULATION.md` §4.3 would have required per pattern):

```rust
pub fn apply_subdivision_checked<S: ScopeMarker>(
    nbhd: &Neighborhood,
    write_target: &ScopedView<S>,
    sub: &Subdivision,
) -> Result<Neighborhood, String> {
    let eligible: HashSet<&str> = write_target.entity_ids();
    for id in sub.replaced_parcel_ids.iter().chain(&sub.replaced_open_space_ids).chain(&sub.replaced_building_ids) {
        if !eligible.contains(id.as_str()) {
            return Err(format!("operator wrote outside its declared scope: {id}"));
        }
    }
    Ok(apply_subdivision(nbhd, sub))
}
```

This is honest about what's actually achievable in safe Rust: full compile-time enforcement of "the returned patch only touches X" would need something like an effect system or session types, which is not a fit for this codebase. What's realistic — and still a real improvement over §4.3's bespoke per-operator tests — is compile-time restriction on *what can be read/reasoned about*, plus a *generic, write-once* runtime check on what actually got written, instead of a hand-written test per pattern.

### 4.3 Sequencing note

This is meaningfully easier to build well *after* §1 (ECS) lands — "write scope" becomes "the set of entities carrying component `PadRole::BuildingPad`," a query, rather than a hand-written predicate re-implementing the current `use_category == "..."` string match inside `Scope`'s definition. It can still ship a v1 against the current `Vec<Parcel>` model (predicates over `spec`/`use_category` strings, same information the code already uses, just centralized into `Scope` instead of repeated per operator) — it doesn't strictly require §1, it just gets cleaner once §1 exists. Recommend shipping the v1 first (cheap, immediately useful, forces the `Scope` taxonomy to get named explicitly) and simplifying it in place once/if §1 lands.

### 4.4 Milestone

**Ships when:** every one of the 14 existing pattern operators is migrated to `ScopedOperator`, `apply_subdivision_checked` is what `run_corrected_pipeline_with_p37` calls instead of the unchecked `apply_subdivision`, and a deliberately-broken test operator (one that tries to write outside its declared `WriteScope`, added specifically to prove the check works) fails loudly instead of silently corrupting an unrelated block.

---

## 5. Pass manager: `requires` / `writes` / `preserves` / `invalidates`

### 5.1 Motivation

`PATTERN_LANGUAGE_SIMULATION.md` §4.1 proposed a `LANGUAGE` graph with `requires`/`completes` edges, validated by topological sort against the actual call order in `pipeline.rs`. That's necessary but not sufficient — it's missing the relation that the *existing* prose comments in `pipeline.rs` and `registry.rs` spend the most words on. Re-reading those comments: they aren't mostly about "X needs Y to exist first" (a `requires` fact). They're mostly about "X *breaks an assumption* Y made" — P108 (Connected Buildings) merging pads "deviates from Alexander's numbering... because daylight-depth shaping needs to see the real, final connected footprint," i.e., running P107 before P108 would mean P107 computed daylight depth against pad boundaries that P108 was about to erase. That is not a missing-dependency bug, it's an **invalidated-assumption** bug — the exact class LLVM's `PassManager` was built to catch mechanically, decades ago, via a fourth relation beyond "depends on": passes declare which of *other passes'* established properties they leave intact (`preserves`) versus break (`invalidates`), and by default a pass is assumed to invalidate everything it doesn't explicitly preserve.

### 5.2 Design

```rust
pub struct PassInfo {
    pub id: &'static str,
    pub alexander_number: Option<u32>,
    pub scale: Scale, // Site | Block | Category(&'static str)

    /// Must already be established (by SOME earlier pass) before this one runs.
    pub requires: &'static [&'static str],
    /// Newly established by this pass.
    pub writes: &'static [&'static str],
    /// Properties established by OTHER passes that remain valid after this
    /// one runs. Default (unlisted) = NOT preserved — conservative, matches
    /// LLVM's own default-invalidate stance, forces every pass author to
    /// think about the question rather than silently assume nothing changed.
    pub preserves: &'static [&'static str],
}

pub struct PassOrderViolation {
    pub pass: &'static str,
    pub missing_requirement: &'static str,
    pub reason: ViolationReason, // NeverEstablished | EstablishedThenInvalidated { invalidated_by: &'static str }
}

pub struct PassManager {
    passes: HashMap<&'static str, PassInfo>,
}

impl PassManager {
    /// Walk `order`, tracking a live set of established properties. At each
    /// step: every `requires` entry must be in the live set (else
    /// `NeverEstablished` or `EstablishedThenInvalidated`, depending on
    /// whether it WAS live earlier and got dropped). Then update the live
    /// set: drop everything not in this pass's `preserves` (unless
    /// re-established by its own `writes`), add `writes`.
    pub fn validate_order(&self, order: &[&'static str]) -> Result<(), Vec<PassOrderViolation>> { ... }

    /// Stretch goal, not MVP (see §5.3): derive a valid order from
    /// dependencies alone. Necessary-but-not-sufficient — the hand-picked
    /// order in pipeline.rs may encode reasons beyond pure dependency
    /// correctness (e.g. "site-scale before block-scale" as a design
    /// choice, not a hard requirement), so a derived schedule is not
    /// guaranteed to match the intentional one even when both are valid.
    pub fn schedule(&self, goal_writes: &[&'static str]) -> Vec<&'static str> { ... }
}
```

Concretely wiring this in: `registry.rs`'s ~90-line prose ordering rationale and `pipeline.rs`'s ~80-line header become a `const PASSES: &[PassInfo]` table (one entry per operator, each with its own short `preserves`/`invalidates`-driven "why here" note, same information the prose has today, just structured) plus one new test:

```rust
#[test]
fn corrected_pipeline_order_is_valid() {
    let order = ["p37", "p52_path_network", "p29", "p61", "p95", "p108", "p96", "p107", "p127", "p129", "p131", "p221"];
    PASS_MANAGER.validate_order(&order).expect("pipeline order violates a pass dependency");
}
```

This is the exact bug class the current comments describe having found by hand (P95-before-P37 fragmentation; P108-after-P96/P107 seeing stale pad boundaries) — mechanically caught on every commit instead of caught the next time someone reorders the pipeline without reading both doc comments first.

### 5.3 Relationship to §1 (ECS)

`requires`/`writes` here and `System::reads`/`System::writes` in §1 are the same information. Ship §5 first against the current string-tag model (cheap, immediately useful, `requires`/`writes` entries are hand-written strings matching existing `spec`/`use_category` conventions) — then, if/when §1 lands, `PassInfo::requires`/`writes` should be *derived* from each `System`'s typed component signature instead of hand-maintained separately. Flagging this now so the string-based v1 isn't wasted work: it's the correct MVP, not a detour.

### 5.4 Relationship to §1's parallelism claim

This is where §1's "two systems with disjoint writes can run concurrently" claim gets its actual proof: two passes are safe to parallelize iff neither's `requires` intersects the other's `writes`, neither invalidates something the other reads, and their `writes` sets are disjoint. `PassManager` is the thing that checks this, not a hunch. (Restating §1.4's caveat: this proof licenses parallel *execution*, which is only actually exploitable where threads exist — native and CI, not the WASM browser deploy target without further infrastructure.)

### 5.5 Milestone

**Ships when:** `PASSES` covers all 14 current operators, `validate_order` passes against the real `run_corrected_pipeline_with_p37` sequence, and deliberately reordering two passes that genuinely conflict (e.g. moving P108 back to Alexander's literal numbering, after P107) makes the test fail with a specific, actionable `PassOrderViolation` rather than requiring someone to notice the resulting geometry looks wrong in a render.

---

## 7. Multi-objective steering loop: Pareto frontier + beam search

### 7.1 Motivation

`SPEC.md` §3.5's steering loop is pseudocode with two undefined functions and one structural mismatch:

```
state = NIR(initial_or_imagined)
for step in budget:
    deficit_opinions = opinions_with_lowest_outputs(state)
    candidate_patterns = patterns_that_might_address(deficit_opinions)
    sequence = choose_sequence(candidate_patterns, state)   # ← undefined
    new_state = apply_sequence(state, sequence)
    if all_equity_guards_held(new_state):
        state = new_state
    else:
        backtrack                                            # ← undefined
```

`choose_sequence` and `backtrack` are undefined because the pseudocode is silently a single-path greedy search (`state` is one value, not a set) over something `SPEC.md` §3.4/§6.1 explicitly refuses to collapse to a single number — the opinion chorus, by design, has no scalar aggregate. A greedy walk needs a scalar to be greedy *about*. That's not a missing implementation detail, it's a structural contradiction between the search strategy implied by the pseudocode and the "never collapse the chorus" rule the rest of the spec insists on. Optimization theory has a name for the version of this problem SPEC.md actually has (many objectives, no agreed weighting between them, want to preserve and present disagreement rather than resolve it): multi-objective search with an explicit Pareto frontier instead of a scalar objective.

### 7.2 Design

**State.** A candidate is not just a `Neighborhood` — it's a point in the DAG from §2, plus its evaluated opinion vector:

```rust
pub struct Candidate {
    pub id: NeighborhoodId,         // §2
    pub opinions: BTreeMap<String, f64>, // from street-smarts-opinions::evaluate_all, NOT reduced
    pub trajectory: Vec<&'static str>,   // pass ids applied since the root, for the ledger
}
```

**Dominance, not ranking.** `A` dominates `B` iff `A`'s value is ≥ `B`'s on every shared opinion axis and strictly greater on at least one. This is a partial order, matching the chorus's actual structure — most pairs of candidates will be mutually non-dominating (each better on some axis, worse on another), which is not a bug in the search, it's the disagreement the whole system exists to surface.

```rust
pub struct ParetoSet {
    frontier: Vec<Candidate>,
}
impl ParetoSet {
    /// Insert `c`. If something already in the frontier dominates `c`, it's
    /// rejected. Otherwise `c` is added and anything IT dominates is pruned.
    pub fn insert(&mut self, c: Candidate) -> bool { ... }
}
```

**Dimensionality caveat — do not Pareto-rank on 50+ raw axes.** With 15 geometric properties, ~40 pattern-presence opinions, and several activist axes, a literal Pareto frontier over every raw opinion value will be enormous and useless — in high dimensions almost nothing dominates almost anything, so "the frontier" degenerates toward "everything survives." Use `street-smarts-opinions::registry::group_by_family` (already exists) to collapse to a handful of *composite* axes for frontier purposes — one per `OpinionFamily` (Geometric, Pattern; Activist stays a categorical guard, never a frontier axis, per §6.4 below) — 3–4 dimensions, not 50+. This is not just a tractability hack: "this option is stronger on the geometric properties, that one is stronger on pattern presence" is a legible question for a human browsing the frontier; "this option beats that one by 0.03 on raw axis 37 of 52" is not, and would defeat the entire point of surfacing disagreement legibly.

**Equity guards stay categorical, untouched.** `SPEC.md` §3.5/§6.4 is explicit and this proposal changes nothing about it: guards filter candidates out *before* they're ever offered to `ParetoSet::insert` at all. Optimization theory governs ranking among guard-passing candidates; it has no vote on the guards themselves. Restating this because it's the one part of §3.5's pseudocode that isn't actually broken and shouldn't be touched.

**Search strategy: beam search + tabu, replacing the undefined `choose_sequence`/`backtrack`.**

```rust
pub fn steer(
    root: NeighborhoodId,
    budget: usize,
    beam_width: usize,
    seed: u64,
    history: &mut impl HistoryStore,      // §2
) -> ParetoSet {
    let mut frontier = ParetoSet::from(vec![Candidate::root(root)]);
    let mut tabu: LruCache<(NeighborhoodId, &'static str), ()> = LruCache::new(1024);

    for step in 0..budget {
        let mut next = Vec::new();
        for candidate in frontier.top_k(beam_width) {          // beam, not one path
            let deficits = opinions_with_lowest_outputs(&candidate.opinions);
            for pass in patterns_that_might_address(&deficits) {
                if tabu.contains(&(candidate.id, pass.id)) { continue; }
                let child_seed = derive_seed(seed, step, candidate.id, pass.id); // deterministic, reproducible
                match history.get_or_compute(candidate.id, pass, &pass.default_params_json(), child_seed) {
                    Ok(child_id) if equity_guards_hold(&child_id) => {
                        next.push(Candidate::evaluate(child_id, &candidate.trajectory, pass.id));
                    }
                    _ => { tabu.insert((candidate.id, pass.id), ()); }
                }
            }
        }
        for c in next { frontier.insert(c); }
    }
    frontier
}
```

**Output: the frontier, not a winner.** `steer()` returns a set of mutually non-dominated proposals — this is a direct upgrade to what `SPEC.md` §5.1 already sketches in the UI ("Buttons: ... 'make your own' ...") from "one generated proposal" to "here are the live, genuinely different options, go argue about which one you want" — which is the actual product `SPEC.md` §9 describes, not a side effect of the algorithm choice.

**Determinism.** `derive_seed` must be a pure function of `(root seed, step, parent candidate id, pass id)` — same convention the existing per-block loop already uses (`seed + block_index + 1`), generalized. This is required for `SubdivisionTrace.params`'s ledger-citability promise ("coalition can cite the exact parameters that produced a specific proposal") to hold for search-produced candidates, not just single hand-invoked operator calls.

### 7.3 Risks

- Beam width and budget are real tuning knobs with no principled starting value yet — start conservative (small beam, small budget) and measure against the Eastside Commons fixture before widening.
- The composite-axis collapse in §7.2 is itself a design decision with real consequences (which opinions get grouped together shapes what "the frontier" looks like) — should be reviewed explicitly, not left as an implementation detail nobody signed off on.
- This entire proposal is close to useless without §2 (repeated re-evaluation of near-identical candidate states, with no memoization, is the dominant cost) — sequencing matters here more than for the other four.

### 7.4 Milestone

**Ships when:** `steer()` run against the Eastside Commons baseline for a small budget (e.g. 3 steps, beam width 3) produces a frontier with more than one candidate (proving the search isn't secretly degenerating back to a single greedy path), every candidate's trajectory replays deterministically from its recorded seed, and the UI's existing "browse variants" affordance can enumerate the frontier instead of showing one generated proposal.

---

## 8. Suggested build order across all five

1. **§5 (pass manager)** — cheapest, zero dependency on the others, immediately replaces the highest-risk prose (the ordering rationale most likely to silently drift from the actual code).
2. **§2 (content-addressed history)** — independent of §1/§4/§5, unlocks caching immediately, and gives `street-smarts-ledger`'s stub a concrete foundation instead of starting from nothing.
3. **§4 v1 (scoped types, against the current string-tag model)** — cheap once the `Scope` taxonomy is named (which §5's `PassInfo.scale` field basically forces you to do anyway).
4. **§1 (ECS)** — the real migration. Do this once §5 exists as a regression check ("does the pipeline still validate the same way after the data model changed") and §2 exists as a correctness check ("does the migrated pipeline produce byte-identical output for the existing fixtures").
5. **§7 (multi-objective steering loop)** — build last; it's the payoff that makes the most sense once §2 (memoization) is already load-bearing infrastructure rather than a nice-to-have.

§4 gets a "v2, simplified" pass after §1 lands, per §4.3's own sequencing note.
