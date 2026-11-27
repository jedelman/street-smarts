// The site bundles and the pieces of the walk that read them: fact templates, the "keep" front grammar
// (doors and windows stay where they are), and each bundle's own integrity.
import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { factsFor, readElements, touchable, promptOf, answerOf, arrivalOf } from "../../public/sparrow/answers.js";
import { BUSINESSES, ALL_BUSINESSES, growKeep, growFront } from "../../public/sparrow/shopfronts.js";

const pub = (p) => new URL(`../../public/${p}`, import.meta.url);
const json = (p) => JSON.parse(readFileSync(pub(p), "utf8"));

test("facts: an element's own, by source, before its group's template", () => {
  const facts = { "parcel:1": { name: "715 Colonial Avenue", answer: ["Its own."] }, "group:buildings": { name: "a building", answer: ["The template."] } };
  assert.deepEqual(factsFor(facts, { source: "parcel:1", group: "buildings" }).answer, ["Its own."]);
  assert.deepEqual(factsFor(facts, { source: "building:9", group: "buildings" }).answer, ["The template."]);
  assert.equal(factsFor(facts, { source: "x", group: "streets" }), null);
  assert.equal(factsFor(null, { source: "parcel:1", group: "buildings" }), null);
});

test("facts: templates fill {field|fallback} and {field?text with #}", () => {
  const t = { "group:trees": { name: "{typology|a tree}", answer: ["{typology|A tree}{floors?, # inches across the trunk}: there."] } };
  const oak = factsFor(t, { group: "trees", typology: "Willow oak", floors: 21 });
  assert.equal(oak.name, "Willow oak");
  assert.equal(oak.answer[0], "Willow oak, 21 inches across the trunk: there.");
  const bare = factsFor(t, { group: "trees", typology: null, floors: null });
  assert.equal(bare.name, "a tree");
  assert.equal(bare.answer[0], "A tree: there.");
});

test("facts drive the touch: touchable, prompt, answer, arrival", () => {
  const table = { version: 1, strings: ["buildings", "building_shaped", "parcel:1", "streets", "street_arterial", "street:COLLEY AVENUE"],
    elements: [[0, 1, 2, -1, 2, -1, 120], [3, 4, 5, -1, null, -1, null]],
    facts: { "parcel:1": { name: "715 Colonial Avenue", prompt: "Read 715 Colonial Avenue", answer: ["715 Colonial Avenue: a house, built 1888.", "Why it may: …"] },
      "street:COLLEY AVENUE": { name: "Colley Avenue", arrival: "Onto Colley Avenue, an arterial or collector street.", answer: ["Colley Avenue."] } } };
  const [house, street] = readElements(table);
  assert.ok(touchable(house));
  assert.equal(promptOf(house), "Read 715 Colonial Avenue");
  assert.deepEqual(answerOf(house), table.facts["parcel:1"].answer);
  assert.equal(arrivalOf(street), "Onto Colley Avenue, an arterial or collector street.");
  assert.ok(!touchable(street), "a street speaks on arrival; it doesn't glint");
});

test("a keep front adds signs, awnings and a yard, never a new window", () => {
  let s = 7;
  const rng = () => (s = (s * 16807) % 2147483647) / 2147483647;
  const windows = [{ x: 1.2, y0: 1.45, y1: 3.0, w: 0.85 }, { x: 5.6, y0: 1.45, y1: 3.0, w: 0.85 }];
  for (const biz of ALL_BUSINESSES) {
    const { parts, word } = growKeep({ biz, L: 8, door: { x: 3.2, top: 2.65 }, windows, rng, boxes: false });
    assert.ok(!parts.some((p) => p.k === "display"), `${biz.id}: the building's own windows show the inside, no new panes`);
    for (const p of parts.filter((q) => q.k === "glass")) {
      const w = windows.find((x) => Math.abs(x.x - p.x) < 0.01);
      assert.ok(w && p.y - p.sy / 2 >= w.y0 - 0.01 && p.y + p.sy / 2 <= w.y1 + 0.01, `${biz.id}: a shade or glow stays inside its window`);
    }
    assert.ok(parts.some((p) => p.k === "letters"), `${biz.id}: a lettered board over the door`);
    const has = (sym) => word.some((t) => t.s === sym);
    if (biz.family === "office") assert.ok(has("Plaque") && word.filter((t) => t.s === "Shade").length === windows.length && !has("Awning"), `${biz.id}: an office draws its shades, with a plaque`);
    else if (biz.family === "gallery") assert.ok(word.filter((t) => t.s === "Spot").length === windows.length, `${biz.id}: a gallery lights each window`);
    else if (biz.stripes && ["shop", "food", "salon"].includes(biz.family)) assert.equal(word.filter((t) => t.s === "Awning").length, windows.length, `${biz.id}: an awning over each window`);
    if (biz.id === "barber") assert.ok(has("Pole"), "a barber's pole by the door");
    if (biz.id === "yoga") assert.ok(has("Mats"), "mats out front of the yoga studio");
    for (const p of parts.filter((q) => q.k === "near" && q.out > 1.2)) assert.ok(p.out >= 1.4, "what's put out stands in the yard, past the door");
    for (const p of parts) assert.ok(p.x >= 0 && p.x <= 8, `${biz.id}: ${p.k} at ${p.x} stays on its run`);
  }
});

test("services differ: no two of them grow the same front", () => {
  const services = ALL_BUSINESSES.filter((b) => ["office", "studio", "gallery", "fitness", "salon"].includes(b.family) || ["tailor", "cleaners"].includes(b.id));
  const shapes = new Set();
  for (const biz of services) {
    let s = 11;
    const rng = () => (s = (s * 16807) % 2147483647) / 2147483647;
    const { word } = growKeep({ biz, L: 8, door: { x: 3.2, top: 2.65 }, windows: [{ x: 1.2, y0: 1.45, y1: 3.0, w: 0.85 }, { x: 5.6, y0: 1.45, y1: 3.0, w: 0.85 }], rng, boxes: false });
    shapes.add(`${biz.inside}|${biz.icon}|${word.map((t) => t.s).join(",")}`);
  }
  assert.equal(shapes.size, services.length);
});

test("a storefront still grows glass (the generated plan's fronts are unchanged)", () => {
  let s = 3;
  const rng = () => (s = (s * 16807) % 2147483647) / 2147483647;
  const { parts } = growFront({ biz: BUSINESSES.find((b) => b.id === "coffee"), L: 8, at: 4, doorWidth: 1, rng });
  assert.ok(parts.some((p) => p.k === "display"));
});

for (const id of ["eastside-commons", "ghent-streets"]) {
  test(`site bundle: ${id}`, () => {
    const site = json(`sites/${id}/site.json`);
    assert.equal(site.version, 1);
    assert.equal(site.id, id);
    assert.ok(site.model, "names its model");
    assert.ok(Array.isArray(site.tours) && site.tours.length, "has a tour");
    for (const t of site.tours) {
      assert.ok(t.title && t.stops.length, `${t.id}: a title and stops`);
      for (const st of t.stops) assert.equal(["pattern", "element", "waypoint"].filter((k) => k in st).length, 1, "each stop is one kind");
    }
    for (const mod of Object.values(site.renderers || {})) {
      const path = mod.startsWith("/") ? mod.slice(1) : `sites/${id}/${mod}`;
      assert.ok(existsSync(pub(path)), `renderer ${mod} exists`);
    }
  });
}

test("Ghent's bundle: six tour stops, every one a waypoint; the corners and the numbers agree", () => {
  const site = json("sites/ghent-streets/site.json");
  const numbers = json("sites/ghent-streets/numbers.json");
  const stops = site.tours[0].stops;
  assert.equal(stops.length, 6);
  assert.ok(stops.every((s) => typeof s.waypoint === "string"));
  assert.equal(site.corners.length, numbers.corners);
  assert.ok(site.corners.every((c) => c.element.startsWith("parcel:") && c.label));
  const kinds = numbers.converted_by_kind, total = kinds.food + kinds.retail + kinds.services;
  assert.equal(total, numbers.status.converted);
  assert.ok(Math.abs(kinds.food / total - 0.5) < 0.05 && Math.abs(kinds.retail / total - 0.3) < 0.05, "Brief 2's mix, near enough");
});
