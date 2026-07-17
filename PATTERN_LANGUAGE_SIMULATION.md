# Simulating *The Timeless Way of Building* in street-smarts

**Status:** proposal, not yet implemented.
**Author:** Claude (for Jason), 2026-07-17.
**Source:** Christopher Alexander, *The Timeless Way of Building* (Oxford University Press, 1979) — volume 1 of the trilogy that continues with *A Pattern Language* (already street-smarts' primary source) and *The Oregon Experiment*. Read in full outline via its own detailed table of contents (27 chapters, each opening with the author's one-sentence chapter thesis) plus targeted reading of chapters 1, 2, 9, 14–17, and 18–21 for the argument's mechanics. Not redistributed here or anywhere in this repo — same policy the README already states for *A Pattern Language*'s full text. Anyone implementing this proposal should read the source directly; this document paraphrases the ideas, it does not stand in for them.

---

## 1. Why this book, and why now

Street-smarts already builds a *simulation* of Alexander's system — `street-smarts-patterns` runs numbered pattern operators (P37, P95, P107...) in something close to Alexander's own sequence, and `street-smarts-opinions` scores the result against his named properties. What it simulates today is mostly *A Pattern Language*: the 253-pattern catalog and their forces.

*The Timeless Way* is the theory that makes the catalog work. It is not a second catalog — it has no pattern numbers of its own. It is Alexander's argument for *why* a pattern language is the right unit of design at all, *what makes a pattern real* (as opposed to a plausible-sounding rule someone made up), and *what disciplined process* turns a shared language into a living building instead of a Frankenstein assembly of parts. Three ideas from it map onto street-smarts' own software architecture almost without translation:

1. **The quality without a name (QWAN)** — the thing a building or town has when it's alive — can't be measured directly, only triangulated by partial, disagreeing views and confirmed by how it makes people feel. Street-smarts' `Opinion` protocol (`street-smarts-core/src/opinion.rs`) and the conflict-engine framing in `SPEC.md` already refuse to output a single score for exactly this reason. This document doesn't need to invent that idea — it needs to point out street-smarts already built it, on purpose or not, and say where it's still incomplete.
2. **A pattern language is a *graph*, not a list.** Chapter 16 ("The structure of a language") is explicit that what makes a set of patterns a *language*, and not just a pile of good ideas, is the network of "this pattern needs that smaller one to complete it" connections between them. Street-smarts currently encodes that graph as prose — two multi-hundred-word doc comments in `pipeline.rs` and `registry.rs` narrating why P29 runs out of Alexander's own order, why P108 runs before P96/P107, etc. That prose is correct and carefully reasoned. It is also not a data structure, so nothing checks it, and every new pattern author has to re-derive the ordering rules by reading two files top to bottom.
3. **A pattern only earns a place in the language once it passes a reality test** (chapters 14–15, "Patterns which can be shared" / "The reality of patterns"): does resolving the forces it names actually produce the well-being it claims to, checked against real cases, not just argued for. Street-smarts has the mechanism for this test built (`Opinion`, `OpinionOutput`, pattern-presence detectors in `street-smarts-opinions/src/pattern/`) but applies it inconsistently: of the 14 pattern *generators* in `street-smarts-patterns`, only 4 have a matching *detector* opinion that checks whether the thing they claim to produce is actually present in the output (§4.2 below has the exact list).

The rest of this document turns those three mappings into concrete engineering work, organized around the two goals Jason asked for: **development speed** (§3, via shared pattern primitives) and **accuracy** (§4, via test and architecture discipline).

---

## 2. The theoretical mapping, in one table

| *Timeless Way* concept | Where street-smarts already has it | What's missing |
|---|---|---|
| Quality without a name — real but unnameable, recognized not measured (ch. 2–8) | `Opinion` protocol has no `confidence` field on purpose; `SPEC.md` §3.4 makes disagreement the primary output, not a score | The decision ledger (`street-smarts-ledger`) that would let *humans* confirm or deny QWAN is still a stub — see §5 |
| A pattern language is a network of "needs"/"completes" relationships between patterns, not a flat list (ch. 16) | The real ordering lives in prose in `pipeline.rs`/`registry.rs` doc comments | No machine-checkable graph; nothing fails CI if the prose and the code drift apart |
| Patterns are validated empirically, repeatedly, against real cases — not by argument alone (ch. 14–15) | `street-smarts-opinions/src/pattern/*` is exactly this mechanism | Only 4 of 14 generator operators have a paired detector (§4.2) |
| Building proceeds by *unfolding* — one pattern differentiates the whole further, never assembling pre-made parts (ch. 19–21) | `Subdivision` + `apply_subdivision` is structurally a diff/patch model, not a rebuild-from-parts model — already the right shape | No test enforces that an operator's diff stays inside its declared scope (a P61 that quietly rewrote an unrelated block's parcels would currently pass) |
| The language is a shared *seed*; millions of local, independently-seeded acts under one grammar generate the whole without central control (ch. 18) | The per-block loop in `run_corrected_pipeline_with_p37` (derived seed `seed + block_index + 1`, tolerant skip-on-failure) already does this | The loop body is copy-pasted logic, not a named, reusable combinator — costs time on the next block-scale pattern |
| A pattern's identity includes the *forces* it resolves, not just its geometry (ch. 14) | Every pattern module's doc comment states forces informally, in prose, well | Forces aren't structured data, so nothing can query "which patterns respond to a deficit in daylight" the way `SPEC.md`'s steering loop (§3.5) needs to |

---

## 3. Development speed: common pattern primitives

The claim from *Timeless Way* worth taking literally: a language is what lets an ordinary person generate an unbounded variety of buildings from a *small, shared, reusable vocabulary* — the same way ordinary grammar lets someone generate sentences they've never heard. The failure mode it warns against (ch. 12–13, "the creative power of language" / "the breakdown of language") is a vocabulary that's grown too large, too ad hoc, and too undocumented for any one person to hold in their head — at which point every new act of building has to reinvent basics from scratch. That is a precise description of what happens to a codebase's "add pattern #15" cost if every operator keeps hand-rolling the same primitives.

Four concrete extractions, each targeting a specific duplication that exists today:

### 3.1 A typed scope/selector primitive, replacing stringly-typed filters

Today, operators select their targets by ad hoc string matching baked into each file: `spec.as_deref().unwrap_or("").starts_with("BLOCK_")` (pipeline.rs, block selection), `use_category == "p95_building_pad"` (P107's filter, per its module doc). This works but it's untyped — a typo in a prefix string fails silently (empty selection, not a compile error or a loud runtime error), and every new operator has to know the informal string-tagging convention by reading other operators' source rather than a schema.

Proposal: a small `Scope` enum in `street-smarts-core` (`Block`, `BuildingPad`, `Building(BuildingKind)`, `OpenSpace(OpenSpaceKind)`, `All`) plus a `Neighborhood::select(&self, scope: Scope) -> impl Iterator<Item = &Parcel>` (and building/open-space equivalents). Existing `spec`/`use_category` string fields become the *serialization* of `Scope`, not its only representation — `Scope` round-trips through the same strings so NIR JSON stays unchanged, but new operator code writes `nbhd.select(Scope::Block)` instead of re-deriving the string convention. This is the single highest-leverage change for onboarding speed: every pattern-operator file currently spends its first ~10 lines re-deriving "how do I find the parcels I care about," and every one of those 10-line blocks is a place a new pattern can subtly get scope wrong.

### 3.2 A `run_per_block` combinator for the site→block→pattern fan-out

`run_corrected_pipeline_with_p37` (pipeline.rs:167–182) implements: derive a per-block seed, run an operator, tolerate a failing block by skipping it rather than aborting the run, fold the result back with `apply_subdivision`. This loop shape is Alexander's ch. 18 "genetic power of language" made literal — decentralized, independently-seeded local acts under one shared rule — and it is exactly the shape any future block-scale pattern (a P29 successor, a hypothetical density-responsive pattern) will need again. Extract it once:

```rust
pub fn run_per_block<P, F>(
    nbhd: &Neighborhood,
    block_ids: &[String],
    base_seed: u64,
    mut f: F,
) -> Neighborhood
where
    F: FnMut(&Neighborhood, &str, u64) -> Result<Subdivision, String>,
{ /* seed derivation, tolerant apply, fold — written once */ }
```

`P61`+`P95`'s current per-block body becomes one call to this; the next block-scale pattern gets it for free instead of copy-pasting the loop and its seed-derivation convention (`seed + block_index + 1`, currently only correct because every call site remembers to do it the same way).

### 3.3 A `Parameters` derive macro

`parameters.rs` is a clean, small trait (`schema`, `defaults`, `as_vector`, `from_vector`, `as_map`) — but every operator currently hand-writes all five methods for its own `Params` struct, field by field, in the same mechanical way every time (see any `P*Params` struct today). This is exactly the kind of boilerplate a `#[derive(Parameters)]` proc macro removes, reading `#[param(min = ..., max = ..., default = ..., unit = "...")]` field attributes and generating the trait impl. Net effect: a new pattern's parameter set goes from ~40 lines of repetitive trait-method code to ~10 lines of field annotations. Low risk (purely additive, opt-in per struct, doesn't touch the trait's public shape) and the single biggest per-pattern line-count reduction available.

### 3.4 The pattern-language graph as data (also §4.1 below)

Filed under both goals deliberately — see §4.1 for the accuracy argument. The speed argument: today, writing pattern #15 requires reading two files' worth of prose (`pipeline.rs`'s ~80-line header, `registry.rs`'s ~90-line header) to figure out where in the sequence it belongs and why. A queryable graph — "what does pattern X require to already exist, what does it complete, what would break if it ran before/after Y" — turns that into a lookup instead of a read-and-infer exercise. It is the machine-checkable version of what Alexander calls a pattern's position in the language.

---

## 4. Accuracy: test and architecture discipline

*Timeless Way*'s core methodological claim (ch. 14–15) is that a pattern earns real status only by being checked against many real cases, repeatedly — not by sounding right once. Applied to software: a generator operator's claim ("this produces Wings of Light," "this produces an Intimacy Gradient") is exactly the kind of claim that needs a matching, independent check, or it's just an assertion in a doc comment.

### 4.1 Make the pattern-language graph a real, validated data structure

Add `crates/street-smarts-patterns/src/language_graph.rs`:

```rust
pub struct PatternNode {
    pub id: &'static str,              // "p107"
    pub alexander_number: u32,         // 107
    pub scale: Scale,                  // Site | Block | Building
    pub requires: &'static [&'static str],  // must already exist in the neighborhood
    pub completes: &'static [&'static str], // patterns this one is a component of
}
pub const LANGUAGE: &[PatternNode] = &[ /* one row per operator */ ];
```

Then: (a) a unit test that topologically sorts `LANGUAGE` and asserts the sort order is compatible with the actual call order in `run_corrected_pipeline_with_p37` — this is the check that currently doesn't exist, and is precisely the thing that would have caught, mechanically, the exact class of bug the current prose comments describe having found and fixed by hand (P95-before-P37 fragmentation, P108-after-P96/P107 ordering, etc.); (b) `registry.rs`'s long ordering-rationale comment becomes generated documentation from `LANGUAGE`'s data plus each node's own one-line "why here" string, so the explanation and the enforced rule can't drift apart the way free-standing prose can.

### 4.2 Pair every generator with a detector — close the 10-pattern gap

Concretely, today:

| Generator (`street-smarts-patterns`) | Paired detector (`street-smarts-opinions/src/pattern/`) |
|---|---|
| P95 Building Complex | ✅ `p95_building_complex.rs` |
| P96 Number of Stories / P21 Four-Story Limit | ✅ `p21_four_story_limit.rs` |
| P107 Wings of Light (via P159's daylight claim) | ✅ `p159_light_on_two_sides.rs` |
| (positive-space claim, cross-cutting) | ✅ `p106_positive_outdoor_space.rs` |
| P29 Density Rings | ❌ |
| P37 House Cluster | ❌ |
| P61 Small Public Squares | ❌ |
| P108 Connected Buildings | ❌ |
| P127 Intimacy Gradient | ❌ |
| P129 Common Areas at the Heart | ❌ |
| P131 The Flow Through Rooms | ❌ |
| P221 Natural Doors and Windows | ❌ |

Every generator without a paired detector is a pattern whose claim to be "real," in Alexander's exact sense, is currently untested against its own output — the code asserts it produces the pattern; nothing checks that it did. Closing this is the highest-value accuracy work in this proposal, and it's incremental: each new detector is one small `Opinion` implementation (the crate already has four working examples to copy the shape of) plus one round-trip test:

```rust
#[test]
fn p37_generates_detectable_house_clusters() {
    let nbhd = run_p37(&fixture, seed);
    let opinion = P37HouseClusterOpinion; // new
    match opinion.evaluate(&nbhd) {
        OpinionOutput::Value { value, .. } => assert!(value > THRESHOLD),
        OpinionOutput::NoView { reason, .. } => panic!("detector had no view: {reason}"),
    }
}
```

This is a direct implementation of Alexander's reality test as CI: generate, then independently check the generated thing actually exhibits the property it was generated to have, on every commit, across the existing fixture set — not once, by a person eyeballing a render.

### 4.3 Scope-containment tests for `Subdivision`

*Timeless Way* ch. 19–21's "unfolding, one pattern at a time" claim, translated: each step should differentiate what's already there without silently disturbing unrelated parts of the whole. `Subdivision`'s diff/patch shape (`new_*`, `replaced_*_ids`) already matches this — but nothing currently checks that an operator's returned `Subdivision` only touches entities inside the scope it was invoked on. A property test worth adding to `street-smarts-patterns/tests/`: for every operator, run it on `parcel_id = X`, and assert every ID in `replaced_parcel_ids`/`replaced_open_space_ids`/`replaced_building_ids` was either `X` itself or an entity `select()`-reachable from `X`'s scope (§3.1 makes this assertion cheap to write once `Scope` exists — it's a generic combinator, not per-operator bespoke code). This is the mechanical version of Alexander's insistence that a pattern completes the whole rather than fighting parts of it that aren't its concern.

### 4.4 Property-based fuzzing per pattern, generalized

Existing tests (`tests/p37_house_cluster.rs`, `tests/p61_small_public_squares.rs`, etc.) each hand-assert one seed's output looks right. *Timeless Way* ch. 15's actual bar is higher: real across *many* cases, not one. Generalize the existing hand-written assertions into a reusable `assert_pattern_invariant(operator, fixture_set, seeds, invariant_fn)` harness in a new `street-smarts-patterns/tests/common/mod.rs`, then run each pattern's already-known invariant (P37's cluster-size bounds, P61's few-not-many square count, P96's four-story cap) across N seeds × the existing fixture set instead of one hardcoded seed. Cheap to add per-pattern once the harness exists; catches seed-dependent regressions the current single-seed tests structurally cannot.

### 4.5 The honest limit: none of this tests QWAN itself

Worth stating plainly, because *Timeless Way* is explicit that this is exactly the trap to avoid (ch. 2, "the quality without a name is precise, but cannot be named" — a warning against mistaking a rule-following check for the thing itself): §4.1–4.4 all test that a generator did what it *claims* to do, and that a mechanical process was followed correctly. None of them test whether the *result is alive* — whether it actually has the quality without a name. Alexander's own answer to that is not a better algorithm, it's disciplined recourse to human judgment against real, felt experience. Street-smarts' architecture already has a place for exactly this (`street-smarts-ledger`'s `opinion_offered` / `disagreement_resolved` events, `SPEC.md` §3.6's "the only accurate part of the system"), but the ledger crate is a stub — see §5. This document's test proposals close the "did the mechanism work" gap; only the ledger, once built, closes the "does anyone actually feel this is alive" gap, and the two should never be conflated in how results get reported.

---

## 5. What this doesn't propose

Consistent with `README.md`'s "what this is not" and `SPEC.md`'s equity-guard framing:

- **Not** a numeric "aliveness score." Every artifact above stays inside the existing `Opinion`/disagreement framing — a detector opinion is one more cited voice in the chorus, not an oracle.
- **Not** a new pattern catalog. This is theory-of-process work applied to the *existing* 14 operators and their existing 11-pattern-language sequence; it adds zero new Alexander patterns.
- **Not** a replacement for `street-smarts-ledger`. §4.5 is explicit that the test-discipline work here cannot substitute for the human-judgment loop the ledger is meant to capture; building the ledger stays separately scoped, unchanged by this document.
- **Not** a rewrite. `Subdivision`, `PatternOperator`/`DynOperator`, and `Opinion` are all already the right shape (per the mapping in §2) — every proposal above is additive (a new module, a new derive, a new test harness) and none requires touching those three interfaces' public contracts.

---

## 6. Suggested sequencing

Roughly cheapest/highest-leverage first, each independently shippable:

1. `Scope` primitive (§3.1) — unblocks §4.3's containment tests and simplifies every future operator.
2. `Parameters` derive macro (§3.3) — pure boilerplate removal, no behavior change, easy to review.
3. Language graph as data (§4.1) — makes the existing prose-documented ordering checkable; low risk, high documentation value.
4. Detector opinions for the 8 ungapped patterns (§4.2) — the single highest-value accuracy item, doable incrementally, one pattern at a time (fittingly).
5. `run_per_block` combinator (§3.2) and generalized fuzz harness (§4.4) — smaller, do whenever the next block-scale pattern or the next flaky-seed bug makes the case for them concrete.

---

*This document was produced by reading Alexander's own detailed table of contents and select chapters of* The Timeless Way of Building*, and by reading the street-smarts codebase directly (`crates/street-smarts-patterns`, `crates/street-smarts-core`, `crates/street-smarts-opinions`, `SPEC.md`, `README.md`) as of 2026-07-17. It contains no invented facts about the book's content beyond its published table of contents and chapter structure, and every codebase claim above (file paths, gap counts, trait shapes) was verified against the actual source, not inferred.*
