# Pattern ordering audit: fields vs. individuals in the corrected pipeline

## 1. The question

`pipeline.rs`'s real execution order (`crates/street-smarts-patterns/src/pipeline.rs`,
also generated live into `public/index.html` via `language_graph::render_arrow_chain`)
is:

```
P37 → P52 → P29 → P61 → P95 → P108 → P96 → P107 → P124 → P117 → P197 →
P127 → P116 → P130 → P129 → P131 → P221 → P133 → P118 → P119 → P160
```

**Update, same session:** after item 1 (P29) and the three free reorders
(§6b) shipped, the real order is now

```
P29 → P37 → P52 → P61 → P95 → P108 → P96 → P107 → P117 → P124 → P127 →
P197 → P116 → P129 → P130 → P131 → P221 → P133 → P118 → P119 → P160
```

— P29 moved to its own true canonical position ahead of P37, and P117/P124,
P127/P197, P129/P130 each swapped to match ascending numbering at zero real
cost. See §6/§6b for the real implementation and what it actually
cost/changed.

This is **not** Alexander's own ascending pattern-number order. `registry.rs`'s own
doc comment already names the actual organizing principle: *"larger, more-fixed
patterns first, smaller ones nested inside what came before"* — Alexander's real
large-to-small scale ordering (Towns → Buildings → Construction, and largest-to-
smallest within each), not the literal 1–253 catalog numbering, which was never
meant to double as an execution order on its own.

The question raised in session: several of the deviations from ascending order look
less like "a smaller pattern legitimately nests inside a larger one" and more like
*larger, more general patterns being forced to wait for concrete output only a
smaller, later pattern produces* — i.e. the pipeline is over-concretizing early,
so a genuinely prior/larger pattern has nowhere to attach its own output until
something smaller has already been individuated. The suggested lens: Gilbert
Simondon's individuation theory, where a "pre-individual field" carries real
potential/structure before anything discrete crystallizes out of it, and premature
concretization forecloses that field instead of letting it be sampled later.

**Citation honesty note:** the Simondon framing below is a conceptual borrowing,
not a sourced quote — this repo has no verified Simondon text to cite against
(unlike every Alexander reference below, which is checked against this codebase's
own already-verified module-doc citations or `data/apl-pattern-graph.json`'s real
cross-reference data). Treat "field"/"individuation"/"pre-individual" here as
working vocabulary borrowed for this audit's own argument, not as claims about
Simondon's text.

## 2. Method

For every place the real execution order runs a lower Alexander number after a
higher one, this audit:

1. Re-read that operator's own module doc — every generator in this crate already
   cites real Alexander text (page numbers or patternlanguage.cc URLs) and states,
   in its own words, why it runs where it does. Nothing below is invented; it's a
   synthesis of what the code already says, checked against the actual `.rs` files.
2. Checked `data/apl-pattern-graph.json` — the real, programmatically-fetched
   253-pattern cross-reference graph — for a direct citation edge between the two
   patterns in each violated pair (i.e. does Alexander's own text actually connect
   these two patterns at all, or is the ordering purely an artifact of this
   pipeline's implementation).
3. Where the doc comments didn't settle the question, read the actual `apply()`
   logic to check what data each operator's computation genuinely touches.

Result: **none** of the ten violated pairs have a direct citation edge in
`apl-pattern-graph.json` — Alexander's own text never explicitly discusses any of
these ten pairs as related to each other. Every deviation below is purely an
artifact of this codebase's own implementation, not a documented disagreement with
Alexander. That makes the classification below entirely about *this pipeline's*
data-flow, not about correcting a misreading of the source text.

## 3. The ten deviations, classified

| # | Deviation (runs after, canonically before) | Class | Status |
|---|---|---|---|
| 1 | P52 → **P29** | **A. Premature individuation** | ✅ Fixed (§6) |
| 2 | P108 → **P96** | **C. Mixed** (partially A, partially genuine) | Open |
| 3 | P124 → **P117** | **D. Free reorder** | ✅ Fixed (§6b) |
| 4 | P197 → **P127** | **D. Free reorder** | ✅ Fixed (§6b) |
| 5 | P127 → **P116** | **A. Premature individuation** (see §4.5 — also self-critique) | Open |
| 6 | P130 → **P129** | **D. Free reorder** (admitted in the code itself) | ✅ Fixed (§6b) |
| 7 | P221 → **P133** | **B. Hoisted-derived-attribute bug** | Open |
| 8 | P133 → **P118** | **B. Hoisted-derived-attribute bug** | Open |
| 9 | P133 → **P119** | **B. Hoisted-derived-attribute bug** | Open |
| 10 | P133 → **P160** | **C. Genuine sequential dependency** | N/A — correct as-is |

- **A — Premature individuation (real field candidates).** The larger pattern's
  own computation doesn't actually need anything a smaller, later pattern
  produces — it's only sequenced late because the *schema* has nowhere to store
  its output except by tagging an already-individuated smaller entity.
- **B — Hoisted-derived-attribute bug.** The dependency is real today, but only
  because a cheap scalar derivation got bundled inside a much larger, unrelated
  operator instead of happening as soon as its own real inputs existed. Moving the
  derivation earlier removes the dependency entirely — no new primitive needed.
- **C — Genuine sequential/individuation dependency.** The later pattern's
  computation actually, unavoidably needs something concrete only the earlier
  (numerically larger) pattern produces. Not a bug; this is what real
  individuation looks like — some things genuinely can't be decided until
  something more specific has already emerged.
- **D — Free/arbitrary reorder.** No real dependency either way. Current order is
  implementation convenience; could be swapped to match Alexander's own sequence
  for free.

## 4. Detail

### 4.1 P52 → P29 (Class A)

`p29_density_rings.rs`'s own module doc, verbatim:

> Alexander's own numbering puts Density Rings (29) well before House Cluster
> (37) -- deciding the site's overall density gradient before carving individual
> clusters. This codebase's schema has no way to annotate undivided raw land with
> a zone/tier -- there's nothing to attach the tag to until real parcels exist.
> So this operator runs on P37's `BLOCK_n` children instead... This is a
> practical adaptation to what the schema can express, not a claim to match
> Alexander's literal sequencing.

The operator's own real computation (`crates/street-smarts-patterns/src/
p29_density_rings.rs`): a center point (area-weighted centroid, optionally shifted
by `eccentricity_frac`) plus a radial falloff from that center to
`core_target_stories`/`edge_target_stories`. **Every input to this computation is
available from the raw site polygon alone** — it needs no blocks, no pads, nothing
P37 produces. The only reason it currently runs after P37 is that `density_tier`/
`target_stories` are stored as fields on a `Parcel`, and there's no undivided-land
entity to hang them on yet.

This is the clean case: a real, closed-form field (`center`, `radius_m`, two
target-story endpoints, `eccentricity_frac`) computed once from raw geometry,
storable directly on `Neighborhood` rather than smuggled onto a `Parcel`, sampled
by P37 (or P95) at the moment a block/pad is actually individuated.

### 4.2 P108 → P96 (Class C, mixed)

`p96_number_of_stories.rs`: assigns `target_stories` to every `p95_building_pad`,
grouped by `density_tier` (P29's output — itself a field-sampling problem per
§4.1, once fixed). Two real sub-decisions:

- **Base tier assignment** — purely a function of which `density_tier` group a
  pad belongs to. Doesn't need P108's merge at all.
- **Which pads get the rare "tall exception"** — picked "largest-pad-first" (own
  doc: *"a taller building deserves a correspondingly larger footprint"*) and
  checked against `min_tall_spacing_m`. Confirmed by reading `p108_connected_
  buildings.rs`: merged pads keep the SAME `use_category: "p95_building_pad"` tag
  P96 filters on, so P96 genuinely sees a **different set of pad areas**
  depending on whether P108 has already run. This sub-decision is legitimately
  about which *specific, already-merged* footprints are large enough to deserve a
  tall exception — that's a real individuation question, not a field one.

So this pair is genuinely mixed: the base tier value could be attached as soon as
the density field exists (§4.1), but exception selection is correctly deferred
until pads have their final, merged shape.

### 4.3 P124 → P117 (Class D)

`p124_activity_pockets.rs` bumps building footprints outward toward a Plaza.
`p117_sheltering_roof.rs` assigns roof form purely from `height_m` — it never
reads footprint shape (`eave_height_m = height_m`, `ridge_height_m = height_m +
roof_rise_m`, full stop). `render.py`'s roof cap is drawn from whatever footprint
the `Building` carries **at render time**, not frozen at the point P117 ran, so
even the final rendered geometry is unaffected by which of these two runs first.
No real dependency either direction — free to reorder to `P117 → P124` (canonical)
at zero cost.

### 4.4 P197 → P127 (Class D)

Neither operator's own module doc claims a dependency on the other.
`p197_thick_walls.rs` sets a scalar `wall_thickness_m` capped relative to the
building's own bounding box; `p127_intimacy_gradient.rs` partitions the ground
floor into cells. Neither reads the other's output. `pipeline.rs`'s own step-11
rationale for running P197 late is about surviving every downstream *clone-and-
mutate* stage intact, not about needing to follow P127 specifically. Free to
reorder to `P127 → P197` (canonical) at zero cost.

### 4.5 P127 → P116 (Class A — including a self-critique)

`p116_cascade_of_roofs.rs` (added this session) genuinely needs P127's real
`interior_cells` (it reuses their polygons directly as the roof's wing partition)
and P117's whole-building `roof` (it interpolates `ridge_height_m` from it). As
built, this is a real, correct dependency — not a bug.

But it's worth naming honestly: **this is itself an instance of the pattern being
audited.** Alexander's own number for Cascade of Roofs is 116 — lower than both
117 (Sheltering Roof) and 127 (Intimacy Gradient), i.e. canonically a *larger,
prior* pattern, not a detail that waits on two smaller ones. Read that way, P116
isn't really "which room is biggest, so which roof segment should be tallest" (a
question that needs P127's finished room layout to already exist) — it's closer
to "which parts of this building's mass matter more" (a *general disposition*
about the building, not yet about any specific partition). That's exactly the
kind of thing a field would carry: a single "significance" gradient over the
footprint, computed once, right after massing exists (near P107), which BOTH
P127 (room depth) and P116 (roof ridge) could independently sample as two
separate individuations of the same underlying field — rather than P116 having to
wait on P127's finished individuation and reuse its exact polygons.

This session's P116 implementation is real and correctly dependency-ordered given
the current schema, and isn't being reverted here. It's flagged as the clearest
concrete second target once a field primitive exists (after P29), since the
"reuse someone else's already-individuated cells" move it makes is precisely the
workaround this audit is arguing against in the abstract.

### 4.6 P130 → P129 (Class D — already admitted in the code)

`p130_entrance_room.rs`'s own module doc, verbatim:

> Alexander's own cited sequence (127 -> 128 -> 129 -> 130 -> 131...) actually
> places 130 AFTER 129, but this operator never changes cell geometry or count...
> so nothing about P129's own center-of-gravity computation depends on whether the
> entrance cell has been relabeled yet. Running it here... keeps the two next to
> each other in the pipeline instead of splitting a tightly-coupled pair across
> P129.

No dependency claim at all — purely a locality preference in the source file.
Free to reorder to `P129 → P130` (canonical) at zero cost.

### 4.7 P221 → {P133, P118, P119} (Class B, all three)

`p133_staircase_as_a_stage.rs`'s own module doc, verbatim, on why it runs after
P221 rather than right after P131 (its canonical neighbor):

> `Building.floors` (this operator's own multi-story filter) isn't set by P96 --
> P96 only sets `target_stories` on the PARCEL/pad; `Building.floors` itself
> stays `None` until P221 derives a real story count from height... Running P133
> in Alexander's own position... was the first version's actual bug: every
> building's `floors` read `None` there, so the multi-story filter matched
> nothing.

Checked `p221_natural_doors_and_windows.rs` directly: the `floors` value is
computed at line 356 as `((height_m) / floor_to_floor_m).round()` — **a pure
function of `Building.height_m` alone**, computed and discarded internally before
P221 ever touches doors or windows. `pipeline.rs`'s own reasoning for P118 and
P119 running after P221 is the identical claim (*"needs real `Building.floors` to
rank by"* / *"Also needs real `Building.floors`"*) — same bug, three places.

`Building.height_m` is already final by the time P107 runs (step 8). There's no
reason `floors` has to wait for P221 at all — it could be derived directly from
`height_m` as soon as P107 assigns it (or even folded into P107/P96 itself), which
would let P133, P118, and P119 all run at whatever position their OWN real
dependencies actually require, independent of door/window placement entirely.
This is a real bug, not a design choice, by the original P133 author's own
admission — cheap to fix, no new primitive needed.

### 4.8 P133 → P160 (Class C — genuine dependency)

`pipeline.rs`'s own step 21: *"places a real wall niche flanking every real door
P221 already placed... Runs after P221 for the real door data to flank."* This is
not a floors problem — P160 reads P221's actual placed `Opening` geometry (which
specific bay, which wall edge) to decide where the niche goes. There is no way to
flank a door that doesn't exist yet; this is real, correct individuation-order
dependency, not a bug.

## 5. Recommendation

Two independent, differently-scoped fixes fall out of this audit, plus one
already-safe cleanup:

1. **`DensityField` primitive** (Class A, §4.1) — the cleanest, most isolated
   case. A small analytic struct on `Neighborhood`, computed from raw site
   geometry, sampled by P37/P95 at individuation time. Real schema addition,
   real operator restructuring, but self-contained. Natural proof of concept
   before deciding whether the same primitive generalizes to §4.5's roof/room
   "significance field" question.
2. **Hoist `floors` derivation out of P221** (Class B, §4.7) — cheap, mechanical,
   fixes three deviations at once, no new primitive. `floors = round(height_m /
   floor_to_floor_m)` moves to wherever `height_m` is finalized (P107, or a new
   tiny stage right after it); P133/P118/P119 each get re-evaluated for where
   their *remaining* real dependencies actually put them.
3. **Free reorders** (Class D, §§4.3/4.4/4.6) — zero-cost, whenever convenient:
   swap P117↔P124, P127↔P197, P129↔P130 to match Alexander's own sequence.

Classes B and C don't need a new primitive at all — B is a plain refactor, C is
correct as-is and just needs the ordering *rationale* to stop reading like an
apology. `PRIMITIVES_SPEC.md` §5's proposed pass-manager (`requires`/`writes`/
`preserves`/`invalidates`) is the right permanent home for recording all of this
once it ships — the `preserves`/`invalidates` relation is exactly the vocabulary
for stating "P108 invalidates P96's pad-area assumptions" (§4.2) precisely,
instead of prose. A `Field`/individuation distinction (this audit's actual
contribution) would need a fifth relation §5 doesn't have yet: a pass that
produces something *sampled*, not *consumed*, by everything downstream — worth
proposing as a `PassInfo.field_writes` addition to that spec once the P29
prototype (item 1) proves the pattern in real code.

## 6. Item 1, implemented: `DensityField`, P29 before P37

Shipped in the same session. What actually landed, and two real findings from
building it:

- **The primitive**: `street_smarts_core::nir::PatternField` (an enum, one
  variant per producing pattern) and `DensityField` (P29's own data: `center`,
  `radius_m`, `core_target_stories`, `edge_target_stories`, `n_rings`) — pure
  data in the schema crate, same split every other operator-specific NIR type
  uses (`RoofForm` is data, `p116_cascade_of_roofs` is the math). Named
  `PatternField`, not `Field` — `street_smarts_patterns::field` already
  defines an unrelated `Field` (a rasterized pressure grid for Voronoi seed
  placement), and `p37_house_cluster.rs` needs both in scope at once.
  `Neighborhood.pattern_fields: Vec<PatternField>` and a matching
  `Subdivision.new_fields` carry it through the existing subdivision/
  apply-and-merge machinery unchanged.
- **P29 itself**: now takes the same raw site `parcel_id` `p37_house_cluster`
  is about to carve (not `"*"`), reads that parcel's own polygon directly, and
  returns a `Subdivision` that touches no parcel at all — only
  `new_fields: vec![PatternField::Density(field)]`. Runs genuinely first in
  the real pipeline now (`P29 → P37 → P52 → ...`), needing nothing P37 used to
  provide.
- **P37 samples it**: `p29_density_rings::sample_density_field` (and a
  `_ring` variant returning the raw `(ring_idx, n_rings)` for the native
  `DensityTier` component path) is called once per new block, at the moment
  it's individuated, to set `density_tier`/`target_stories` directly — no
  separate later pass. Missing field → both stay `None`, identical to a
  pipeline that never ran P29, so this is fully backward compatible.
  `p29_density_rings::run_native`'s old dual-write responsibility (writing a
  `DensityTier` component) moved to `p37_house_cluster::run_native` for the
  same reason — P29 no longer individuates anything, so it has nothing left
  to dual-write.

**Finding 1 — this really was a live bug, not a hypothetical.** Confirmed by
building it: P29's field needs a center and radius from the RAW site polygon
alone, computed via the exact same vertex-averaging convention every other
operator in this crate already uses for a footprint's own "origin." Nothing
about the math needed blocks to exist — the old block-dependency was pure
schema limitation, exactly as diagnosed in §4.1.

**Finding 2 — the eccentricity/radius approximation changed, honestly, and a
test that depended on the OLD approximation's accidental guarantee broke.**
The pre-field version measured `radius_m` as the distance to the farthest
*block centroid*, which tautologically put something at the outer edge by
construction (blocks are inside the site, so the farthest one IS the edge,
by definition). The field-based version measures `radius_m` from the raw
parcel's own farthest *vertex* — a real, honest measure of the site's own
irregular shape, but not one that guarantees any block will ever sample into
it. Confirmed on the real `MILITARY_CIRCLE_ASSEMBLED` fixture: the site's
farthest vertex sits ~584m from the field center, but P37's own Voronoi-
carved blocks only ever land within ~371m of it (normalized distance ≤
0.635, never reaching the outer third/"edge" tier). This is not a bug to
paper over by shrinking the radius until a block happens to land past 0.667
— it's an honest reflection of a real, irregularly-shaped 25-parcel
assembly whose block layout doesn't explore its own full, jagged extent.
The test that assumed "at least one Edge block" (a guarantee that only ever
held because of the old, tautological radius definition) was rewritten to
check real tier *variety* instead — see `tests/p37_run_native.rs`'s own
`run_native_gives_real_tier_variety_on_a_real_multi_ring_site` for the full
reasoning.

Full workspace test suite green (including `pipeline_ledger_consistency`,
`corrected_pipeline_real_traced_order_is_valid`, and
`every_cascade_contract_holds_on_the_real_fixture`, all of which exercise the
real reordered pipeline against the real fixture), clippy clean, and
`scripts/vibe-render.sh` confirms the real gallery caption on
`public/index.html` now reads `P29 → P37 → P52 → ...` automatically (no
manual edit) with the perceptual-hash regression gate still passing.

Item 2 (P116's own "reuses someone else's individuation" pattern, §4.5) and
the Class B `floors`-hoisting fix (§4.7) remain open, not attempted here.

## 6b. The three free reorders (Class D), also shipped this session

§5's recommendation 3 — zero real dependency either way, per §§4.3/4.4/4.6 —
implemented immediately after item 1, since they cost nothing to verify. Real
execution order for this cluster is now:

```
... P107 → P117 → P124 → P127 → P197 → P116 → P129 → P130 → P131 → P221 ...
```

(was `P124 → P117 → P197 → P127 → P116 → P130 → P129 → P131`). Three swaps:

- **P117 ↔ P124** — P117 only reads `height_m`, never footprint; moved ahead
  to match ascending numbering (117 < 124).
- **P127 ↔ P197** — neither reads the other's output; moved P127 ahead to
  match ascending numbering (127 < 197). P116 (which needs both P117's roof
  and P127's cells) stays valid since both still precede it.
- **P129 ↔ P130** — P130 was already admitted arbitrary in its own module
  doc; moved P129 ahead to match BOTH ascending numbering (129 < 130) and
  Alexander's own cited textual sequence (127 → 128 → 129 → 130 → 131...).

All three updated in `pipeline.rs` (real call order + module doc), the ledger
mirror (`corrected_pipeline.rs`), `language_graph.rs`'s checkable `LANGUAGE`
table, and `registry.rs`'s operator list, for consistency. Full workspace
test suite green with zero changes needed to any test — exactly what "no
real dependency" predicts. `scripts/vibe-render.sh` confirms the real
gallery caption picks up the new order automatically and the perceptual-hash
gate still passes (expected: these are independent scalar-setting stages,
reordering them doesn't change final geometry).

Remaining open items: the Class C mixed case (P108→P96, §4.2 — splitting
P96 into a field-sampled base assignment plus a still-necessarily-late
exception-selection step), the Class B `floors`-hoist (§4.7, fixes P133/
P118/P119 at once), and item 2 (P116's own field, §4.5).

## 7. Non-goals

- This audit does not claim every remaining ascending-order stretch (P37→P52→P61,
  P95→P108, P96→P107, P107→P124, P117→P197, P127→P129→P131, P131→P221) is free of
  its own issues — only the ten actual deviations from ascending order were
  examined here.
- Does not touch, revert, or re-litigate any already-shipped code — P116's real
  implementation (§4.5) stays as-is; it's flagged, not undone.
- Does not commit to Simondon's actual philosophical apparatus beyond the single
  borrowed distinction (pre-individual field vs. individuated instance) — see the
  citation-honesty note in §1.
