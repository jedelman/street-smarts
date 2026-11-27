// Shop fronts, grown by an L-system. Each front starts as one symbol,
// Front(0, L), and rules rewrite it, a generation at a time, into piers,
// bays, glass, doors, signs, awnings and what spills onto the pavement,
// until only terminals are left; a turtle then walks the terminals along the
// wall and lays down the parts (frontages.js turns each into an instance).
// Rules are per business family (a shop, an office, a school, a cafe) with
// a business's own touches on top: a bike shop racks bikes out front, an
// accountant's front is frosted with a brass plaque, an architects' studio
// is glass to the ceiling with models in the window.
//
// Which business is where is ours, not data: the pipeline says where a
// shop front is (P87, render.py's shop rows), not what it sells. BUSINESSES'
// weights are the walk's own mix (see the comment on WEIGHTS).
//
// Pure, no three.js: tools/sparrow/shopfronts.test.mjs tests it in Node.

export const G = 3.5; // the ground floor's height (render.py FLOOR_TO_FLOOR_M)
export const ST = G - 0.62; // top of the storefront, under the sign band
export const GLASS_TOP = ST - 0.55; // top of the display glass, under the transom
export const BAY = 3.3; // pier spacing

const GHENT_TRIMS = ["#efe6d2", "#e2d3ac", "#f6f1e7", "#c9b78a"];
const GHENT_BRICKS = ["#8a4a36", "#9a5a40", "#7a3f30", "#a8705a"];
const serif = (w, s = 92, it = "") => `${it}${w} ${s}px Georgia, 'Times New Roman', serif`;
const sans = (w, s = 86) => `${w} ${s}px 'Helvetica Neue', Arial, sans-serif`;
const mono = (w, s = 80) => `${w} ${s}px 'DM Mono', ui-monospace, Menlo, monospace`;

/**
 * Each business: its family (which rules grow its front), its sign, its
 * paints, what you see through its glass (`inside`, a tile in frontages.js's
 * interiors atlas), its blade-sign icon, and what it puts on the pavement.
 */
export const BUSINESSES = [
  // Shops selling goods.
  { id: "greengrocer", noun: "greengrocer's", family: "shop", sign: "GREENGROCER", font: sans(800, 80), ink: "#f2ecd8",
    paints: ["#3f6b35", "#2f5d3a", "#4a5a2a"], stripes: ["#3f6b35", "#f2ecd8"], inside: "produce", icon: "apple", spill: "crates" },
  { id: "florist", noun: "florist's", family: "shop", sign: "FLOWERS", font: serif(700, 88, "italic "), ink: "#f6efe0",
    paints: ["#7a4a6b", "#5e3b5e", "#8a5a7a"], stripes: ["#c96a8a", "#f6efe0"], inside: "flowers", icon: "flower", spill: "buckets" },
  { id: "wine", noun: "wine shop", family: "shop", sign: "WINE", font: serif(700, 96, "italic "), ink: "#f0d9a8",
    paints: ["#4a1f24", "#3a1a1e", "#2a2a22"], stripes: ["#6b2430", "#f0d9a8"], inside: "bottles", icon: "bottle", spill: "barrel" },
  { id: "bikes", noun: "bike shop", family: "shop", sign: "BICYCLES", font: sans(800, 80), ink: "#f2d27a",
    paints: ["#1f3a5a", "#2b2d31", "#8a2b1f"], stripes: null, inside: "bikes", icon: "bike", spill: "bikes" },
  { id: "bakery", noun: "bakery", family: "shop", sign: "BAKERY", font: serif(700), ink: "#f6efe0",
    paints: ["#8b5a2b", "#a0522d", "#6e4b3a"], stripes: ["#c9a46a", "#f6efe0"], inside: "bread", icon: "loaf", spill: "bench" },
  { id: "books", noun: "bookshop", family: "shop", sign: "BOOKS", font: serif(700), ink: "#f3ede0",
    paints: ["#6b2430", "#5a2a2a", "#7a3b2e", "#4a2a3a"], stripes: ["#6b2430", "#efe6d2"], inside: "shelves", icon: "book", spill: "books" },
  { id: "hardware", noun: "hardware store", family: "shop", sign: "HARDWARE", font: sans(800, 80), ink: "#f2d27a",
    paints: ["#8a2b1f", "#2b3a55", "#3a2f22"], stripes: ["#c8a23a", "#2b2b2b"], inside: "tools", icon: "hammer", spill: "crates" },
  { id: "records", noun: "record shop", family: "shop", sign: "RECORDS", font: "700 84px 'Courier New', ui-monospace, monospace", ink: "#e9d9a8",
    paints: ["#1f2a44", "#3b2a3f", "#2a2a2a"], stripes: ["#d9b46a", "#1f2a44"], inside: "records", icon: "disc", spill: "aframe" },
  { id: "tailor", noun: "tailor's", family: "shop", sign: "TAILOR", font: serif(500, 88), ink: "#efe6d2",
    paints: ["#2b2d31", "#3a3d42", "#4a3a2a"], stripes: null, inside: "racks", icon: "scissors", spill: null },
  // A service with a shop front: among the three commonest ground-floor businesses in Paris and Barcelona both.
  { id: "hairdresser", noun: "hairdresser's", family: "shop", sign: "HAIR", font: serif(700, 96, "italic "), ink: "#f6efe0",
    paints: ["#1a1714", "#3b2a3f", "#2f5d62", "#8a4a6b"], stripes: null, inside: "salon", icon: "comb", spill: "planters" },
  // Food and drink.
  { id: "coffee", noun: "coffee shop", family: "food", sign: "COFFEE", font: serif(700), ink: "#f3ede0",
    paints: ["#2f4a3a", "#34503c", "#2f5d62", "#3e5a3a"], stripes: ["#2f4a3a", "#efe6d2"], inside: "counter", icon: "cup", spill: "tables" },
  { id: "bistro", noun: "bistro", family: "food", sign: "BISTRO", font: serif(600, 92, "italic "), ink: "#f0d9a8",
    paints: ["#1f2a44", "#1a1714", "#3b2a3f", "#22313b"], stripes: ["#8a2b1f", "#f0e3c8"], inside: "lamps", icon: "glass", spill: "tables3" },
  // Work at street level: offices and studios, and a little school.
  { id: "accountant", noun: "accountant's office", family: "office", sign: "ACCOUNTANTS", font: serif(500, 72), ink: "#2a2a2a",
    paints: ["#2b2d31", "#3a3d42", "#4a4f55"], band: "#e9e4d8", stripes: null, inside: "office", icon: "ledger", spill: "planters" },
  { id: "architect", noun: "architects' studio", family: "studio", sign: "ARCHITECTS", font: mono(500, 72), ink: "#1a1714",
    paints: ["#1d1f22", "#2b2d31"], band: "#f3f1ec", stripes: null, inside: "models", icon: "setsquare", spill: null },
  { id: "design", noun: "design studio", family: "studio", sign: "STUDIO", font: mono(500), ink: "#1a1714",
    paints: ["#1d1f22", "#2b2d31", "#3a3d42"], band: "#f3f1ec", stripes: null, inside: "desks", icon: "square", spill: null },
  { id: "school", noun: "little school", family: "school", sign: "LITTLE SCHOOL", font: sans(800, 70), ink: "#ffffff",
    paints: ["#2f7a8a", "#d9772f", "#5a8a3a", "#8a4a8a"], stripes: null, inside: "school", icon: "abc", spill: "school" },
];

/**
 * More services, for Ghent Streets: Policy Brief No. 1's by-right uses that the list above lacks (a dental
 * office among the "ground-floor professional offices (medical and dental included)", an art gallery, a small
 * fitness studio) and two more personal services (a barber, a dry cleaner, the brief's own example). Not in
 * Eastside Commons' mix (WEIGHTS), so its plan grows exactly as before; a pipeline that names one gets it.
 */
export const MORE_BUSINESSES = [
  { id: "dentist", noun: "dentist's office", family: "office", sign: "DENTIST", font: sans(700, 74), ink: "#24474a",
    paints: ["#3d6f73", "#2f5d62", "#4a6b6e"], band: "#eef3f2", stripes: null, inside: "dental", icon: "tooth", spill: "planters" },
  { id: "gallery", noun: "art gallery", family: "gallery", sign: "GALLERY", font: serif(500, 86, "italic "), ink: "#1a1714",
    paints: ["#1a1714", "#3a3d42", "#5a2a2a"], band: "#f6f4ee", stripes: null, inside: "gallery", icon: "frame", spill: "easel" },
  { id: "yoga", noun: "yoga studio", family: "fitness", sign: "YOGA", font: sans(600, 86), ink: "#f3ede0",
    paints: ["#6b8a5a", "#8a6b4a", "#4a6b7a"], stripes: ["#6b8a5a", "#efe6d2"], inside: "yoga", icon: "lotus", spill: "mats" },
  { id: "barber", noun: "barbershop", family: "salon", sign: "BARBER", font: serif(700, 92), ink: "#f6efe0",
    paints: ["#1f2a44", "#7a2430", "#1a1714"], stripes: ["#b23a2e", "#f4efe4"], inside: "barber", icon: "pole", spill: "bench" },
  { id: "cleaners", noun: "dry cleaner's", family: "shop", sign: "CLEANERS", font: sans(800, 74), ink: "#f2f2ee",
    paints: ["#2b4a7a", "#3a3d42", "#2f5d62"], stripes: ["#2b4a7a", "#f2f2ee"], inside: "cleaners", icon: "hanger", spill: null },
];
/** Every business the walk can draw: Eastside's mix first (its atlas rows don't move), then the rest. */
export const ALL_BUSINESSES = [...BUSINESSES, ...MORE_BUSINESSES];

/**
 * How often each business comes up. The split between kinds of ground floor
 * is El Born's district, Ciutat Vella, in Barcelona's 2024 census of
 * ground-floor businesses ("Estudi de les activitats econòmiques en planta
 * baixa", table 2.4): everyday food 923, non-food shops 1,902, bars and
 * restaurants 1,580, services 1,603 (and 616 "other": gyms, associations,
 * religious, not drawn here). Within each, the split is ours, guided where it
 * can be by the city's commonest activities (table 2.7: offices 3,411,
 * hairdressers 2,807, education 2,214): the census doesn't count bike shops or
 * architects separately.
 */
const DISTRICT = { food: 923, retail: 1902, hospitality: 1580, services: 1603 };
const WITHIN = {
  food: { greengrocer: 0.45, bakery: 0.35, wine: 0.2 },
  retail: { tailor: 0.25, florist: 0.15, books: 0.15, bikes: 0.15, hardware: 0.15, records: 0.15 },
  hospitality: { coffee: 0.5, bistro: 0.5 },
  services: { accountant: 0.17, architect: 0.12, design: 0.115, hairdresser: 0.333, school: 0.262 },
};
export const WEIGHTS = Object.fromEntries(Object.entries(WITHIN).flatMap(([k, split]) => Object.entries(split).map(([id, f]) => [id, DISTRICT[k] * f])));

const pick = (rng, xs) => xs[Math.floor(rng() * xs.length)];

/** A business for a front, by WEIGHTS, never one of the `recent` few next to it. */
export function chooseBusiness(rng, recent = []) {
  const pool = BUSINESSES.filter((b) => !recent.includes(b.id));
  const total = pool.reduce((s, b) => s + (WEIGHTS[b.id] || 1), 0);
  let r = rng() * total;
  for (const b of pool) { r -= WEIGHTS[b.id] || 1; if (r <= 0) return b; }
  return pool[pool.length - 1];
}

// ---------------------------------------------------------------- the L-system
// A symbol is { s: name, ...params }. A rule maps a symbol to the symbols that
// replace it, or returns null when the symbol is a terminal. Every generation
// rewrites every non-terminal at once, as in an L-system; it stops when
// nothing rewrites (or at MAX_GENERATIONS, as a guard).
const MAX_GENERATIONS = 12;

export function derive(axiom, rules, ctx) {
  let word = axiom;
  for (let gen = 0; gen < MAX_GENERATIONS; gen++) {
    let changed = false;
    const next = [];
    for (const sym of word) {
      const rule = rules[sym.s];
      const out = rule ? rule(sym, ctx) : null;
      if (out) { next.push(...out); changed = true; } else next.push(sym);
    }
    word = next;
    if (!changed) break;
  }
  return word;
}

/** The rules every front grows by; `ctx` carries the business, its look and the entry. */
export const RULES = {
  // The whole front: what's above the shop, the front itself, and what's in front of it.
  Front: ({ a, b }, c) => [{ s: "Cornice", a, b }, { s: "Band", a, b }, { s: "Facade", a, b }, { s: "Spill", a, b }],
  // Split around the entry; a pier closes each side of it.
  Facade: ({ a, b }, c) => {
    const e0 = c.at - c.entryW / 2, e1 = c.at + c.entryW / 2;
    const out = [];
    if (e0 - 0.18 - a > 0.9) out.push({ s: "Run", a, b: e0 - 0.18 });
    out.push({ s: "Pier", x: e0 - 0.18 }, { s: "Entry", x: c.at }, { s: "Pier", x: e1 + 0.18 });
    if (b - (e1 + 0.18) > 0.9) out.push({ s: "Run", a: e1 + 0.18, b });
    return out;
  },
  // A run of wall halves at a pier until each piece is about a bay wide.
  Run: ({ a, b }) => {
    const w = b - a;
    if (w > BAY * 1.5) { const m = a + Math.round(w / BAY / 2) * BAY; return [{ s: "Run", a, b: m }, { s: "Pier", x: m }, { s: "Run", a: m, b }]; }
    return [{ s: "Bay", a: a + 0.2, b: b - 0.2 }];
  },
  // A bay, by family.
  Bay: ({ a, b }, c) => {
    if (b - a < 0.6) return [{ s: "Blank" }];
    switch (c.biz.family) {
      case "studio": // glass floor to ceiling, a thin frame, nothing below
        return [{ s: "Kick", a, b }, { s: "Lights", a, b, y0: 0.18, y1: ST - 0.08 }];
      case "office": // a plain base, the lower glass frosted, blinds above
        return [{ s: "Bulkhead", a, b, style: "plain" }, { s: "Sill", a, b }, { s: "Frost", a, b }, { s: "Lights", a, b, y0: 1.35, y1: GLASS_TOP, tile: "office" },
          { s: "Head", a, b }, { s: "Transom", a, b }];
      case "school": // low windows (a child's eye height), bright frames
        return [{ s: "Bulkhead", a, b, style: "low" }, { s: "Lights", a, b, y0: 0.42, y1: GLASS_TOP, frame: true }, { s: "Head", a, b }, { s: "Transom", a, b }];
      default: // shops and cafes: a bulkhead, a sill, display glass, a transom, maybe an awning
        return [{ s: "Bulkhead", a, b, style: c.bulk }, { s: "Sill", a, b }, { s: "Lights", a, b, y0: 0.62, y1: GLASS_TOP }, { s: "Head", a, b },
          { s: "Transom", a, b }, ...(c.awning ? [{ s: "Awning", a, b }] : [])];
    }
  },
  // Wide glass divides by a mullion, recursively.
  Lights: (sym, c) => {
    const { a, b } = sym;
    if (b - a > 2.4) { const m = (a + b) / 2; return [{ ...sym, b: m - 0.035 }, { s: "Mullion", x: m, y0: sym.y0, y1: sym.y1 }, { ...sym, a: m + 0.035 }]; }
    return [{ ...sym, s: "Glass" }];
  },
  // The way in.
  Entry: ({ x }, c) => {
    const parts = [{ s: "Door", x }, { s: "DoorLight", x }, { s: "Brass", x }, { s: "EntryHead", x }, { s: "EntryTransom", x }, { s: "SideLights", x }];
    if (c.biz.family === "office" || c.biz.family === "studio") parts.push({ s: "Plaque", x });
    else parts.push({ s: "Blade", x: Math.min(c.L - 0.3, x + c.entryW / 2 + 0.18) });
    return parts;
  },
  // The sign: painted letters on a band for shops, cafes and the school; a quieter
  // band with smaller letters for offices and studios.
  Band: ({ a, b }, c) => [{ s: "SignBand", a, b }, { s: "Letters", a, b, small: c.biz.family === "office" || c.biz.family === "studio" }],
  // What a business puts on the pavement.
  Spill: ({ a, b }, c) => {
    const at = c.at, f = c.flip, s = c.biz.spill;
    const out = [];
    const away = (d) => at - d * f; // along the wall, away from the door on one side
    if (s === "crates" || s === "buckets") for (let k = 0; k < 4; k++) out.push({ s: s === "crates" ? "Crate" : "Bucket", x: at + (k < 2 ? -1 : 1) * (1.6 + (k % 2) * 0.7) });
    else if (s === "bikes") { out.push({ s: "Rack", x: away(2.2) }); for (let k = 0; k < 3; k++) out.push({ s: "Bike", x: away(1.5 + k * 0.6) }); }
    else if (s === "barrel") out.push({ s: "Barrel", x: away(1.6) }, { s: "Crate", x: away(2.4), bottles: true }, { s: "AFrame", x: at + 1.4 * f });
    else if (s === "books") out.push({ s: "BookTable", x: away(2.2) });
    else if (s === "tables" || s === "tables3") { const n = s === "tables3" ? 3 : 2; for (let k = 0; k < n; k++) out.push({ s: "Table", x: ((k + 0.5) * (b - a)) / n + a, out: 2.3 + (k % 2) * 0.3 }); }
    else if (s === "bench") out.push({ s: "Bench", x: away(2.0) });
    else if (s === "aframe") out.push({ s: "AFrame", x: at + 1.4 * f });
    else if (s === "school") out.push({ s: "Bench", x: away(2.2) }, { s: "Planter", x: at - (c.entryW / 2 + 0.5) }, { s: "Planter", x: at + c.entryW / 2 + 0.5 }, { s: "Scooter", x: away(1.3) });
    if ((s === "planters" || (s !== "school" && c.planters))) out.push({ s: "Planter", x: at - (c.entryW / 2 + 0.5) }, { s: "Planter", x: at + c.entryW / 2 + 0.5 });
    return out.filter((p) => p.s === "Table" ? Math.abs(p.x - at) >= 1.2 : true).filter((p) => p.x > 0.2 && p.x < c.L - 0.2);
  },
};

/**
 * The parts for one shop front of length L (metres along the wall), with its
 * door centred at `at`: the L-system's terminals, each turned into a part
 * description ({ k: part kind, x, y, out, sx, sy, sz, color, tex, rx, ry })
 * for frontages.js to place. `flip` is which way along the wall is "away".
 */
export function growFront({ biz, L, at, doorWidth, rng, arcade = false, flip = 1 }) {
  const paint = pick(rng, biz.paints);
  const trim = biz.family === "studio" || biz.family === "office" ? "#d9d6cf" : pick(rng, GHENT_TRIMS);
  const brick = pick(rng, GHENT_BRICKS);
  const bulk = biz.family === "shop" || biz.family === "food" ? pick(rng, ["panel", "panel", "brick"]) : "plain";
  const pier = biz.family === "studio" ? "#1d1f22" : bulk === "brick" || rng() < 0.5 ? brick : paint;
  const awning = !!biz.stripes && !arcade && rng() < 0.85; // a P119 arcade is shelter enough
  const awnTilt = 0.32 + rng() * 0.12;
  const entryW = Math.min(2.4, Math.max(1.5, doorWidth + 0.5));
  const doorLeaf = Math.min(1.1, Math.max(0.9, doorWidth));
  const ctx = { biz, L, at, entryW, rng, flip, bulk, awning, planters: rng() < 0.6 };
  const word = derive([{ s: "Front", a: 0, b: L }], RULES, ctx);
  const parts = [];
  const P = (k, x, y, out, sx, sy, sz, color, extra = {}) => parts.push({ k, x, y, out, sx, sy, sz, color, ...extra });
  const v = { biz, rng, flip, paint, trim, brick, pier, awning, awnTilt, entryW, doorLeaf };
  for (const t of word) terminalParts(t, v, P);
  return { parts, paint, trim, word };
}

/** One terminal of a front's word, as parts. `v` is the front's look: biz, rng, flip, paint, trim, brick, pier,
 * awning, awnTilt, entryW, doorLeaf. */
function terminalParts(t, v, P) {
  const { biz, rng, flip, paint, trim, brick, pier, awning, awnTilt, entryW, doorLeaf } = v;
  const fill = (kind) => pick(rng, kind === "crate" ? ["#c23f2a", "#e0a030", "#6a9a3a", "#d9c04a"] : ["#c96a8a", "#f2c94a", "#e8e0f0", "#f08a4a"]);
  switch (t.s) {
    case "Pier":
      P("detail", t.x, ST / 2, 0.09, biz.family === "studio" ? 0.12 : 0.36, ST, 0.22, pier);
      if (!awning && biz.family !== "studio") P("near", t.x, ST - 0.1, 0.18, 0.3, 0.26, 0.3, trim);
      break;
    case "Bulkhead": {
      const sw = t.b - t.a, cx = (t.a + t.b) / 2, h = t.style === "low" ? 0.42 : 0.55;
      P("detail", cx, h / 2, 0.06, sw, h, 0.14, t.style === "brick" ? brick : paint);
      if (t.style === "panel") P("near", cx, 0.28, 0.14, sw - 0.2, 0.34, 0.02, trim);
      break;
    }
    case "Kick": P("detail", (t.a + t.b) / 2, 0.09, 0.05, t.b - t.a, 0.18, 0.12, "#1d1f22"); break;
    case "Sill": P("detail", (t.a + t.b) / 2, 0.58, 0.08, t.b - t.a + 0.1, 0.07, 0.24, trim); break;
    case "Head": P("detail", (t.a + t.b) / 2, GLASS_TOP + 0.04, 0.06, t.b - t.a + 0.05, 0.08, 0.12, paint); break;
    case "Transom": P("glass", (t.a + t.b) / 2, ST - 0.26, 0.035, t.b - t.a, 0.42, 1, "#4a3826"); break;
    case "Frost": P("glass", (t.a + t.b) / 2, 0.62 + (1.35 - 0.62) / 2, 0.036, t.b - t.a, 1.35 - 0.62, 1, "#dfe3e2"); break;
    case "Glass": {
      const cx = (t.a + t.b) / 2, h = t.y1 - t.y0;
      P("display", cx, t.y0 + h / 2, 0.035, t.b - t.a, h, 1, 1, { tex: { atlas: "insides", name: t.tile || biz.inside } });
      if (t.frame) P("detail", cx, t.y0 + h / 2, 0.0, t.b - t.a + 0.14, h + 0.14, 0.04, paint); // behind the glass: a coloured border round it
      if (biz.family === "office") for (let k = 0; k < 4; k++) P("near", cx, t.y1 - 0.12 - k * 0.12, 0.05, t.b - t.a - 0.04, 0.03, 0.01, "#e8e6df"); // blinds
      break;
    }
    case "Mullion": P("near", t.x, (t.y0 + t.y1) / 2, 0.06, biz.family === "studio" ? 0.05 : 0.07, t.y1 - t.y0, 0.08, biz.family === "studio" ? "#1d1f22" : paint); break;
    case "Awning": {
      const sw = t.b - t.a + 0.15, cx = (t.a + t.b) / 2, d = 1.25;
      P("awnings", cx, ST - 0.02 - Math.sin(awnTilt) * d / 2, 0.14 + Math.cos(awnTilt) * d / 2, sw, d, 1, 1, { tex: { atlas: "stripes" }, rx: awnTilt - Math.PI / 2 });
      P("awnings", cx, ST - 0.02 - Math.sin(awnTilt) * d - 0.12, 0.14 + Math.cos(awnTilt) * d, sw, 0.24, 1, 0.92, { tex: { atlas: "stripes" } });
      break;
    }
    case "Door": P("mid", t.x, 1.12, 0.03, doorLeaf, 2.24, 0.06, biz.family === "school" ? pick(rng, ["#d9772f", "#2f7a8a", "#c23f2a"]) : paint); break;
    case "DoorLight": P("glass", t.x, 1.45, 0.065, doorLeaf * 0.66, 1.2, 1, biz.family === "office" ? "#dfe3e2" : "#d8b77a"); break;
    case "Brass": P("near", t.x + 0.36 * flip, 1.05, 0.08, 0.06, 0.06, 0.08, "#b8913f"); break;
    case "EntryHead": P("detail", t.x, 2.3, 0.06, entryW, 0.1, 0.12, trim); break;
    case "EntryTransom": P("glass", t.x, (2.36 + ST) / 2, 0.035, entryW, ST - 2.36, 1, "#4a3826"); break;
    case "SideLights": {
      const side = (entryW - doorLeaf) / 2 - 0.08;
      if (side > 0.2) for (const sd of [-1, 1]) P("display", t.x + sd * (doorLeaf / 2 + 0.04 + side / 2), 1.2, 0.035, side, 2.0, 1, 1, { tex: { atlas: "insides", name: biz.inside } });
      break;
    }
    case "Plaque": P("near", t.x + (doorLeaf / 2 + 0.32) * flip, 1.55, 0.04, 0.34, 0.24, 0.02, "#b8913f"); break;
    case "Blade":
      P("near", t.x, 2.4, 0.72, 0.05, 0.05, 1.05, "#1d1b19");
      P("near", t.x, 2.22, 0.25, 0.04, 0.4, 0.04, "#1d1b19");
      P("detail", t.x, 2.08, 0.82, 0.03, 0.46, 0.62, paint);
      for (const sd of [-1, 1]) P("icons", t.x + sd * 0.018 * flip, 2.08, 0.82, 0.5, 0.36, 1, "#ffffff", { tex: { atlas: "icons", name: biz.icon }, ry: sd * Math.PI / 2 });
      break;
    case "SignBand": P("mid", (t.a + t.b) / 2, ST + 0.28, 0.16, t.b - t.a - 0.1, 0.56, 0.12, biz.band || paint); break;
    case "Letters": P("letters", (t.a + t.b) / 2, ST + 0.28, 0.225, t.b - t.a, t.small ? 0.26 : 0.36, 1, "#ffffff", { tex: { atlas: "letters" } }); break;
    case "Cornice": P("mid", (t.a + t.b) / 2, G - 0.04, 0.2, t.b - t.a + 0.3, 0.16, 0.42, trim); break;
    // The pavement.
    case "Crate":
      P("near", t.x, 0.25, 0.7, 0.6, 0.5, 0.45, "#7a5a3a");
      P("near", t.x, 0.55, 0.7, 0.52, 0.12, 0.38, t.bottles ? "#3a1a1e" : fill("crate"));
      break;
    case "Bucket":
      P("near", t.x, 0.25, 0.7, 0.6, 0.5, 0.45, "#5b6670");
      P("near", t.x, 0.6, 0.7, 0.56, 0.22, 0.42, fill("bucket"));
      break;
    case "Barrel": P("near", t.x, 0.45, 0.8, 0.62, 0.9, 0.62, "#6b4526"); P("near", t.x, 0.92, 0.8, 0.66, 0.04, 0.66, "#2a2622"); break;
    case "Rack": P("near", t.x, 0.42, 1.1, 1.9, 0.06, 0.06, "#5b6670"); break;
    // Parked along the wall, side on to the street (a blade sign's icons turn; these don't).
    case "Bike": P("icons", t.x, 0.5, 1.1, 1.0, 0.62, 1, "#1d1b19", { tex: { atlas: "icons", name: "bike" } }); break;
    case "Scooter": P("icons", t.x, 0.35, 0.9, 0.6, 0.6, 1, "#c23f2a", { tex: { atlas: "icons", name: "scooter" } }); break;
    case "BookTable":
      P("near", t.x, 0.55, 1.0, 1.4, 0.7, 0.6, "#5a3a1e");
      for (let k = 0; k < 10; k++) P("near", t.x - (0.6 - k * 0.12) * flip, 1.03, 1.0, 0.09, 0.26, 0.2, pick(rng, ["#7a2430", "#2f4a3a", "#d9b46a", "#3d4f7a", "#a8643a", "#efe6d2"]));
      break;
    case "Table":
      P("near", t.x, 0.74, t.out, 0.72, 0.04, 0.72, biz.spill === "tables3" ? "#f2ece0" : "#e8e0cc");
      P("near", t.x, 0.37, t.out, 0.06, 0.72, 0.06, "#2a2622");
      for (const sd of [-1, 1]) { P("near", t.x + sd * 0.62, 0.45, t.out, 0.4, 0.04, 0.4, "#2a2622"); P("near", t.x + sd * 0.8, 0.68, t.out, 0.04, 0.42, 0.4, "#2a2622"); }
      break;
    case "Bench": P("near", t.x, 0.45, 0.6, 1.5, 0.06, 0.42, "#7a5a3a"); P("near", t.x, 0.22, 0.6, 1.3, 0.44, 0.06, "#2a2622"); break;
    case "AFrame": for (const sd of [-1, 1]) P("near", t.x, 0.42, 1.6 + sd * 0.13, 0.55, 0.85, 0.03, "#2b2b2b", { rx: sd * 0.2 }); break;
    case "Planter": P("near", t.x, 0.25, 0.5, 0.5, 0.5, 0.5, "#6b5a46"); P("near", t.x, 0.72, 0.5, 0.56, 0.48, 0.56, "#55704a"); break;
    default: break; // Blank and anything unknown draws nothing
  }
}

/**
 * A ground floor adapted, not remodelled (Ghent Streets, Jason, 2026-11-27: "keep doors and windows where they
 * are. It's an adaptive-reuse project, we're adapting the existing building, not remodeling it."). The
 * building's own door and windows stay where they are, and frontages.js lights them with what's inside now.
 * What the front gains grows by its own L-system, from one symbol, Keep: a sign, something at the door, a
 * treatment for each window, and the yard. The rules go by family (an office keeps its dignity: shades and
 * gold-leaf lettering; a salon has its pole; a gallery lights its windows), with each business's touches on top.
 */
export const KEEP_RULES = {
  Keep: (sym, c) => [{ s: "Sign" }, { s: "AtDoor" }, ...c.windows.map((w, i) => ({ s: "Window", i })), { s: "Yard" }],
  // A lettered board over the door; then a brass plaque (offices), a shingle hung from a bracket, or a blade.
  Sign: (sym, c) => {
    const quiet = c.biz.family === "office" || c.biz.family === "studio" || c.biz.family === "gallery";
    if (quiet) return [{ s: "Board", small: true }, { s: "Plaque" }];
    return [{ s: "Board" }, { s: c.rng() < 0.4 ? "Shingle" : "Blade" }];
  },
  AtDoor: (sym, c) => {
    if (c.biz.id === "barber") return [{ s: "Pole" }];
    if (c.biz.family === "gallery" || c.biz.family === "fitness") return [{ s: "Lantern" }];
    return null;
  },
  // Each window, by family: a terminal list, rewritten once.
  Window: ({ i }, c) => {
    const out = [];
    const first = i === 0;
    switch (c.biz.family) {
      case "office": out.push({ s: "Shade", i, drop: 0.38 }); if (first) out.push({ s: "Lettering", i }); break;
      case "studio": if (first) out.push({ s: "Lettering", i }); out.push({ s: "Shade", i, drop: 0.18 }); break;
      case "gallery": out.push({ s: "Spot", i }); if (first) out.push({ s: "Lettering", i }); break;
      case "fitness": if (first) out.push({ s: "Lettering", i }); else out.push({ s: "Awning", i }); break;
      case "salon": out.push({ s: c.biz.stripes ? "Awning" : "Shade", i, drop: 0.25 }); if (first) out.push({ s: "Lettering", i }); break;
      default: out.push({ s: c.biz.stripes ? "Awning" : "Shade", i, drop: 0.2 }); break;
    }
    // A window box under some windows of a house: flowers or herbs, never on an office.
    if (c.boxes && c.biz.family !== "office" && c.rng() < 0.55) out.push({ s: "WindowBox", i });
    return out;
  },
  // The yard: the storefront grammar's pavement where the business has one, or the family's own.
  Yard: (sym, c) => {
    const at = c.door.x, f = c.flip;
    if (c.biz.spill === "mats") return [{ s: "Mats", x: at - 1.6 * f }, { s: "Rack", x: at + 2.2 * f }, { s: "Planter", x: at + 0.9 * f }];
    if (c.biz.spill === "easel") return [{ s: "Plinth", x: at - 2.0 * f }, { s: "AFrame", x: at + 1.4 * f }];
    return [{ s: "Spill", a: 0, b: c.L }];
  },
  Spill: RULES.Spill,
};

/** One terminal of a keep front, as parts; anything the storefront grammar knows goes to terminalParts. */
function keepParts(t, v, P) {
  const { biz, rng, flip, paint, trim, door, windows, L } = v;
  const W = t.i != null ? windows[t.i] : null;
  switch (t.s) {
    case "Board": {
      const bw = Math.min(2.8, Math.max(1.5, L * 0.5)), by = door.top + 0.42;
      P("mid", door.x, by, 0.03, bw + 0.12, 0.62, 0.05, trim);
      P("mid", door.x, by, 0.06, bw, 0.5, 0.08, biz.band || paint);
      P("letters", door.x, by, 0.105, bw - 0.2, t.small ? 0.24 : 0.3, 1, "#ffffff", { tex: { atlas: "letters" }, cap: bw - 0.2 });
      break;
    }
    case "Plaque": P("near", door.x + 0.78 * flip, door.top - 0.7, 0.04, 0.34, 0.24, 0.02, "#b8913f"); break;
    case "Blade": {
      const bx = Math.max(0.3, Math.min(L - 0.3, door.x + 1.0 * flip)), y = door.top + 0.05;
      P("near", bx, y + 0.32, 0.55, 0.05, 0.05, 0.9, "#1d1b19");
      P("near", bx, y + 0.14, 0.18, 0.04, 0.4, 0.04, "#1d1b19");
      P("detail", bx, y, 0.66, 0.03, 0.46, 0.62, paint);
      for (const sd of [-1, 1]) P("icons", bx + sd * 0.018 * flip, y, 0.66, 0.5, 0.36, 1, "#ffffff", { tex: { atlas: "icons", name: biz.icon }, ry: sd * Math.PI / 2 });
      break;
    }
    case "Shingle": {
      // A painted board hung square to the wall from an iron arm, swinging just above head height.
      const bx = Math.max(0.4, Math.min(L - 0.4, door.x + 1.1 * flip)), y = door.top - 0.05;
      P("near", bx, y + 0.45, 0.5, 0.04, 0.04, 0.95, "#1d1b19");
      for (const dz of [0.25, 0.75]) P("near", bx, y + 0.3, dz, 0.015, 0.3, 0.015, "#1d1b19");
      P("detail", bx, y, 0.5, 0.04, 0.42, 0.7, paint);
      for (const sd of [-1, 1]) P("icons", bx + sd * 0.024 * flip, y, 0.5, 0.56, 0.34, 1, "#ffffff", { tex: { atlas: "icons", name: biz.icon }, ry: sd * Math.PI / 2 });
      break;
    }
    case "Pole": {
      // A barber's pole beside the door: red, white and blue bands, a brass cap and foot.
      const x = Math.max(0.2, Math.min(L - 0.2, door.x - 0.85 * flip)), y0 = door.top - 1.3;
      P("near", x, y0 + 0.5, 0.14, 0.15, 1.0, 0.15, "#f4efe4");
      for (let k = 0; k < 5; k++) P("near", x, y0 + 0.12 + k * 0.19, 0.145, 0.16, 0.06, 0.16, k % 2 ? "#2b4a7a" : "#b23a2e");
      for (const y of [y0 - 0.03, y0 + 1.03]) P("near", x, y, 0.14, 0.2, 0.07, 0.2, "#b8913f");
      break;
    }
    case "Lantern": for (const sd of [-1, 1]) P("near", door.x + sd * 0.75, door.top - 0.35, 0.12, 0.14, 0.24, 0.14, "#f2c27a"); break;
    case "Shade": {
      // A roller blind drawn partway down inside the glass.
      const h = (W.y1 - W.y0) * t.drop;
      P("glass", W.x, W.y1 - h / 2, 0.042, W.w - 0.04, h, 1, biz.family === "office" ? "#ece6d6" : "#d9cbb0");
      break;
    }
    case "Lettering": {
      // The name in gold leaf (or the business's own ink) low on the glass.
      P("letters", W.x, W.y0 + (W.y1 - W.y0) * 0.3, 0.046, W.w - 0.1, 0.17, 1, "#ffffff", { tex: { atlas: "letters" }, cap: W.w - 0.1 });
      break;
    }
    case "Spot": {
      // A gallery's picture light over the window, and its glow.
      P("near", W.x, W.y1 + 0.18, 0.18, W.w * 0.6, 0.05, 0.3, "#1d1b19");
      P("glass", W.x, W.y1 - 0.12, 0.044, W.w * 0.7, 0.16, 1, "#fff3cf");
      break;
    }
    case "Awning": {
      if (!biz.stripes) break;
      const d = 0.72, tilt = 0.55, sw = W.w + 0.24;
      P("awnings", W.x, W.y1 + 0.16 - Math.sin(tilt) * d / 2, 0.06 + Math.cos(tilt) * d / 2, sw, d, 1, 1, { tex: { atlas: "stripes" }, rx: tilt - Math.PI / 2 });
      P("awnings", W.x, W.y1 + 0.16 - Math.sin(tilt) * d - 0.07, 0.06 + Math.cos(tilt) * d, sw, 0.14, 1, 0.92, { tex: { atlas: "stripes" } });
      break;
    }
    case "WindowBox": {
      const herbs = rng() < 0.35;
      P("near", W.x, W.y0 - 0.12, 0.16, W.w + 0.1, 0.2, 0.22, trim);
      P("near", W.x, W.y0 + 0.02, 0.16, W.w, 0.14, 0.18, herbs ? "#4f6b3e" : "#5d7a45");
      if (!herbs) for (let k = 0; k < 4; k++) P("near", W.x - W.w / 2 + (k + 0.5) * W.w / 4, W.y0 + 0.1, 0.18, 0.12, 0.08, 0.1, pick(rng, ["#c96a8a", "#f2c94a", "#e8e0f0", "#d9772f"]));
      break;
    }
    case "Mats": for (let k = 0; k < 3; k++) P("near", t.x + k * 0.22 * flip, 0.32, 1.6, 0.14, 0.64, 0.14, pick(rng, ["#6b8a5a", "#c96a5a", "#4a6b7a", "#d9b46a"])); break;
    case "Plinth":
      P("near", t.x, 0.4, 1.4, 0.55, 0.8, 0.55, "#e9e4d8");
      P("near", t.x, 1.05, 1.4, 0.3, 0.5, 0.18, pick(rng, ["#8a3a2a", "#2b4a7a", "#b8913f"]));
      break;
    default: terminalParts(t, v, P);
  }
}

export function growKeep({ biz, L, door, windows, rng, flip = 1, yard = 1.4, boxes = true }) {
  const paint = pick(rng, biz.paints);
  const trim = pick(rng, GHENT_TRIMS);
  const parts = [];
  const P = (k, x, y, out, sx, sy, sz, color, extra = {}) => parts.push({ k, x, y, out, sx, sy, sz, color, ...extra });
  const wins = windows.map((w) => ({ y0: w.y0 != null ? w.y0 : w.y1 - 1.5, ...w }));
  const ctx = { biz, L, door, windows: wins, rng, flip, boxes, at: door.x, entryW: 1.6, planters: rng() < 0.7 };
  const word = derive([{ s: "Keep" }], KEEP_RULES, ctx);
  const v = { biz, rng, flip, paint, trim, door, windows: wins, L, brick: pick(rng, GHENT_BRICKS), pier: paint, awning: false, awnTilt: 0, entryW: 1.6, doorLeaf: 1 };
  // What stands in the yard stands a yard's depth out: everything at the wall's foot or on the pavement.
  const yardP = (k, x, y, out, ...rest) => P(k, x, y, out + yard, ...rest);
  const YARD = new Set(["Crate", "Bucket", "Barrel", "Rack", "Bike", "Scooter", "BookTable", "Table", "Bench", "AFrame", "Planter", "Mats", "Plinth"]);
  for (const t of word) keepParts(t, v, YARD.has(t.s) ? yardP : P);
  return { parts, paint, trim, word };
}
