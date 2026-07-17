# Implementation plan: pattern-language, primitives, and hardening work

**Status:** proposal, not yet implemented.
**Author:** Claude (for Jason), 2026-07-17.
**Scope:** sequences every concrete work item from `PATTERN_LANGUAGE_SIMULATION.md`, `PRIMITIVES_SPEC.md`, and `HARDENING_SPEC.md` into one reasonable build order. Not an optimized schedule — a sensible one. No calendar dates; relative sizing only (S = hours-to-a-day, M = a few days, L = one-to-two weeks, XL = a real migration, measured in weeks not days). Adjust as reality intervenes.

Chorus calibration's full build (`HARDENING_SPEC.md` §6) is explicitly **not** scheduled here — it depends on the VLM opinion family, which none of the three source documents proposed building; `SPEC.md` §5.2 defers that to v0.2+. Its one actionable, VLM-independent piece is folded into Phase 0 below.

---

## Why this order

Four judgment calls, stated up front so the ordering doesn't look arbitrary:

1. **Cheap, zero-dependency wins go first**, regardless of which source document they came from — no reason to sequence a one-line fix behind a multi-week migration just because it was mentioned in a later conversation.
2. **Close correctness gaps in the system that already exists (the 14 shipped pattern operators, the 7 shipped opinions) before making architecture bets.** The detector-opinion gap, the predicate-consistency gap, and the fuzzing-coverage gap are all bugs-in-waiting in code that's live today. The ECS/history/steering-loop work is valuable but speculative by comparison — it makes the *next* system better, not the current one safer.
3. **Where a later document's proposal strictly supersedes an earlier one** (the pass-manager's `requires`/`writes`/`preserves`/`invalidates` supersedes the plain `requires`/`completes` graph; `ScopedView` compile-time containment supersedes the runtime containment test) **, build the cheap version first and upgrade it in place later**, rather than either skipping straight to the expensive version or building both separately. This is explicit below wherever it applies — look for "upgrades Phase N's ___."
4. **Of the two genuinely large architecture bets (content-addressed history, ECS), do history first.** It's independent of ECS, it's lower-risk (it doesn't touch every pattern operator's signature the way ECS does), and it gives the still-stubbed `street-smarts-ledger` crate a concrete foundation immediately — value on its own, not just as a stepping stone.

---

## Phase 0 — Immediate, no design required

Zero dependencies on anything else in this plan. Do these regardless of what's decided about the rest.

| Item | Source | Size |
|---|---|---|
| Investigate `wasm-opt = false` in `street-smarts-web/Cargo.toml`; re-enable with `-Oz` (or confirm and document a real reason it's off) | `HARDENING_SPEC.md`, top callout | S |
| Reserve `OpinionOutput::model_version: Option<String>` field, unused for now | `HARDENING_SPEC.md` §6.3 | S |

**Exit criteria:** the real, current WASM gzip size is known and trustworthy (needed as the seed value for Phase 2's bundle budget) — either because `wasm-opt` got turned back on, or because there's a documented reason it stays off.

---

## Phase 1 — Foundational primitives (dev-speed quick wins)

All four items are additive, don't touch any existing operator's behavior, and don't depend on each other except where noted. This phase is what makes every later phase faster to build.

| Item | Source | Size | Notes |
|---|---|---|---|
| `Parameters` derive macro | `PATTERN_LANGUAGE_SIMULATION.md` §3.3 | M | Pure boilerplate removal; do this before Phase 2's new detector opinions so they're written against the derive, not by hand. |
| `run_per_block` combinator | `PATTERN_LANGUAGE_SIMULATION.md` §3.2 | S | Extracted from the existing P61/P95 loop in `pipeline.rs`; no behavior change. |
| Name the `Scope` taxonomy + a plain `Neighborhood::select(scope)` helper | `PATTERN_LANGUAGE_SIMULATION.md` §3.1 | M | Foundational — reused by Phase 2's containment test, Phase 5's `ScopedView`, and eventually ECS's component queries. Build once here as a plain enum + filter function against the *existing* `Vec<Parcel>` model; do not wait for ECS. |
| Simple pattern-language graph: `requires`/`completes` only, plus a `validate_order` test | `PATTERN_LANGUAGE_SIMULATION.md` §3.4/§4.1 | M | Replaces the ~170 combined lines of ordering prose in `pipeline.rs`/`registry.rs`. **Deliberately the cheap version** — gets upgraded to the full pass-manager relation (`preserves`/`invalidates`) in Phase 5, not rebuilt from scratch, just extended with two more fields once that phase is already touching every operator. |
| Capability-typed opinions: `Opinion::capabilities()` + a `vlm` Cargo feature wall | `HARDENING_SPEC.md` §3 | S | Cheap, zero migration cost for the 7 existing opinions. Doing this now, before any VLM code exists, is the entire point — it's much harder to retrofit after a network client is already in the dependency graph. |

**Exit criteria:** `registry.rs`'s and `pipeline.rs`'s prose ordering comments are replaced by a data table + a passing `validate_order` test; every existing pattern operator's `Params` struct uses the derive macro; `street-smarts-web`'s build is structurally incapable of linking a `Capability::Network` opinion.

---

## Phase 2 — Close the accuracy gap in the shipped system

The highest-value phase for correctness, and the one most independent of everything else — none of this requires Phase 1 to be fully done first (only the detector opinions benefit from the Phase 1 derive macro; the rest can start in parallel).

| Item | Source | Size | Notes |
|---|---|---|---|
| Detector opinions for the 8 generators currently missing one (P29, P37, P61, P108, P127, P129, P131, P221) | `PATTERN_LANGUAGE_SIMULATION.md` §4.2 | L (one S-sized opinion × 8) | Ship incrementally, one pattern at a time, in any order — each is independent and low-risk. Don't block this on any other phase. |
| Scope-containment property test (runtime, using Phase 1's `Scope` taxonomy) | `PATTERN_LANGUAGE_SIMULATION.md` §4.3 | S | **Deliberately the cheap version** — upgraded to compile-time `ScopedView` containment in Phase 5. |
| Generalized seed-variance fuzz harness (`assert_pattern_invariant`) | `PATTERN_LANGUAGE_SIMULATION.md` §4.4 | M | Runs against the existing Eastside Commons fixture initially; gets its second axis (shape variance) from this same phase's synthetic-fixture item below. |
| Robust predicates, Tier 1 (centralize into `predicates.rs`, one shared epsilon) + instrument the pipeline's skip-tolerance with rejection reasons | `HARDENING_SPEC.md` §1.2 (Tier 1 only), §1.3 | M | Tier 2 (adaptive-precision arithmetic) is explicitly **not** scheduled here — only escalate to it if Tier 1's own tests still show inconsistency once synthetic fixtures (next row) are exercising it. |
| Procedural/synthetic fixture generator (aspect ratio, concavity, area, density, vertex count axes) | `HARDENING_SPEC.md` §5 | L | Deliberately paired with the predicates row above — this is what would actually trigger the degenerate-geometry bug class Tier 1 is defending against. Feeds the fuzz harness's shape axis once both exist. |
| WASM bundle-size budget in CI, replacing the measure-only step | `HARDENING_SPEC.md` §2 | S | Seed the budget from Phase 0's now-trustworthy size number. |

**Exit criteria:** all 14 pattern operators have a paired detector opinion; the fuzz harness runs real + synthetic fixtures across many seeds in CI; a synthetic fixture has found at least one real bug the single real fixture never exercised (the actual proof this phase was worth it, not a coverage-percentage vanity metric); CI fails a PR that regresses the WASM bundle past budget.

---

## Phase 3 — Visual regression (parallelizable with Phase 2)

Small, self-contained, gated on its own prerequisite check rather than on anything else in this plan. Can genuinely run alongside Phase 2 if there's bandwidth for both — listed as its own phase only because the determinism check might turn up more work than expected and shouldn't block the accuracy-gap work above.

| Item | Source | Size | Notes |
|---|---|---|---|
| Confirm `render.py`'s cadquery/OpenCascade output is deterministic (same input → same hash, checked on the actual CI runner image) | `HARDENING_SPEC.md` §4.2 | S | Prerequisite — if this fails, fix determinism before building anything on top of it. |
| Perceptual-hash regression check wired into `vibe-render.yml`, against the 4 scenarios `deploy.yml` already names | `HARDENING_SPEC.md` §4.2 | M | |
| `scripts/update-vibe-baseline.sh` re-baselining script | `HARDENING_SPEC.md` §4.2 | S | Without this, the gate rots into something people route around. |

**Exit criteria:** a deliberately-broken test PR fails the perceptual-hash check; the re-baseline script has been exercised at least once on a real, intentional rendering change.

---

## Phase 4 — Content-addressed history

The first of the two big architecture bets. Independent of Phase 5 (ECS) — build this one first per the reasoning in "Why this order" above.

| Item | Source | Size | Notes |
|---|---|---|---|
| `street-smarts-history` (or built directly inside `street-smarts-ledger`'s currently-stubbed crate — recommended, see rationale in `PRIMITIVES_SPEC.md` §2.5): `blake3`-hashed commits, `Subdivision`-as-patch storage, LRU-cached materialization | `PRIMITIVES_SPEC.md` §2.2 | XL | |
| `algorithm_version` tagging on commits, so cache entries from a different code version are treated as misses, not hits | `PRIMITIVES_SPEC.md` §2.4 | S | |
| IndexedDB storage backend for `street-smarts-web` | `PRIMITIVES_SPEC.md` §2.2 | M | Reuses the caching mechanism `SPEC.md` §3.1 already commits to for adapter data — one more table, not new infrastructure. |
| Wire `run_corrected_pipeline_with_p37`'s 11 internal steps to record real commits instead of only narrating them in trace strings | `PRIMITIVES_SPEC.md` §2.6 | M | |

**Exit criteria:** `materialize()` reproduces byte-identical output to the current direct-call pipeline for both existing fixtures; re-running the same `(baseline, parcel_id, seed)` twice is a measurable cache hit on the second call.

---

## Phase 5 — Pass manager upgrade, scoped types, and ECS

The second, larger architecture bet. Grouped into one phase deliberately: these three are mutually reinforcing (the pass manager's `requires`/`writes` becomes *derived* from ECS component signatures instead of hand-maintained; `ScopedView`'s write-scope becomes a component query instead of a hand-written predicate), so doing them together avoids building a throwaway version of each in isolation.

| Item | Source | Size | Notes |
|---|---|---|---|
| Upgrade Phase 1's `requires`/`completes` graph to the full pass manager (`requires`/`writes`/`preserves`/`invalidates`) | `PRIMITIVES_SPEC.md` §5.2 | M | This is an *extension* of Phase 1's table, not a rewrite — two new fields per existing entry. |
| ECS substrate, Phase A: `World` as a read/write adapter over the existing `Neighborhood`, zero schema change | `PRIMITIVES_SPEC.md` §1.3 | L | |
| ECS substrate, Phase B: new typed components (`DensityTier`, `BlockMembership`, `PadRole`, etc.) as dual-written sidecars alongside the existing string fields | `PRIMITIVES_SPEC.md` §1.3 | XL | The real migration — touches every operator that currently reads `spec`/`use_category`/`density_tier`. Port one operator at a time; recommend starting with P29 Density Rings per `PRIMITIVES_SPEC.md` §1.5's own milestone suggestion. |
| ECS substrate, Phase C (optional): deprecate the shadow string fields, bump schema version | `PRIMITIVES_SPEC.md` §1.3 | L | Not committed to up front — decide once there's real experience with Phase B. |
| Upgrade Phase 2's runtime containment test to compile-time `ScopedView` (`ReadScope`/`WriteScope` split, generic write-validation) | `PRIMITIVES_SPEC.md` §4.2 | L | Now genuinely cheaper than it would have been pre-ECS, since write-scope is a component query rather than a hand-written string predicate. |

**Exit criteria:** `World::from_neighborhood`/`to_neighborhood` round-trip both fixtures byte-for-byte; at least P29 is fully ported to a `System`; every operator is migrated to `ScopedOperator`, and a deliberately-broken test operator that tries to write outside its declared scope fails loudly instead of corrupting an unrelated block; `PassInfo::requires`/`writes` for ported operators are derived from their component signatures, not hand-written strings.

---

## Phase 6 — Multi-objective steering loop

The payoff phase — this is what turns `SPEC.md` §3.5's undefined pseudocode (`choose_sequence`, `backtrack`) into something real, and it's genuinely not worth building before Phase 4 exists (repeated re-evaluation of near-identical candidates with no memoization is the dominant cost this phase would otherwise be fighting).

| Item | Source | Size | Notes |
|---|---|---|---|
| `Candidate`/`ParetoSet` types, family-level composite axes (not raw 50+-axis Pareto ranking) | `PRIMITIVES_SPEC.md` §7.2 | M | |
| Beam search + tabu list over the history store from Phase 4 | `PRIMITIVES_SPEC.md` §7.2 | L | |
| Deterministic per-step seed derivation | `PRIMITIVES_SPEC.md` §7.2 | S | Same convention as the existing per-block seed derivation, generalized. |
| Wire the returned frontier into the "browse variants" UI affordance `SPEC.md` §5.1 already sketches | `PRIMITIVES_SPEC.md` §7.4 | M | |

**Exit criteria:** `steer()` against the Eastside Commons baseline produces a frontier with more than one candidate (proving it isn't secretly degenerating to a single greedy path); every candidate's trajectory replays deterministically from its recorded seed; the UI can enumerate the frontier instead of showing one generated proposal.

---

## What's deliberately not in this plan

- **Robust predicates, Tier 2** (adaptive-precision arithmetic) — only build this if Phase 2's Tier 1 work, once exercised by synthetic fixtures, still shows measurable inconsistency. Don't build it speculatively.
- **Chorus calibration's full build** (`HARDENING_SPEC.md` §6, beyond the Phase 0 field reservation) — blocked on the VLM opinion family, which isn't part of this plan; `SPEC.md` §5.2 defers VLM to v0.2+ and none of the three source documents proposed building it.
- **ECS Phase C** (deprecating the shadow string fields) — intentionally left as a future decision, not committed to now.

---

## One-page summary

```
Phase 0  Immediate           wasm-opt, model_version field                    (no deps)
Phase 1  Foundations         Scope, Parameters derive, run_per_block,          (no deps)
                              simple pattern graph, capability typing
Phase 2  Accuracy            8 detector opinions, containment test, fuzz       (uses Phase 1's Scope)
                              harness, predicates Tier 1, synthetic fixtures,
                              bundle budget
Phase 3  Visual regression   determinism check, perceptual-hash gate           (parallel w/ Phase 2)
Phase 4  History              content-addressed commit DAG                     (independent of Phase 5)
Phase 5  ECS + pass manager  full pass manager, World/System, ScopedView       (upgrades Phase 1 & 2 work)
Phase 6  Steering loop       Pareto frontier + beam search                     (needs Phase 4, benefits from 5)
```
