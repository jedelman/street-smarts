# Phase 5 finish criterion: full pattern coverage against the 68-pattern list

**Status:** proposal, not yet implemented.
**Author:** Claude (for Jason), 2026-07-19.
**Scope:** redefines this project's "Phase 5" from `IMPLEMENTATION_PLAN.md`'s original ECS/pass-manager
migration to a pattern-coverage goal, per direct instruction: **every pattern on the 68-pattern list
should have a detector; every detector that can have a generator should have one.** The original
ECS/pass-manager work (`IMPLEMENTATION_PLAN.md` Phase 5) isn't abandoned -- it's still real, still
partially done (`System` trait, `PadRole`/`BuildingTypology`/`StreetClassification` dual-write
components) -- just no longer what "finishing Phase 5" means going forward.

The 68-pattern list is the numbered set from https://patternlanguage.cc/, as given directly:
`10, 14, 15, 17, 22, 23, 28, 30, 31, 32, 33, 36, 38, 41, 46, 48, 49, 50, 51, 52, 53, 59, 60, 67, 68,
88, 89, 93, 95, 98, 99, 100, 101, 102, 105, 106, 107, 108, 110, 112, 114, 115, 116, 117, 118, 119,
120, 121, 122, 123, 126, 128, 129, 139, 147, 159, 160, 161, 162, 163, 164, 165, 166, 191, 192, 195,
197, 198`.

Every pattern name/quote below is real: for the 30 patterns that had neither a detector nor a
generator before this doc (scattered across §D/§E/§F below, by what each one turned out to need),
fetched directly from patternlanguage.cc for this doc. For the 33 that already had a detector (§C/§D/§E),
read directly from this repo's own already-cited opinion files. None of it is inferred or guessed.

---

## A side note on scope

Nine patterns this codebase already fully builds (generator + opinion) are **not on the 68-list at
all**: P29 Density Rings, P37 House Cluster, P61 Small Public Squares, P96 Number of Stories, P127
Intimacy Gradient, P130 Entrance Room, P131 The Flow Through Rooms, P133 Staircase as a Stage, P221
Natural Doors and Windows. They exist because the pipeline is structurally load-bearing on them (you
can't shape a building without P37 clustering pads first, can't do interior rooms without P127, etc.),
not because they were on the target list. No action needed -- flagged here only so the count below
isn't confusing.

---

## §A. Already fully closed (3)

Generator + detector both exist, both real.

| # | Pattern | Generator | Detector |
|---|---|---|---|
| 95 | Building Complex | `p95_building_complex.rs` | `p95_building_complex.rs` |
| 108 | Connected Buildings | `p108_connected_buildings.rs` | `p108_connected_buildings.rs` |
| 129 | Common Areas at the Heart | `p129_common_areas_at_the_heart.rs` | `p129_common_areas_at_the_heart.rs` |

## §B. Generator exists, detector missing (2)

Mechanical, well-understood work -- the exact same recipe as every detector-opinion batch already
shipped this project. No feasibility judgment needed; just write them.

| # | Pattern | Generator | Real text (patternlanguage.cc) |
|---|---|---|---|
| 52 | Network of Paths and Cars | `path_network.rs` | *(already cited in P49/P121's own opinion files via path_network's shared generator)* |
| 107 | Wings of Light | `p107_wings_of_light.rs` | Already cited in `p159_light_on_two_sides.rs` (P159 checks the same daylight-depth claim P107 generates for) |

## §C. Already effectively closed, filed under a different number (6)

These have a real detector opinion today, AND the geometry they check is already produced by an
**existing** generator that just isn't filed under the same pattern number -- confirmed by reading
each opinion's own doc comment, not assumed from the number match alone. No new generator needed;
flagged separately from §A only because the file names don't match.

| # | Pattern | Detector | Satisfied by | Why |
|---|---|---|---|---|
| 49 | Looped Local Roads | `p49_looped_local_roads.rs` | `path_network.rs`'s `local_loop_budget` | Added this session specifically to close this gap -- see path_network's own "v0.3" module doc and P49's own "generator's own current default clears the check" test. |
| 67 | Common Land | `p67_common_land.rs` | `p37_house_cluster.rs`'s `common_land_fraction` | Raised 12%→26% this session specifically to close this gap -- see P37's own module doc and P67's "generators own 26 percent default clears Alexander's target" test. |
| 110 | Main Entrance | `p110_main_entrance.rs` | `p221_natural_doors_and_windows.rs` | P221 already places the door "on whichever wall faces the nearest street/open space" -- functionally a main-entrance-selection generator, just not filed as P110. |
| 121 | Path Shape | `p121_path_shape.rs` | `path_network.rs`'s `bulge_centerline` | Added this session specifically to close this gap -- see P121's own "generator's own 1.5x bulge clears the check" test. |
| 122 | Building Fronts | *(no dedicated file -- covered by P160/general setback checks)* | `p95_building_complex.rs`'s `pad_inset_m` | Already tuned to a construction-joint-sized 0.1m default (see `p108_connected_buildings.rs`'s own module doc) -- "build right up to the paths" is already the default behavior. |
| 159 | Light on Two Sides | `p159_light_on_two_sides.rs` | `p107_wings_of_light.rs` | P107's entire purpose (both its name and its courtyard/wing-carving logic) IS generating for this exact claim; P159's detector is already grading P107's real output. |

## §D. Real generator candidates (40)

Patterns with a real detector today (or about to get one from §E) where this pipeline's *existing*
geometry model (parcels, streets, buildings, open space, interior cells, window/door openings) makes a
real generator or generator extension plausible -- not a guess, reasoned from what data each pattern's
actual prescription needs against what the pipeline already computes. Grouped by which existing
generator each would extend, since that's the real dependency structure.

### Extend `p29_density_rings.rs` / `p61_small_public_squares.rs` (activity & density shape)
| # | Pattern | Real prescription | Why it's plausible |
|---|---|---|---|
| 28 | Eccentric Nucleus | "Encourage growth... to form a clear configuration of peaks and valleys." | P29 already computes a density gradient from a center point -- this asks for that center to be genuinely off-center/eccentric rather than the site centroid. |
| 30 | Activity Nodes | "Create nodes of activity... spread about 300 yards apart," built from existing concentration spots. | P61 already places a site-wide square budget: could be reframed to seed at real concentration points instead of pure area-proportional allocation. |
| 36 | Degrees of Publicness | "Give every neighborhood about equal numbers" of quiet/busy/in-between homes. | P29 tiers + street classification (local/pedestrian) already produce the raw signal; needs a balancing pass toward ~1/3 each, not new geometry. |
| 38 | Row Houses | Row houses at 15-30/acre, perpendicular paths, shared gardens. | A specialization of the existing P95→P108 pad-and-merge pipeline at a specific density range, not new machinery. |
| 68 | Connected Play | Common land connecting 64+ households, no traffic crossing. | Refinement of P37's common-land placement to guarantee contiguity across many pads, not just area fraction. |
| 99 | Main Building | Tag one building as "main," central position, higher roof. | P29 already knows distance-from-center; P96 already assigns story counts -- tagging the nearest-to-center building and boosting its height is a real, small extension. |
| 126 | Something Roughly in the Middle | One object (fountain/tree/statue) near where paths cross a square. | P61 squares already exist; this is placing one marker point inside them near path intersections -- small, concrete addition. |

### Extend `p107_wings_of_light.rs` / building massing (facade & roof)
No roof geometry exists at all today (`render.py`'s own docs: "no roof forms" is an explicit, named
caveat) -- P116/117/118/162 share that same real prerequisite gap, flagged honestly, not hidden.
| # | Pattern | Real prescription | Why it's plausible |
|---|---|---|---|
| 115 | Courtyards Which Live | Courtyard needs a real view out, not fully enclosed. | P107's courtyard ring is currently a closed loop by construction (P108's own doc: "a closed loop for free on courtyard buildings"). A real, concrete change: leave a real breach in the ring. |
| 116 | Cascade of Roofs | Roofs step down toward wing ends, following the social hierarchy below. | Needs real roof geometry added to the (currently flat-topped) massing model -- a real, non-trivial extension, not a refinement. |
| 117 | Sheltering Roof | Sloped/vaulted roof, visible surface, low eaves at entrances. | Same roof-geometry prerequisite as P116; natural to build together. |
| 118 | Roof Garden | Flat, usable sections of roof, direct access from a lived-in floor. | Depends on P116/117 landing first (needs real roof geometry to carve a flat section from). |
| 119 | Arcades | Covered walkway at a building's edge, connecting buildings. | P221 already computes which wall faces the street; a thin extruded canopy along that wall is a real, scoped addition. |
| 160 | Building Edge | Treat the edge as a volume/zone, not a line. | Same facade-depth family as P119/P166 -- real, but needs a "wall has depth" concept this pipeline doesn't have yet (see P197 below, same root gap). |
| 162 | North Face | Cascade the north face down so sun reaches the ground beside it. | Same roof-geometry prerequisite as P116/117, applied by cardinal direction (this pipeline already computes real lng/lat, so "north" is real, not approximated). |
| 166 | Gallery Surround | Porches/balconies/arcades at building edges facing public space. | Same family as P119 -- bundle together. |

**2026-07-21 update, not part of the original proposal above:** P117/162 shipped for real
(`p117_sheltering_roof.rs`, a new generator, not an extension of `p107_wings_of_light.rs` as
this table originally proposed) -- a real `Building.roof` (shed roof, ridge above the
building's own real `height_m`, sloped to true north) now exists, closing both patterns'
own detector opinions from `NoView` to a real check, and rendered in `render.py` as a plain
extrusion (no boolean). P116/118/119/166/160's own real gaps are UNCHANGED by this -- P116
needs real per-wing roof segments (a richer schema addition keyed to `p127_intimacy_gradient`'s
cell graph, not built), P118 still needs P116 first, P119/166's own canopy/gallery geometry is a
different real primitive (not a roof slope) not attempted, and P160's own "wall has real depth"
prerequisite is untouched. See `p117_sheltering_roof.rs`'s own module doc for the full reasoning
and what was deliberately left out.

**2026-07-21 update, later the same day: schema now exists for all five, no generator for any
of them yet.** `street-smarts-core/src/nir.rs` gained four real, purely-additive fields, each
keyed to a specific pattern's own literal claim, not a generic catch-all:
- `Building.roof_segments: Vec<RoofSegment>` (a real sub-polygon + its own `RoofForm`) for P116's
  "roofs step down toward wing ends" -- still nothing to key segments to (no wing-detection
  generator exists; `p107_wings_of_light` explicitly doesn't produce discrete wing entities).
- `RoofForm.occupiable: bool` for P118's "usable as roof gardens" -- `p117_sheltering_roof` still
  only ever assigns a sloped `Shed` roof, never `Flat`/`occupiable`.
- `Building.canopies: Vec<Canopy>` (`CanopyKind::Arcade | Gallery`, a real wall-edge span + depth +
  clearance height + floor number) for P119 Arcades and P166 Gallery Surround -- `p221_natural_
  doors_and_windows` already computes which wall edges face the street, the natural real input a
  generator would need, but none exists yet.
- `Building.wall_niches: Vec<WallNiche>` (a real local bulge in an exterior wall's own depth,
  additive to P197's uniform `wall_thickness_m`) for P160's own literal "deep enough to contain
  seats, bookshelves, bay windows" claim.

All five opinions (`p116_cascade_of_roofs.rs`, `p118_roof_garden.rs`, `p119_arcades.rs`,
`p166_gallery_surround.rs`, `p160_building_edge.rs`) were flipped from an unconditional `NoView`
to a real check against these fields, same discipline as the P117/P162 flip above -- verified
against synthetic fixtures in each file's own tests, not just claimed. On every real fixture this
pipeline ships today they still return `NoView` (P160 falls back to its pre-existing shape-index
proxy instead, since it already had one), because no generator populates any of the four new
fields yet -- the honest reason changed from "the schema can't represent this" to "nothing
produces it yet", which is real forward progress, not the same gap restated. The next real step
for this cluster is generator work, not more schema: a P116 wing-partition generator, a P118 flat-
roof-garden generator (needs P116 first), and a P119/P166 canopy generator keyed to `p221`'s
existing street-facing-wall computation.

**2026-07-22 update: generator cluster complete, all five patterns.** `p118_roof_garden.rs`,
`p119_arcades.rs` (also closes P166), and `p160_building_edge.rs` landed first (in that order,
NOT gated on P116 the way the note above predicted -- P118's top-N tallest-buildings pick and
P119/P160's own wall-edge geometry turned out not to need per-wing roof segments at all).
`p116_cascade_of_roofs.rs` (the generator, distinct from the opinion of the same name) landed
last: it reuses `p127_intimacy_gradient`'s own depth-ordered `interior_cells` polygons directly as
the roof's wing partition (rather than building an independent wing-detection pass, which this
doc's earlier note assumed would be needed), and cascades `ridge_height_m` down with each cell's
real `depth`. All five opinions now score real `Value`s (not `NoView`) on every real fixture this
pipeline ships, wired into both `pipeline.rs` and the ledger mirror, and into `render.py`.

**2026-07-23 update: P115 closed too, separately -- it was never actually part of the "all
five" cluster above.** `p115_courtyards_which_live` doesn't need roof geometry at all; its own
real gap was that `p221_natural_doors_and_windows` shaped every courtyard's hole ring with an
empty door-edge list, so every real courtyard this pipeline produced had windows only, zero
doors, on its own courtyard wall. `p221`'s own new `choose_courtyard_door_edges` (its "v0.3"
module doc) now places `courtyard_door_target` (default 3, matching Alexander's own "two or
three") real doors, spread around the hole ring by angle so they sit on genuinely different
walls. Measured on the real fixture: P115 Value 1.000 (17/17 real courtyard buildings, 3/3 real
courtyard doors each). New `CascadeContract` entry added (`cascade_contracts.rs`) recording this
-- honestly noted there as a real code-level relationship this project's own audit found, not an
Alexander cross-reference (no direct citation edge exists between P221 and P115 in
`data/apl-pattern-graph.json`).

### Extend `p221_natural_doors_and_windows.rs` (openings)
| # | Pattern | Real prescription | Why it's plausible |
|---|---|---|---|
| 102 | Family of Entrances | Multiple entrances, mutually visible, forming a group. | P221/P110 already place one entrance per building; extending to ensure multiple entrances (for merged P108 buildings especially) stay mutually visible is a real refinement. |
| 112 | Entrance Transition | A real transition space (light/sound/direction/view change) between street and door. | P130 already tags a cell "entrance" at depth 0; extending to guarantee a real geometric threshold bay exists is concrete. |
| 164 | Street Windows | Window seats in rooms facing busy streets. | P221 already places windows from real wall geometry; biasing placement toward street-facing walls specifically is a real, scoped extension. |
| 165 | Opening to the Street | Ground-floor wall opens fully onto the street, not just windows. | Same mechanism as P221's door placement, widened for ground-floor street-facing units. |
| 192 | Windows Overlooking Life | Windows face "life" (street/garden), not blank walls. | P221 already faces doors toward the nearest street/open space; extending the same logic to windows specifically is a direct, natural extension. |

### Extend `p127_intimacy_gradient.rs` / `p131_the_flow_through_rooms.rs` / `p133_staircase_as_a_stage.rs` (interior)
| # | Pattern | Real prescription | Why it's plausible |
|---|---|---|---|
| 100 | Pedestrian Street | Buildings form a pedestrian street; entrances and open stairs run directly from upper stories to the street. | Real, but needs new geometry this pipeline has never modeled -- exterior stairs. Bigger lift than most of this table, not a quick refinement. |
| 101 | Building Thoroughfare | An indoor "street" through a large complex, not a corridor. | P127/P131's cell graph already models room connectivity; a through-corridor is a specific cell-chain shape this graph can already represent. |
| 128 | Indoor Sunlight | Active-use rooms placed on the sunny (south) side. | P127's cell ordering could bias high-use cells toward south-facing walls -- real lng/lat means "south" is computed, not guessed. |
| 191 | Shape of Indoor Space | Rough rectangles, near-right-angles. | P127's cell-cutting geometry can be directly validated/constrained against this -- a real, checkable shape rule. |
| 195 | Staircase Volume | Real stair dimensions: riser+tread = 17.5in, 2-story volume, one structural bay. | P133 already carves a stair-core strip -- this is dimensioning what P133 puts inside it. Strong candidate given P133 is already real. |
| 197 | Thick Walls | Walls with real volume/depth, holding niches/built-ins. | Real, but large: every wall in this pipeline is currently zero-thickness (render.py's own caveat: punches "pierce solid mass," no real thickness). A prerequisite for P160/P198 too, not a quick win. |
| 198 | Closets Between Rooms | Closets on interior walls, between rooms, at transitions. | P127/P131's room-adjacency graph already knows which cells share a wall -- placing a closet cell there is a real, scoped extension. |

**2026-07-24 update: P112 closed.** `p130_entrance_room`'s own module doc used to name this
directly as future work: give the entrance cell a real, deliberate size instead of whatever
`band_depth_m`/bay-spacing happened to produce for every other band too. `p127_intimacy_gradient`
now has an `entrance_depth_m` parameter (`solid_bands`/`courtyard_bays`, its own "v0.4" module
doc) that carves the shallowest band/bay to a real, distinct size, with the rest of the
span/perimeter still divided into ordinary bands as before. The default (3.5m) was picked
empirically, not just architecturally: an initial, more obviously "modest" 2.0m default
measurably regressed `p112_entrance_transition`'s own real score on the eastside-baseline fixture
(0.68 mean pre-fix -> 0.32 at 2.0m), because that opinion's `MIN_FRACTION = 0.03` floor was
implicitly calibrated against the old uniform banding, and a small enough real entrance pushes
plenty of real buildings back under it. 3.5m keeps the entrance band genuinely distinct from
`band_depth_m`'s own 5.0m default (30% smaller) while measuring AT OR ABOVE the pre-fix baseline
on the real fixture across three seeds: Value 0.649-0.683 (37-54 real buildings with a real
entrance cell per seed; 65-68% fall in the real 3-35% target). `CascadeContract` entry for
p127->p112 (already present, added in an earlier session) updated with the new measured range and
its real citation-graph backing (`data/apl-pattern-graph.json`: P112 itself cites 127 directly).

### Extend `p95_building_complex.rs` / open-space shaping (enclosure quality)
| # | Pattern | Real prescription | Why it's plausible |
|---|---|---|---|
| 31 | Promenade | A promenade linking the main activity nodes, within 10 minutes' walk, attractions at both ends. | Real extension of `path_network.rs`/P61: route one path explicitly through P30's activity nodes / P61's squares as a spine, same mechanism as P120 below. |
| 53 | Main Gateways | Mark where paths cross a meaningful boundary. | The site's own perimeter (where internal streets connect to the outside) is a real, computable boundary-crossing set. |
| 59 | Quiet Backs | A protected walk behind buildings, away from the noisy front. | P105/P160-adjacent orientation data (which facade faces the street) already exists; routing a back-side path is a real extension. |
| 60 | Accessible Green | A public green within 3 minutes' (~750ft) walk of every house. | P61 already places a site-wide square budget; extending its placement to optimize for walking-distance COVERAGE (not just area-proportional allocation) is real and concrete. |
| 98 | Circulation Realms | Hierarchical zones marked by progressively-smaller gateways, for orientation in large complexes. | Bundles with P53 -- P95/P107's blocks and buildings already form implicit realms; real but the least certain of this group, since "hierarchical scale" needs a genuine multi-level zoning concept this pipeline doesn't have yet. |
| 105 | South Facing Outdoors | Buildings to the north, outdoor space to the south, no shade band between. | Already has a detector (`p105_south_facing_outdoors.rs`); P95/P107's orientation choice could be directly tuned toward this preference -- strong, concrete candidate. |
| 106 | Positive Outdoor Space | Give leftover space real enclosure, not formless spill. | P61/P95's open-space placement could be directly constrained for an enclosure ratio -- a real, checkable shape metric. |
| 114 | Hierarchy of Open Space | A smaller space forming a "back" for a larger one. | P37 common land + P61 squares + P95 courtyards already form an implicit hierarchy; a generator could deliberately shape a backing space. |
| 120 | Paths and Goals | Build paths by connecting real goals, not an abstract grid. | `path_network.rs` could route explicitly through P61 squares / P99's main building as goals instead of generic block connections. |
| 161 | Sunny Place | A real south-facing outdoor room, wind-protected, 6ft+ deep. | Direct structural parallel to the already-built P105 South Facing Outdoors detector (see above) -- a generator could carve/tag a specific zone within existing open space. |
| 163 | Outdoor Room | Enough enclosure to feel like a room, distinct from open garden. | Same enclosure-ratio mechanism as P106 -- bundle together. |

### New minor addition
| # | Pattern | Real prescription | Why it's plausible |
|---|---|---|---|
| 50 | T Junctions | Road intersections meet at ~90°, 3-way not 4-way. | `path_network.rs` already generates intersections; this is a real, enforceable geometric constraint on that existing output. |
| 51 | Green Streets | Local street surface is grass + paving stones, not asphalt. | `Street` already carries `classification`/`row_width_m`; adding a surface-material field for local/pedestrian streets is small and concrete. |

## §E. Structurally detector-only (12)

Not a gap in ambition -- these are fundamentally about program/use/social/regional data this
site-scale geometry pipeline doesn't (and mostly shouldn't) control. Same category P32 Shopping
Street, P46 Market of Many Shops, and P89 Corner Grocery already occupy today as detector-only
opinions (all three already built, grading whatever `use_category`/program tags exist in fixture
data, never inventing them).

| # | Pattern | Why it's program data, not geometry |
|---|---|---|
| 22 | Nine Per Cent Parking | Needs a real "parking lot" geometry type this pipeline has never had -- not a refinement of an existing generator, a wholly new capability. Possible future generator candidate if parking generation gets built, but not today. |
| 32 | Shopping Street | Retail tenant mix -- real economic/business data. |
| 33 | Night Life | Which businesses stay open at night -- operating-hours data. |
| 41 | Work Community | Employer/workplace assignment -- real organizational data. |
| 46 | Market of Many Shops | Same as P32 -- tenant mix. |
| 48 | Housing In Between | Mixing housing into nonresidential fabric -- needs a ground-floor-use-assignment generator that doesn't exist (shared blocker with P32/P46/P89). |
| 88 | Street Cafe | A specific business type -- program data. |
| 89 | Corner Grocery | Same as P32/P46 -- already detector-only today, for the same real reason. |
| 93 | Food Stands | A specific business type -- program data. |
| 139 | Farmhouse Kitchen | Interior furniture/room-combination choice -- a real design decision, not something this pipeline's geometric abstraction generates. |
| 147 | Communal Eating | An institutional practice/schedule -- no geometry claim at all. |
| 123 | Pedestrian Density | Needs the actual mean number of people present (P) -- unknowable without a real foot-traffic simulation this pipeline doesn't have. Likely not even honestly *detectable*, let alone generatable, without inventing a proxy that isn't real data. |

## §F. Can't build a real detector either (5)

The honest "doesn't fit this pipeline's scale at all" set -- flagged separately from §E because these
aren't even meaningfully *checkable* against a single site's geometry, let alone generatable.

| # | Pattern | Why |
|---|---|---|
| 10 | Magic of the City | A regional, multi-town downtown-spacing policy across a whole metro area -- this pipeline has no "region" or "metro area" concept, only ever operates on one site. |
| 14 | Identifiable Neighborhood | Needs multi-neighborhood context (fiscal autonomy, adjacent boundaries) this pipeline never models -- it generates one site, not a neighborhood-of-neighborhoods. |
| 15 | Neighborhood Boundary | Same -- needs an adjacent-neighborhood relationship this pipeline has no representation for. |
| 17 | Ring Roads | Regional highway network -- entirely outside site scale. |
| 23 | Parallel Roads | "Local transport area" scale (multiple neighborhoods between major roads) -- this pipeline's internal streets are site circulation, not the major-road network this pattern addresses. |

---

## Summary counts

```
§A  Already fully closed                    3
§B  Generator exists, write the detector     2
§C  Already closed under a different number  6
§D  Real generator candidates               40
§E  Structurally detector-only              12
§F  Can't build a real detector either       5
                                            ---
    Total (68-list)                        68
```
Six of the 68 (§C) already have both a real detector and a real (if differently-filed) generator,
so they're not counted again in §B/§D/§E/§F -- 3 (§A) + 6 (§C) = 9 patterns need **zero** new work.
2 (§B) need only a detector. 40 (§D) are real generator-extension candidates once they have a
detector. 17 (§E + §F) are honest non-candidates for a generator, 12 of which still deserve a
detector opinion (the 5 in §F likely can't get an honest one either).

## Suggested build order (not started -- proposal only)

1. **§B** (2 patterns): write detectors for P52 and P107 -- the generators already exist, this is
   the exact same batch recipe already used repeatedly this project.
2. **§E's 12** (skip the 5 in §F): write detector opinions for the structurally-generator-less
   patterns, since "every pattern should have a detector" doesn't wait on generator feasibility.
3. **§D's 40**: write detectors first (same recipe), THEN pick generator work by cluster (roof/facade
   family is the biggest single lift since it needs new roof geometry as a prerequisite; the
   density/activity and opening/entrance clusters are the cheapest since they extend generators that
   already do almost the right thing).
4. **§F's 5**: revisit only if the project's scope ever grows past single-site generation.
