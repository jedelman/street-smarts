// "Day to day": the walk network's arithmetic (walkgraph.js) on a toy grid, and Ghent's everyday.json as built.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { prepare, snap, fromPoint, metresTo, block, summary, minutesOf, everyOf, tractAt } from "../../public/sites/ghent-streets/walkgraph.js";

// A 3×3 grid of 100 m blocks: nodes 0..8, row by row.
function grid() {
  const nodes = [], edges = [];
  for (let r = 0; r < 3; r++) for (let c = 0; c < 3; c++) nodes.push([c * 100, r * 100]);
  for (let r = 0; r < 3; r++) for (let c = 0; c < 2; c++) edges.push([r * 3 + c, r * 3 + c + 1, 100]);
  for (let r = 0; r < 2; r++) for (let c = 0; c < 3; c++) edges.push([r * 3 + c, (r + 1) * 3 + c, 100]);
  return { nodes, edges };
}

test("snap finds the nearest point on the nearest street", () => {
  const g = prepare(grid());
  const [e, along, off] = snap(g, 30, 4);
  assert.deepEqual(g.edges[e].slice(0, 2), [0, 1]);
  assert.ok(Math.abs(along - 30) < 1e-9 && Math.abs(off - 4) < 1e-9);
});

test("walks run along the streets, not as the crow flies", () => {
  const g = prepare(grid());
  const D = fromPoint(g, snap(g, 0, 0));
  assert.equal(D.dist[8], 400); // two blocks over and two up
  assert.equal(metresTo(g, D, snap(g, 150, 200)), 350);
  const mid = snap(g, 50, 0);
  assert.equal(metresTo(g, fromPoint(g, mid), snap(g, 70, 0)), 20, "along the same street, straight there");
});

test("a block is the street between the corners", () => {
  const g = prepare({ nodes: [[0, 0], [50, 0], [100, 0], [100, 100], [200, 0]], edges: [[0, 1, 50], [1, 2, 50], [2, 3, 100], [2, 4, 100]] });
  assert.deepEqual([...block(g, 0)].sort(), [0, 1], "the bend at node 1 isn't a corner; node 2, where three streets meet, is");
  assert.deepEqual([...block(g, 2)], [2]);
});

test("words for minutes and for how often", () => {
  assert.equal(minutesOf(40, 1.34), "under a minute");
  assert.equal(minutesOf(400, 1.34), "5 min");
  assert.equal(minutesOf(5000, 1.34), "over 15 min");
  assert.equal(minutesOf(Infinity, 1.34), "over 15 min");
  assert.equal(everyOf(26), "about every 2 weeks");
  assert.equal(everyOf(0.25), "about every 4 years");
  assert.equal(everyOf(0), null);
});

const pub = (p) => JSON.parse(readFileSync(new URL(`../../public/${p}`, import.meta.url), "utf8"));

test("Ghent's everyday.json: every place on the network, every converted ground floor placed, rent by tract", () => {
  const d = pub("sites/ghent-streets/everyday.json");
  const numbers = pub("sites/ghent-streets/numbers.json");
  const n = d.nodes.length;
  assert.ok(d.edges.every(([i, j, len]) => i < n && j < n && len > 0));
  assert.equal(d.proposed.length, numbers.converted_by_kind.food + numbers.converted_by_kind.retail + numbers.converted_by_kind.services - 7,
    "every converted ground floor with a door (seven sit where a door can't be placed)");
  for (const rows of [d.today, d.proposed]) for (const [e, a, o] of rows) assert.ok(e < d.edges.length && a >= 0 && a <= d.edges[e][2] + 0.11 && o >= 0);
  assert.equal(Object.keys(d.lots).length, numbers.parcels);
  assert.ok(d.tracts.length >= 1 && d.tracts.every((t) => t.median_rent > 0 && t.renting > 0 && t.renting < 1));
  for (const s of ["food", "shop", "service"]) assert.ok(d.bds[s].entry > 0.03 && d.bds[s].entry < 0.2, `${s}: a plausible entry rate`);
  assert.ok(Object.values(d.lots).filter((l) => l[3] >= 0).length / numbers.parcels > 0.95, "nearly every lot is in a tract");
});

test("from a converted lot, the proposal only adds places, and its own ground floor is near and on its block", () => {
  const d = prepare(pub("sites/ghent-streets/everyday.json"));
  const g = d.proposed[0][4];
  const S = summary(d, d.lots[g].slice(0, 3), 10);
  for (const k of S.kinds) {
    assert.ok(k.proposed.within >= k.today.within && k.proposed.nearest <= k.today.nearest, `${k.id}: the proposal only adds`);
  }
  assert.ok(S.near.food + S.near.shop + S.near.service >= 1, "its own ground floor is within reach");
  assert.ok(S.onBlock.proposed.some((p) => p.gpin === g), "and on its own block");
  assert.ok(tractAt(d, ...d.nodes[0]) >= -1);
});
