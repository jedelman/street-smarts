# Floor-plan legibility: where we are, where to get to

**Status:** design note, not yet implemented.
**Inspiration:** a promotional floor-plan drawing from a Swiss architecture firm, shown to me by Jason for its plan legibility. Not reproduced here or anywhere in this repo — same policy this project already applies to *A Pattern Language*'s full text (cited and linked in `README.md`, never redistributed). What follows is my own description of the techniques that make it read well, not a copy of it, and it should be read as inspiration for a *direction*, not a spec to match exactly — the source is a hand-drafted architectural presentation drawing, not something this pipeline's 2D-line-art renderer is going to fully replicate.

---

## What makes the reference plan legible

Four things, in descending order of how much work they're doing:

1. **A hard two-tier line-weight hierarchy.** Exterior and party walls read as solid black bands, not single strokes — real drawn thickness, not just a heavier line. Interior partitions and furniture are thin, light strokes by comparison. The contrast between "this is structure" and "this is everything else" is doing most of the legibility work, even at small scale.
2. **Furniture drawn light and schematic** (beds, sofas, dining sets, kitchen counters) — enough to read a room's program at a glance, deliberately subordinate to the wall weight so it never competes with the structure.
3. **Circulation (the stair core) rendered with a distinct fill/hatch**, instantly separable from habitable rooms.
4. **Small dimension-point markers** on the perimeter instead of dimension strings, and **repeated, mirrored unit modules** creating a visible rhythm across the whole plan.

## Where `render_floor_plan` actually is today

Read directly against `tools/vibe-render/render.py`, not from memory:

- **Wall weight**: exterior walls draw at `linewidth=1.4`, interior (door-gapped) walls at `linewidth=1.0` — a real but subtle ~1.4x contrast, both single-stroke lines. The function's own docstring already names the gap driving this: there's no modeled exterior wall *thickness* to draw as a filled band (`punch_openings`'s own caveat: "a punch just pierces solid mass" — there's no wall material to give a cross-section to). This is the single highest-leverage change available, and it's already a known, named limitation, not a new discovery.
- **Furniture**: none. Zero furniture icons anywhere in `render_floor_plan`.
- **Stair core**: the data already exists — `p133_staircase_as_a_stage` tags a `kind: "stair"` cell, and `STAIR_FILL_COLOR` is already defined in `render.py` (used elsewhere in the file, per its own comment: "circulation, not a step in the public/private sequence, and shouldn't read as one"). `render_floor_plan` itself currently applies **no fill at all** — it's pure line art per its own docstring ("plain 2D line art, no CSG"). The color exists; it's just not wired into this function yet.
- **Depth-based room fill**: `depth_to_fill_color` (public→private warm gradient, already defined for `p127_intimacy_gradient`'s depth field) is also not currently applied inside `render_floor_plan`.
- **Dimension markers**: not present. Genuinely new work, not sitting on existing infrastructure the way the other three are.

## The honest tension worth naming, not glossing over

The furniture gap isn't purely a rendering TODO — it runs into a real, deliberate design decision. `InteriorCell`'s own doc comment in `street-smarts-core` is explicit: cells are "FORM-only... Alexander's own pattern language describes spatial relationships, not prescribed uses, and this project doesn't assume a program this pipeline has no way to know." A bed icon asserts "this is a bedroom" — a program label this pipeline has never claimed to know. Closing this gap fully (to match what makes the reference read so well) would mean either:

- inferring/labeling room programs somewhere upstream (a real scope expansion, and arguably outside what this project has said it's for), or
- finding a way to signal legible scale and use *without* program-specific furniture — e.g. a generic occupiable-area marker, or leaning harder into wall-thickness + the depth-gradient fill that already exists but isn't wired in.

Not resolving that here. Flagging it so the eventual furniture work doesn't happen without someone deciding which side of that line to land on.

## Concrete, ordered next steps

1. **Wall thickness as a filled band**, not a heavier stroke — offset each wall line to a real thickness (matching `P95`/`P108`'s own construction-joint/party-wall constants for scale) and fill it. Highest leverage, sits directly on a gap the code already names.
2. **Wire in the fill colors that already exist** — `STAIR_FILL_COLOR` for stair cells, `depth_to_fill_color`'s gradient for ordinary cells. No new color design needed, just applying what `render.py` already defines elsewhere.
3. **Dimension-point markers** on building perimeters — smallest, most self-contained addition.
4. **Furniture** — blocked on the program-labeling decision above; don't start this one without resolving that first.
