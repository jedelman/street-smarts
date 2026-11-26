// node --test tools/sparrow/
// The flow overlay's model: spenders sized from floor uses and footprints,
// Huff's shares within a five-minute walk, money conserved from spender to
// shop fronts, and lines that only say what the model computed.
import test from "node:test";
import assert from "node:assert/strict";
import { FLOW, SF, spendersOf, sharesFrom, computeFlows, topFlows, lineFor, summaryLine, money } from "../../public/sparrow/flows.js";

const close = (a, b, eps = 1e-6) => assert.ok(Math.abs(a - b) <= eps * Math.max(1, Math.abs(b)), `${a} != ${b}`);

test("a building's homes and desks come from its floors of each use", () => {
  const s = spendersOf({ area: 1000, uses: "pwwlll" });
  close(s.homes, 3 * 1000 * SF * FLOW.usable / FLOW.homeSf);
  close(s.desks, 2 * 1000 * FLOW.usable / FLOW.m2PerDesk);
  close(s.home, s.homes * FLOW.spendPerHome);
  close(s.desk, s.desks * FLOW.spendPerWorkweek * FLOW.workweeks);
  const none = spendersOf({ area: 1000, uses: "p" });
  assert.equal(none.homes + none.desks, 0);
  assert.equal(spendersOf({ area: null, uses: "pll" }).home, 0);
});

test("shares sum to one, favour nearer shops, and ignore those past the walk", () => {
  const shops = [{ x: 50, z: 0 }, { x: 250, z: 0 }, { x: 900, z: 0 }];
  const sh = sharesFrom(0, 0, shops);
  close(sh.reduce((a, b) => a + b, 0), 1);
  assert.ok(sh[0] > sh[1]);
  close(sh[0] / sh[1], Math.exp(200 / FLOW.decayM));
  assert.equal(sh[2], 0);
  // Nothing within the walk: the nearest takes it all.
  assert.deepEqual(sharesFrom(0, 0, [{ x: 800, z: 0 }, { x: 1200, z: 0 }]), [1, 0]);
});

test("every dollar a building spends lands on a shop front", () => {
  const buildings = [
    { id: "A", area: 800, uses: "plll", x: 0, z: 0 },
    { id: "B", area: 1200, uses: "pwwl", x: 120, z: 0 },
    { id: "C", area: 500, uses: "pw", x: 2000, z: 0 }, // far from every other shop
  ];
  const shops = [
    { building: "A", x: 5, z: 10 }, { building: "A", x: -5, z: 10 },
    { building: "B", x: 120, z: 12 }, { building: "C", x: 2000, z: 8 },
  ];
  const m = computeFlows({ buildings, shops, station: { x: 60, z: -40 } });
  const spent = m.totals.home + m.totals.desk;
  const landed = [...m.buildings.values()].reduce((s, b) => s + b.inHome + b.inDesk, 0);
  close(landed, spent);
  const fromFlows = m.flows.filter((f) => f.kind !== "tram").reduce((s, f) => s + f.amount, 0);
  close(fromFlows, spent);
  assert.equal(m.buildings.get("A").shops, 2);
  // C's shops are past the walk from A and B, and C's own desks spend there.
  assert.ok(!m.flows.some((f) => f.to === "C" && f.from !== "C"));
  // The tram's riders: shares, no money.
  const tram = m.flows.filter((f) => f.kind === "tram");
  assert.ok(tram.length > 0 && tram.every((f) => f.amount === 0));
  close(tram.reduce((s, f) => s + f.share, 0), 1);
});

test("topFlows keeps flows between two places, biggest first, of each kind", () => {
  const flows = [
    { from: "A", to: "A", kind: "home", amount: 99 },
    { from: "A", to: "B", kind: "home", amount: 5 },
    { from: "B", to: "A", kind: "home", amount: 7 },
    { from: "B", to: "A", kind: "desk", amount: 3 },
    { from: "station", to: "A", kind: "tram", amount: 0, share: 0.6 },
  ];
  const top = topFlows(flows, 1);
  assert.deepEqual(top.map((f) => `${f.from}${f.to}${f.kind}`), ["BAhome", "BAdesk", "stationAtram"]);
});

test("the lines say the model's own numbers", () => {
  const buildings = [{ id: "A", area: 1000, uses: "pwll", x: 0, z: 0 }];
  const shops = [{ building: "A", x: 0, z: 10 }, { building: "A", x: 10, z: 0 }];
  const m = computeFlows({ buildings, shops });
  const b = m.buildings.get("A");
  const line = lineFor(m, "A");
  assert.match(line, /^Its 2 shop fronts take in about \$/);
  for (const v of [b.inHome + b.inDesk, b.inHome, b.inDesk]) assert.ok(line.includes(money(v)), line);
  assert.equal(money(758e3), "$758k");
  assert.equal(money(1.24e6), "$1.2M");
  assert.equal(money(58.4e6), "$58M");
  assert.equal(lineFor(m, "nowhere"), null);
  assert.match(summaryLine(m), /homes spend \$.* desks \$/);
});
