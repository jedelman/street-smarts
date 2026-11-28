// Where people walk (walkmask.js): routes keep to the mask, go round what isn't on it, and Ghent's real mask keeps
// people out of the lots.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { makeMask } from "../../public/sparrow/walkmask.js";

// A mask from rows of text ('#' walkable), the first row the southernmost; cell 1 m from (0, 0).
function fromRows(rows) {
  const h = rows.length, w = rows[0].length, bits = new Uint8Array(Math.ceil((w * h) / 8));
  rows.forEach((line, r) => [...line].forEach((ch, c) => { if (ch === "#") bits[(r * w + c) >> 3] |= 1 << (7 - ((r * w + c) & 7)); }));
  return makeMask(bits, { x0: 0, y0: 0, cell: 1, w, h });
}
const at = (x, y) => ({ x, z: -y }); // survey (x, y) to the walk's (x, z)

test("a route keeps to the mask and goes round a gap", () => {
  const m = fromRows([
    "##########",
    "#........#",
    "#........#",
    "##########",
  ]);
  const p = m.path(at(0.5, 1.5), at(9.5, 1.5)); // across the middle is off the mask: round by a side
  assert.ok(p && p.length >= 3, "a route with turns");
  for (let i = 1; i < p.length; i++) assert.ok(m.clear(p[i - 1].x, p[i - 1].z, p[i].x, p[i].z), `leg ${i} stays on the mask`);
  assert.equal(m.path(at(0.5, 0.5), at(9.5, 0.5)).length, 2, "a straight sidewalk is one straight leg");
  assert.equal(m.walkable(4.5, -1.5), false);
  assert.deepEqual(m.nearest(4.5, -1.2, 3), { x: 4.5, z: -0.5 });
});

test("a front walk carved from a door joins it to the sidewalk", () => {
  const m = fromRows(["#####", ".....", ".....", ".....", ".....", ".....", "....."]);
  assert.equal(m.path(at(2.5, 5.5), at(0.5, 0.5)), null, "the door, 5 m back, is cut off");
  m.carve(2.5, -5.5, 2.5, -0.5, 1);
  const p = m.path(at(2.5, 5.5), at(0.5, 0.5));
  assert.ok(p, "now it isn't");
});

test("Ghent's mask: the sidewalks connect, and the lots aren't on it", () => {
  const site = JSON.parse(readFileSync(new URL("../../public/sites/ghent-streets/site.json", import.meta.url), "utf8"));
  const meta = site.walkmask;
  const bits = inflateSync(readFileSync(new URL(`../../public/sites/ghent-streets/${meta.url}`, import.meta.url)));
  let n = 0;
  for (const b of bits) for (let k = 0; k < 8; k++) n += (b >> k) & 1;
  assert.equal(n, meta.walkable, "as many walkable cells as the build counted");
  const m = makeMask(bits, meta);
  const S = JSON.parse(readFileSync(new URL("../../data/ghent/survey.json", import.meta.url), "utf8"));
  // The middle of every lot of some size is off the mask.
  let checked = 0;
  for (const p of S.parcels) {
    if (!p.lot_sqft || p.lot_sqft < 8000) continue;
    const xs = p.poly.map((q) => q[0]), ys = p.poly.map((q) => q[1]);
    const cx = (Math.min(...xs) + Math.max(...xs)) / 2, cy = (Math.min(...ys) + Math.max(...ys)) / 2;
    // only where the box's middle is inside the lot (a convex-enough lot)
    let inside = false;
    for (let i = 0, j = p.poly.length - 1; i < p.poly.length; j = i++) {
      const [xi, yi] = p.poly[i], [xj, yj] = p.poly[j];
      if (yi > cy !== yj > cy && cx < ((xj - xi) * (cy - yi)) / (yj - yi) + xi) inside = !inside;
    }
    if (!inside) continue;
    assert.equal(m.walkable(cx, -cy), false, `${p.address}'s middle is off the sidewalks`);
    if (++checked >= 200) break;
  }
  assert.ok(checked > 50, `checked ${checked} lots`);
  // Two sidewalk spots a few blocks apart are joined by a way that stays on the mask.
  const a = m.nearest(26, -695, 10), b = m.nearest(120, -620, 10);
  const way = m.path(a, b, { margin: 60 });
  assert.ok(way, "a way between them");
  for (let i = 1; i < way.length; i++) assert.ok(m.clear(way[i - 1].x, way[i - 1].z, way[i].x, way[i].z), `leg ${i} on the mask`);
});

test("spots sampled from a mask are on it", () => {
  const m = fromRows(["#.........", "..........", ".........#"]);
  let seed = 3; const rng = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  for (let i = 0; i < 20; i++) { const q = m.sample(rng); assert.ok(q && m.walkable(q.x, q.z)); }
});
