// node --test tools/sparrow/
// The walk page's pure parts: what it says about the model, how the camera
// eases, and how the governor steers resolution. The answers matter most:
// every pattern named must be the one the element's own record names.
import test from "node:test";
import assert from "node:assert/strict";
import { readElements, patternsOf, nounOf, answerOf, promptOf, touchable, arrivalOf, PATTERNS } from "../../public/sparrow/answers.js";
import { smoothDamp, smoothDampAngle, angleTo, shotWeight, bankFor } from "../../public/sparrow/camerawork.js";
import { createGovernor, median, scaleRange } from "../../public/sparrow/governor.js";

const el = (group, kind, source = null, typology = null, floors = null) => ({ group, kind, source, typology, floors });

test("readElements unpacks the interned table", () => {
  const table = { version: 1, fields: ["group", "kind", "source", "typology", "floors"], strings: ["plazas", "plaza", "X_p61_new_sq0"], elements: [[0, 1, 2, -1, null]] };
  const [e] = readElements(table);
  assert.deepEqual({ group: e.group, kind: e.kind, source: e.source, typology: e.typology, floors: e.floors }, { group: "plazas", kind: "plaza", source: "X_p61_new_sq0", typology: null, floors: null });
  assert.deepEqual(readElements({ version: 2 }), []);
  assert.deepEqual(readElements(null), []);
});

test("open spaces are credited to whoever carved them, not their whole lineage", () => {
  // Real ids from clean_baseline (seed 42).
  assert.deepEqual(patternsOf(el("plazas", "plaza", "MILITARY_CIRCLE_ASSEMBLED_BLOCK_1_p61_new_sq0")), [61]);
  assert.deepEqual(patternsOf(el("plazas", "plaza", "MILITARY_CIRCLE_ASSEMBLED_BLOCK_1_P95_courtyard_p0.3")), [95]);
  assert.deepEqual(patternsOf(el("plazas", "plaza", "p108_merged_1_P107_courtyard")), [107], "the courtyard is P107's; P108 made the building");
  assert.deepEqual(patternsOf(el("plazas", "common", "MILITARY_CIRCLE_ASSEMBLED_BLOCK_1_common")), [37]);
  assert.deepEqual(patternsOf(el("plazas", "common", "p108_merged_12_building_p161_sunny")), [161]);
  assert.deepEqual(patternsOf(el("plazas", "plaza", "something_else")), []);
});

test("markers: P126 middles, P61 squares, P124 pockets, P53 gateways", () => {
  assert.deepEqual(patternsOf(el("activity", "activity_civic", "MILITARY_CIRCLE_ASSEMBLED_BLOCK_1_P95_courtyard_p0.0_p126_middle")), [126]);
  assert.deepEqual(patternsOf(el("activity", "activity_civic", "MILITARY_CIRCLE_ASSEMBLED_BLOCK_1_p61_new_sq0_activity")), [61]);
  assert.deepEqual(patternsOf(el("activity", "activity_civic", "BLOCK_3_pocket_2_activity")), [124]);
  assert.deepEqual(patternsOf(el("activity", "activity_gateway", "site_perimeter_p53_gateway")), [53]);
  assert.deepEqual(patternsOf(el("activity_glow", "activity_civic", "X_p126_middle")), [126], "a halo answers for its marker");
});

test("streets from the path network name P52 with P98 and P120", () => {
  for (const id of ["path_BLOCK_1_to_BLOCK_4", "loop_BLOCK_5_to_BLOCK_6", "localloop_BLOCK_1_to_BLOCK_2"]) {
    assert.deepEqual(patternsOf(el("streets", "local", id)), [52, 98, 120]);
  }
  const lines = answerOf(el("streets", "pedestrian", "loop_BLOCK_1_to_BLOCK_3"));
  assert.equal(lines[0], "A pedestrian street.");
  assert.match(lines[1], /^P52 Network of Paths and Cars: /);
  assert.equal(lines[2], "With P98 Circulation Realms and P120 Paths and Goals.");
  assert.deepEqual(patternsOf(el("streets", "local", "S1")), [], "an id that names no pattern names none");
});

test("buildings: typology and merge; roofs name nothing", () => {
  assert.deepEqual(patternsOf(el("buildings", "building_shaped", "p108_merged_2_building", "p107_courtyard_v01", 4)), [107, 108]);
  assert.deepEqual(patternsOf(el("buildings", "building_shaped", "MILITARY_CIRCLE_ASSEMBLED_BLOCK_5_P95_cell_2_building", "p107_solid_v01", 4)), [95, 107]);
  assert.deepEqual(patternsOf(el("buildings", "roof", "p108_merged_2_building", "p107_courtyard_v01", 4)), [], "a roof's record is its building's");
  assert.deepEqual(patternsOf(el("buildings", "roof_garden", "p108_merged_2_building")), [118]);
  assert.deepEqual(patternsOf(el("canopies", "canopy_arcade", "b")), [119]);
  assert.deepEqual(patternsOf(el("canopies", "canopy_gallery", "b")), [166]);
  assert.deepEqual(patternsOf(el("wall_niches", "wall_niche", "b")), [160]);
  assert.deepEqual(patternsOf(el("context", "context")), []);
  assert.equal(nounOf(el("buildings", "building_shaped", "b", "p107_courtyard_v01", 4)), "four-story courtyard building");
  assert.equal(answerOf(el("buildings", "roof", "b", null, 6))[0], "The roof of a six-story building.");
  assert.equal(answerOf(el("context", "context"))[0], "A building that stands there today, from Overture Maps.");
});

test("every pattern named has a name and a gloss", () => {
  for (const [n, p] of Object.entries(PATTERNS)) {
    assert.ok(p.name && p.gist, `P${n}`);
    assert.ok(!p.gist.endsWith("."), `P${n} gloss ends without a period (answerOf adds it)`);
  }
});

test("prompts, touchability and arrivals read as English", () => {
  assert.equal(promptOf(el("plazas", "plaza", "X_p61_new_sq0")), "Sit in the small public square");
  assert.equal(promptOf(el("activity", "activity_civic", "X_p126_middle")), "Chirp at the marker");
  assert.equal(promptOf(el("canopies", "canopy_arcade")), "Perch on the arcade");
  assert.equal(touchable(el("buildings", "building_shaped")), false);
  assert.equal(touchable(el("activity_glow", "activity_civic")), false, "halos don't glint; their markers do");
  assert.equal(touchable(el("activity", "activity_civic")), true);
  assert.equal(arrivalOf(el("streets", "arterial", "path_A_to_B")), "Onto an arterial road.");
  assert.equal(arrivalOf(el("plazas", "plaza", "X_P95_courtyard_p0.1")), "Into a courtyard.");
  assert.equal(arrivalOf(el("buildings", "building_shaped")), null);
});

test("camera springs arrive without overshoot and take the short way round", () => {
  let x = 0, v = 0;
  for (let i = 0; i < 240; i++) { [x, v] = smoothDamp(x, 10, v, 0.3, 1 / 60); assert.ok(x <= 10 + 1e-9); }
  assert.ok(Math.abs(x - 10) < 1e-3);
  assert.ok(Math.abs(angleTo(3, -3) - (2 * Math.PI - 6)) < 1e-9);
  let a = 3.0, av = 0;
  for (let i = 0; i < 300; i++) [a, av] = smoothDampAngle(a, -3.0, av, 0.2, 1 / 60);
  assert.ok(Math.abs(angleTo(a, -3.0)) < 1e-3 && a > 3.0, "went up through pi, not down through zero");
  assert.equal(shotWeight(-1, 3), 0);
  assert.equal(shotWeight(1.5, 3), 1);
  assert.equal(Object.is(bankFor(0, 1), -0), false);
  assert.ok(Math.abs(bankFor(10, 1)) <= 0.21);
});

test("the governor steps down when slow, settles, and never leaves its range", () => {
  const range = scaleRange(3, true);
  assert.deepEqual(range, { start: 1, min: 0.7, max: 1.5 });
  assert.deepEqual(scaleRange(1, false), { start: 1, min: 0.6, max: 1 });
  const g = createGovernor(range);
  assert.equal(g.step(16.7), 1.1, "meeting 60 Hz: sharper");
  assert.equal(g.step(30), 0.94, "slow: softer");
  for (let i = 0; i < 10; i++) g.step(16.7);
  assert.ok(g.scale <= 1.1 * 0.95 + 0.05, "won't climb straight back to the scale that was too slow");
  for (let i = 0; i < 20; i++) g.step(80);
  assert.equal(g.scale, 0.7);
  assert.equal(median([5, 1, 9]), 5);
  assert.equal(median([4, 1, 9, 2]), 3);
});

test("trees, the meadow and green streets: P171, P172, P51", () => {
  // Real ids from clean_baseline (seed 42).
  assert.deepEqual(patternsOf(el("trees", "umbrella", "p171_umbrella_0291")), [171]);
  assert.deepEqual(patternsOf(el("trees", "grove", "p171_grove_1500")), [171]);
  assert.deepEqual(patternsOf(el("trees", "grove", "someone_elses_tree")), [], "an id that names no pattern names none");
  assert.equal(nounOf(el("trees", "square", "p171_square_0400")), "tree ringing a square");
  assert.equal(nounOf(el("trees", "avenue", "p171_avenue_0001")), "tree in an avenue");
  assert.equal(arrivalOf(el("trees", "umbrella", "p171_umbrella_0291")), "Up in a spreading tree.");
  assert.equal(promptOf(el("trees", "grove", "p171_grove_1500")), "Perch in the tree in a grove");
  assert.equal(touchable(el("trees", "grove", "p171_grove_1500")), false, "two thousand glints would drown the squares");

  assert.deepEqual(patternsOf(el("ground", "meadow", "site_perimeter_p172_meadow")), [172]);
  assert.equal(arrivalOf(el("ground", "meadow", "site_perimeter_p172_meadow")), "Into the meadow.");

  const green = el("streets", "local_grass", "path_BLOCK_1_to_BLOCK_4");
  assert.deepEqual(patternsOf(green), [52, 98, 120, 51]);
  assert.equal(nounOf(green), "local street of grass and paving stones");
  assert.equal(arrivalOf(green), "Onto a local street of grass and paving stones.");
  const lines = answerOf(green);
  assert.equal(lines[0], "A local street of grass and paving stones.");
  assert.equal(lines[3], `P51 Green Streets: ${PATTERNS[51].gist}.`);
});
