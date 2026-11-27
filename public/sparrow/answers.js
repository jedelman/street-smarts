// What a piece of the model is, in words, and which patterns placed it.
//
// Everything here is read off the element's own record (see
// tools/vibe-render/pack_glb.py): its kind, and the pattern tokens in the id
// of the source record it came from. The pipeline names ids after what made
// them ("..._P95_courtyard_p0.3", "..._p61_new_sq0",
// "site_perimeter_p53_gateway"), so the patterns named here are the ones the
// data names. When the id says nothing, the answer names no pattern.
// Each gloss paraphrases the first line of that pattern's own module doc in
// crates/street-smarts-patterns/src/.
//
// Pure, so Node can test it: tools/sparrow/answers.test.mjs.

import { BUSINESSES } from "./shopfronts.js";

export const PATTERNS = {
  34: { name: "Interchange", gist: "a station that is a center: continuous with the streets, ringed by homes and workplaces" },
  37: { name: "House Cluster", gist: "each cluster of houses set apart by common land" },
  51: { name: "Green Streets", gist: "grass, with paving stones set in for the wheels" },
  52: { name: "Network of Paths and Cars", gist: "the path system is the spine" },
  53: { name: "Main Gateways", gist: "a marker where the paths that enter the site cross its edge" },
  61: { name: "Small Public Squares", gist: "small enough to feel intimate, not desolate" },
  9: { name: "Scattered Work", gist: "work scattered through the homes, so no part of town is all one or the other" },
  87: { name: "Individually Owned Shops", gist: "many very small shops along the street, each with its own door" },
  95: { name: "Building Complex", gist: "a cluster of buildings, not one tower on a lot" },
  98: { name: "Circulation Realms", gist: "distinct walking and driving zones" },
  107: { name: "Wings of Light", gist: "no point inside far from an outside wall" },
  108: { name: "Connected Buildings", gist: "neighbors with no reason to stand apart, joined by a party wall" },
  118: { name: "Roof Garden", gist: "flat, usable roof, grown into a garden" },
  119: { name: "Arcades", gist: "a covered walk along the building's edge" },
  120: { name: "Paths and Goals", gist: "paths go where people actually want to walk" },
  124: { name: "Activity Pockets", gist: "a small, partly enclosed pocket where a building meets a square" },
  126: { name: "Something Roughly in the Middle", gist: "a public space without a middle is likely to stay empty" },
  160: { name: "Building Edge", gist: "an edge deep enough for seats and bay windows" },
  161: { name: "Sunny Place", gist: "a sheltered, sunny spot just outside" },
  166: { name: "Gallery Surround", gist: "covered walks at the upper stories" },
  171: { name: "Tree Places", gist: "trees planted to make places: avenues, squares, groves, a spreading tree" },
  172: { name: "Garden Growing Wild", gist: "the open ground grows as meadow, with no bare earth" },
  64: { name: "Pools and Streams", gist: "the rain kept above ground, in ponds and streams you can walk beside" },
};

// The walk's businesses (shopfronts.js BUSINESSES), as nouns.
const SHOP_NOUNS = Object.fromEntries(BUSINESSES.map((b) => [b.id, b.noun]));

// P171's forms, as the tree's own kind names them.
const TREE_NOUNS = { avenue: "tree in an avenue", square: "tree ringing a square", umbrella: "spreading tree", grove: "tree in a grove", cypress: "bald cypress" };

const NUMBER = ["", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"];

/** The element table from a packed GLB's root extras, as plain objects. */
export function readElements(table) {
  if (!table || table.version !== 1) return [];
  const s = (i) => (i >= 0 ? table.strings[i] : null);
  return table.elements.map(([g, k, src, typ, floors, uses, area], index) => {
    const el = {
      index, group: s(g), kind: s(k), source: s(src), typology: s(typ), floors: floors ?? null,
      uses: uses == null ? null : s(uses), // P9's floor uses, ground floor first: "pwwll"
      area: area ?? null, // the building's footprint, m² (render.py); older models don't carry it
    };
    el.facts = factsFor(table.facts, el);
    return el;
  });
}

/**
 * The words a pipeline attached to a place (the model's `extras.streetSmarts.facts`): its own, by source id, or
 * its group's template ("group:trees"), filled from the element's fields. In a template, {field|fallback} is the
 * field's value or the fallback, and {field?text} is the text with # for the value, or nothing.
 */
export function factsFor(facts, el) {
  if (!facts) return null;
  if (el.source && facts[el.source]) return facts[el.source];
  const t = facts[`group:${el.group}`];
  if (!t) return null;
  const fill = (text) => text && text.replace(/\{(\w+)([|?])([^}]*)\}/g, (_, f, op, rest) => {
    const v = el[f];
    const has = v != null && v !== "";
    return op === "|" ? (has ? String(v) : rest) : has ? rest.replace("#", String(v)) : "";
  });
  return { ...t, name: fill(t.name), prompt: fill(t.prompt), arrival: fill(t.arrival), answer: (t.answer || []).map(fill) };
}

/**
 * Pattern numbers that made this element. An id is a lineage (a block, then
 * a merged building, then its courtyard...), so only the tokens that name
 * what made this kind of thing count: a P107 courtyard on a P108-merged
 * building is P107's, and the building is P108's.
 */
export function patternsOf(el) {
  const id = el.source || "";
  const kind = el.kind || "";
  const has = (re) => re.test(id);
  const out = [];
  const add = (n) => { if (!out.includes(n)) out.push(n); };
  switch (el.group) {
    case "streets":
      if (has(/^(path|loop|localloop)_/)) [52, 98, 120].forEach(add);
      if (has(/_p61_link_/)) add(61);
      if (/_grass$/.test(kind)) add(51); // render.py's kind for path_network's grass_pavers surface
      break;
    case "trees":
      if (has(/^p171_/)) add(171);
      if (has(/^p64_/)) add(64);
      break;
    case "ground":
      if (has(/_p172_meadow$/)) add(172);
      if (has(/^p64_/)) add(64);
      break;
    case "transit": // P34's track, platform and canopy
      if (has(/p34/)) add(34);
      break;
    case "shops": // a shop front at a door: P87's own door, or the building's
      if (el.p87) add(87);
      break;
    case "plazas": // open space: whoever carved it
      if (has(/_P107_courtyard/)) add(107);
      else if (has(/_P95_courtyard/)) add(95);
      else if (has(/_p61_(new_)?(sq|undecided)\d*$/)) add(61);
      else if (has(/_p161_sunny$/)) add(161);
      else if (has(/_BLOCK_\d+_common$/)) add(37);
      break;
    case "activity":
    case "activity_glow":
      if (has(/_p34_station$/)) add(34);
      else if (has(/_p126_middle$/)) add(126);
      else if (has(/_p53_gateway$/)) add(53);
      else if (has(/_p61_(new_)?sq\d+_activity$/)) add(61);
      else if (has(/_activity$/)) add(124); // "<pocket id>_activity"
      break;
    default: // buildings and what hangs off them: the record is the building's
      if (kind === "roof_garden") add(118); // only P118 sets a roof occupiable
      else if (kind === "canopy_arcade") add(119);
      else if (kind === "canopy_gallery") add(166);
      else if (kind === "wall_niche") add(160); // only P160 adds wall niches
      else if (kind === "building_unshaped") add(95); // a P95 pad P107 never shaped
      else if (kind === "building_shaped") {
        if (has(/_P95_cell_/) || /^p95_/.test(el.typology || "")) add(95);
        if (/^p107_/.test(el.typology || "")) add(107);
        if (has(/^p108_merged_/)) add(108);
        if (/w/.test(el.uses || "")) add(9);
      }
  }
  return out;
}

const article = (w) => (/^[aeiou]/i.test(w) ? "an" : "a");

/** What it is, as a noun phrase without an article: "courtyard", "four-story building". */
export function nounOf(el) {
  const k = el.kind || "";
  if (el.group === "trees") return TREE_NOUNS[k] || "tree";
  if (el.group === "shops") return SHOP_NOUNS[k] || "shop";
  if (/_grass$/.test(k)) return `${nounOf({ ...el, kind: k.slice(0, -6) })} of grass and paving stones`;
  const p = patternsOf(el);
  const storied = (word) => (el.floors > 0 && el.floors <= 12 ? `${NUMBER[el.floors]}-story ${word}` : word);
  switch (k) {
    case "plaza":
      if (p.includes(61)) return "small public square";
      if (/courtyard/i.test(el.source || "")) return "courtyard";
      return "plaza";
    case "common": return p.includes(161) ? "sunny place" : "common";
    case "pocket": return "activity pocket";
    case "undecided": return "patch of undecided ground";
    case "meadow": return "meadow";
    case "pond": return "pond";
    case "marsh": return "marsh";
    case "stream": return "stream";
    case "boardwalk": return "boardwalk";
    case "track_bed": return "light rail track, grass between the rails";
    case "rail": return "rail";
    case "platform": return "light rail platform";
    case "platform_canopy": return "platform canopy";
    case "platform_post": return "platform canopy's post";
    case "activity_transit": return "light rail stop";
    case "local": return "local street";
    case "pedestrian": return "pedestrian street";
    case "arterial": return "arterial road";
    case "street": return "street";
    case "building_shaped": return storied(/courtyard/.test(el.typology || "") ? "courtyard building" : "building");
    case "building_unshaped": return "building pad, not yet shaped";
    case "roof": return el.floors > 0 && el.floors <= 12 ? `roof of a ${NUMBER[el.floors]}-story building` : "roof";
    case "roof_garden": return "roof garden";
    case "canopy_arcade": return "arcade";
    case "canopy_gallery": return "gallery";
    case "wall_niche": return "nook in the building's edge";
    case "interior_wall": return "room's wall";
    case "floor_plate": return "floor";
    case "window_glow": return "lit window";
    case "context": return "building that stands there today";
    case "activity_gateway": return "gateway";
    default:
      if (k.startsWith("activity_")) {
        if (p.includes(126)) return "marker, roughly in the middle";
        return "gathering point";
      }
      return k.replace(/_/g, " ") || "thing";
  }
}

/** The things worth a glint and a prompt (not every wall). */
export function touchable(el) {
  if (el.group === "shops" || el.kind === "platform") return true;
  if (el.facts && (el.group === "buildings" || el.facts.touch)) return true;
  return /^(plaza|common|pocket|undecided|roof_garden|canopy_arcade|canopy_gallery|wall_niche)$/.test(el.kind || "")
    || (el.group === "activity" && (el.kind || "").startsWith("activity_"));
}

/** The prompt's verb phrase: "Sit in the courtyard". */
export function promptOf(el) {
  const n = el.facts && el.facts.name && el.group === "trees" ? el.facts.name : nounOf(el);
  const k = el.kind || "";
  if (el.group === "trees") return `Perch in the ${n}`;
  if (el.facts && el.facts.prompt) return el.facts.prompt;
  if (el.group === "shops") return `Look in the ${n}`;
  if (el.kind === "platform" || el.kind === "activity_transit") return "Wait for the tram";
  if (/^(plaza|common|pocket|undecided)$/.test(k)) return `Sit in the ${n}`;
  if (k === "roof_garden") return "Peck around the roof garden";
  if (k === "canopy_arcade" || k === "canopy_gallery") return `Perch on the ${n}`;
  if (k === "wall_niche") return "Tuck into the nook";
  if (k.startsWith("activity_")) return `Chirp at the ${n.replace(/,.*$/, "")}`;
  return `Look at the ${n}`;
}

/** Lines for the log when you touch it: what it is, then each pattern that placed it. */
export function answerOf(el) {
  if (el.facts && el.facts.answer && el.facts.answer.length) return el.facts.answer.slice();
  const n = nounOf(el);
  const head = el.kind === "context"
    ? "A building that stands there today, from Overture Maps."
    : n.startsWith("roof of") ? `The ${n}.`
    : `${article(n)[0].toUpperCase()}${article(n).slice(1)} ${n}.`;
  const lines = [head];
  if (el.group === "transit" || el.kind === "activity_transit") {
    lines.push("The line is a sketch from the direction of the Tide's Newtown Road station, not HRT's route; where the station stands is the pipeline's choice.");
  }
  const floorsLine = usesLine(el.uses);
  if (floorsLine) lines.push(floorsLine);
  if (el.group === "shops") {
    // Which shop it is, its paint and sign, are the walk's; where its door is, is the pipeline's.
    lines.push(el.p87 ? "Which shop is ours; its door is the pipeline's." : "At the building's own door; which shop is ours.");
  }
  const nums = patternsOf(el);
  if (nums[0] === 52) {
    // The path network pulls from three patterns; the spine one leads.
    lines.push(`P52 ${PATTERNS[52].name}: ${PATTERNS[52].gist}.`, `With P98 ${PATTERNS[98].name} and P120 ${PATTERNS[120].name}.`);
    nums.splice(0, 3);
  }
  for (const num of nums) {
    const pat = PATTERNS[num];
    if (pat) lines.push(`P${num} ${pat.name}: ${pat.gist}.`);
  }
  return lines;
}

/**
 * What a building's floors are for, from P9's uses ("pwwll", ground floor
 * first): "Shops at street level, work on the next two floors, homes on the
 * three above." Null without uses.
 */
export function usesLine(uses) {
  if (!uses) return null;
  const ground = { p: "Shops at street level", w: "Workshops at street level", l: "Homes at street level" }[uses[0]];
  const parts = [ground];
  let i = 1;
  while (i < uses.length) {
    let j = i;
    while (j < uses.length && uses[j] === uses[i]) j++;
    const n = j - i, what = { w: "work", l: "homes", p: "shops" }[uses[i]];
    const where = i === 1 ? (n === 1 ? "on the floor above" : `on the ${NUMBER[n] || n} floors above`)
      : (n === 1 ? "on the next floor up" : `on the ${NUMBER[n] || n} floors above that`);
    parts.push(`${what} ${where}`);
    i = j;
  }
  return parts.join(", ") + ".";
}

/** A short line for stepping onto something new underfoot, or null. */
export function arrivalOf(el) {
  if (el.facts) return el.facts.arrival || (el.group === "trees" ? `Up in ${el.facts.name || "a tree"}.` : null);
  const k = el.kind || "";
  const n = nounOf(el);
  if (el.group === "trees") return `Up in ${article(n)} ${n}.`;
  if (k === "meadow") return "Into the meadow.";
  if (k === "marsh") return "Into the marsh, among the rushes.";
  if (k === "pond" || k === "stream") return `Down on the ${k}'s water.`;
  if (k === "boardwalk") return "Onto the boardwalk, out over the swamp.";
  if (k === "platform") return "Onto the platform.";
  if (k === "track_bed" || k === "rail") return "Onto the tracks. Mind the tram.";
  if (/^(plaza|common|pocket|undecided)$/.test(k)) return `Into ${article(n)} ${n}.`;
  if (/^(local|pedestrian|arterial|street)(_grass)?$/.test(k)) return `Onto ${article(n)} ${n}.`;
  if (k === "roof_garden") return "Down into a roof garden.";
  if (k === "roof") return "Up on a roof.";
  if (k === "canopy_arcade" || k === "canopy_gallery") return `Up on ${article(n)} ${n}.`;
  return null;
}
