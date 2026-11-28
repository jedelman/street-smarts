// showcase.mjs: this build's showcase shots for Ghent, chosen from the build's own data by stated criteria, each with
// why it was chosen in numbers; then (with --capture) photographed in the walk, so the gallery shows what you'll see
// when you go there.
//
//   node tools/ghent/showcase.mjs                         choose: writes public/sites/ghent-streets/showcase.json
//   node tools/ghent/showcase.mjs --capture http://localhost:8765
//                                                         and photograph each shot into showcase/<id>.jpg
//   node tools/ghent/showcase.mjs --tour --glb /archive/ghent-streets/ghent-first/model.glb --out <dir> --capture <url>
//                                                         an archived build's showcase: its tour's six stops
//
// The criteria (one shot each, never two of the same lot):
//   porches   the block face with the most porches (at least six house fronts), from the yard grammar (yards.js)
//   cafe      a converted café or bistro whose house keeps a full porch: the adaptive reuse in one frame
//   stacked   the apartment building with the widest stacked porch
//   twotone   the block face with the most two-tone houses (brick below, shingle or siding above)
//   lanterns  the densest run of Colonial lanterns, where Dominion's records put them
//   everyday  the ground floor that would bring the most lots a weekly need (coffee, bread, groceries, hardware, a
//             haircut, dry cleaning) within five minutes that today is more than ten minutes away (Day to day's network)
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { growYard } from "../../public/sparrow/yards.js";
import { prepare, fromPoint, metresTo } from "../../public/sites/ghent-streets/walkgraph.js";

const ROOT = new URL("../../", import.meta.url).pathname;
const SITE = `${ROOT}public/sites/ghent-streets/`;
const args = process.argv.slice(2);
const opt = (k) => (args.includes(k) ? args[args.indexOf(k) + 1] : null);

// ---------------------------------------------------------------- the build's data
function readGlb(path) {
  const b = readFileSync(path);
  const n = b.readUInt32LE(12), gltf = JSON.parse(b.subarray(20, 20 + n).toString());
  const T = gltf.nodes[0].extras.streetSmarts, d = T.detail, bv = gltf.bufferViews[d.walls.bufferView];
  const s0 = 20 + n + 8, off = b.byteOffset + s0 + (bv.byteOffset || 0);
  const W = new Float32Array(b.buffer.slice(off, off + d.walls.count * 36));
  return { gltf, T, W };
}
const nice = (a) => (a || "").split(/\s+/).map((w) => (/^\d/.test(w) ? w : w[0] + w.slice(1).toLowerCase())).join(" ");
const BIZ = { coffee: "a coffee shop", bistro: "a bistro", bakery: "a bakery", books: "a bookshop", greengrocer: "a greengrocer's",
  florist: "a florist's", hardware: "a hardware store", bikes: "a bike shop", records: "a record shop", hairdresser: "a hairdresser's",
  barber: "a barbershop", cleaners: "a dry cleaner's", tailor: "a tailor's", accountant: "an accountant's office",
  architect: "an architects' studio", design: "a design studio", dentist: "a dentist's office", gallery: "an art gallery", yoga: "a yoga studio" };

function rngFor(seed) { // the walk's (frontages.js), so the grammar grows what the walk grows
  let h = 2166136261;
  for (const ch of String(seed)) { h ^= ch.charCodeAt(0); h = Math.imul(h, 16777619); }
  return () => { h += 0x6d2b79f5; let t = h; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}

function chooseShots() {
  const { T, W } = readGlb(`${SITE}model.glb`);
  const S = JSON.parse(readFileSync(`${ROOT}data/ghent/survey.json`));
  const PR = JSON.parse(readFileSync(`${ROOT}data/ghent/proposal.json`));
  const E = prepare(JSON.parse(readFileSync(`${SITE}everyday.json`)));
  const parcel = Object.fromEntries(S.parcels.map((p) => [p.gpin, p]));
  const plan = PR.parcels;
  const wall = (i) => ({ A: [W[i * 9], W[i * 9 + 1]], u: [W[i * 9 + 2], W[i * 9 + 3]], n: [W[i * 9 + 4], W[i * 9 + 5]], L: W[i * 9 + 6] });
  const at = (w, along, out) => [w.A[0] + w.u[0] * along + w.n[0] * out, w.A[1] + w.u[1] * along + w.n[1] * out];
  const r2 = (p) => p.map((v) => Math.round(v * 100) / 100);
  // Every front's yard, as the walk grows it.
  const fronts = T.yards.rows.map((y) => {
    const w = wall(y[0]);
    const row = { door: y[1], depth: y[2], lotLo: y[3], lotHi: y[4], form: y[5], first: y[6], floors: y[7], built: y[8], converted: y[9] === 1 };
    const g = growYard({ row, L: w.L, windows: [], rng: rngFor(`yard:${y[10]}`) });
    const a = parcel[y[10]] && parcel[y[10]].address, m = /^(\d+)\s+(.+)$/.exec(a || "");
    return { y, w, row, g, gpin: y[10], twotone: y[11] === 1, address: a, num: m ? +m[1] : null, street: m ? m[2] : null };
  });
  const used = new Set(), shots = [];
  // Is the camera's boom clear? No tree trunk and no building within it (behind the sparrow, toward the camera).
  const trunks = S.trees.map((t) => [t[0], -t[1]]);
  const footprints = S.buildings.filter((b) => b.poly.length >= 3).map((b) => b.poly.map(([x, y]) => [x, -y]));
  const inside = (ring, x, z) => { let c = false; for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) { const [xi, zi] = ring[i], [xj, zj] = ring[j]; if ((zi > z) !== (zj > z) && x < ((xj - xi) * (z - zi)) / (zj - zi) + xi) c = !c; } return c; };
  const clear = (stand, look) => {
    const dx = look[0] - stand[0], dz = look[2] - stand[1], d = Math.hypot(dx, dz), hx = dx / d, hz = dz / d;
    for (let k = 0; k <= 5; k += 0.5) {
      const x = stand[0] - hx * k, z = stand[1] - hz * k;
      if (trunks.some(([tx, tz]) => Math.hypot(tx - x, tz - z) < 1.3)) return false;
      if (footprints.some((r) => Math.abs(r[0][0] - x) < 60 && Math.abs(r[0][1] - z) < 60 && inside(r, x, z))) return false;
    }
    return true;
  };
  const facing = (f, out, side = 0, lookY = 2.6, pitch = -60) => {
    const look = at(f.w, f.row.door, 0.5), L = [look[0], lookY, look[1]];
    for (const sd of [side, -side, side * 1.6, -side * 1.6, 0]) {
      for (const o of [out, out - 1.2, out + 1.5]) {
        const stand = at(f.w, f.row.door + sd, f.row.depth + o);
        if (clear(stand, L)) return { stand: r2(stand), look: r2(L), pitch };
      }
    }
    return { stand: r2(at(f.w, f.row.door + side, f.row.depth + out)), look: r2(L), pitch };
  };
  const blockOf = (f) => `${f.street}|${Math.floor(f.num / 100) * 100}|${f.num % 2}`;
  const blocks = new Map();
  for (const f of fronts) if (f.street && f.num != null) { const k = blockOf(f); if (!blocks.has(k)) blocks.set(k, []); blocks.get(k).push(f); }
  const alongBlock = (fs, h = 4.5, lookY = 3) => {
    // Stand on the sidewalk before the first front and look along the block to the last.
    const u = fs[0].w.u, key = (f) => { const d = at(f.w, f.row.door, 0); return d[0] * u[0] + d[1] * u[1]; };
    const o = [...fs].sort((a, b) => key(a) - key(b)), first = o[0], last = o[o.length - 1];
    const s = at(first.w, first.row.door - 5, first.row.depth + h), l = at(last.w, last.row.door, 1);
    return { stand: r2(s), look: r2([l[0], lookY, l[1]]), pitch: -60 };
  };
  const diagonal = (fs, lookY = 3) => {
    // From the street, at a slant onto the middle of the block, so three or four fronts show at once; the first
    // stand along the block whose camera has a clear view.
    const u = fs[0].w.u, key = (f) => { const d = at(f.w, f.row.door, 0); return d[0] * u[0] + d[1] * u[1]; };
    const mid = [...fs].sort((a, b) => key(a) - key(b))[Math.floor(fs.length / 2)];
    for (const side of [-7, 7, -5, 5, -9, 9, -11, 11, -3, 3]) {
      for (const out of [5.5, 4.8, 6.3]) {
        const st = at(mid.w, mid.row.door + side, mid.row.depth + out), l = at(mid.w, mid.row.door - Math.sign(side) * 5, 0.5);
        const look = [l[0], lookY, l[1]];
        if (clear(st, look)) return { stand: r2(st), look: r2(look), pitch: -60, zoom: 0.75 };
      }
    }
    const st = at(mid.w, mid.row.door - 7, mid.row.depth + 5.5), l = at(mid.w, mid.row.door + 5, 0.5);
    return { stand: r2(st), look: r2([l[0], lookY, l[1]]), pitch: -60, zoom: 0.75 };
  };
  const label = (k) => { const [st, b, par] = k.split("|"); return { street: nice(st), block: b, side: par === "1" ? "odd" : "even" }; };

  // porches
  {
    let best = null;
    for (const [k, fs] of blocks) {
      const houses = fs.filter((f) => f.row.form === "house");
      if (houses.length < 6) continue;
      const p = houses.filter((f) => f.g.porch).length;
      if (!best || p > best.p || (p === best.p && p / houses.length > best.p / best.n)) best = { k, fs: houses, p, n: houses.length };
    }
    const L = label(best.k);
    best.fs.forEach((f) => used.add(f.gpin));
    shots.push({ id: "porches", title: `A porch street: the ${L.block} block of ${L.street}`,
      caption: `${best.p} of ${best.n} house fronts with a porch, on the ${L.side}-numbered side.`,
      why: [`Chosen as the block face in Ghent where the most house fronts get a porch: ${best.p} of its ${best.n} houses, among blocks with at least six.`,
        "Porches come from how often they appear in 121 of the City assessor's own street photos of Ghent: four in five houses have one, mostly on round columns behind white balusters, under a low hip roof (tools/ghent/yard-photos.md).",
        "The City's footprints are traced from above, so a porch's roof is already inside them: here a porch is cut into the ground floor, a shaded recess with its columns at the wall, and only the steps, the walk and the yard come out in front."],
      look_for: "Round columns and balusters, the shade of the porch, steps down to clipped boxwoods along the foundation.",
      source: `parcel:${best.fs[0].gpin}`, ...diagonal(best.fs.filter((f) => f.g.porch)) });
  }
  // cafe
  {
    const c = fronts.filter((f) => f.row.converted && !used.has(f.gpin) && f.g.porch && f.row.depth >= 3.5 &&
      ["coffee", "bistro"].includes(plan[f.gpin] && plan[f.gpin].business));
    c.sort((a, b) => (b.g.porch.hi - b.g.porch.lo) - (a.g.porch.hi - a.g.porch.lo));
    const f = c[0];
    used.add(f.gpin);
    const biz = BIZ[plan[f.gpin].business];
    shots.push({ id: "cafe", title: `${nice(f.address)}: ${biz} on a porch`,
      caption: `The house keeps its porch, door and windows; ${biz} shows through them.`,
      why: [`Chosen as the converted café or bistro whose house keeps the widest porch (${(f.g.porch.hi - f.g.porch.lo).toFixed(1)} m): adaptive reuse in one frame.`,
        `Its ground floor would hold ${biz} (${plan[f.gpin].use}), up to ${plan[f.gpin].unit_sqft.toLocaleString("en-US")} sq ft. Why it may: ${plan[f.gpin].reasons.map((k) => PR.rules[k].label).join("; ")}.`,
        "Doors and windows stay where they are: the building is adapted, not remodelled. The new use shows through its own windows, with a sign over the door."],
      look_for: "The sign over the door, the lit windows, the porch as it was.", source: `parcel:${f.gpin}`, ...facing(f, 5, 2.5) });
  }
  // stacked
  {
    const c = fronts.filter((f) => f.row.form === "apartment" && !used.has(f.gpin) && f.g.word.some((t) => t.s === "Upper") && f.row.depth >= 2);
    c.sort((a, b) => (b.g.porch.hi - b.g.porch.lo) * b.row.floors - (a.g.porch.hi - a.g.porch.lo) * a.row.floors);
    const f = c[0];
    used.add(f.gpin);
    shots.push({ id: "stacked", title: `${nice(f.address)}: porches stacked`, caption: "An apartment building's porch, one storey on another.",
      why: [`Chosen as the apartment building with the widest stacked porch (${(f.g.porch.hi - f.g.porch.lo).toFixed(1)} m across, ${f.row.floors} storeys).`,
        "In the assessor's photos, half the apartment buildings' porches are stacked two or more storeys (13 of 26), several three or four tiers; their railings are mostly iron, their roofs mostly flat."],
      look_for: "The second porch over the first, its railing, the columns running up through both.", source: `parcel:${f.gpin}`, ...facing(f, 8, 3, 4.4) });
  }
  // twotone
  {
    let best = null;
    for (const [k, fs] of blocks) {
      const free = fs.filter((f) => !used.has(f.gpin) && f.row.form !== "apartment");
      const t = free.filter((f) => f.twotone).length;
      if (free.length >= 5 && (!best || t > best.t)) best = { k, fs: free, t, n: free.length };
    }
    const L = label(best.k);
    best.fs.forEach((f) => used.add(f.gpin));
    shots.push({ id: "twotone", title: `Two-tone houses: the ${L.block} block of ${L.street}`,
      caption: `${best.t} of ${best.n} houses brick below, shingle or siding above.`,
      why: [`Chosen as the block face with the most two-tone houses: ${best.t} of ${best.n}.`,
        "A third of the houses in the assessor's photos are two-tone, mostly a brick ground floor with shingle or siding above, so walls are painted by building type from those photos: brick for most apartment buildings, and for houses brick, two-tone, siding, shingle or stucco in the photos' proportions."],
      look_for: "The line where the brick stops and the shingle begins, a storey up.", source: `parcel:${best.fs[0].gpin}`, ...diagonal(best.fs.filter((f) => f.twotone), 4.2) });
  }
  // lanterns
  {
    const col = T.lights.rows.filter((r) => r[2] === "colonial");
    let best = null;
    for (const r of col) {
      const near = col.filter((q) => Math.hypot(q[0] - r[0], q[1] - r[1]) < 45);
      if (!best || near.length > best.near.length) best = { r, near };
    }
    // The run's direction: its spread's main axis.
    const mx = best.near.reduce((s, q) => s + q[0], 0) / best.near.length, mz = best.near.reduce((s, q) => s + q[1], 0) / best.near.length;
    let sxx = 0, szz = 0, sxz = 0;
    for (const q of best.near) { sxx += (q[0] - mx) ** 2; szz += (q[1] - mz) ** 2; sxz += (q[0] - mx) * (q[1] - mz); }
    const th = 0.5 * Math.atan2(2 * sxz, sxx - szz), dir = [Math.cos(th), Math.sin(th)];
    const proj = (q) => (q[0] - mx) * dir[0] + (q[1] - mz) * dir[1];
    const first = best.near.reduce((a, q) => (proj(q) < proj(a) ? q : a));
    const toStreet = [Math.sin(first[4]), Math.cos(first[4])]; // each light's yaw faces its street
    const stand = [first[0] + toStreet[0] * 4 - dir[0] * 10, first[1] + toStreet[1] * 4 - dir[1] * 10];
    const look = [first[0] + dir[0] * 40, 2.8, first[1] + dir[1] * 40];
    const total = T.lights.rows.length, ncol = col.length;
    shots.push({ id: "lanterns", title: "Lanterns", caption: `${best.near.length} Colonial lanterns within 45 m of each other.`,
      why: [`Chosen as the densest run of Colonial lanterns: ${best.near.length} within 45 m of each other, where Dominion Energy's records (the City's open data) put them.`,
        `${ncol} of the ${total} streetlights in and by the League's area are Colonials, by our reading of the data's fixture codes against Dominion's catalogue: a lantern on a 13-foot pole. The rest are cobra heads and shoeboxes on arms over the busier streets, and floodlights.`],
      look_for: "The lanterns lit along the sidewalk, and their pools of light.", stand: r2(stand), look: r2(look), pitch: -60 });
  }
  // everyday
  {
    const mps = E.speed_mps, five = 5 * 60 * mps, ten = 10 * 60 * mps;
    const lots = Object.entries(E.lots);
    const todayNearest = new Map();
    const searches = new Map();
    for (const [g, l] of lots) {
      const D = fromPoint(E, l.slice(0, 3), 16 * 60 * mps);
      searches.set(g, D);
      const near = new Array(E.kinds.length).fill(Infinity);
      for (const [e, a, o, k] of E.today) near[k] = Math.min(near[k], metresTo(E, D, [e, a, o]));
      todayNearest.set(g, near);
    }
    const WEEKLY = new Set(["coffee", "bakery", "groceries", "hardware", "hair", "cleaners"].map((id) => E.kinds.findIndex((k) => k.id === id)));
    let best = null;
    for (const [e, a, o, k, gpin] of E.proposed) {
      if (k < 0 || !WEEKLY.has(k) || used.has(gpin)) continue;
      let gain = 0;
      for (const [g] of lots) if (todayNearest.get(g)[k] > ten && metresTo(E, searches.get(g), [e, a, o]) <= five) gain++;
      if (!best || gain > best.gain) best = { gpin, k, gain };
    }
    const f = fronts.find((q) => q.gpin === best.gpin);
    const kind = E.kinds[best.k].label.toLowerCase(), biz = BIZ[plan[best.gpin].business];
    used.add(best.gpin);
    const place = f || null;
    const view = place ? facing(place, 5, 2) : (() => {
      const sh = T.detail.shops.rows.find((r) => r[10] === `parcel:${best.gpin}`), w = wall(sh[0]);
      const s = at(w, sh[3] + 2, 6), l = at(w, sh[3], 0.5);
      return { stand: r2(s), look: r2([l[0], 2.6, l[1]]), pitch: -60 };
    })();
    shots.push({ id: "everyday", title: `${nice(parcel[best.gpin].address)}: ${biz}, five minutes from ${best.gain} lots`,
      caption: `It would bring ${kind} within five minutes of ${best.gain} lots where today it's more than ten.`,
      why: [`Chosen as the ground floor that would change the most neighbours' weeks, among the weekly needs (coffee, bread, groceries, hardware, a haircut, dry cleaning): if it opened, ${best.gain} lots would have ${kind === "groceries" ? "groceries" : `${kind.startsWith("a ") || kind.startsWith("an ") ? kind : `a place for ${kind}`}`} within a five-minute walk where today the nearest is more than ten minutes away.`,
        "Walks run along the City's street centerlines at 3 miles an hour (ours); today's places are Overture Maps' (open, at confidence 0.6 or more). Day to day shows it from any lot.",
        `The map opens every ground floor the proposal would allow; Policy Brief No. 2 expects far fewer: “${E.pace.says}”. At that pace, any one of them has about a ${Math.round(100 * E.pace.lo / E.pace.of)}% to ${Math.round(100 * E.pace.hi / E.pace.of)}% chance of opening.`],
      look_for: "Its sign and its lit windows; then open Day to day from here.", source: `parcel:${best.gpin}`, ...view });
  }
  return shots;
}

function tourShots(site) {
  // An archived build's showcase: its tour's stops, and why the tour chose them.
  const PR = JSON.parse(readFileSync(`${ROOT}data/ghent/proposal.json`));
  const S = JSON.parse(readFileSync(`${ROOT}data/ghent/survey.json`));
  const parcel = Object.fromEntries(S.parcels.map((p) => [p.gpin, p]));
  const rings = S.buildings.filter((b) => b.poly.length >= 3).map((b) => b.poly);
  const inBuilding = (x, y) => rings.some((r) => Math.abs(r[0][0] - x) < 60 && Math.abs(r[0][1] - y) < 60 && (() => {
    let c = false;
    for (let i = 0, j = r.length - 1; i < r.length; j = i++) { const [xi, yi] = r[i], [xj, yj] = r[j]; if ((yi > y) !== (yj > y) && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) c = !c; }
    return c;
  })());
  return PR.waypoints.map((w, i) => {
    const r = PR.parcels[w.gpin], p = parcel[w.gpin];
    return { id: `stop-${i + 1}`, title: `${nice(p.address)}: ${BIZ[r.business]}`, caption: `Tour stop ${i + 1}: a ${r.typology} built ${p.built}, opening as ${BIZ[r.business]}.`,
      why: [`One of the tour's six stops, chosen to differ from the other five as much as possible (the building, its age and size, what it becomes, why it may), and placed so that no stop can see another's.`,
        `Its ground floor would hold ${BIZ[r.business]} (${r.use}), up to ${r.unit_sqft.toLocaleString("en-US")} sq ft. Why it may: ${r.reasons.map((k) => PR.rules[k].label).join("; ")}.`],
      look_for: "The sign, the lit windows, the tables in the yard.", source: `parcel:${w.gpin}`,
      // The tour's stand is where its camera stood; the sparrow stands about 14 m from what it looks at, its camera
      // behind it.
      stand: (() => {
        const d = Math.hypot(w.look[0] - w.stand[0], w.look[1] - w.stand[1]), f0 = Math.max(0, 1 - 14 / d);
        for (const f of [f0, f0 * 0.7, f0 * 0.4, 0]) {
          const x = w.stand[0] + (w.look[0] - w.stand[0]) * f, y = w.stand[1] + (w.look[1] - w.stand[1]) * f;
          if (!inBuilding(x, y)) return [x, -y];
        }
        return [w.stand[0], -w.stand[1]];
      })(),
      look: [w.look[0], 2.4, -w.look[1]], pitch: -60 };
  });
}

// ---------------------------------------------------------------- photographing them
async function capture(base, shots, outDir, page = "/ghent/", glb = null) {
  const require = createRequire(import.meta.url);
  const { chromium } = require("playwright");
  const b = await chromium.launch({ args: ["--use-gl=swiftshader", "--enable-unsafe-swiftshader"] });
  const p = await (await b.newContext({ viewport: { width: 960, height: 600 } })).newPage();
  const errs = [];
  p.on("pageerror", (e) => errs.push(String(e)));
  await p.goto(`${base}${page}?debug${glb ? `&glb=${glb}` : ""}`, { waitUntil: "load" });
  await p.waitForFunction(() => window.__walk && window.__walk.frontages, null, { timeout: 240000 });
  await p.addStyleTag({ content: ".hud, #log, #hint, #prompt, #bubble, #intro, #pad, #iteration-note, #flyout, #peek { display: none !important; }" });
  await p.waitForTimeout(9000); // past the opening crane shot: by the walk's own clock, which a slow machine runs slower
  await p.waitForFunction(() => !window.__walk.director.shooting, null, { timeout: 120000 });
  mkdirSync(`${outDir}showcase`, { recursive: true });
  for (const s of shots) {
    await p.evaluate((s) => new Promise((done) => window.__walk.warp(s, done)), s);
    await p.waitForTimeout(3200);
    await p.screenshot({ path: `${outDir}${s.image}`, type: "jpeg", quality: 78 });
    console.log("photographed", s.id);
  }
  await b.close();
  if (errs.length) throw new Error(errs.join("\n"));
}

const tour = args.includes("--tour");
const outDir = opt("--out") || SITE;
const shots = (tour ? tourShots() : chooseShots()).map((s) => ({ ...s, image: `showcase/${s.id}.jpg` }));
const showcase = { version: 1, site: "ghent-streets",
  intro: tour ? "The first Ghent build's showcase: the tour's six ground floors, as different from each other as they come."
    : "What this build changed, in six places, each chosen by a rule from the build's own data. Tap one to go there.",
  method: tour ? "The tour's stops (tools/ghent/propose.py)." : "Chosen by tools/ghent/showcase.mjs: one shot per criterion, never two of the same lot.",
  shots };
writeFileSync(`${outDir}showcase.json`, JSON.stringify(showcase, null, 1) + "\n");
console.log(`showcase.json: ${shots.length} shots`, shots.map((s) => `${s.id}: ${s.title}`));
if (opt("--capture")) await capture(opt("--capture"), shots, outDir, "/ghent/", opt("--glb"));
