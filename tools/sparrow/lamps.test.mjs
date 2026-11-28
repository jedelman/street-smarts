// Ghent's streetlights (lamps.js): every family stands on the ground, lights its head, and reaches over the street.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { lampParts } from "../../public/sparrow/lamp-parts.js";

test("each family: a pole from the ground, a lit head on top, a pool of light below", () => {
  for (const fam of ["colonial", "acorn", "cobra", "shoebox", "other"]) {
    const parts = lampParts(fam, "concrete");
    const pole = parts.filter((p) => p[0] === "pole");
    assert.ok(pole.length && Math.min(...pole.map((p) => p[2] - p[5] / 2)) <= 0.01, `${fam}: a pole from the ground`);
    const lit = parts.filter((p) => p[0] === "glow" || p[0] === "glowball");
    const top = Math.max(...pole.map((p) => p[2] + p[5] / 2));
    assert.ok(lit.length && lit.every((p) => p[2] > top - 0.6), `${fam}: its light is up at the head`);
    assert.equal(parts.filter((p) => p[0] === "pool").length, 1, `${fam}: one pool of light`);
  }
  // A Colonial is the catalogue's 13-foot pole; a cobra head hangs out over the street.
  const col = lampParts("colonial", "fiberglass"), cob = lampParts("cobra", "wood");
  assert.ok(Math.abs(Math.max(...col.filter((p) => p[0] === "pole").map((p) => p[2] + p[5] / 2)) - 13 * 0.3048) < 0.01);
  assert.ok(cob.find((p) => p[0] === "glow")[3] > 1.5, "the cobra's head reaches out toward the street");
});

test("Ghent's lights: from the City's data, most of them Colonial", () => {
  const L = JSON.parse(readFileSync(new URL("../../data/ghent/lights.json", import.meta.url), "utf8"));
  const fam = {};
  for (const l of L.lights) fam[l[2]] = (fam[l[2]] || 0) + 1;
  assert.ok(L.lights.length > 900);
  assert.ok(fam.colonial / L.lights.length > 0.5, "lanterns are the most common");
});
