// The signs: names from each business's grammar, faces that suit it, and the atlas that letters them.
import test from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { nameFor, faceFor, lettered, packSigns, FACES } from "../../public/sparrow/signs.js";
import { ALL_BUSINESSES } from "../../public/sparrow/shopfronts.js";

const seeded = (seed) => () => (seed = (seed * 16807) % 2147483647) / 2147483647;
// Norfolk's own names (streets, neighborhoods, landmarks): a sign must never pass for a real shop on a real street.
const LOCAL = /\b(Norfolk|Ghent|Colley|Colonial|Olney|Granby|Mowbray|Stockley|Raleigh|Graydon|Blow|Hague|Military|Circle|Brambleton|Fairfax|Yarmouth|Redgate|Llewellyn|Botetourt|Westover|Boissevain|Pembroke|Princess|Freemason|Chrysler|Elizabeth|Lafayette|Larchmont|Wards|Riverview|Waterside|Tidewater|Hampton|Virginia)\b/i;

test("every business has a name of its own, short enough for a sign, never local", () => {
  for (const biz of ALL_BUSINESSES) {
    const names = new Set();
    for (let k = 1; k <= 400; k++) {
      const name = nameFor(biz, seeded(k * 7919));
      assert.ok(name.length <= 26, `${biz.id}: "${name}" is too long`);
      assert.ok(!LOCAL.test(name), `${biz.id}: "${name}" uses a Norfolk name`);
      const words = name.split(/[\s&,.']+/).filter((w) => w.length > 2);
      assert.equal(new Set(words).size, words.length, `${biz.id}: "${name}" repeats a word`);
      names.add(name);
    }
    assert.ok(names.size >= 40, `${biz.id}: only ${names.size} different names in 400 draws`);
  }
});

test("the same front keeps its name", () => {
  const biz = ALL_BUSINESSES.find((b) => b.id === "coffee");
  assert.equal(nameFor(biz, seeded(42)), nameFor(biz, seeded(42)));
});

test("faces suit the business, and every face is served with its licence", () => {
  for (const biz of ALL_BUSINESSES) {
    const faces = new Set();
    for (let k = 1; k <= 200; k++) faces.add(faceFor(biz, seeded(k * 104729)).id);
    assert.ok(faces.size >= 2, `${biz.id} always gets the same face`);
  }
  const script = ALL_BUSINESSES.find((b) => b.id === "accountant");
  for (let k = 1; k <= 200; k++) assert.notEqual(faceFor(script, seeded(k * 31)).id, "pacifico", "no accountant letters in a script");
  for (const f of FACES) {
    assert.ok(existsSync(new URL(`../../public/sparrow/fonts/${f.file}.woff2`, import.meta.url)), `${f.file}.woff2`);
    assert.ok(existsSync(new URL(`../../public/sparrow/fonts/${f.file}.LICENSE.txt`, import.meta.url)), `${f.file}'s licence`);
  }
  assert.equal(lettered("The Copper Kettle", FACES.find((f) => f.caps)), "THE COPPER KETTLE");
});

test("the atlas packs every sign without overlap, shrinking to fit", () => {
  const face = FACES[0];
  const signs = Array.from({ length: 900 }, (_, i) => ({ key: `s${i}`, text: `Sign number ${i}`, face }));
  const measure = (text, font) => text.length * parseInt(/(\d+)px/.exec(font)[1], 10) * 0.55;
  const p = packSigns(signs, measure);
  assert.ok(p.height <= 4096 && p.width === 4096);
  const byRow = new Map();
  for (const r of p.rows) {
    assert.ok(r.x + r.w <= p.width);
    if (!byRow.has(r.y)) byRow.set(r.y, []);
    byRow.get(r.y).push(r);
  }
  for (const row of byRow.values()) row.sort((a, b) => a.x - b.x).forEach((r, i) => { if (i) assert.ok(r.x >= row[i - 1].x + row[i - 1].w); });
  assert.equal(p.rows.length, 900);
});
