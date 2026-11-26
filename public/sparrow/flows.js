// The neighbourhood's money, as a gravity model sees it: who spends at the
// plan's shop fronts, and which of them. Pure (no three.js), so it's tested
// on its own (tools/sparrow/flows.test.mjs); flowlines.js draws it.
//
// Two kinds of spender, each sized from P9's floor uses and the building's
// footprint (the element table's `area`):
//   homes: the floors given to living x 85% usable / 900 sq ft a home, each
//          spending what the landing page's households spend;
//   desks: the floors given to work x 85% usable / 12 m² a desk, each
//          spending what office workers spend near the office.
// The tram's riders are drawn too, but not sized: there's no ridership
// estimate for a station that doesn't exist yet.
//
// Where the money goes is the assumption, and it's ours: each spender splits
// its spending among the shop fronts within a five-minute walk, favouring
// nearer ones (a shop 200 m farther away pulls e^-1 as much), after Huff's
// gravity model of retail trade. Every shop front pulls the same, whatever
// its business; distance is straight-line, not along the streets.

export const SF = 10.7639; // square feet in a square metre

export const FLOW = {
  usable: 0.85,             // ours, the landing page's (the Employment Density Guide puts office gross at 15-20% over net)
  homeSf: 900,              // ours, the landing page's
  spendPerHome: 15662,      // BLS Consumer Expenditure Survey 2023, the landing page's basket, a year
  m2PerDesk: 12,            // HCA Employment Density Guide (3rd ed., 2015): professional services offices, m² net per full-time worker
  spendPerWorkweek: 129.18, // ICSC, Office-Worker Retail Spending in a Digital Age (2012; surveyed Oct 2011): near the office, less transport and online
  workweeks: 48,            // ours: a year less four weeks off
  walkM: 400,               // ours: a five-minute walk
  decayM: 200,              // ours: the distance over which a shop's pull falls by e
};

const count = (uses, c) => (uses ? [...uses].filter((u) => u === c).length : 0);

/** A building's spenders from its footprint (m²) and floor uses ("pwwll"). */
export function spendersOf({ area, uses }, F = FLOW) {
  const a = area || 0;
  const homes = count(uses, "l") * a * SF * F.usable / F.homeSf;
  const desks = count(uses, "w") * a * F.usable / F.m2PerDesk;
  return { homes, desks, home: homes * F.spendPerHome, desk: desks * F.spendPerWorkweek * F.workweeks };
}

/** Each shop's share of a spender at (x, z): Huff's probabilities, within the walk. */
export function sharesFrom(x, z, shops, F = FLOW) {
  const w = shops.map((s) => {
    const d = Math.hypot(s.x - x, s.z - z);
    return d <= F.walkM ? Math.exp(-d / F.decayM) : 0;
  });
  let sum = w.reduce((a, b) => a + b, 0);
  if (sum === 0 && shops.length) {
    // Nothing within the walk: it all goes to the nearest.
    let best = 0, bd = Infinity;
    shops.forEach((s, i) => { const d = Math.hypot(s.x - x, s.z - z); if (d < bd) { bd = d; best = i; } });
    w[best] = 1; sum = 1;
  }
  return w.map((v) => (sum ? v / sum : 0));
}

/**
 * The flows. `buildings`: [{ id, area, uses, x, z }]; `shops`: [{ building,
 * x, z }] (a shop front's door); `station`: { x, z } or null. Returns
 * { buildings: Map(id -> { homes, desks, home, desk, shops, inHome, inDesk, inTram }),
 *   flows: [{ from, to, kind: "home"|"desk"|"tram", amount, share }],
 *   totals: { homes, desks, home, desk, shops } }.
 * `amount` is dollars a year (0 for the tram, which isn't sized); `share` is
 * the fraction of that source's spending (or riders) going there.
 */
export function computeFlows({ buildings, shops, station = null }, F = FLOW) {
  const out = new Map();
  for (const b of buildings) out.set(b.id, { ...spendersOf(b, F), shops: 0, inHome: 0, inDesk: 0, inTram: 0 });
  for (const s of shops) if (out.has(s.building)) out.get(s.building).shops++;
  const pairs = new Map();
  const addPair = (from, to, kind, amount, share) => {
    const key = `${from}|${to}|${kind}`;
    const p = pairs.get(key) || { from, to, kind, amount: 0, share: 0 };
    p.amount += amount; p.share += share;
    pairs.set(key, p);
  };
  const spend = (from, x, z, amounts) => {
    const shares = sharesFrom(x, z, shops, F);
    shares.forEach((sh, j) => {
      if (!sh) return;
      const to = shops[j].building, dest = out.get(to);
      for (const [kind, amount] of amounts) {
        addPair(from, to, kind, amount * sh, sh);
        if (dest) dest[kind === "home" ? "inHome" : kind === "desk" ? "inDesk" : "inTram"] += kind === "tram" ? sh : amount * sh;
      }
    });
  };
  for (const b of buildings) {
    const s = out.get(b.id), amounts = [];
    if (s.home > 0) amounts.push(["home", s.home]);
    if (s.desk > 0) amounts.push(["desk", s.desk]);
    if (amounts.length) spend(b.id, b.x, b.z, amounts);
  }
  if (station) spend("station", station.x, station.z, [["tram", 0]]);
  const totals = { homes: 0, desks: 0, home: 0, desk: 0, shops: shops.length };
  for (const s of out.values()) { totals.homes += s.homes; totals.desks += s.desks; totals.home += s.home; totals.desk += s.desk; }
  return { buildings: out, flows: [...pairs.values()], totals };
}

/** The biggest `n` flows of each kind between two different places, for drawing. */
export function topFlows(flows, n) {
  const pick = (kind, by) => flows.filter((f) => f.kind === kind && f.from !== f.to).sort((a, b) => b[by] - a[by]).slice(0, n);
  return [...pick("home", "amount"), ...pick("desk", "amount"), ...pick("tram", "share")];
}

/** Dollars, the way the lines say them: $758k, $1.2M, $58M. */
export const money = (v) => (v >= 1e6 ? `$${(v / 1e6).toFixed(v >= 1e7 ? 0 : 1)}M` : `$${Math.round(v / 1e3)}k`);
const num = (v) => Math.round(v).toLocaleString("en-US");
const round = (v, to) => Math.round(v / to) * to;

/** What one building's shop fronts take in, in a line; null if it has none. */
export function lineFor(model, id) {
  const b = model.buildings.get(id);
  if (!b || !b.shops) return null;
  const all = b.inHome + b.inDesk;
  const fronts = b.shops === 1 ? "Its shop front takes" : `Its ${b.shops} shop fronts take`;
  const tram = b.inTram >= 0.05 ? ` It gets ${Math.round(b.inTram * 100)}% of the tram's riders who shop.` : "";
  return `${fronts} in about ${money(all)} a year from the neighbourhood: ${money(b.inHome)} from homes, ${money(b.inDesk)} from desks.${tram}`;
}

/** The overlay's opening line: the whole neighbourhood. */
export function summaryLine(model) {
  const t = model.totals;
  return `Flows: about ${num(round(t.homes, 100))} homes spend ${money(t.home)} a year here, and ${num(round(t.desks, 100))} desks ${money(t.desk)} (in 2011 dollars, every desk filled five days a week).`;
}
