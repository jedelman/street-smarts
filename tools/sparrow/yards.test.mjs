// Ghent's yards and porches (yards.js): every part stays on its lot and in its yard, porches are cut into the front
// (nothing of a porch stands out past its steps), and the frequencies read like the photos.
import test from "node:test";
import assert from "node:assert/strict";
import { growYard, YARD_FREQ } from "../../public/sparrow/yards.js";

const seeded = (seed) => () => (seed = (seed * 16807) % 2147483647) / 2147483647;
const windows = [{ x: 1.4, y0: 1.4, y1: 2.95, w: 0.85, floor: 0 }, { x: 6.6, y0: 1.4, y1: 2.95, w: 0.85, floor: 0 },
  { x: 1.4, y0: 4.5, y1: 6.05, w: 0.85, floor: 1 }, { x: 6.6, y0: 4.5, y1: 6.05, w: 0.85, floor: 1 }];

test("every part stays on the lot, in the yard, and above the ground", () => {
  for (const form of ["house", "row", "apartment"]) {
    for (let k = 1; k <= 300; k++) {
      const depth = (k % 9) * 0.9, row = { door: 4, depth, lotLo: -0.5, lotHi: 8.5, form, first: 0.6, floors: 2 + (k % 2), converted: k % 5 === 0 };
      const { parts } = growYard({ row, L: 8, windows, rng: seeded(k * 7919) });
      for (const p of parts) {
        assert.ok(p.x - p.sx / 2 >= row.lotLo - 0.31 && p.x + p.sx / 2 <= row.lotHi + 0.31, `${form} ${p.k} at x ${p.x} leaves the lot`);
        assert.ok(p.out + (p.rx ? 0 : p.sz / 2) <= Math.max(depth, 0) + 1.2 + 0.01, `${form} ${p.k} at out ${p.out} stands past the yard (${depth})`);
        assert.ok(p.y - p.sy / 2 >= -0.01, `${form} ${p.k} goes under the ground`);
      }
    }
  }
});

test("a porch is cut into the front: its parts stand at the wall, not out in the yard", () => {
  let porches = 0;
  for (let k = 1; k <= 400; k++) {
    const row = { door: 4, depth: 5, lotLo: 0, lotHi: 8, form: "house", first: 0.6, floors: 2, converted: false };
    const g = growYard({ row, L: 8, windows, rng: seeded(k) });
    if (!g.porch) continue;
    porches++;
    const cols = g.parts.filter((q) => q.role === "column");
    assert.ok(cols.length >= 2 && cols.every((p) => p.out < 0.3 && p.x >= g.porch.lo && p.x <= g.porch.hi), "columns stand at the wall, along the porch");
    assert.ok(g.porch.lo >= 0 && g.porch.hi <= 8 && g.porch.top > 2.5);
  }
  const share = porches / 400, want = 1 - YARD_FREQ.house.porch.none;
  assert.ok(Math.abs(share - want) < 0.08, `houses have porches about ${Math.round(want * 100)}% of the time (got ${Math.round(share * 100)}%)`);
});

test("no shutters under a porch, and none on a converted front", () => {
  for (let k = 1; k <= 300; k++) {
    const row = { door: 4, depth: 4, lotLo: 0, lotHi: 8, form: "house", first: 0.6, floors: 2, converted: k % 2 === 0 };
    const g = growYard({ row, L: 8, windows, rng: seeded(k * 31) });
    const shutters = g.word.some((t) => t.s === "Shutters");
    if (row.converted) assert.ok(!shutters);
    if (shutters && g.porch) {
      for (const p of g.parts.filter((q) => q.role === "shutter" && q.y < 3.2)) assert.ok(p.x + 0.19 <= g.porch.lo || p.x - 0.19 >= g.porch.hi, "a shutter clear of the porch");
    }
  }
});

test("a shallow yard gets a stoop's few steps and no tree or fence", () => {
  for (let k = 1; k <= 200; k++) {
    const row = { door: 3, depth: 0.9, lotLo: 0, lotHi: 6, form: "row", first: 0.6, floors: 3, converted: false };
    const g = growYard({ row, L: 6, windows, rng: seeded(k * 101) });
    assert.ok(!g.word.some((t) => t.s === "Tree"));
    assert.equal(g.word.find((t) => t.s === "Edge").edge, "none");
  }
});
