//! The corrected pipeline, end to end: Alexander's own pattern numbering
//! (29 < 37 < 52 < 61 < 95 < 107) run in that order, instead of the old
//! pipeline's P95 -> BlockGrouping -> PathNetwork -> P107 -> P61.
//!
//! Sequence:
//!   1. P29 Density Rings (once, site-scale): computes a real density
//!      FIELD over the raw site parcel's own polygon -- Alexander's own
//!      true canonical position (29 < 37), now possible because
//!      `Neighborhood.pattern_fields` gives a larger, prior pattern
//!      somewhere to attach its own output without waiting for a smaller
//!      one (a block) to already exist. See p29's own module doc and
//!      `PATTERN_ORDERING_AUDIT.md` (repo root) for the real bug this
//!      closes. Not fatal if it fails (a degenerate site polygon) --
//!      P37 below tolerates a missing field exactly like a pipeline that
//!      never ran P29 at all.
//!   2. P37 (once, site-scale): carve the raw parcel into BLOCK_n blocks,
//!      each with an informal `OpenSpaceKind::Common` patch reserved for
//!      that cluster's own shared land (see p37's module doc). Samples
//!      P29's real field (if step 1 produced one) at each new block's own
//!      centroid as it's created, setting `density_tier`/`target_stories`
//!      directly -- the individuation moment itself, not a separate later
//!      pass over already-carved blocks.
//!   3. PathNetwork/P52 (once, site-scale): connect the blocks -- unchanged
//!      code, already filters by `spec.starts_with("BLOCK_")`, which P37
//!      produces directly. Also populates the site's own real perimeter
//!      `Boundary` (its own "v0.5" module doc), which step 4 depends on.
//!   4. P53 Main Gateways (once, site-scale): marks each real site
//!      `Boundary` with a real Gateway-kind `ActivityNode` at its nearest
//!      crossing street endpoint, when one exists within
//!      `gateway_threshold_m` -- the exact same real proxy definition
//!      `p53_main_gateways.rs`'s own opinion already scores against, so
//!      the generator and the opinion can't silently disagree about what
//!      counts as a marked gateway. Runs right after step 3 since it only
//!      needs `Neighborhood.boundaries`/`.streets`, both of which
//!      PathNetwork just populated -- doesn't touch parcels/blocks, so its
//!      position relative to steps 5+ doesn't matter. Not fatal if it
//!      fails (no boundary sits close enough to any real street endpoint).
//!   5. P61 (site-scale budget, spread across blocks): Alexander's "a few"
//!      public squares means a handful across the WHOLE site, not
//!      `max_squares` repeated on every block -- see p61's own module doc,
//!      "v0.6" section, for why that was the biggest single contributor to
//!      block-level fragmentation. `allocate_squares_by_area` splits the
//!      site's `max_squares` budget across blocks proportional to block
//!      area (largest-remainder rounding); most blocks get zero. Squares
//!      are seeded on whatever land P37's common land didn't already claim
//!      on that block (`place_new_squares_n` subtracts existing reserved
//!      open space before seeding).
//!   6. Per block: P95 (reworked) builds pads around whatever P37/P61
//!      placed on that block, plus street corridors from step 3 -- each pad
//!      inherits its source block's P29 density tier/target. `pad_inset_m`
//!      is now a construction-joint-sized 0.1m, not a real setback.
//!   7. P21 Four-Story Limit (once, site-scale): caps every pad's
//!      field-inherited target at the ordinary ceiling -- a pure per-pad
//!      read of whatever P29/P37 already sampled onto it, needing nothing
//!      P108 (next) produces. Split out of P96 (PATTERN_ORDERING_AUDIT.md
//!      §4.2 -- a "mixed" deviation, half of which really was a free-
//!      standing field read forced to wait on P108 for no real reason), so
//!      it now runs at its own true earliest position, right after P95.
//!   8. P108 Connected Buildings (once, site-scale): merges pads separated
//!      by nothing but a construction joint into one continuous
//!      party-wall footprint -- pads separated by a real reserved gap
//!      (street, square, common land) stay apart. Runs before P96/P107 so
//!      daylight-depth shaping sees the real, final connected mass; see
//!      p108's own module doc for why that deviates from Alexander's
//!      numbering (108, after Wings of Light).
//!   9. P96 Number of Stories (once, site-scale): picks the very few pads
//!      that get to exceed P21's ordinary cap, "placed with great care...
//!      widely spaced" -- genuinely needs P108's final merged footprint to
//!      rank (largest-first) and space them, so runs here, not alongside
//!      P21 above. Resamples P29's own field at each pad's real, final
//!      centroid rather than trusting inherited `density_tier`, closing a
//!      caveat P108's own module doc names honestly (merged pads don't
//!      reconcile tier against their cluster-mates).
//!   10. P107 (once, site-scale): shape every P95/P108 pad for daylight
//!      depth, reading P96's `target_stories` assignment for real height --
//!      unless P21/P96 didn't run, the flat `assumed_height_m` fallback
//!      applies exactly as before either existed. Already filters by
//!      `use_category == "p95_building_pad"` across the whole neighborhood
//!      regardless of which block a pad came from, and no longer applies
//!      its own setback on top of a P95/P108 pad's own gap (see p107's
//!      "v0.2" module doc).
//!   11. P117 Sheltering Roof (once, site-scale): assigns every real
//!      building with a real `height_m` a real shed roof (eave = its own
//!      real height_m, ridge = that plus a real, modest roof_rise_m),
//!      sloping down to true north -- also closes P162 North Face's own
//!      specifically-north claim by the same real geometry. Only needs
//!      real height (already real by this point via P96/P107's own
//!      fallback), not footprint or wall thickness -- no real dependency
//!      on P124/P127/P197 either direction (PATTERN_ORDERING_AUDIT.md
//!      §4.3/§4.4: a free reorder), so placed here, ahead of P124, to
//!      match Alexander's own ascending numbering (117 < 124) at zero
//!      cost. See p117_sheltering_roof's own module doc for why one shed
//!      roof honestly satisfies both patterns at once, and what it
//!      deliberately doesn't claim (P116's per-wing cascade, P118's roof
//!      garden, P119/P166's canopy geometry).
//!   12. P118 Roof Garden (once, site-scale): overwrites the site's own
//!      `max_gardens` tallest real buildings from P117's sloped shed roof
//!      to a flat, occupiable garden roof. Used to wait until after P221
//!      purely for real `Building.floors` to rank by -- P107 now derives
//!      floors itself, at the same moment it derives height_m (PATTERN_
//!      ORDERING_AUDIT.md §4.7), so this runs right after P117 to match
//!      Alexander's own ascending numbering (117 < 118) at zero real cost:
//!      P118 only ever touches `roof`, nothing P124/P127/P197/P221 read
//!      or write. A real bonus, not just neutral -- P116 (below) now sees
//!      each building's FINAL roof, already flat where P118 applied,
//!      instead of building a cascade from a shed roof about to be
//!      overwritten anyway. A small fixed count, not a floor threshold --
//!      see p118_roof_garden's own module doc for why a threshold was
//!      tried and rejected (it tanked P117/P162's own real sloped-roof
//!      score site-wide on this pipeline's real fixture). Not fatal if no
//!      building qualifies.
//!   13. P124 Activity Pockets (once, site-scale): bumps a small, real,
//!      partly enclosed pocket out from up to max_pockets_per_plaza
//!      buildings bordering each real Plaza (from P61), reading "jut
//!      forward into the open space" literally -- a real projection
//!      OUTWARD from the building's own footprint, toward the plaza, not
//!      a recession into it. Runs right after P107/P117/P118 and strictly
//!      before P127/P197/P119/P221 -- all of which need the FINAL building
//!      footprint, not a stale pre-bump one. See p124_activity_pockets's
//!      own module doc for the geometric reading and its own hard-won
//!      note on why it splices the bump into the ring directly rather
//!      than using subtract_convex + union_pieces (the same reassembly-
//!      reliability problem P95/P133 already hit).
//!   14. P119 Arcades / P166 Gallery Surround (once, site-scale): places a
//!      real ground-floor arcade canopy, plus a gallery canopy at every
//!      real upper story, on whichever wall best faces the nearest real
//!      street/open space. Reads the FINAL footprint (whatever P124 left
//!      it as -- not a hard requirement, same real reason P197 doesn't
//!      hard-require P124 either: it's a real but skippable step,
//!      currently a no-op on the real eastside-baseline fixture) and real
//!      `Building.floors` for P166's "every story" claim -- floors no
//!      longer needs P221 (see P118 above), so this runs right after
//!      P124, its own real earliest valid position. Not fatal if no
//!      building has a real street/open-
//!      space target within threshold. See p119_arcades's own module doc.
//!   15. P127 Intimacy Gradient (once, site-scale): partitions every
//!      building's ground floor into a depth-ordered sequence of cells
//!      (public wall/entrance bay -> deepest point). Still needs P107's
//!      final footprint, which P124 (above) may have bumped -- P124
//!      itself already requires running before P127 (see its own module
//!      doc). No real dependency on P197 either direction
//!      (PATTERN_ORDERING_AUDIT.md §4.4: a free reorder), so placed here,
//!      ahead of P197, to match Alexander's own ascending numbering
//!      (127 < 197) at zero cost. See `p127_intimacy_gradient`'s own
//!      module doc for the full sourced sequence Alexander's own text
//!      lays out (127 -> 128 -> 129 -> 130 -> 131 -> 132 -> 133...).
//!   16. P197 Thick Walls (once, site-scale): assigns every real building a
//!      real, nonzero `wall_thickness_m`, capped relative to its own real
//!      footprint. Runs after P107, P124, and P117 -- every downstream
//!      stage clones and mutates the buildings those produced, so this
//!      field survives untouched to the end. Deliberately scalar-only,
//!      not carved geometry -- see p197_thick_walls's own module doc.
//!   17. P116 Cascade of Roofs (once, site-scale): partitions every
//!      already-roofed building's roof into per-cell `RoofSegment`s whose
//!      ridge height cascades with P127's own cell `depth` -- the same
//!      "social hierarchy of the spaces below" Alexander's own text names,
//!      reused rather than independently re-derived. Runs right after P127
//!      -- needs both P117's (possibly P118-flattened) whole-building
//!      `roof` and P127's real `interior_cells` to exist first. See
//!      p116_cascade_of_roofs's own module doc for why it reuses P127's
//!      cell polygons as the roof's wing partition instead of computing a
//!      separate one.
//!   18. P129 Common Areas at the Heart (once, site-scale): marks which of
//!      P127's cells is nearest the plan's center of gravity. No real
//!      dependency on P130 either direction (PATTERN_ORDERING_AUDIT.md
//!      §4.6 -- P130 never changes cell geometry or count) -- placed here,
//!      ahead of P130, to match BOTH Alexander's own ascending numbering
//!      (129 < 130) AND his own cited textual sequence (127 -> 128 -> 129
//!      -> 130 -> 131...) at zero cost.
//!   19. P130 Entrance Room (once, site-scale): tags the cell P127 built at
//!      depth 0.0 as `kind: "entrance"` -- a label only, no geometry
//!      change, see the module's own doc for why.
//!   20. P131 The Flow Through Rooms (once, site-scale): connects P127's
//!      cells -- a closed loop for free on courtyard buildings (the ring
//!      already is one), a chain for solid buildings, closed into a real
//!      loop with one passage cell only when short and wide enough (Pattern
//!      132's own cited ~50ft/15m threshold, folded into this operator).
//!   21. P133 Staircase as a Stage (once, site-scale): carves a real
//!      stair-core strip out of the common-area cell of every multi-story
//!      building, open to the room it interrupts. Needs P131's
//!      `connects_to` graph, P129's `is_common` flag, and real
//!      `Building.floors` -- all three are real by this point (P107 now
//!      derives floors itself; PATTERN_ORDERING_AUDIT.md §4.7), so this
//!      runs right after P131, Alexander's own exact canonical position
//!      (131 < 133) -- restoring the very ordering this operator's own
//!      module doc used to record as its real, hard-won bug (running here
//!      used to leave every building's `floors` at `None`, since only
//!      P221 derived it, silently matching nothing). Doesn't read or
//!      write anything P221 touches (confirmed: p221 never reads
//!      `interior_cells`) -- real, independent geometry. See the module's
//!      own doc for the clip_half_plane-based strip technique, borrowed
//!      from P131's own passage cell, and the union_pieces bug it
//!      replaced.
//!   22. P221 (once, site-scale): place real window/door openings on every
//!      building P107 just produced -- floor count from real height, window
//!      bays from real wall geometry, door on whichever wall faces the
//!      nearest street/open space. No randomness. See
//!      `p221_natural_doors_and_windows`'s own module doc for the pattern
//!      graph this closes (P107 -> P159 -> P221).
//!   23. P192 Windows Overlooking Life (once, site-scale): relocates a
//!      real ground-floor outer window P221 already placed to a different
//!      real wall edge when its current position fails a real proximity
//!      check (no street/open space within view_threshold_m, or blocked by
//!      a nearby building within blocked_threshold_m). Never adds or
//!      removes a window, only moves one that's already there. Needs
//!      P221's real openings to exist first. See
//!      p192_windows_overlooking_life's own module doc for the exact same
//!      proxy its own opinion already checks.
//!   24. P164 Street Windows (once, site-scale): adds a real ground-floor
//!      window to a street-adjacent building that P221 left blind (no
//!      window near a Local/Pedestrian street). Runs after P192 so windows
//!      it relocates away from a blind wall aren't immediately
//!      "un-blinded" by this step reading stale state, and before P165/
//!      P100 below since those also add openings to the same walls. See
//!      p164_street_windows's own module doc.
//!   25. P165 Opening to the Street (once, site-scale): adds real
//!      ground-floor windows along a building's own door wall edge until
//!      real opening-width coverage on that edge reaches
//!      `target_coverage`. Needs P221's real door placement to know which
//!      edge is the street-facing one. See p165_opening_to_the_street's
//!      own module doc.
//!   26. P100 Pedestrian Street (once, site-scale): adds a real
//!      ground-floor door to a doorless building near a Pedestrian-
//!      classified street until real entrance density along that street
//!      reaches `min_entrances_per_100m`. Never gives a second door to a
//!      building that already has one (see p100_pedestrian_street's own
//!      module doc for the real P110 Main Entrance tension this creates:
//!      P110 scores `1.0 / door_count` per building, so a future version
//!      of this operator that added second doors would lower P110's
//!      score -- this one never does). Runs after P165 so the coverage
//!      target above is computed before new doors change which edge
//!      qualifies as the "door wall."
//!   27. P160 Building Edge (once, site-scale): places a real wall niche
//!      flanking every real door P221 already placed, on the same wall
//!      edge. Runs after P221 for the real door data to flank -- a
//!      genuine Class C dependency (PATTERN_ORDERING_AUDIT.md §4.8), not a
//!      free reorder like the others above. Not fatal if no door has room
//!      for a niche on either side. See p160_building_edge's own module
//!      doc.
//!   28. P161 Sunny Place (once, site-scale): adds a small, real,
//!      south-facing OpenSpace (reusing `OpenSpaceKind::Common` -- no
//!      dedicated "Sunny" variant exists, see p161_sunny_place's own
//!      module doc for why that's an honest reuse, not a perfect fit) next
//!      to a building that doesn't already have one, rejecting any
//!      candidate that would overlap a real building footprint. Runs
//!      LAST, after every other stage: earlier placement would change
//!      what P221 (long since run by now) reads as "real life" near a
//!      wall, perturbing every downstream stage and test expectation for
//!      zero real benefit. Also flags a real cross-pattern regression:
//!      these small spaces will lower `p60_accessible_green`'s own
//!      qualifying-size sub-score, since nothing about the site's actual
//!      large greens changed.
//!
//! This is the single real orchestration function; `tests/corrected_pipeline.rs`
//! is the proof it composes end to end on real data, and `examples/dump_pipeline.rs`
//! is what produces fixtures for the external 3D vibe-check
//! (`tools/vibe-render/`). The web UI's "Run full pipeline" button re-implements
//! the same sequence (including the area-proportional square allocation)
//! client-side in JS, since it needs to update the map after each stage
//! rather than only at the end.

use crate::p21_four_story_limit::{P21FourStoryLimit, P21Params};
use crate::p107_wings_of_light::{P107Params, P107WingsOfLight};
use crate::p108_connected_buildings::{P108ConnectedBuildings, P108Params};
use crate::p116_cascade_of_roofs::{P116CascadeOfRoofs, P116Params};
use crate::p127_intimacy_gradient::{P127IntimacyGradient, P127Params};
use crate::p129_common_areas_at_the_heart::{P129CommonAreasAtTheHeart, P129Params};
use crate::p130_entrance_room::{P130EntranceRoom, P130Params};
use crate::p131_the_flow_through_rooms::{P131Params, P131TheFlowThroughRooms};
use crate::p124_activity_pockets::{P124ActivityPockets, P124Params};
use crate::p117_sheltering_roof::{P117Params, P117ShelteringRoof};
use crate::p118_roof_garden::{P118Params, P118RoofGarden};
use crate::p119_arcades::{P119Arcades, P119Params};
use crate::p100_pedestrian_street::{P100Params, P100PedestrianStreet};
use crate::p133_staircase_as_a_stage::{P133Params, P133StaircaseAsAStage};
use crate::p160_building_edge::{P160BuildingEdge, P160Params};
use crate::p161_sunny_place::{P161Params, P161SunnyPlace};
use crate::p164_street_windows::{P164Params, P164StreetWindows};
use crate::p165_opening_to_the_street::{P165OpeningToTheStreet, P165Params};
use crate::p192_windows_overlooking_life::{P192Params, P192WindowsOverlookingLife};
use crate::p197_thick_walls::{P197Params, P197ThickWalls};
use crate::p221_natural_doors_and_windows::{P221NaturalDoorsAndWindows, P221Params};
use crate::p29_density_rings::{P29DensityRings, P29Params};
use crate::p37_house_cluster::{P37HouseCluster, P37Params};
use crate::p53_main_gateways::{P53MainGateways, P53Params};
use crate::p61_small_public_squares::{place_new_squares_n, P61Params, P61SmallPublicSquares};
use crate::p95_building_complex::{P95BuildingComplex, P95Params};
use crate::p96_number_of_stories::{P96NumberOfStories, P96Params};
use crate::path_network::{PathNetwork, PathNetworkParams};
use crate::{apply_subdivision, Parameters, PatternOperator, Subdivision};
use street_smarts_core::nir::Neighborhood;
use street_smarts_core::Scope;

/// Run `f` once per block in `block_ids`, in declared order. `f` receives
/// the neighborhood state as of the start of that block's turn and this
/// block's own derived seed (`base_seed + index + 1` -- the exact
/// convention the P61/P95 per-block loop below already used by hand), and
/// returns an ORDERED sequence of steps for that block (e.g. `[p61_result,
/// p95_result]`), since later steps within one block routinely depend on
/// earlier ones already being applied -- P95 must see the block parcel
/// AFTER P61 has carved a square out of it, not before. `f` is expected to
/// fold its own steps internally (starting from a clone of the state it
/// was given) purely to compute each step correctly against the one
/// before it; `run_per_block` then re-applies that same ordered sequence
/// to its own running state, which is deterministic and produces an
/// identical result without `f` needing write access to the outer loop's
/// state directly. This is that loop's shape, extracted so the next
/// block-scale pattern doesn't have to re-derive it. See
/// PATTERN_LANGUAGE_SIMULATION.md §3.2.
///
/// A step that returns `Err` is skipped, not aborted -- same tolerance the
/// original loop and the web UI's per-block loop apply, since a real
/// rejection (a block too small to be worthwhile) is expected often enough
/// not to be a hard failure. Unlike the original loop, the skip reason is
/// collected rather than silently dropped -- see HARDENING_SPEC.md §1.3,
/// which wanted exactly this so a future check can tell "too small" apart
/// from "geometry op produced garbage" instead of both looking identical
/// after the fact.
pub fn run_per_block<F>(
    nbhd: &Neighborhood,
    block_ids: &[String],
    base_seed: u64,
    mut f: F,
) -> (Neighborhood, Vec<(String, String)>)
where
    F: FnMut(&Neighborhood, &str, u64) -> Vec<Result<Subdivision, String>>,
{
    let mut out = nbhd.clone();
    let mut skipped = Vec::new();
    for (i, block_id) in block_ids.iter().enumerate() {
        let block_seed = base_seed + i as u64 + 1;
        for step in f(&out, block_id, block_seed) {
            match step {
                Ok(sub) => out = apply_subdivision(&out, &sub),
                Err(reason) => skipped.push((block_id.clone(), reason)),
            }
        }
    }
    (out, skipped)
}

/// Split a total square budget across blocks proportional to block area,
/// using largest-remainder rounding so the sum of the returned counts is
/// exactly `total_squares` (not `total_squares` per block). Most blocks in
/// a real site end up with zero -- matching real precedent (Barcelona's
/// superilles place plaza nodes roughly one per several acres, not one per
/// block).
pub fn allocate_squares_by_area(block_areas_m2: &[f64], total_squares: usize) -> Vec<usize> {
    let total_area: f64 = block_areas_m2.iter().sum();
    if total_area <= 0.0 || total_squares == 0 {
        return vec![0; block_areas_m2.len()];
    }
    let raw_shares: Vec<f64> = block_areas_m2.iter()
        .map(|a| total_squares as f64 * a.max(0.0) / total_area)
        .collect();
    let mut counts: Vec<usize> = raw_shares.iter().map(|s| s.floor() as usize).collect();
    let assigned: usize = counts.iter().sum();
    let mut leftover = total_squares.saturating_sub(assigned);

    let mut remainders: Vec<(usize, f64)> = raw_shares.iter().map(|s| s.fract()).enumerate().collect();
    remainders.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (i, _) in remainders {
        if leftover == 0 { break; }
        counts[i] += 1;
        leftover -= 1;
    }
    counts
}

/// Run the full corrected pipeline on `parcel_id` within `baseline`, using
/// each operator's default parameters. `seed` drives P37's block seeding and
/// PathNetwork's site-scale steps directly; each block's P61/P95 pass gets
/// a distinct derived seed (`seed + block_index + 1`) so blocks don't all
/// place identical squares.
///
/// A block that fails P61 or P95 (e.g. too small to be worthwhile) is
/// skipped rather than aborting the whole run -- same tolerance the web UI's
/// per-block loop applies.
pub fn run_corrected_pipeline(baseline: &Neighborhood, parcel_id: &str, seed: u64) -> Neighborhood {
    run_corrected_pipeline_with_p37(baseline, parcel_id, seed, &P37Params::defaults())
}

/// Same as `run_corrected_pipeline`, but with an explicit P37 parameter set
/// -- lets callers (currently just `examples/dump_pipeline_seeding.rs`, for
/// comparing P37's `seeding_mode` prototype against production) swap P37's
/// behavior without touching every other stage. `run_corrected_pipeline`
/// itself stays a thin wrapper over this with P37's defaults, so production
/// behavior is unchanged.
pub fn run_corrected_pipeline_with_p37(
    baseline: &Neighborhood,
    parcel_id: &str,
    seed: u64,
    p37_params: &P37Params,
) -> Neighborhood {
    run_corrected_pipeline_with_p37_traced(baseline, parcel_id, seed, p37_params).0
}

/// Same as `run_corrected_pipeline_with_p37`, but also returns the real,
/// literal sequence of operator ids that actually ran -- respecting every
/// `if let Ok(...)` skip -- as it executed, not a hand-copied guess at what
/// the function does. This is what closes `language_graph.rs`'s own
/// documented limitation ("does not, by itself, keep this table in sync
/// with `pipeline.rs`'s actual call sequence"): its test calls this
/// function and validates the returned trace, not a second, independently-
/// maintained literal that could silently drift from this one.
/// `run_corrected_pipeline_with_p37` is a thin wrapper that discards the
/// trace, so every existing caller/test is unaffected.
pub fn run_corrected_pipeline_with_p37_traced(
    baseline: &Neighborhood,
    parcel_id: &str,
    seed: u64,
    p37_params: &P37Params,
) -> (Neighborhood, Vec<&'static str>) {
    let mut trace: Vec<&'static str> = Vec::new();

    // P29 now runs at its own true canonical position (29 < 37) -- see
    // p29_density_rings's own module doc and PATTERN_ORDERING_AUDIT.md.
    // It needs nothing but the RAW site parcel `baseline` already has, and
    // only attaches a real DensityField to `pattern_fields`; it doesn't
    // touch any parcel. Not fatal if it fails (a degenerate parcel
    // polygon): P37 below tolerates a missing field exactly like a
    // pipeline that never ran P29 at all.
    let mut nbhd = baseline.clone();
    if let Ok(sub29) = P29DensityRings.apply(&nbhd, parcel_id, &P29Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub29);
        trace.push(P29DensityRings.name());
    }

    let sub37 = P37HouseCluster.apply(&nbhd, parcel_id, p37_params, seed).unwrap();
    nbhd = apply_subdivision(&nbhd, &sub37);
    trace.push(P37HouseCluster.name());

    let sub52 = PathNetwork.apply(&nbhd, "*", &PathNetworkParams::defaults(), seed).unwrap();
    nbhd = apply_subdivision(&nbhd, &sub52);
    trace.push(PathNetwork.name());

    // P53 Main Gateways (once, site-scale): marks the site's own real
    // perimeter boundary (just populated above) with a real Gateway node
    // at its nearest crossing street endpoint, if one exists within the
    // real threshold. Not fatal if it fails (no street endpoint close
    // enough to any boundary) -- see p53_main_gateways.rs's own module doc.
    if let Ok(sub53) = P53MainGateways.apply(&nbhd, "*", &P53Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub53);
        trace.push(P53MainGateways.name());
    }

    let block_ids: Vec<String> = nbhd.select_ids(&Scope::Block);
    let block_areas: Vec<f64> = block_ids.iter()
        .map(|id| nbhd.parcels.iter().find(|p| &p.id == id).map(|p| p.polygon.area_m2()).unwrap_or(0.0))
        .collect();
    let total_squares = P61Params::defaults().max_squares.round().max(1.0) as usize;
    let square_counts = allocate_squares_by_area(&block_areas, total_squares);

    // Tracked at pipeline-sequence granularity ("did this stage run at all
    // this pipeline pass"), not per-block -- matching what `LANGUAGE`
    // models. A block that's skipped (too small, etc.) doesn't change
    // whether P61/P95 count as having run, as long as at least one block
    // succeeded.
    let p61_ran = std::cell::Cell::new(false);
    let p95_ran = std::cell::Cell::new(false);
    let (folded, _skipped) = run_per_block(&nbhd, &block_ids, seed, |state, block_id, block_seed| {
        let mut steps = Vec::new();
        let mut local = state.clone();
        let n_squares = square_counts[block_ids.iter().position(|b| b == block_id).unwrap()];
        if n_squares > 0 {
            if let Some(block_parcel) = local.parcels.iter().find(|p| &p.id == block_id).cloned() {
                let sub61 = place_new_squares_n(
                    &local, &block_parcel, n_squares, &P61Params::defaults(), block_seed, P61SmallPublicSquares.source(),
                );
                if let Ok(sub) = &sub61 {
                    local = apply_subdivision(&local, sub);
                    p61_ran.set(true);
                }
                steps.push(sub61);
            }
        }
        let sub95 = P95BuildingComplex.apply(&local, block_id, &P95Params::defaults(), block_seed);
        if sub95.is_ok() {
            p95_ran.set(true);
        }
        steps.push(sub95);
        steps
    });
    nbhd = folded;
    if p61_ran.get() {
        trace.push(P61SmallPublicSquares.name());
    }
    if p95_ran.get() {
        trace.push(P95BuildingComplex.name());
    }

    // P21 Four-Story Limit (once, site-scale): caps every pad's
    // field-inherited target at the ordinary ceiling -- a pure per-pad
    // read that needs nothing from P108's merge (PATTERN_ORDERING_AUDIT.md
    // §4.2), so it runs right here, before P108, at its own real earliest
    // position. p96_number_of_stories (below) picks the very few
    // exceptions after P108 has produced final, merged footprints.
    if let Ok(sub21) = P21FourStoryLimit.apply(&nbhd, "*", &P21Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub21);
        trace.push(P21FourStoryLimit.name());
    }

    if let Ok(sub108) = P108ConnectedBuildings.apply(&nbhd, "*", &P108Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub108);
        trace.push(P108ConnectedBuildings.name());
    }

    // P96 Number of Stories (once, site-scale): picks the very few pads
    // that get to exceed P21's ordinary cap, "placed with great care...
    // widely spaced" -- needs P108's final merged footprint to rank and
    // space them (PATTERN_ORDERING_AUDIT.md §4.2), so runs here, after
    // P108, not before it like P21 above.
    if let Ok(sub96) = P96NumberOfStories.apply(&nbhd, "*", &P96Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub96);
        trace.push(P96NumberOfStories.name());
    }

    if let Ok(sub107) = P107WingsOfLight.apply(&nbhd, "*", &P107Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub107);
        trace.push(P107WingsOfLight.name());
    }

    // P124 Activity Pockets (once, site-scale): bumps a real, small
    // pocket out from up to max_pockets_per_plaza buildings bordering each
    // real Plaza. Runs right after P107 (needs real final building
    // footprints AND real Plazas from P61) and strictly BEFORE P197 (so
    // wall thickness applies to the post-bump footprint), P127 (so
    // interior cells partition the post-bump footprint, not a stale
    // one), and P221 (so window/door ring_index references the final
    // outer ring). Not fatal if no building qualifies -- currently never
    // qualifies on the real eastside-baseline fixture (see
    // p124_activity_pockets's own module doc for the real measured
    // numbers), so this stage is a real no-op there today, not broken.
    // P117 Sheltering Roof (once, site-scale): assigns every real building
    // with a real height_m a real shed roof, sloped down to true north --
    // also closes P162 North Face's own specifically-north claim by the
    // same geometry. Only needs height_m (already real by this point via
    // P96/P107's own fallback), not footprint or wall thickness -- no real
    // dependency on P124/P127/P197 either direction (PATTERN_ORDERING_
    // AUDIT.md §4.3/§4.4: a free reorder), so placed here, ahead of P124,
    // to match Alexander's own ascending numbering (117 < 124) at zero
    // cost. See p117_sheltering_roof's own module doc for what it does
    // and deliberately doesn't claim.
    if let Ok(sub117) = P117ShelteringRoof.apply(&nbhd, "*", &P117Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub117);
        trace.push(P117ShelteringRoof.name());
    }

    // P118 Roof Garden (once, site-scale): overwrites the site's own
    // max_gardens tallest real buildings from P117's sloped shed roof to a
    // flat, occupiable garden roof. Used to wait for P221 purely for real
    // Building.floors -- P107 now derives floors itself (PATTERN_ORDERING_
    // AUDIT.md §4.7), so this runs right after P117 to match Alexander's
    // own ascending numbering (117 < 118) at zero real cost: P118 only
    // touches `roof`, never footprint/cells/openings. A real bonus, not
    // just neutral -- p116_cascade_of_roofs (below) now sees each
    // building's FINAL roof (already flat where P118 applied) instead of
    // building degenerate cascade segments from a shed roof P118 was about
    // to overwrite anyway. Not fatal if no building qualifies.
    if let Ok(sub118) = P118RoofGarden.apply(&nbhd, "*", &P118Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub118);
        trace.push(P118RoofGarden.name());
    }

    if let Ok(sub124) = P124ActivityPockets.apply(&nbhd, "*", &P124Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub124);
        trace.push(P124ActivityPockets.name());
    }

    // P119 Arcades / P166 Gallery Surround (once, site-scale): places a
    // real ground-floor arcade plus upper-story galleries on each
    // building's real street/open-space-facing wall. Needs P124's FINAL
    // footprint (a real dependency -- Class C, not free) and real
    // Building.floors for P166's "every story" claim -- P107 now derives
    // floors itself (PATTERN_ORDERING_AUDIT.md §4.7), so this no longer
    // needs to wait for P221 specifically; runs right after P124, its own
    // real earliest valid position. Not fatal if no building has a real
    // facing target within threshold.
    if let Ok(sub119) = P119Arcades.apply(&nbhd, "*", &P119Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub119);
        trace.push(P119Arcades.name());
    }

    // P127 Intimacy Gradient (once, site-scale): partitions every
    // building's ground floor into a depth-ordered cell sequence. No real
    // dependency on P197 either direction (PATTERN_ORDERING_AUDIT.md
    // §4.4: a free reorder) -- placed here, ahead of P197, to match
    // Alexander's own ascending numbering (127 < 197) at zero cost. Still
    // needs P107's final footprint, which P124 (just above) may have
    // bumped -- P124 itself already requires running before P127 (see its
    // own module doc), so this ordering is unaffected.
    if let Ok(sub127) = P127IntimacyGradient.apply(&nbhd, "*", &P127Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub127);
        trace.push(P127IntimacyGradient.name());
    }

    // P197 Thick Walls (once, site-scale): assigns every real building a
    // real wall_thickness_m, capped relative to its own footprint. Runs
    // after P107, P124, and P117, so a bumped, roofed building's final
    // footprint is what gets a thickness -- every downstream stage
    // clones-and-mutates from here, so the field survives to the end of
    // the pipeline untouched. See p197_thick_walls's own module doc.
    if let Ok(sub197) = P197ThickWalls.apply(&nbhd, "*", &P197Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub197);
        trace.push(P197ThickWalls.name());
    }

    if let Ok(sub116) = P116CascadeOfRoofs.apply(&nbhd, "*", &P116Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub116);
        trace.push(P116CascadeOfRoofs.name());
    }

    // P129 Common Areas at the Heart (once, site-scale): marks which of
    // P127's cells is nearest the plan's center of gravity. No real
    // dependency on P130 either direction (PATTERN_ORDERING_AUDIT.md
    // §4.6 -- P130 never changes cell geometry or count, so nothing about
    // this operator's own center-of-gravity computation depends on
    // whether the entrance cell has been relabeled yet) -- placed here,
    // ahead of P130, to match BOTH Alexander's own ascending numbering
    // (129 < 130) AND his own cited textual sequence (127 -> 128 -> 129 ->
    // 130 -> 131...) at zero cost.
    if let Ok(sub129) = P129CommonAreasAtTheHeart.apply(&nbhd, "*", &P129Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub129);
        trace.push(P129CommonAreasAtTheHeart.name());
    }

    if let Ok(sub130) = P130EntranceRoom.apply(&nbhd, "*", &P130Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub130);
        trace.push(P130EntranceRoom.name());
    }

    if let Ok(sub131) = P131TheFlowThroughRooms.apply(&nbhd, "*", &P131Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub131);
        trace.push(P131TheFlowThroughRooms.name());
    }

    // P133 Staircase as a Stage (once, site-scale): carves a real
    // stair-core strip out of the common-area cell of every multi-story
    // building, open to the room it interrupts. Needs P131's connects_to
    // graph, P129's is_common flag, and real Building.floors -- all three
    // are real by this point (P107 now derives floors itself; PATTERN_
    // ORDERING_AUDIT.md §4.7), so this runs right after P131, Alexander's
    // own exact canonical position (131 < 133), restoring the numbering
    // this operator's own module doc records as its original real bug
    // (running here used to leave floors at None, since only P221 derived
    // it). Doesn't read or write anything P221 touches (openings) -- real,
    // independent geometry (interior stair core vs exterior wall
    // punches), confirmed by reading p221's own code: it never reads
    // interior_cells.
    if let Ok(sub133) = P133StaircaseAsAStage.apply(&nbhd, "*", &P133Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub133);
        trace.push(P133StaircaseAsAStage.name());
    }

    if let Ok(sub221) = P221NaturalDoorsAndWindows.apply(&nbhd, "*", &P221Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub221);
        trace.push(P221NaturalDoorsAndWindows.name());
    }

    // P192 Windows Overlooking Life (once, site-scale): relocates a real
    // ground-floor window P221 just placed if it fails a real proximity
    // check. Needs P221's real openings. See pipeline.rs's own step 23 doc.
    if let Ok(sub192) = P192WindowsOverlookingLife.apply(&nbhd, "*", &P192Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub192);
        trace.push(P192WindowsOverlookingLife.name());
    }

    // P164 Street Windows (once, site-scale): adds a real window to a
    // street-adjacent building P221/P192 left blind. See pipeline.rs's
    // own step 24 doc.
    if let Ok(sub164) = P164StreetWindows.apply(&nbhd, "*", &P164Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub164);
        trace.push(P164StreetWindows.name());
    }

    // P165 Opening to the Street (once, site-scale): adds real windows
    // along a building's own door wall until opening coverage meets
    // target. See pipeline.rs's own step 25 doc.
    if let Ok(sub165) = P165OpeningToTheStreet.apply(&nbhd, "*", &P165Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub165);
        trace.push(P165OpeningToTheStreet.name());
    }

    // P100 Pedestrian Street (once, site-scale): adds a real door to a
    // doorless building near a Pedestrian street until entrance density
    // meets target. See pipeline.rs's own step 26 doc (including the real
    // P110 Main Entrance tension this creates).
    if let Ok(sub100) = P100PedestrianStreet.apply(&nbhd, "*", &P100Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub100);
        trace.push(P100PedestrianStreet.name());
    }

    // P160 Building Edge (once, site-scale): places a real wall niche
    // flanking every real door P221 already placed. Needs real Opening
    // data, so runs after P221. Not fatal if no door has room for one.
    if let Ok(sub160) = P160BuildingEdge.apply(&nbhd, "*", &P160Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub160);
        trace.push(P160BuildingEdge.name());
    }

    // P161 Sunny Place (once, site-scale): adds a small real south-facing
    // open space next to a building that lacks one. Runs LAST -- see
    // pipeline.rs's own step 28 doc for why (and the real P60 Accessible
    // Green cross-pattern regression this creates).
    if let Ok(sub161) = P161SunnyPlace.apply(&nbhd, "*", &P161Params::defaults(), seed) {
        nbhd = apply_subdivision(&nbhd, &sub161);
        trace.push(P161SunnyPlace.name());
    }

    (nbhd, trace)
}
