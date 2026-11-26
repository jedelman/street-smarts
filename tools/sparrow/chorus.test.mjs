// node --test tools/sparrow/
// Eavesdropping: what the walk's passers-by say. They say it plainly
// (plain.js), but every number they say must be one the opinion reported;
// the opinion's own words (tidied, never added to) go in the log beside it;
// and the opener only says how the opinion scored.
import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { moodOf, shortName, tidy, forSpeech, pickOpinion, lineFor, questionLine } from "../../public/sparrow/chorus.js";
import { plainFor, PLAIN_NAMES } from "../../public/sparrow/plain.js";

// Real findings and sources from clean_baseline (seed 42), via chorus_of.
const OPS = [
  { name: "p118_roof_garden", family: "pattern", source: "Alexander et al., A Pattern Language, Pattern 118 (Roof Garden)", value: 0.05,
    says: "39 real roofed building(s) checked; 2 (5%) have at least one real, flat, occupiable roof plane.", features: ["p108_merged_12_building"] },
  { name: "p192_windows_overlooking_life", family: "pattern", source: "Alexander et al., A Pattern Language, Pattern 192 (Windows Overlooking Life)", value: 0.99,
    says: "1512 ground-floor window(s) checked; 99% have real street/open-space activity within 25m and no neighboring building within 6m.", features: [] },
  { name: "levels_of_scale", family: "geometric", source: "Salingaros 2025, Property 1 of 15 (Levels of Scale)", value: 0.7688,
    says: "Across 8 effective levels (smallest 30 m² → largest 113906 m²), the consecutive magnification ratios are 6.2×, 2.7×. Salingaros's ideal range is 2–5×.", features: [] },
  { name: "ownership_pattern", family: "activist", source: "Jason Edelman + Eastside Commons coalition framework (2026)", value: 1, says: "Of 39 parcels tagged with ownership (7.5% of area), 100% is in commons hands.", features: [] },
  { name: "p198_closets_between_rooms", family: "pattern", source: "Alexander et al., A Pattern Language, Pattern 198 (Closets Between Rooms)", value: null,
    says: "No closet/storage concept exists anywhere in this pipeline's schema -- InteriorCell.kind only ever takes \"room\", \"passage\", or \"stair\". See this opinion's own module doc.", features: [] },
  { name: "p100_pedestrian_street", family: "pattern", source: "Alexander et al., A Pattern Language, Pattern 100 (Pedestrian Street)", value: 0.378,
    says: "3 Pedestrian street(s); mean entrance density 0.38 relative to the 3-per-100m target.", features: ["loop_BLOCK_5_to_BLOCK_6"] },
];
const CHORUS = { version: 1, opinions: OPS, questions: [{ question: "The activist chorus likes this place's ownership pattern (1.00). The geometric chorus is less convinced about its form (0.74). What's the gap?", why: "…" }] };
const seeded = (s = 7) => () => (s = (s * 16807) % 2147483647) / 2147483647;
// The finding's words, after the only changes tidy() may make: "(s)" plurals, "a/b" counts, "--" dashes.
const words = (s) => s.toLowerCase().replace(/(\w)\((e?s)\)/g, "$1$2").replace(/(\d)\/(\d)/g, "$1 of $2").replace(/--/g, "—").match(/[\p{L}\p{N}%.²×–—'"-]+/gu) || [];

test("an opinion's value sets the mood; abstaining is a shrug", () => {
  assert.equal(moodOf(0.99), "love");
  assert.equal(moodOf(0.75), "love");
  assert.equal(moodOf(0.6), "meh");
  assert.equal(moodOf(0.35), "gripe");
  assert.equal(moodOf(0), "gripe");
  assert.equal(moodOf(null), "shrug");
});

test("short names come from the source", () => {
  assert.equal(shortName(OPS[0]), "P118 Roof Garden");
  assert.equal(shortName(OPS[2]), "Levels of Scale (Salingaros)");
  assert.equal(shortName(OPS[3]), "Ownership pattern", "a year in parentheses isn't a name");
});

test("tidying keeps whole clauses and never splits a decimal", () => {
  assert.equal(tidy(OPS[0].says), "39 real roofed buildings checked; 2 (5%) have at least one real, flat, occupiable roof plane.");
  assert.equal(tidy(OPS[5].says), "3 Pedestrian streets; mean entrance density 0.38 relative to the 3-per-100m target.");
  assert.match(tidy(OPS[2].says), /^Across 8 effective levels .* 6\.2×, 2\.7×\./);
  assert.equal(tidy("25 Common patch(es), 17 nearby pairs."), "25 Common patches, 17 nearby pairs.");
  assert.equal(tidy("39/39 building(s) have at least one door."), "39 of 39 buildings have at least one door.");
  const long = tidy(OPS[4].says);
  assert.ok(long.length <= 260, long.length);
});

test("for the ear: units in words, ids not read out", () => {
  assert.equal(forSpeech("within 25m and 30 m² → 6.2× (>= 2)"), "within 25 metres and 30 square metres to 6.2 times ( at least 2)");
  assert.equal(forSpeech("tallest is p108_merged_25_building at 24.7m"), "tallest is one of them at 24.7 metres");
  assert.equal(forSpeech("Alexander's ~18m bound, 6ft"), "Alexander's about 18 metres bound, 6 feet");
});

test("every finding is the opinion's own words, and the opener only carries the mood", () => {
  const rng = seeded();
  for (const op of OPS) {
    for (let k = 0; k < 6; k++) {
      const line = lineFor(op, { rng });
      assert.equal(line.text.toLowerCase(), `${line.opener} ${line.said}`.toLowerCase());
      assert.equal(line.said, line.plain ? plainFor(op) : line.finding);
      const own = new Set(words(op.says));
      for (const w of words(line.finding.replace(/…$/, ""))) assert.ok(own.has(w.replace(/[.,;:]$/, "")) || own.has(w), `${op.name}: "${w}" isn't in its own finding`);
      assert.equal(line.mood, moodOf(op.value));
      assert.ok(/[0-9]\.[0-9]{2}$|no view$/.test(line.cite), line.cite);
    }
  }
});

test("openers match the mood they claim", () => {
  const rng = seeded(3);
  const loves = new Set(), gripes = new Set();
  for (let k = 0; k < 40; k++) { loves.add(lineFor(OPS[1], { rng }).opener); gripes.add(lineFor(OPS[0], { rng }).opener); }
  for (const o of loves) assert.ok(!gripes.has(o), `"${o}" used for both a love and a gripe`);
});

test("nobody repeats an opinion until the chorus has run out", () => {
  const heard = new Set(), rng = seeded(11), seen = [];
  for (let k = 0; k < OPS.length; k++) { const o = pickOpinion(CHORUS, { heard, rng }); seen.push(o.name); heard.add(o.name); }
  assert.equal(new Set(seen).size, OPS.length);
  const again = pickOpinion(CHORUS, { heard, rng });
  assert.ok(again, "starts over once all are heard");
  assert.equal(heard.size, 0);
});

test("what's nearby and a reply's mood both tilt the pick", () => {
  const rng = seeded(5);
  let nearHits = 0, against = 0;
  for (let k = 0; k < 400; k++) {
    if (pickOpinion(CHORUS, { near: new Set(["loop_BLOCK_5_to_BLOCK_6"]), rng }).name === "p100_pedestrian_street") nearHits++;
    if (["love"].includes(moodOf(pickOpinion(CHORUS, { against: "gripe", rng }).value))) against++;
  }
  assert.ok(nearHits > 400 / OPS.length * 1.8, `near: ${nearHits}`);
  assert.ok(against > 400 * 0.45, `replies disagree: ${against}`);
});

test("the chorus's own question is asked plainly, with the original in the log", () => {
  const q = questionLine(CHORUS, seeded());
  assert.equal(q.finding, CHORUS.questions[0].question);
  assert.equal(q.plain, true);
  assert.doesNotMatch(q.text, /chorus|\d/);
  assert.equal(questionLine({ opinions: OPS, questions: [] }), null);
  // A question worded some new way is asked as it is.
  const odd = questionLine({ opinions: OPS, questions: [{ question: "What now?", why: "" }] }, seeded());
  assert.equal(odd.text, "What now?");
});

// ---- plain speech, against the real chorus (a snapshot of clean_baseline, seed 42; and the
// freshly rendered one too, when there is one).
const REAL = [JSON.parse(readFileSync(new URL("./fixtures/clean_baseline_chorus.json", import.meta.url), "utf8"))];
const fresh = new URL("../../target/vibe-render/clean_baseline_chorus.json", import.meta.url);
if (existsSync(fresh)) REAL.push(JSON.parse(readFileSync(fresh, "utf8")));
/** Every number written in digits ("1,512", "24.7", "17 to 20"). */
const numbersIn = (s) => (s.match(/\d[\d,]*(?:\.\d+)?/g) || []).map((x) => parseFloat(x.replace(/,/g, "")));

test("every opinion in the chorus has a plain line, and today's output fills it", () => {
  for (const chorus of REAL) {
    for (const op of chorus.opinions) {
      assert.ok(PLAIN_NAMES.includes(op.name), `${op.name} has no plain line in plain.js`);
      assert.ok(plainFor(op), `${op.name}: its plain line couldn't be filled from its output`);
    }
  }
});

test("every number a person says is one the opinion reported", () => {
  for (const chorus of REAL) {
    for (const op of chorus.opinions) {
      const line = plainFor(op);
      if (!line) continue;
      const own = new Set(numbersIn(`${op.says} ${Object.values(op.details || {}).join(" ")}`));
      for (const x of numbersIn(line)) assert.ok(own.has(x), `${op.name} says ${x}, which isn't in its output: "${line}"`);
    }
  }
});

test("plain lines are short enough to say, and say no ids", () => {
  for (const op of REAL[0].opinions) {
    const line = plainFor(op);
    assert.ok(line.length <= 200, `${op.name}: ${line.length} characters`);
    assert.doesNotMatch(line, /_|\bP\d+\b|BLOCK/, `${op.name}: "${line}"`);
  }
});

test("a line that can't be filled falls back to the opinion's own words", () => {
  const changed = { ...OPS[0], says: "A finding worded some new way.", details: {} };
  assert.equal(plainFor(changed), null);
  const line = lineFor(changed, { rng: seeded() });
  assert.equal(line.plain, false);
  assert.equal(line.said, "A finding worded some new way.");
  assert.equal(plainFor({ name: "an_opinion_nobody_wrote_for", says: "x" }), null);
});

test("a reply that runs on keeps the sentence going", () => {
  const rng = seeded(2);
  for (let k = 0; k < 30; k++) {
    const line = lineFor(OPS[0], { rng, after: "love" });
    if (/[.?!]$/.test(line.opener)) continue;
    assert.match(line.text, new RegExp(`^${line.opener} [a-z0-9]`), line.text);
  }
});
