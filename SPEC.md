# `wholeness` — A Christopher Alexander provocation engine for neighborhood imagination

**Draft v0.2 · 2026-05-17**
**Author:** Claude (for Jason)
**Status:** Spec proposal. Open questions flagged inline.

---

## 1. Premise

Most neighborhood-analysis tools claim to measure what makes a place good. They don't. They measure what capital wants — yield per acre, ROI, walk score, Opportunity Zone eligibility. The pretense of objectivity is the move; the developer pitch deck wears a lab coat.

`wholeness` makes the opposite move. **It does not claim to measure aliveness.** It can't, and nobody can. Aliveness is what Alexander called the *quality without a name* — by definition not fully namable. Anyone selling you a number is selling you a deck.

What `wholeness` does instead:

1. **Provoke.** Take a real or imagined neighborhood. Run it through Alexander's three corpora — 253 patterns, 15 properties of wholeness, sequence-based application — encoded as opinions in math. Surface what each opinion sees.
2. **Disagree.** Run multiple opinions in parallel. Classical algorithms, vision-language models, the solver's own preferences, real-world comparison points. Surface their disagreements as the **primary output**.
3. **Imagine.** Let people apply pattern operators to dead places, see what comes out, argue about it.
4. **Record.** Track who looked at what, who said what about it, what got decided afterward. The score is a provocation. **The decision ledger is the data.**

The library is named honestly. It does not validate developer aesthetics. It does not pretend to objectivity. It is a tool for activists and neighborhood groups to fuel their imagination, sharpen their arguments, and build collective sense of what they want where they live.

> **What's accurate is the record of what people decided. Everything else is opinion.**

---

## 2. Positioning

### What it is
A browser-deployable **provocation engine + decision ledger** for neighborhood imagination. Generates and scores neighborhood proposals using Alexander's three corpora (patterns, properties, sequences) as **opinions encoded in math and prompts**. Surfaces disagreements between those opinions as the primary output. Captures human responses as training data for the next iteration.

### What it is not
- Not a measurement tool. No objective scores.
- Not a planner's tool. Planners can fork; they're not the audience.
- Not a developer's tool. Categorically.
- Not a "smart cities" platform. No surveillance integration. No third-party data sharing.
- Not cloud-dependent in the activist path. Runs on a phone in a community center with no Wi-Fi if needed.

### Primary audience
**Activists and neighborhood groups.** People who already know their neighborhood, want to imagine better futures for it, and need a tool to fuel collective conversation about what's possible. They will not run Python. They will open a webpage on their phone, play with a generated proposal for their block, save what they made, share it, and argue about it.

### Tertiary audience
Radical planners, CDFI staff, planning faculty critical of the field. Can use the Python experimentation/training layer. Not first-class.

### Explicit non-audience
- Master developers, REITs, Opportunity-Zone funds
- City planning offices acting as ZBA stenographers
- Smart-cities vendors
- Anyone who wants an "objective" score to put in a slide deck

### Eastside Commons working example
73 acres, Military Circle, Norfolk. EC_FieldSolver produces 2,400–4,000 units, 96% affordable, 169 buildings via Gaussian pattern pressure fields. `wholeness` should:

1. **Surface the disagreement** between EC_FieldSolver's preferred proposal, the 15-property opinion, the pattern-presence opinion, and Liz Albert's actual judgment
2. **Generate variants** Jason and the coalition can browse, modify, and argue about
3. **Capture the arguments** as the data layer
4. **Output coalition-facing artifacts** for council meetings, zines, newsletters
5. **Run on edge** so a neighborhood group meeting in a church basement with patchy Wi-Fi can use it

---

## 3. Core architecture

```
                          ┌──────────────────────────────┐
                          │   INPUT ADAPTERS              │
                          │   (browser-runnable)          │
                          │   · GIS (geojson)             │
                          │   · OSM bbox fetch            │
                          │   · scene-graph (solver out)  │
                          │   · hand-drawn sketch (v0.3+) │
                          └────────────────┬──────────────┘
                                           │
                                           ▼
                          ┌──────────────────────────────┐
                          │  Neighborhood Intermediate   │
                          │  Representation (NIR)        │
                          │  — single canonical schema   │
                          └────────────────┬──────────────┘
                                           │
       ┌──────────────────────┬────────────┴──────────────┬──────────────────────┐
       ▼                      ▼                           ▼                      ▼
 ┌───────────┐         ┌───────────┐              ┌───────────┐         ┌───────────┐
 │ classical │         │ pattern   │              │ activist  │         │ VLM (opt) │
 │ geometric │         │ presence  │              │ axes      │         │ Salingaros│
 │ opinions  │         │ opinions  │              │ opinions  │         │ prompts   │
 └─────┬─────┘         └─────┬─────┘              └─────┬─────┘         └─────┬─────┘
       │                     │                          │                     │
       └─────────┬───────────┴────────────┬─────────────┴─────────┬───────────┘
                 ▼                        ▼                       ▼
          ┌──────────────────────────────────────────────────────────────┐
          │              CONFLICT ENGINE                                  │
          │   · detect disagreements between opinions                     │
          │   · render disagreements as the primary output                │
          │   · invite human input on disagreements                       │
          └──────────────────────┬───────────────────────────────────────┘
                                 │
                                 ▼
          ┌──────────────────────────────────────────────────────────────┐
          │              DECISION LEDGER                                  │
          │   · who looked at what, when                                  │
          │   · who said what about disagreements                         │
          │   · what proposal got picked / modified / rejected            │
          │   · this is the only "accurate" part of the system            │
          │   · opt-in telemetry; user-exportable; user-deletable         │
          └──────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
                       ┌─────────────────────────┐
                       │  GENERATOR + STEERING   │
                       │  · edge-runnable model  │
                       │  · pattern operators    │
                       │  · sequence policy      │
                       │  · equity guards (hard) │
                       │  (this is EC_FieldSolver│
                       │   evolved + distilled)  │
                       └─────────────────────────┘
```

### 3.1 Neighborhood Intermediate Representation (NIR)

Browser-runnable schema. JSON-serializable, no Python types in the path.

```typescript
interface Neighborhood {
  id: string;
  bbox_wgs84: [number, number, number, number];
  crs: string;
  parcels:        Parcel[];
  buildings:      Building[];
  streets:        Street[];
  open_space:     OpenSpace[];
  boundaries:     Boundary[];
  activity_nodes: ActivityNode[];
  raster_layers:  Record<string, Float32Array>;
  metadata: {
    source: string;
    fetched_at: string;
    license: string;
    layer_provenance: Record<string, ProvenanceTag>;
  };
}
```

Adapters live in `wholeness/adapters/*.js`. Each adapter:
- Projects to common CRS in-browser (proj4js)
- Tags every feature with provenance
- Caches in IndexedDB for offline use (consistent with edge-deployable contract)

### 3.2 Opinion protocol

Every scorer is now an **Opinion**, not a measurement.

```typescript
interface Opinion {
  name: string;
  family: 'geometric' | 'pattern' | 'activist' | 'vlm' | 'human';
  source: SourceCitation;  // whose opinion this encodes
  range: [number, number];

  evaluate(n: Neighborhood): OpinionOutput;
  explain(n: Neighborhood): Explanation;
  visualize(n: Neighborhood): SVG | RasterOverlay;
}

interface OpinionOutput {
  value: number;
  contributing_features: FeatureRef[];   // provenance
  caveats: string[];                     // what this opinion explicitly doesn't see
  method_summary: string;                // 1-line description for the human reader
  runtime_ms: number;
}
```

Note what's missing: `confidence`. No false precision. Every opinion either *says something* or *flags that it can't speak to this case*. The "I don't have a view here" output is first-class.

Every opinion declares its source: Salingaros's prose, Alexander's pattern chapter, an algorithm author's heuristic, a coalition vote. **The library never speaks in its own voice.** It speaks as a chorus of cited opinions, and shows you the chorus.

### 3.3 Opinion families

#### Family A: Geometric opinions (`wholeness/opinions/geometric/*`)

One per Alexander property. Algorithms from Salingaros 2025 and downstream work, each marked with its origin.

| # | Property | Geometric handle (an opinion, not a measure) |
|---|---|---|
| 1 | Levels of Scale | Histogram of feature sizes; scale-magnification ratios (target 2–5×) |
| 2 | Strong Centers | Density-of-attention local maxima; nestedness depth |
| 3 | Thick Boundaries | Boundary-thickness-to-enclosure ratio (~1/3 target) |
| 4 | Alternating Repetition | Autocorrelation of facade/street rhythm |
| 5 | Positive Space | Convexity of pedestrian/open space; figure-ground duality |
| 6 | Good Shape | Compactness; nested-symmetry depth |
| 7 | Local Symmetries | Bilateral symmetry detection at multiple scales |
| 8 | Deep Interlock & Ambiguity | Boundary fractal dimension; interpenetration |
| 9 | Contrast | Multi-scale variance in form/size |
| 10 | Gradients | Smoothness of transitions in density, height, use |
| 11 | Roughness | Deviation from regularity at small scales |
| 12 | Echoes | Self-similarity across scales (box-counting) |
| 13 | The Void | Presence and definition of unfilled complementary spaces |
| 14 | Simplicity & Inner Calm | Coherence-per-bit |
| 15 | Not-Separateness | Edge permeability; connectedness to surroundings |

Each implementation is one author's encoded opinion of how to detect the property. Multiple competing implementations per property are welcome and expected. The conflict engine surfaces their disagreements.

#### Family B: Pattern presence opinions (`wholeness/opinions/patterns/*`)

~40 patterns at neighborhood scale, each scored for presence / quality / coverage. Reuse the existing EC_FieldSolver pattern detectors, marked as opinions.

#### Family C: Activist opinions (`wholeness/opinions/activist/*`)

Affordability, ownership, displacement risk, public space share, ecological function, mobility without a car, cultural continuity. Each is an opinion about what counts as good for people who live there. **Equity opinions act as hard guards in the generator** (§3.5), not aggregable axes — a proposal that improves geometric opinions but worsens affordability is rejected.

#### Family D: VLM opinions (`wholeness/opinions/vlm/*`)

Salingaros 2025 prompts pinned to model version. Run against a rendering of the neighborhood. **Optional** — the library degrades gracefully without it. When present, treated as one more voice in the chorus, neither privileged nor dismissed. Model version logged with every output for drift tracking.

#### Family E: Human opinions (`wholeness/opinions/human/*`)

The most important family. A coalition member taps "this plaza is wrong" or "this whole block feels right" — that's an opinion, captured, attributed, stored in the decision ledger. Human opinions can override others in the conflict engine; they cannot override equity guards.

### 3.4 Conflict engine (`wholeness/conflict/`)

**This is the heart of the library.** Where I previously had "scoring" as the primary output, the actual primary output is structured disagreement.

```typescript
interface DisagreementReport {
  subject: NeighborhoodOrProposal;
  opinions: OpinionOutput[];
  disagreements: Disagreement[];
  agreements: Agreement[];
  questions_for_humans: HumanPrompt[];
}

interface Disagreement {
  axis: string;                  // e.g., "Strong Centers"
  opinion_a: OpinionOutput;
  opinion_b: OpinionOutput;
  delta: number;                 // magnitude of disagreement
  human_prompt: string;          // what to ask the user
  matters_because: string;       // why this disagreement is interesting
}
```

The disagreement report is the **front page** of every output. Not a score grid — a list of "here are the things the chorus argues about, here's what each voice says, here's what to ask yourself or each other."

Example output framing:

> The geometric algorithm says this proposal has 3 strong centers.
> The VLM says it sees 1.
> The pattern detector (P30 Activity Nodes) says 2.
> Where do you see centers in this place?

That question is the primary product. The scores are just the prelude.

### 3.5 Generator (edge-deployable)

EC_FieldSolver, distilled.

**Constraints (hard):**
- Browser-runnable. WebGL2 or WebGPU, no server.
- Bundle < 5 MB compressed (probably stretched; flag for v0.2 measurement)
- Runs offline once loaded
- No API keys in the activist-facing path
- Phone-grade GPU acceptable (Adreno, Mali, Apple Mx)
- Generation step < 10 seconds for a 73-acre site at 10ft grid

**Steering loop:**
```
state = NIR(initial_or_imagined)
for step in budget:
    deficit_opinions = opinions_with_lowest_outputs(state)
    candidate_patterns = patterns_that_might_address(deficit_opinions)
    sequence = choose_sequence(candidate_patterns, state)
    new_state = apply_sequence(state, sequence)
    if all_equity_guards_held(new_state):
        state = new_state
    else:
        backtrack
    log_to_ledger(step, sequence, new_state)
return state, trajectory
```

**Equity guards are categorical, not gradient.** A move that improves geometric opinions but breaks an equity guard is rejected, full stop. Affordability is not tradable for plaza-shape elegance.

Sequences come from Alexander's published trajectories as priors, plus learned policy in v0.3+ once the decision ledger has enough data to train on.

### 3.6 Decision ledger (`wholeness/ledger/`)

**The only accurate part of the system.**

```typescript
interface LedgerEntry {
  timestamp: ISO8601;
  user_id: AnonymousID;             // hashed, no PII
  session_id: string;
  neighborhood_id: string;
  event: LedgerEvent;
}

type LedgerEvent =
  | { type: 'viewed_proposal'; proposal_id: string }
  | { type: 'modified_proposal'; proposal_id: string; modification: Modification }
  | { type: 'opinion_offered'; axis: string; value: number; rationale?: string }
  | { type: 'disagreement_resolved'; disagreement: Disagreement; resolution: Resolution }
  | { type: 'proposal_saved'; proposal_id: string; intent: 'share' | 'archive' | 'submit' }
  | { type: 'proposal_shared'; proposal_id: string; channel: string }
  | { type: 'proposal_used_in_meeting'; proposal_id: string; outcome?: string };
```

**Properties:**
- Opt-in only. Default off. Loud, transparent prompt to enable.
- Local-first (IndexedDB). User can export their ledger as JSON. User can delete entirely.
- Optional sync to a shared bucket (for the flywheel) requires separate explicit consent
- No PII. User IDs are random per-device.
- Anti-harassment: the public-argument forum (Streamlit v0.3+) has rate limits, moderation hooks, and a hard policy against doxxing or targeting individuals

**This is what gets accurate over time.** Not the opinions. The record of decisions.

### 3.7 Synthesis (`wholeness/synth/`)

Per your Q1 answer: **counterfactual-first**. Take a dead American site, apply pattern operators, treat the output as a synthetic positive.

**Hard rules (from your guidance):**
1. **Real-place outputs are ranked categorically above synthetic outputs in every comparison view.** Aggregations that mix the two surface the split explicitly.
2. **Baudrillard caveat shipped in the docs.** The real/synthetic distinction is operationally useful and philosophically contested. We say so out loud. We do not pretend the distinction is clean.
3. **Real-positives sanity floor.** Three hand-curated real living neighborhoods (Siena, Trastevere, one TBD non-European) scored with the same opinions, used as a smell test. If synthetic positives ever score higher than these on the same axes, that's a Goodhart alarm.

**Why counterfactual first works:**
- Eastside Commons is the working example. We have a dead baseline and a solver-transformed proposal already. v0.1 ships with real data.
- Fast iteration loop: solver → score → adjust → re-score.
- Goodhart risk is real but mitigated by (a) the real-positives sanity floor, (b) the conflict engine surfacing disagreements between opinions including VLM and human, (c) the decision ledger tracking what humans actually pick — which is where the truth lives.

**v0.3 adds:**
- Procedural generators (sample patterns under property constraints)
- Begin Synth-B historical reconstructions (Greenwood OK pre-1921, Hayti NC pre-1960, Siena, one non-European)
- Discriminator (real-vs-synthetic classifier) as adversarial probe

---

## 4. Module structure

Two repos:

### `jedelman/wholeness` — browser-first, the activist tool
```
wholeness/
├── src/
│   ├── adapters/                # input format → NIR
│   ├── nir/                     # schema, projection, provenance
│   ├── opinions/
│   │   ├── geometric/           # 15-property opinions
│   │   ├── patterns/            # ~40 pattern opinions
│   │   ├── activist/            # equity opinions
│   │   ├── vlm/                 # optional VLM hooks (Anthropic/OpenAI/local)
│   │   └── human/               # tap/click/draw to express an opinion
│   ├── conflict/                # disagreement detection + rendering
│   ├── ledger/                  # decision ledger, opt-in telemetry
│   ├── generator/               # EC_FieldSolver distilled, WebGL2/WebGPU
│   ├── report/
│   │   ├── disagreement-card.ts # primary output: what to argue about
│   │   ├── coalition-card.ts    # one-page output for meetings
│   │   ├── zine-export.ts       # SVG for print
│   │   └── council-handout.ts   # markdown for officials
│   └── ui/                      # vanilla JS + WebGL, matching existing stack
├── public/                      # static deployment target
├── dist/                        # bundled
└── tests/
    ├── reference-sites/         # 3 alive + 2 dead, hand-curated
    └── synth-validation/
```

### `jedelman/wholeness-lab` — Python experimentation/training
```
wholeness-lab/
├── distillation/                # train edge generator from EC_FieldSolver
├── opinion-experiments/         # try new geometric algorithms
├── synthesis-experiments/       # synth-A, B, C iteration
├── ledger-analysis/             # mine the decision ledger
├── historical-corpus/           # Synth-B reconstructions (v0.3+)
└── notebooks/
```

Activists never touch this repo. It exists for Jason and future collaborators to experiment, train, and feed improvements back into the browser tool.

---

## 5. Public surfaces

### 5.1 v0.1: a webpage
`https://wholeness.jason-edelman.org/eastside-commons` (or wherever)

- Loads Eastside Commons NIR
- Shows the current parcel (dead baseline)
- Shows the EC_FieldSolver-generated proposal
- **Front page is the disagreement card** — "here's what the chorus argues about"
- Buttons: "see the proposal in 3D" (links to existing patterns-3d), "tap to disagree", "make your own", "save / share / print"
- Coalition card export (one-pager)
- Council handout export (markdown)
- Decision ledger is opt-in (off by default)

### 5.2 v0.2: a tool
Same browser app. Now with:
- Steering loop wired up (apply pattern operators, see the effect)
- Hand-drawn modifications (tap to add a plaza, drag a building, redraw a street)
- All 15 property opinions, ~40 pattern opinions, all equity opinions
- VLM hook (Anthropic + OpenAI), pinned versions, falls back gracefully
- JSON export of full proposal + ledger
- 5 reference sites (3 alive, 2 dead) for sanity-floor visibility

### 5.3 v0.3: a forum
Streamlit-hosted argument forum at `wholeness.jason-edelman.org/forum`.
- Public proposals (people share theirs)
- Public arguments (people leave opinions on others' proposals)
- Aggregated decision ledger (anonymous, opt-in contributors)
- Counterfactual synth pipeline live (run pattern transformations on any submitted site)
- Begin historical corpus (Synth-B) integration
- Discriminator as a public sanity check

### 5.4 v0.4+: distillation
- Train smaller generator models from accumulated ledger data
- Sequence policy learned from the trace of what humans actually pick
- Possibly offer offline-mobile builds for low-connectivity contexts

---

## 6. Critical design decisions

### 6.1 What's accurate vs. what's opinion

**Accurate:** the decision ledger. Who looked at what, who said what, what got picked, what got modified, what got submitted to council.

**Opinion:** every numerical output. The geometric algorithms, the VLM scores, the pattern detectors, the synthetic positives, the discriminator. All are opinions encoded in math.

The library never collapses these. Every output is attributed. The chorus stays a chorus.

### 6.2 Conflict is generative

Disagreement between opinions is not a bug to resolve. It is **the primary product**. The library is designed to *generate* productive arguments by:

- Running multiple opinions in parallel
- Surfacing where they disagree
- Asking humans to weigh in
- Recording the human response as the actual data

A proposal where all the algorithms agree is **less interesting** than one where they don't. The conflict engine highlights disagreement.

### 6.3 Edge-deployable, no cloud in the activist path

Hard constraint. The activist-facing webpage runs in a browser on a phone with no Wi-Fi after first load. No API keys required. No third-party telemetry. VLM hooks are optional add-ons, never blockers.

This is a political requirement, not an engineering preference. Tools that require Big Tech accounts are tools Big Tech can shut off.

### 6.4 Equity guards are categorical

Affordability, ownership, displacement, ecology, mobility, cultural continuity. These are not aggregable. A move that improves geometric opinions but breaks an equity guard is rejected by the generator, full stop. No tradeoffs. No "weighting." If you want to override this for academic comparison, you can — and the report layer shouts that you did, in red.

### 6.5 Real > synthetic, hard rule, Baudrillard-flagged

Real-place outputs categorically rank above synthetic-place outputs in any comparison view. Aggregations that mix the two split them explicitly. The library ships a one-paragraph Baudrillard caveat in the docs and in the about page: we use this distinction because it's useful, while knowing it's contested. We don't pretend to have settled it.

### 6.6 The library speaks as a chorus

No score is ever attributed to "the library." Every output is attributed to its source: Salingaros's prose, an algorithm author, a pattern chapter, a coalition voter. The library is a chorus master. It does not sing in its own voice.

### 6.7 Honest about what we don't know

"I don't have a view here" is a first-class opinion output. The library refuses to fake confidence. If the geometric algorithm can't speak to a case, it says so, and that fact is part of the disagreement report.

---

## 7. Risks & unknowns (honest)

### Known risks

1. **The flywheel might not turn.** v0.3's "argue with people on the Internet" assumes substantive arguments emerge. Could be just trolls and noise. Mitigation: rate limits, moderation, no anonymous posting at the forum tier (anonymous use is fine; anonymous *public* arguing is not).

2. **Edge deployment under-delivers.** Bundle size, GPU performance, mobile thermals. May force compromises in generator capability. Will measure in v0.2.

3. **The opinion-as-chorus framing might confuse activists.** People want answers, not arguments. Need to test the UI early with actual coalition members. If "here's a disagreement to think about" reads as "the tool can't decide," we've failed. The framing has to land as **"here's what the tool wants you to know it doesn't know."** Subtle.

4. **Ledger as data layer creates surveillance risk** even with opt-in/local-first design. Aggregated decision data from a neighborhood coalition is sensitive. A subpoena could compel disclosure of opt-in synced data. Mitigation: aggressive local-first design, optional sync only with explicit consent, no shared bucket for v0.1 — defer the shared layer until we've thought through threat models properly.

5. **Counterfactual synthesis Goodhart.** Real-positives sanity floor is the defense, but it's a sample of 3 sites in v0.1. Could be gamed inadvertently.

6. **Cultural specificity of Alexander's framework.** A Berkeley-trained mathematician's claim about universal beauty. Mitigated by treating his framework as one chorus voice, not the chorus master. Other voices (other corpora, other traditions) can be added as opinion families later.

7. **VLM consistency.** Frontier models drift. Pin versions. Log every score with its model version. Accept that VLM opinions in 2026 won't match VLM opinions in 2028.

### Unknowns

- **Bundle size feasibility.** Need to measure. EC_FieldSolver's solver worker is already 130KB; with all opinions + generator + UI, we're looking at maybe 2–5 MB compressed. Should fit but unconfirmed.
- **Whether the steering loop converges under categorical equity guards.** Might be over-constrained. Empirical.
- **Whether Liz Albert and the RFS-WM-EP coalition will actually use a webpage.** Need to test in front of them in v0.1. This is a deployment risk, not technical.
- **Whether the Goodhart watchdog catches divergence in time.** Statistical question. Synth-B historical anchors in v0.3 help.
- **Whether opt-in telemetry rates are high enough to feed the flywheel.** Could be 5%, could be 50%. Affects v0.4+ planning.

### Where my research is incomplete

- I haven't read Salingaros's full 2025 main paper, only the appendix
- I haven't read EC_FieldSolver code line-by-line
- I haven't surveyed Sidewalk Labs CityGraph, MIT Senseable City, Sustasis circle work — claim of novelty needs verification
- Edge-deployable generative-model literature in 2025–26 — there's recent work on small WebGPU models that may apply
- Streamlit's threat model for hosting argument forums under hostile attention — unstudied
- Alexander's *Mexicali* / *The Production of Houses* community-process apparatus deserves a real read-through before v0.2

---

## 8. Milestones

### v0.1 — A webpage (~2 weeks)
**Ships when:** Liz Albert can open `wholeness.jason-edelman.org/eastside-commons` on her phone, see the EC_FieldSolver proposal, see what the opinion-chorus argues about, tap "this is wrong" or "this is right" on specific elements, and export a one-pager for the next coalition meeting.

**Includes:**
- NIR schema (TypeScript) + 2 adapters (GIS, scene-graph)
- 8 of 15 geometric opinions: Levels of Scale, Strong Centers, Thick Boundaries, Positive Space, Local Symmetries, Echoes, The Void, Not-Separateness
- 5 activist opinions: affordability, ownership, displacement, ecology, mobility
- Conflict engine (basic): disagreement detection between opinions, ranked list
- Disagreement card (HTML, the primary output)
- Coalition card + council handout exports
- Decision ledger (local-only, opt-in, IndexedDB)
- Eastside Commons fully loaded as the working example
- 3 real positive sanity-floor sites visible for comparison (Siena, Trastevere, one non-European TBD)

### v0.2 — A tool (~3 weeks after v0.1)
**Ships when:** A coalition member can take EC_FieldSolver's proposal, modify it directly in the browser (move a building, add a plaza, redraw a street), see how the chorus's disagreement shifts, and steer the generator toward a result they like, with equity guards refusing trades they don't.

**Includes:**
- Remaining 7 geometric opinions
- ~40 pattern opinions
- Generator wired to steering loop, edge-runnable
- Direct manipulation UI (hand-drawn modifications)
- VLM opinion family (Anthropic, OpenAI, with local fallback)
- 5 reference sites (3 alive, 2 dead) for sanity-floor visibility
- JSON export of full state + ledger
- Begin Jason-as-arbiter session captures as ledger seed data

### v0.3 — A forum (~6 weeks after v0.2)
**Ships when:** Strangers on the Internet can argue with Jason about Eastside Commons through a hosted Streamlit interface, and their arguments feed into the decision ledger, and a public discriminator probe reports on synthetic-vs-real Goodhart drift.

**Includes:**
- Streamlit forum at `wholeness.jason-edelman.org/forum`
- Counterfactual synth pipeline live (run pattern transformations on submitted sites)
- Procedural generators (Synth-A)
- 5 historical reconstructions begun (Synth-B): Siena, Bologna, Greenwood OK pre-1921, Hayti NC pre-1960, one non-European TBD
- Discriminator (real-vs-synth) as adversarial probe with public dashboard
- Public decision ledger (aggregated, anonymized, opt-in)
- Moderation hooks + anti-harassment policy

### v0.4+ — Distillation and beyond (open-ended)
- Train smaller generator from ledger data
- Sequence policy learned from human picks
- Offline-mobile builds for low-connectivity contexts
- Multi-neighborhood comparative atlas
- Paper / zine

---

## 9. The political ask

This is not a tool for cities or developers. It is not a measurement device. It is a chorus of opinions — Alexander's, Salingaros's, algorithm authors', VLMs', and most importantly the people who will live with the consequences of any neighborhood decision — arranged so that their disagreements become productive.

The accurate part of the system is not the math. It's the record of what humans decided, captured honestly and made available to the next decision.

Activists win when they can imagine concretely what they want, argue clearly about it, and bring legible artifacts to power. `wholeness` is built for those three moves. The math is in service of the argument; the argument is in service of imagination; the imagination is in service of building a world that does not yet exist.

It will not stop the next REIT. But every coalition meeting where someone says *"here's what we want instead, here are five things to argue about, here's what we decided together"* — that's a piece of imagination capital cannot enclose.

---

## 10. Open questions still on the table

1. **Library name confirmation.** `wholeness` (Alexander's own term). Veto if you have a better one.
2. **JS framework substrate.** Vanilla JS + WebGL (matches existing EC stack) vs. React vs. Svelte. My default: vanilla, smallest bundle, matches what's there.
3. **License.** AGPL-3.0 (anti-enclosure). Confirm or replace.
4. **Telemetry/consent design** — proper threat-modeling pass needed in v0.2. v0.1 ships local-only.
5. **Real-positive site #3** (non-European). Possible candidates: Fez medina, a precolonial Yoruba urban example, an Iranian bazaar district, Lhasa's old city, Edo Tokyo. Need a real source with adequate documentation.
6. **EC_FieldSolver code review** — I need to read the actual implementation before v0.1 starts, not just the spec. Can you point me at the canonical file path?
7. **Streamlit vs. alternative for v0.3 forum.** Streamlit is what you mentioned, fine default. Note: hosting an argument forum may need more than Streamlit's threat model affords. Defer.

---

## Appendix A — Salingaros 2025 canonical descriptions

Full text inlined in the library at `wholeness/opinions/vlm/salingaros_2025.md`. Each prompt invokes the relevant property description, asks for a 0–1 value with prose explanation. Prompts pinned to model version; drift logged in every ledger entry.

Source: Salingaros, N.A. *Living geometry, AI tools, and Alexander's 15 fundamental properties.* Front. Archit. Res. 14(6): 1491–1515, 2025. CC BY-NC-ND. DOI 10.1016/j.foar.2025.01.002.

---

## Appendix B — Reference sites (initial corpus)

**Alive (real, sanity floor):**
- Siena, IT
- Trastevere, Rome
- Non-European site TBD (Fez medina, Lhasa old city, or Iranian bazaar district)

**Dead (real, working baselines):**
- Military Circle Mall, Norfolk (current state — our working baseline)
- Tysons Corner, VA (pre-2010)

**v0.3 additions:**
- Bologna porticoes district
- Greenwood, Tulsa, pre-1921
- Hayti, Durham NC, pre-1960

**v0.3 mixed/contested:**
- Pre-2005 Williamsburg, Brooklyn
- Greenwich Village 2024

---

## Appendix C — On the real/synthetic distinction (Baudrillard caveat)

The library treats real-place data and synthesis-generated data as categorically different, with real ranked higher in comparisons. We do this because it is operationally useful: a tool that cannot tell its own outputs from the world is a tool that disappears into its own outputs.

We acknowledge — explicitly, in the docs and in the about page — that Baudrillard's argument about the simulacrum applies. In an environment saturated with images of "good urbanism" generated by tools like ours, the distinction between a real neighborhood and a synthetic one gets harder to draw the longer the synthesis runs. A neighborhood that exists because a generative model said it should is not categorically less real than one that exists because a zoning code said it should; the zoning code is also a simulation we built and forgot we built.

We are not trying to settle this. We are using a useful operational distinction with full awareness of its limits. The library says so out loud rather than pretending to have escaped the problem. Anyone using `wholeness` should know that ranking real above synthetic is a *choice* we made, not a *fact* we discovered.

---

*End draft v0.2.*
