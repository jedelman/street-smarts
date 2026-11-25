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

export const PATTERNS = {
  37: { name: "House Cluster", gist: "each cluster of houses set apart by common land" },
  51: { name: "Green Streets", gist: "grass, with paving stones set in for the wheels" },
  52: { name: "Network of Paths and Cars", gist: "the path system is the spine" },
  53: { name: "Main Gateways", gist: "a marker where the paths that enter the site cross its edge" },
  61: { name: "Small Public Squares", gist: "small enough to feel intimate, not desolate" },
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
};

// The walk's shop kinds (frontages.js KITS), as nouns.
const SHOP_NOUNS = {
  coffee: "coffee shop", books: "bookshop", bistro: "bistro", studio: "design studio", bakery: "bakery",
  grocer: "grocer's", florist: "florist's", hardware: "hardware store", records: "record shop", tailor: "tailor's",
};

// P171's forms, as the tree's own kind names them.
const TREE_NOUNS = { avenue: "tree in an avenue", square: "tree ringing a square", umbrella: "spreading tree", grove: "tree in a grove" };

const NUMBER = ["", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"];

/** The element table from a packed GLB's root extras, as plain objects. */
export function readElements(table) {
  if (!table || table.version !== 1) return [];
  const s = (i) => (i >= 0 ? table.strings[i] : null);
  return table.elements.map(([g, k, src, typ, floors], index) => ({
    index, group: s(g), kind: s(k), source: s(src), typology: s(typ), floors: floors ?? null,
  }));
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
      break;
    case "ground":
      if (has(/_p172_meadow$/)) add(172);
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
      if (has(/_p126_middle$/)) add(126);
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
  if (el.group === "shops") return true;
  return /^(plaza|common|pocket|undecided|roof_garden|canopy_arcade|canopy_gallery|wall_niche)$/.test(el.kind || "")
    || (el.group === "activity" && (el.kind || "").startsWith("activity_"));
}

/** The prompt's verb phrase: "Sit in the courtyard". */
export function promptOf(el) {
  const n = nounOf(el);
  const k = el.kind || "";
  if (el.group === "trees") return `Perch in the ${n}`;
  if (el.group === "shops") return `Look in the ${n}`;
  if (/^(plaza|common|pocket|undecided)$/.test(k)) return `Sit in the ${n}`;
  if (k === "roof_garden") return "Peck around the roof garden";
  if (k === "canopy_arcade" || k === "canopy_gallery") return `Perch on the ${n}`;
  if (k === "wall_niche") return "Tuck into the nook";
  if (k.startsWith("activity_")) return `Chirp at the ${n.replace(/,.*$/, "")}`;
  return `Look at the ${n}`;
}

/** Lines for the log when you touch it: what it is, then each pattern that placed it. */
export function answerOf(el) {
  const n = nounOf(el);
  const head = el.kind === "context"
    ? "A building that stands there today, from Overture Maps."
    : n.startsWith("roof of") ? `The ${n}.`
    : `${article(n)[0].toUpperCase()}${article(n).slice(1)} ${n}.`;
  const lines = [head];
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

/** A short line for stepping onto something new underfoot, or null. */
export function arrivalOf(el) {
  const k = el.kind || "";
  const n = nounOf(el);
  if (el.group === "trees") return `Up in ${article(n)} ${n}.`;
  if (k === "meadow") return "Into the meadow.";
  if (/^(plaza|common|pocket|undecided)$/.test(k)) return `Into ${article(n)} ${n}.`;
  if (/^(local|pedestrian|arterial|street)(_grass)?$/.test(k)) return `Onto ${article(n)} ${n}.`;
  if (k === "roof_garden") return "Down into a roof garden.";
  if (k === "roof") return "Up on a roof.";
  if (k === "canopy_arcade" || k === "canopy_gallery") return `Up on ${article(n)} ${n}.`;
  return null;
}
