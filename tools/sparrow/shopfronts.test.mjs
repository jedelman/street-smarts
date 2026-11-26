// node --test tools/sparrow/
// The shop-front L-system: a front grows from one symbol into parts by
// rules, the same front for the same seed, a business's own touches on it,
// and nothing drawn outside its own run of wall.
import test from "node:test";
import assert from "node:assert/strict";
import { BUSINESSES, WEIGHTS, chooseBusiness, derive, RULES, growFront, ST, G } from "../../public/sparrow/shopfronts.js";

const seeded = (s = 7) => () => (s = (s * 16807) % 2147483647) / 2147483647;
const biz = (id) => BUSINESSES.find((b) => b.id === id);
const grow = (id, L = 11.2, at = 5.6, seed = 3) => growFront({ biz: biz(id), L, at, doorWidth: 1.0, rng: seeded(seed) });
const has = (f, sym) => f.word.some((t) => t.s === sym);

test("every business has a weight, a family the rules know, and its own sign", () => {
  const families = new Set(["shop", "food", "office", "studio", "school"]);
  for (const b of BUSINESSES) {
    assert.ok(WEIGHTS[b.id] > 0, `${b.id} has no weight`);
    assert.ok(families.has(b.family), `${b.id}: unknown family ${b.family}`);
    assert.ok(b.sign && b.noun && b.inside && b.icon, b.id);
  }
  assert.equal(new Set(BUSINESSES.map((b) => b.id)).size, BUSINESSES.length);
});

test("the L-system rewrites until only terminals are left", () => {
  const word = derive([{ s: "Front", a: 0, b: 11.2 }], RULES, { biz: biz("florist"), L: 11.2, at: 5.6, entryW: 1.5, rng: seeded(), flip: 1, bulk: "panel", awning: true, planters: false });
  for (const t of word) assert.ok(!(t.s in RULES), `${t.s} was left unrewritten`);
  // A wide run splits at piers, and wide glass at mullions: the recursion happened.
  assert.ok(word.filter((t) => t.s === "Pier").length >= 2);
  assert.ok(word.some((t) => t.s === "Mullion"));
});

test("the same seed grows the same front", () => {
  assert.deepEqual(grow("wine", 9, 4).parts, grow("wine", 9, 4).parts);
  assert.notDeepEqual(grow("wine", 9, 4, 3).parts, grow("wine", 9, 4, 11).parts);
});

test("each business grows its own touches", () => {
  assert.ok(has(grow("bikes"), "Bike") && has(grow("bikes"), "Rack"), "bikes racked out front");
  assert.ok(has(grow("accountant"), "Frost") && has(grow("accountant"), "Plaque"), "a frosted office with a plaque");
  assert.ok(!has(grow("accountant"), "Blade"), "offices don't hang a blade sign");
  assert.ok(has(grow("architect"), "Kick") && !has(grow("architect"), "Bulkhead"), "glass to the ground");
  assert.ok(has(grow("wine"), "Barrel"));
  assert.ok(has(grow("school"), "Scooter") && has(grow("school"), "Bench"));
  assert.equal(grow("florist").word.filter((t) => t.s === "Bucket").length, 4);
  assert.equal(grow("greengrocer").word.filter((t) => t.s === "Crate").length, 4);
  const tall = grow("architect").parts.find((p) => p.k === "display");
  assert.ok(tall.sy > 2.4, "an architects' window runs nearly floor to ceiling");
});

test("nothing is drawn outside the front's own run, or above its ground floor", () => {
  for (const b of BUSINESSES) for (const [L, at] of [[5.6, 2.8], [11.2, 5.6], [17, 3.2]]) {
    const f = growFront({ biz: b, L, at, doorWidth: 1.0, rng: seeded(5) });
    for (const p of f.parts) {
      if (p.k === "mid" && p.sy < 0.2) continue; // the cornice runs a little past both ends
      assert.ok(p.x - p.sx / 2 > -0.4 && p.x + p.sx / 2 < L + 0.4, `${b.id} L=${L}: ${p.k} at ${p.x.toFixed(2)} wide ${p.sx.toFixed(2)}`);
      assert.ok(p.y + p.sy / 2 <= G + 0.05, `${b.id}: ${p.k} pokes above the ground floor`);
    }
  }
});

test("the mix follows the weights, and neighbours differ", () => {
  const rng = seeded(9), n = 4000, counts = {};
  for (let k = 0; k < n; k++) { const b = chooseBusiness(rng); counts[b.id] = (counts[b.id] || 0) + 1; }
  const total = Object.values(WEIGHTS).reduce((a, b) => a + b, 0);
  for (const [id, w] of Object.entries(WEIGHTS)) assert.ok(Math.abs(counts[id] / n - w / total) < 0.02, `${id}: ${counts[id]} of ${n}`);
  for (let k = 0; k < 200; k++) assert.ok(!["coffee", "bistro"].includes(chooseBusiness(rng, ["coffee", "bistro"]).id), "a recent business isn't chosen again");
});
