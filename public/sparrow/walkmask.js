// Where people walk (a site's walking mask: Ghent's sidewalks and corner crossings, built from the parcels by
// tools/ghent/build.py walk_mask). A bit a cell, rows south to north in survey metres; the walk's z is -y.
// Routes are A* over the cells (eight ways, no cutting corners), in a window round the trip, then pulled straight
// wherever a straight line stays on the mask. Pure, so Node can test it.

/** Fetch and unpack a site's mask: zlib-compressed bits (DecompressionStream does the inflating). */
export async function loadWalkMask(url, meta) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`${r.status} for ${url}`);
  const raw = await new Response(r.body.pipeThrough(new DecompressionStream("deflate"))).arrayBuffer();
  return makeMask(new Uint8Array(raw), meta);
}

/** A mask over `bits` (packed, high bit first, row-major from the south-west corner). */
export function makeMask(bits, { x0, y0, cell, w, h }) {
  bits = new Uint8Array(bits); // ours to carve
  const ok = (c, r) => c >= 0 && r >= 0 && c < w && r < h && ((bits[(r * w + c) >> 3] >> (7 - ((r * w + c) & 7))) & 1) === 1;
  const set = (c, r) => { if (c >= 0 && r >= 0 && c < w && r < h) bits[(r * w + c) >> 3] |= 1 << (7 - ((r * w + c) & 7)); };
  const col = (x) => Math.floor((x - x0) / cell), row = (z) => Math.floor((-z - y0) / cell);
  const cx = (c) => x0 + (c + 0.5) * cell, cz = (r) => -(y0 + (r + 0.5) * cell);

  /** The nearest walkable cell's middle to (x, z), within `maxM` metres, or null. */
  function nearest(x, z, maxM = 6) {
    const c0 = col(x), r0 = row(z);
    if (ok(c0, r0)) return { x, z };
    const R = Math.ceil(maxM / cell);
    for (let d = 1; d <= R; d++) {
      let best = null, bd = Infinity;
      for (let i = -d; i <= d; i++) {
        for (const [c, r] of [[c0 + i, r0 - d], [c0 + i, r0 + d], [c0 - d, r0 + i], [c0 + d, r0 + i]]) {
          if (!ok(c, r)) continue;
          const dd = (c - c0) ** 2 + (r - r0) ** 2;
          if (dd < bd) { bd = dd; best = { x: cx(c), z: cz(r) }; }
        }
      }
      if (best) return best;
    }
    return null;
  }

  /** Make a way from (ax, az) to (bx, bz) walkable, `width` metres wide: a front walk from a door to the sidewalk. */
  function carve(ax, az, bx, bz, width = 1.2) {
    const len = Math.hypot(bx - ax, bz - az), n = Math.max(1, Math.ceil(len / (cell * 0.5)));
    const px = -(bz - az) / (len || 1), pz = (bx - ax) / (len || 1), k = Math.max(0, Math.round(width / cell / 2));
    for (let i = 0; i <= n; i++) {
      const x = ax + ((bx - ax) * i) / n, z = az + ((bz - az) * i) / n;
      for (let j = -k; j <= k; j++) set(col(x + px * j * cell), row(z + pz * j * cell));
    }
  }

  /** Whether a straight line stays on the mask (sampled every quarter cell). */
  function clear(ax, az, bx, bz) {
    const len = Math.hypot(bx - ax, bz - az), n = Math.max(1, Math.ceil(len / (cell * 0.25)));
    for (let i = 0; i <= n; i++) if (!ok(col(ax + ((bx - ax) * i) / n), row(az + ((bz - az) * i) / n))) return false;
    return true;
  }

  /**
   * A route from a to b ({x, z} each) along the mask: a list of points from a to b, or null (off the mask, or no way
   * within the window, or too far to look). The window is the trip's box grown by `margin` metres.
   */
  function path(a, b, { margin = 40, maxNodes = 150000 } = {}) {
    const s = nearest(a.x, a.z, 3), g = nearest(b.x, b.z, 3);
    if (!s || !g) return null;
    const sc = col(s.x), sr = row(s.z), gc = col(g.x), gr = row(g.z);
    const m = Math.ceil(margin / cell);
    const c0 = Math.max(0, Math.min(sc, gc) - m), r0 = Math.max(0, Math.min(sr, gr) - m);
    const c1 = Math.min(w - 1, Math.max(sc, gc) + m), r1 = Math.min(h - 1, Math.max(sr, gr) + m);
    const W = c1 - c0 + 1, Hh = r1 - r0 + 1, N = W * Hh;
    if (N > 1.2e6) return null;
    const gs = new Float32Array(N).fill(Infinity), from = new Int32Array(N).fill(-1), shut = new Uint8Array(N);
    const heap = []; // [f, i] pairs, a binary heap on f
    const push = (f, i) => {
      heap.push([f, i]);
      for (let k = heap.length - 1; k > 0;) { const p = (k - 1) >> 1; if (heap[p][0] <= heap[k][0]) break; [heap[p], heap[k]] = [heap[k], heap[p]]; k = p; }
    };
    const pop = () => {
      const top = heap[0], last = heap.pop();
      if (heap.length) {
        heap[0] = last;
        for (let k = 0; ;) {
          const l = 2 * k + 1, r = l + 1;
          let q = k;
          if (l < heap.length && heap[l][0] < heap[q][0]) q = l;
          if (r < heap.length && heap[r][0] < heap[q][0]) q = r;
          if (q === k) break;
          [heap[q], heap[k]] = [heap[k], heap[q]];
          k = q;
        }
      }
      return top;
    };
    const hOf = (c, r) => { const dx = Math.abs(c - gc), dy = Math.abs(r - gr); return Math.max(dx, dy) + 0.4142 * Math.min(dx, dy); };
    const si = (sr - r0) * W + (sc - c0), gi = (gr - r0) * W + (gc - c0);
    gs[si] = 0;
    push(hOf(sc, sr), si);
    let expanded = 0, found = false;
    while (heap.length) {
      const [, i] = pop();
      if (shut[i]) continue;
      if (i === gi) { found = true; break; }
      shut[i] = 1;
      if (++expanded > maxNodes) return null;
      const c = (i % W) + c0, r = Math.floor(i / W) + r0;
      for (let dr = -1; dr <= 1; dr++) {
        for (let dc = -1; dc <= 1; dc++) {
          if (!dr && !dc) continue;
          const nc = c + dc, nr = r + dr;
          if (nc < c0 || nc > c1 || nr < r0 || nr > r1 || !ok(nc, nr)) continue;
          if (dr && dc && (!ok(c + dc, r) || !ok(c, r + dr))) continue; // no squeezing past a corner
          const j = (nr - r0) * W + (nc - c0);
          if (shut[j]) continue;
          const ng = gs[i] + (dr && dc ? 1.4142 : 1);
          if (ng < gs[j]) { gs[j] = ng; from[j] = i; push(ng + hOf(nc, nr), j); }
        }
      }
    }
    if (!found) return null;
    const cells = [];
    for (let i = gi; i !== -1; i = from[i]) cells.push({ x: cx((i % W) + c0), z: cz(Math.floor(i / W) + r0) });
    cells.reverse();
    cells[0] = s;
    cells[cells.length - 1] = g;
    // Pulled straight: from each point, on to the furthest one in a clear line.
    const out = [cells[0]];
    for (let i = 0; i < cells.length - 1;) {
      let j = i + 1;
      while (j + 1 < cells.length && clear(cells[i].x, cells[i].z, cells[j + 1].x, cells[j + 1].z)) j++;
      out.push(cells[j]);
      i = j;
    }
    return out;
  }

  return { walkable: (x, z) => ok(col(x), row(z)), nearest, carve, clear, path, cell };
}
