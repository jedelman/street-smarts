// Ghent's front yards and porches: an L-system grown for each house front (the model's yard rows, from
// tools/ghent/build.py). Its rules and their weights come from reading the City assessor's own street photos of the
// neighborhood: corner lots, the tour's stops, and a random sample of mid-block houses (YARD_FREQ, with how many
// photos each figure rests on). What any one house gets is ours: a seeded draw from those frequencies, never a
// claim about that house.
//
// The City's building footprints are traced from above, so a porch's roof is already inside them: the front wall
// the model draws is the porch's front edge. So a porch here is cut into the ground floor (a shadowed recess, with
// its columns and railing standing at the wall), and only the steps, the walk and the yard come out in front.
//
//   Front   → Porch | Stoop   Steps  Walk  Beds  Edge  Tree  Things
//   Porch   → Recess  Skirt  Columns  Railing  Roof  [Upper]  Lantern  Seats
//   Stoop   → Landing  Hood?  Lantern
//   Beds    → Mulch  Shrub*  Cone?        (continuous, sparse or none)
//   Edge    → Hedge | Fence(iron) | Wall(brick) | Chain | ∅
//   Tree    → Magnolia | Crape | Maple | Conifer | Small | ∅
//
// Pure: tools/sparrow/yards.test.mjs tests it in Node. Parts use the shop fronts' frame: x along the wall (absolute),
// y up, out from the wall; kinds: "yard" (box), "shrub" (rounded), "cone", "post" (cylinder), "rail" (a patterned
// quad: tex = [pattern, bar spacing m, bar share, 0]), "glass" (lit).

// How the photos read: tools/ghent/yard-photos.md has the tallies (121 usable photos: 73 houses, 9 rowhouses, 32
// apartment buildings). Rowhouses are few, so their weights lean on the houses'. The skirt, handrails and cones aren't
// in the photos' schema: those weights are ours.
export const YARD_FREQ = {
  house: {
    porch: { none: 0.19, partial: 0.32, full: 0.49 }, // "none" is a stoop; wraparounds (7%) drawn full-width
    columns: { round: 0.7, square: 0.13, piers: 0.13, tapered: 0.04 },
    railing: { balusters: 0.56, iron: 0.25, none: 0.13, knee: 0.06 },
    roof: { hip: 0.53, flat: 0.16, gable: 0.14, shed: 0.1, balustrade: 0.07 },
    upper: 0.03,
    skirt: { lattice: 0.5, brick: 0.5 },
    handrails: 0.35,
    planting: { continuous: 0.64, sparse: 0.36, none: 0 },
    cones: 0.35,
    edge: { none: 0.72, hedge: 0.11, iron: 0.07, wall: 0.07, picket: 0.03 },
    walk: { concrete: 0.5, brick: 0.32, none: 0.18 },
    steps: { brick: 0.5, concrete: 0.39, stone: 0.07, wood: 0.04 },
    tree: 0.72,
    shutters: 0.27,
    lantern: 0.48,
    transom: 0.38,
    sidelights: 0.18,
    pots: 0.32,
    urns: 0.11,
    ferns: 0.1,
    planters: 0.21,
    seats: 0.3,
    wreath: 0.07,
  },
  row: {
    porch: { none: 0.67, partial: 0.11, full: 0.22 },
    columns: { round: 0.4, square: 0.3, piers: 0.3, tapered: 0 },
    railing: { balusters: 0.25, iron: 0.62, none: 0.13, knee: 0 },
    roof: { hip: 0.3, flat: 0.35, gable: 0.15, shed: 0.1, balustrade: 0.1 },
    upper: 0.1,
    skirt: { lattice: 0.2, brick: 0.8 },
    handrails: 0.7,
    planting: { continuous: 0.33, sparse: 0.67, none: 0 },
    cones: 0.2,
    edge: { none: 0.57, hedge: 0.14, iron: 0.15, wall: 0.14, picket: 0 },
    walk: { concrete: 0.7, brick: 0.3, none: 0 },
    steps: { brick: 0.5, concrete: 0.5, stone: 0, wood: 0 },
    tree: 0.45,
    shutters: 0.11,
    lantern: 0.56,
    transom: 0.56,
    sidelights: 0.11,
    pots: 0.33,
    urns: 0.05,
    ferns: 0.05,
    planters: 0.22,
    seats: 0.1,
    wreath: 0.11,
  },
  apartment: {
    porch: { none: 0.4, partial: 0.47, full: 0.13 },
    columns: { round: 0.36, square: 0.29, piers: 0.35, tapered: 0 },
    railing: { balusters: 0.39, iron: 0.56, none: 0, knee: 0.05 },
    roof: { hip: 0.05, flat: 0.65, gable: 0.18, shed: 0.06, balustrade: 0.06 },
    upper: 0.5,
    skirt: { lattice: 0.1, brick: 0.9 },
    handrails: 0.6,
    planting: { continuous: 0.38, sparse: 0.5, none: 0.12 },
    cones: 0.2,
    edge: { none: 0.83, hedge: 0.1, iron: 0.03, wall: 0.04, picket: 0 },
    walk: { concrete: 0.88, brick: 0.04, none: 0.08 },
    steps: { brick: 0.25, concrete: 0.75, stone: 0, wood: 0 },
    tree: 0.47,
    shutters: 0.09,
    lantern: 0.34,
    transom: 0.12,
    sidelights: 0.05,
    pots: 0.09,
    urns: 0.06,
    ferns: 0.09,
    planters: 0.25,
    seats: 0.12,
    wreath: 0.02,
  },
};

const COLORS = {
  trim: [["#f1ece0", 0.74], ["#4a3a2e", 0.07], ["#e6dcc2", 0.08], ["#2f4a35", 0.04], ["#6f7458", 0.03], ["#8a917a", 0.02], ["#2a2a2a", 0.02]],
  door: [["#efeae0", 0.24], ["#1f1f1f", 0.2], ["#6b4a30", 0.16], ["#4a3022", 0.12], ["#7a2620", 0.08], ["#27407a", 0.05],
    ["#2f4a35", 0.05], ["#2f6b6b", 0.03], ["#5b5e60", 0.07]],
  shutter: [["#1f2421", 0.45], ["#2e4434", 0.15], ["#3d6466", 0.1], ["#6b2c24", 0.1], ["#7d756a", 0.1], ["#e6dcc2", 0.1]],
  steps: { brick: "#9a5444", concrete: "#b9b3a8", stone: "#a39d90", wood: "#8a7a64" },
  walk: { brick: "#8e4a3a", concrete: "#c3bdb2" },
  tree: { magnolia: "#2f4b2b", crape: "#9b5a3c", maple: "#a83d24", conifer: "#34503a", small: "#8a5a34" },
};
const BRICK = "#8f4c37", MULCH = "#4a3426", BOX_GREEN = ["#3c5a2e", "#36522b", "#41612f", "#2f4a2a"], IRON = "#1c1c1c", CHAIN = "#2a2a2a";
const RECESS = "#2b2620", CEILING = "#c9c2b1", HEDGE = "#33502b", ROOF = "#4b4f53";
// The rail patterns the shader draws (frontages.js railMaterial): [pattern, bar spacing, bar share].
export const RAIL = { balusters: [0, 0.14, 0.35, 0], iron: [1, 0.12, 0.18, 0], lattice: [2, 0.22, 0.12, 0], picket: [3, 0.15, 0.5, 0],
  chippendale: [4, 0.9, 0, 0] };

const pick = (rng, weights) => {
  const entries = Array.isArray(weights) ? weights : Object.entries(weights);
  const total = entries.reduce((t, [, w]) => t + w, 0);
  let r = rng() * total;
  for (const [k, w] of entries) if ((r -= w) <= 0) return k;
  return entries[entries.length - 1][0];
};
const chance = (rng, p) => rng() < p;

/**
 * One house front's yard and porch. `row` is the model's yard row as an object ({ door, depth, lotLo, lotHi, form,
 * first, floors, converted }); `L` the front wall's length; `windows` its ground- and upper-floor windows ({ x, y0,
 * y1, w, floor }). Returns { parts, word, porch } (porch: { lo, hi, top } along the wall, for awnings to keep clear of).
 */
export function growYard({ row, L, windows = [], rng }) {
  const F = YARD_FREQ[row.form] || YARD_FREQ.house;
  const parts = [], word = [];
  const put = (s, p = {}) => word.push({ s, ...p });
  const part = (k, x, y, out, sx, sy, sz, color, extra = {}) => parts.push({ k, x, y, out, sx, sy, sz, color, ...extra });
  const trim = pick(rng, COLORS.trim), door = row.door, depth = Math.max(0, row.depth);
  const deck = Math.max(0.15, row.first - 0.15); // the porch floor: the door's sill
  const ceiling = deck + 2.75;

  // ---- Porch or stoop
  let kind = pick(rng, F.porch);
  if (L < 4.5 && kind === "full") kind = "partial";
  let porch = null;
  if (kind !== "none") {
    const span = kind === "full" ? [0.25, L - 0.25] : (() => {
      const w = Math.min(L - 0.5, Math.max(2.8, (row.form === "apartment" ? 0.35 : 0.5) * L));
      const lo = Math.max(0.25, Math.min(door - w / 2, L - 0.25 - w));
      return [lo, lo + w];
    })();
    const [lo, hi] = span, w = hi - lo, mid = (lo + hi) / 2;
    porch = { lo, hi, top: ceiling + 0.4 };
    put("Porch", { kind, lo, hi });
    // The recess: the porch's shade, behind its columns (the door and windows stay drawn on it).
    part("yard", mid, (deck + ceiling) / 2, 0.012, w, ceiling - deck, 0.02, RECESS);
    part("yard", mid, ceiling - 0.02, 0.35, w, 0.04, 0.7, CEILING);
    // The skirt under the deck, and the deck's edge.
    const skirt = pick(rng, F.skirt);
    put("Skirt", { skirt });
    if (skirt === "lattice") part("rail", mid, deck / 2, 0.05, w, deck, 1, "#e9e4d8", { tex: RAIL.lattice });
    else part("yard", mid, deck / 2, 0.04, w, deck, 0.08, "#8f4c37");
    part("yard", mid, deck - 0.04, 0.09, w + 0.1, 0.08, 0.18, trim);
    // Columns: at each end and about every 2.6 m.
    const col = pick(rng, F.columns), n = Math.max(2, Math.round(w / 2.6) + 1);
    put("Columns", { col, n });
    for (let i = 0; i < n; i++) {
      const x = lo + 0.12 + (i * (w - 0.24)) / (n - 1);
      if (col === "round") part("post", x, (deck + ceiling) / 2, 0.16, 0.24, ceiling - deck, 0.24, trim, { role: "column" });
      else if (col === "square") part("yard", x, (deck + ceiling) / 2, 0.16, 0.26, ceiling - deck, 0.26, trim, { role: "column" });
      else if (col === "piers") part("yard", x, (deck + ceiling) / 2, 0.18, 0.42, ceiling - deck, 0.42, BRICK, { role: "column" });
      else { // a short tapered column on a brick pier (the bungalows')
        part("yard", x, deck + 0.55, 0.18, 0.46, 1.1, 0.46, BRICK, { role: "column" });
        part("yard", x, (deck + 1.1 + ceiling) / 2, 0.16, 0.26, ceiling - deck - 1.1, 0.26, trim);
      }
    }
    // The railing, open at the steps.
    const rail = pick(rng, F.railing);
    put("Railing", { rail });
    if (rail !== "none") {
      const gapLo = door - 0.75, gapHi = door + 0.75;
      for (const [a, b] of [[lo + 0.24, gapLo], [gapHi, hi - 0.24]]) {
        if (b - a <= 0.4) continue;
        if (rail === "knee") part("yard", (a + b) / 2, deck + 0.45, 0.17, b - a, 0.9, 0.3, BRICK);
        else part("rail", (a + b) / 2, deck + 0.46, 0.17, b - a, 0.9, 1, rail === "iron" ? IRON : trim, { tex: RAIL[rail] });
      }
    }
    // The roof's edge: an entablature, the roof sloping back to the wall, and a pediment over a small porch.
    const roof = pick(rng, F.roof);
    put("Roof", { roof });
    part("yard", mid, ceiling + 0.17, 0.36, w + 0.3, 0.34, 0.12, trim);
    if (roof === "hip" || roof === "shed") {
      const slope = roof === "hip" ? 0.35 : 0.6;
      part("yard", mid, ceiling + 0.4 + 0.3 * slope, 0.24, w + 0.4, 0.07, 0.62, ROOF, { rx: slope }); // sloping down to the eave
    } else {
      part("yard", mid, ceiling + 0.37, 0.25, w + 0.4, 0.07, 0.5, ROOF);
      if (roof === "balustrade") part("rail", mid, ceiling + 0.8, 0.36, w + 0.2, 0.8, 1, trim, { tex: RAIL.balusters });
      if (roof === "gable") { put("Pediment"); part("gable", mid, ceiling + 0.34 + Math.min(0.55, w * 0.12), 0.32, w + 0.3, Math.min(1.1, w * 0.24), 0.16, trim); }
    }
    // Upper porch, stacked (apartments mostly): its own railing and shade, on the floor above.
    if (row.floors >= 2 && chance(rng, F.upper)) {
      const up = deck + 3.1;
      put("Upper");
      part("yard", mid, up + 1.35, 0.012, w, 2.6, 0.02, RECESS);
      part("rail", mid, up + 0.46, 0.17, w - 0.48, 0.9, 1, rail === "iron" || rail === "none" ? IRON : trim, { tex: RAIL[rail === "none" ? "iron" : rail] });
      for (let i = 0; i < n; i++) {
        const x = lo + 0.12 + (i * (w - 0.24)) / (n - 1);
        part(col === "round" ? "post" : "yard", x, up + 1.3, 0.16, 0.22, 2.6, 0.22, trim);
      }
      part("yard", mid, up + 2.75, 0.36, w + 0.3, 0.3, 0.12, trim);
    }
    if (chance(rng, F.seats)) {
      put("Seats");
      for (const dx of [-1, 1]) {
        const x = door + dx * Math.min(1.8, w / 3);
        if (x > lo + 0.4 && x < hi - 0.4) {
          part("yard", x, deck + 0.25, 0.75, 0.55, 0.5, 0.55, pick(rng, [["#f1ece0", 0.5], ["#2f4a35", 0.3], ["#5a3a26", 0.2]]));
          part("yard", x, deck + 0.7, 0.95, 0.55, 0.5, 0.06, "#e6e0d2");
        }
      }
    }
  } else {
    put("Stoop");
    const lw = Math.min(1.8, L - 0.4);
    part("yard", door, deck / 2, 0.55, lw, deck, 1.1, COLORS.steps[pick(rng, F.steps)]);
    if (chance(rng, 0.35)) part("yard", door, row.first + 2.45, 0.35, 1.5, 0.12, 0.7, trim); // a hood over the door
  }

  // ---- Steps, from the porch or stoop down into the yard (as many as the yard has room for).
  const stepsCol = COLORS.steps[pick(rng, F.steps)];
  const startOut = porch ? 0.12 : 1.1;
  const nSteps = Math.max(0, Math.min(Math.round(deck / 0.18), Math.floor((depth - startOut - 0.3) / 0.28)));
  const stepW = porch ? 1.6 : 1.4;
  put("Steps", { n: nSteps });
  for (let i = 0; i < nSteps; i++) {
    const h = deck - (i + 1) * (deck / (nSteps + 1));
    part("yard", door, h / 2, startOut + 0.14 + i * 0.28, stepW, h, 0.3, stepsCol);
  }
  const stepsEnd = startOut + nSteps * 0.28;
  if (nSteps >= 2 && chance(rng, F.handrails)) {
    put("Handrails");
    const run = nSteps * 0.28, tilt = Math.atan2(deck, run);
    for (const dx of [-1, 1]) {
      part("yard", door + dx * (stepW / 2 + 0.03), deck / 2 + 0.85, startOut + run / 2, 0.04, 0.04, Math.hypot(run, deck), IRON, { rx: tilt });
      part("yard", door + dx * (stepW / 2 + 0.03), 0.45, stepsEnd, 0.04, 0.9, 0.04, IRON);
    }
  }

  // ---- The walk, from the steps to the sidewalk.
  const walk = depth - stepsEnd > 0.3 ? pick(rng, F.walk) : "none";
  if (walk !== "none") {
    put("Walk", { walk });
    part("yard", door, 0.025, (stepsEnd + depth) / 2, 1.1, 0.05, depth - stepsEnd, COLORS.walk[walk]);
  }

  // ---- Beds along the foundation, either side of the steps; cones flanking them.
  const planting = depth < 0.8 ? "none" : pick(rng, F.planting);
  put("Beds", { planting });
  if (planting !== "none") {
    const bedOut = Math.min(1.1, depth * 0.45);
    const lo = Math.max(0.2, row.lotLo + 0.3), hi = Math.min(L - 0.2, row.lotHi - 0.3);
    for (const [a, b] of [[lo, door - stepW / 2 - 0.25], [door + stepW / 2 + 0.25, hi]]) {
      if (b - a < 0.5) continue;
      part("yard", (a + b) / 2, 0.02, bedOut / 2, b - a, 0.04, bedOut, MULCH);
      const gap = planting === "continuous" ? 0.75 : 2.2;
      for (let x = a + 0.4; x < b - 0.3; x += gap * (0.85 + 0.3 * rng())) {
        const s = 0.55 + 0.45 * rng();
        part("shrub", x, s * 0.42, bedOut * 0.5, s * 1.1, s * 0.85, Math.min(bedOut, s), BOX_GREEN[Math.floor(rng() * BOX_GREEN.length)]);
      }
    }
    if (chance(rng, F.cones) && depth > stepsEnd + 0.8) {
      put("Cones");
      for (const dx of [-1, 1]) part("cone", door + dx * (stepW / 2 + 0.5), 0.75, stepsEnd + 0.3, 0.55, 1.5, 0.55, "#2f4a2a");
    }
  }

  // ---- The edge along the sidewalk.
  const edge = depth < 1.5 ? "none" : pick(rng, F.edge);
  put("Edge", { edge });
  if (edge !== "none") {
    const lo = row.lotLo + 0.1, hi = row.lotHi - 0.1, out = depth - 0.2;
    const wallCol = pick(rng, [["#8f4c37", 0.6], ["#a39d90", 0.4]]); // one wall, one material
    for (const [a, b] of [[lo, door - 0.65], [door + 0.65, hi]]) {
      if (b - a < 0.4) continue;
      const m = (a + b) / 2, w = b - a;
      if (edge === "hedge") part("yard", m, 0.5, out - 0.2, w, 1.0, 0.7, HEDGE);
      else if (edge === "iron") part("rail", m, 0.55, out, w, 1.1, 1, IRON, { tex: RAIL.iron });
      else if (edge === "picket") part("rail", m, 0.5, out, w, 1.0, 1, "#f1ece0", { tex: RAIL.picket });
      else if (edge === "wall") part("yard", m, 0.28, out, w, 0.55, 0.3, wallCol);
      else if (edge === "chain") {
        for (let x = a; x <= b + 0.01; x += Math.max(1.2, w / Math.max(1, Math.round(w / 2)))) part("yard", x, 0.3, out, 0.08, 0.6, 0.08, CHAIN);
        part("yard", m, 0.45, out, w, 0.02, 0.02, CHAIN);
      }
    }
  }

  // ---- A tree in the yard, off the walk, where there's room (not where a café's tables go).
  if (!row.converted && depth >= 3 && chance(rng, F.tree)) {
    const t = pick(rng, [["maple", 0.24], ["crape", 0.24], ["small", 0.18], ["magnolia", 0.17], ["conifer", 0.17]]);
    const side = rng() < 0.5 ? -1 : 1;
    const x = Math.max(row.lotLo + 1, Math.min(row.lotHi - 1, door + side * (1.6 + 1.6 * rng())));
    const out = Math.max(stepsEnd + 0.8, depth * (0.5 + 0.2 * rng()));
    put("Tree", { t });
    const H = { magnolia: 6.5, crape: 4.2, maple: 3.2, conifer: 5.5, small: 4 }[t] * (0.8 + 0.4 * rng());
    part("post", x, H * 0.22, out, 0.18, H * 0.44, 0.18, "#4a3b2e");
    if (t === "conifer") part("cone", x, H * 0.55, out, 1.6, H * 0.9, 1.6, COLORS.tree.conifer);
    else if (t === "magnolia") part("cone", x, H * 0.6, out, 3.0, H * 0.85, 3.0, COLORS.tree.magnolia);
    else part("shrub", x, H * 0.7, out, H * 0.55, H * 0.45, H * 0.55, COLORS.tree[t]);
  }

  // ---- Things: a lantern, a wreath, pots on the steps; shutters on the front windows.
  if (chance(rng, F.lantern)) { put("Lantern"); part("glass", door + 0.75, row.first + 1.95, 0.08, 0.16, 0.26, 0.16, "#f6cf86"); }
  if (chance(rng, F.transom)) { put("Transom"); part("glass", door, row.first + 2.35, 0.035, 0.95, 0.3, 0.02, "#e9c98a"); }
  if (chance(rng, F.sidelights)) {
    put("Sidelights");
    for (const dx of [-1, 1]) part("glass", door + dx * 0.72, row.first + 1.05, 0.035, 0.28, 1.7, 0.02, "#e9c98a");
  }
  if (porch && chance(rng, F.ferns)) {
    put("Ferns");
    for (let x = porch.lo + 1.2; x < porch.hi - 0.8; x += 2.6) if (Math.abs(x - door) > 0.9) part("shrub", x, ceiling - 0.55, 0.5, 0.6, 0.5, 0.6, "#4f6e34");
  }
  if (porch && chance(rng, F.planters)) {
    put("Planters");
    for (const dx of [-1, 1]) part("post", door + dx * 1.15, deck + 0.3, 0.45, 0.42, 0.6, 0.42, "#3b3b3b");
  }
  if (depth > stepsEnd + 0.8 && chance(rng, F.urns)) {
    put("Urns");
    for (const dx of [-1, 1]) part("cone", door + dx * (stepW / 2 + 0.35), 0.35, stepsEnd + 0.25, 0.5, 0.7, 0.5, "#9a958a");
  }
  if (chance(rng, F.wreath)) { put("Wreath"); part("shrub", door, row.first + 1.65, 0.07, 0.42, 0.42, 0.08, "#2e4a2a"); }
  if (nSteps >= 1 && chance(rng, F.pots)) {
    put("Pots");
    for (const dx of [-1, 1]) part("post", door + dx * (stepW / 2 - 0.15), deck * 0.5 + 0.22, startOut + 0.15, 0.32, 0.36, 0.32, "#a65a3a");
  }
  const shutters = !row.converted && chance(rng, F.shutters);
  if (shutters) {
    put("Shutters");
    const sc = pick(rng, COLORS.shutter);
    for (const wdw of windows) {
      const reach = wdw.w / 2 + 0.41; // a shutter's outer edge
      if (porch && wdw.floor === 0 && wdw.x + reach > porch.lo && wdw.x - reach < porch.hi) continue; // under a porch, no shutters
      for (const dx of [-1, 1]) part("yard", wdw.x + dx * (wdw.w / 2 + 0.22), (wdw.y0 + wdw.y1) / 2, 0.05, 0.38, wdw.y1 - wdw.y0, 0.04, sc, { role: "shutter" });
    }
  }
  return { parts, word, porch, doorColor: pick(rng, COLORS.door), trim };
}
