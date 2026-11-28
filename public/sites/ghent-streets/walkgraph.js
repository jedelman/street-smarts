// The walk network behind "Day to day": how far a neighbour walks, along the streets, to what's there. The data is
// everyday.json (tools/ghent/everyday.py and build.py): the City's street centerlines as a graph, the places open
// today (Overture Maps), the proposal's ground floors at their doors, what has opened since 2019 (the City's new
// business licences, counted at the nearest corner), rent by census tract, and the metro's rates of new places.
//
// Pure: tools/sparrow/everyday.test.mjs tests it in Node. Coordinates are the survey's metres (glTF x, -z).

/** The graph, with each node's neighbours laid out flat for the search. */
export function prepare(d) {
  const n = d.nodes.length, deg = new Uint16Array(n);
  for (const [i, j] of d.edges) { deg[i]++; deg[j]++; }
  const start = new Uint32Array(n + 1);
  for (let i = 0; i < n; i++) start[i + 1] = start[i] + deg[i];
  const nbr = new Uint32Array(start[n]), via = new Uint32Array(start[n]), fill = start.slice(0, n);
  d.edges.forEach(([i, j], e) => {
    nbr[fill[i]] = j; via[fill[i]++] = e;
    nbr[fill[j]] = i; via[fill[j]++] = e;
  });
  return { ...d, deg, start, nbr, via };
}

/** The nearest point on the network to (x, y): [edge, metres along it from its first node, metres off it]. */
export function snap(g, x, y) {
  let best = null;
  for (let e = 0; e < g.edges.length; e++) {
    const [i, j, len] = g.edges[e];
    const [ax, ay] = g.nodes[i], [bx, by] = g.nodes[j];
    const dx = bx - ax, dy = by - ay, L2 = dx * dx + dy * dy || 1;
    const t = Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / L2));
    const off = Math.hypot(x - (ax + t * dx), y - (ay + t * dy));
    if (!best || off < best[2]) best = [e, t * len, off];
  }
  return best;
}

/** Metres from a point on the network to every node, searched out to `max`. */
export function fromPoint(g, origin, max = Infinity) {
  const [e, along, off] = origin;
  const n = g.nodes.length, dist = new Float64Array(n).fill(Infinity);
  const [i, j, len] = g.edges[e];
  const heap = new Heap();
  const seed = (v, d) => { if (d < dist[v]) { dist[v] = d; heap.push(v, d); } };
  seed(i, off + along);
  seed(j, off + (len - along));
  while (heap.size) {
    const [v, d] = heap.pop();
    if (d > dist[v]) continue;
    if (d > max) break;
    for (let k = g.start[v]; k < g.start[v + 1]; k++) {
      const w = g.nbr[k], nd = d + g.edges[g.via[k]][2];
      if (nd < dist[w]) { dist[w] = nd; heap.push(w, nd); }
    }
  }
  return { dist, origin };
}

/** Metres from the search's origin to a point on the network ([edge, along, off]). */
export function metresTo(g, D, [e, along, off]) {
  const [i, j, len] = g.edges[e];
  let m = Math.min(D.dist[i] + along, D.dist[j] + (len - along));
  if (e === D.origin[0]) m = Math.min(m, D.origin[2] + Math.abs(along - D.origin[1]));
  return m + off;
}

/** The edges of an edge's block: the stretch of street between the corners (nodes where streets meet or end). */
export function block(g, e) {
  const edges = new Set([e]), seen = new Set(), stack = [g.edges[e][0], g.edges[e][1]];
  while (stack.length) {
    const v = stack.pop();
    if (seen.has(v) || g.deg[v] !== 2) continue;
    seen.add(v);
    for (let k = g.start[v]; k < g.start[v + 1]; k++) {
      const f = g.via[k];
      if (!edges.has(f)) {
        edges.add(f);
        stack.push(g.nbr[k]);
      }
    }
  }
  return edges;
}

/** Which tract a point is in (by its rings), or -1. */
export function tractAt(g, x, y) {
  return g.tracts.findIndex((t) => t.rings.some((r) => inside(r, x, y)));
}

const SECTORS = ["food", "shop", "service"];

/**
 * Everything the panel says about one spot, for an n-minute walk: per kind, the nearest place and how many are within
 * reach, today and with the proposal; what has opened nearby since 2019; how many of the proposal's ground floors are
 * within reach and on the block; and the rates that turn those into "something new every so often".
 */
export function summary(g, origin, minutes = 10, horizon = 15) {
  const mps = g.speed_mps, reach = minutes * 60 * mps, far = Math.max(horizon, minutes) * 60 * mps;
  const D = fromPoint(g, origin, far);
  const kinds = g.kinds.map((k) => ({ id: k.id, label: k.label, added: 0,
    today: { nearest: Infinity, within: 0 }, proposed: { nearest: Infinity, within: 0 } }));
  const tally = (side, k, m) => {
    const s = kinds[k][side];
    s.nearest = Math.min(s.nearest, m);
    if (m <= reach) s.within++;
  };
  const blk = block(g, origin[0]);
  const onBlock = { today: [], proposed: [] };
  for (const [e, a, o, k] of g.today) {
    const m = metresTo(g, D, [e, a, o]);
    if (m <= far) { tally("today", k, m); tally("proposed", k, m); }
    if (blk.has(e)) onBlock.today.push(k);
  }
  const near = { food: 0, shop: 0, service: 0 };
  for (const [e, a, o, k, gpin, sector] of g.proposed) {
    const m = metresTo(g, D, [e, a, o]);
    if (k >= 0 && m <= far) tally("proposed", k, m);
    if (k >= 0 && m <= reach) kinds[k].added++;
    if (m <= reach) near[sector]++;
    if (blk.has(e)) onBlock.proposed.push({ gpin, k, sector });
  }
  const opened = { food: 0, shop: 0, service: 0 };
  for (const [node, c] of Object.entries(g.opened.by_node)) {
    if (D.dist[+node] <= reach) for (const s of SECTORS) opened[s] += c[s] || 0;
  }
  const years = (Date.parse(g.opened.to) - Date.parse(g.opened.from)) / (365.25 * 864e5);
  const openedPerYear = SECTORS.reduce((t, s) => t + opened[s], 0) / years;
  const newPerYear = SECTORS.reduce((t, s) => t + near[s] * g.bds[s].entry, 0);
  const blockPerYear = onBlock.proposed.reduce((t, p) => t + g.bds[p.sector].entry, 0);
  // Brief 2's pace: the share of the map's ground floors that open at its steady state (lo to hi).
  const share = g.pace ? [g.pace.lo / g.pace.of, g.pace.hi / g.pace.of] : [1, 1];
  return { minutes, mps, kinds, opened, openedPerYear, years, near, newPerYear, onBlock, blockPerYear, share };
}

/** Minutes for a walk of m metres, as words. */
export function minutesOf(m, mps, horizon = 15) {
  if (!isFinite(m) || m > horizon * 60 * mps) return `over ${horizon} min`;
  const t = m / mps / 60;
  return t < 1 ? "under a minute" : `${Math.round(t)} min`;
}

/** "about every 3 weeks", "about every 2 years": the gap between events at `perYear` a year. */
export function everyOf(perYear) {
  if (!(perYear > 0)) return null;
  const weeks = 52.18 / perYear;
  if (weeks < 1.5) return `about every ${Math.max(1, Math.round(7 * weeks))} days`;
  if (weeks < 10) return `about every ${Math.round(weeks)} weeks`;
  const months = 12 / perYear;
  if (months < 18) return `about every ${Math.round(months)} months`;
  return `about every ${Math.round(1 / perYear)} years`;
}

/** "about every 8 to 11 weeks": the gaps at two rates, faster first, in one unit. */
export function everyRange(fast, slow) {
  if (!(fast > 0) || !(slow > 0)) return null;
  const [a, b] = [52.18 / fast, 52.18 / slow];
  if (Math.round(a) === Math.round(b)) return everyOf(fast);
  if (b < 26) return `about every ${Math.round(a)} to ${Math.round(b)} weeks`;
  if (12 / slow < 24) return `about every ${Math.max(1, Math.round(12 / fast))} to ${Math.round(12 / slow)} months`;
  return `about every ${Math.max(1, Math.round(1 / fast))} to ${Math.round(1 / slow)} years`;
}

function inside(ring, x, y) {
  let c = false;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    const [xi, yi] = ring[i], [xj, yj] = ring[j];
    if ((yi > y) !== (yj > y) && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) c = !c;
  }
  return c;
}

class Heap {
  constructor() { this.v = []; this.k = []; }
  get size() { return this.v.length; }
  push(v, k) {
    const a = this.v, b = this.k;
    let i = a.length;
    a.push(v); b.push(k);
    while (i > 0) {
      const p = (i - 1) >> 1;
      if (b[p] <= k) break;
      a[i] = a[p]; b[i] = b[p]; i = p;
    }
    a[i] = v; b[i] = k;
  }
  pop() {
    const a = this.v, b = this.k, top = [a[0], b[0]];
    const v = a.pop(), k = b.pop();
    if (a.length) {
      let i = 0;
      for (;;) {
        let c = 2 * i + 1;
        if (c >= a.length) break;
        if (c + 1 < a.length && b[c + 1] < b[c]) c++;
        if (b[c] >= k) break;
        a[i] = a[c]; b[i] = b[c]; i = c;
      }
      a[i] = v; b[i] = k;
    }
    return top;
  }
}
