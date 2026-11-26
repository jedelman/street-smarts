// Street-level detail: every window on every floor, and storefronts where the
// pipeline opened a building to the street.
//
// The data is the GLB's own detail table (render.frontage_detail, written by
// pack_glb.add_detail): walls, each opening on them, and shop fronts, which are
// a public wall's share around each of its ground-floor doors (most of them
// P87 Individually Owned Shops' own doors). Where a window is, where a door
// is, which walls face a street or a square: data. What kind
// of shop stands at a door, its paint, awning and sign: ours, seeded by the
// shop's id so it never changes between visits, after the Ghent Streets
// explorer's storefronts (jason-edelman.org/ghent-streets/explorer): its kits,
// palettes, 3.3 m bays, piers, bulkheads, display glass, transoms, sign bands,
// cornices and canvas awnings.
//
// Cheap by construction: every part is an instance of a unit box or quad, in a
// handful of materials, grouped into chunks the camera's frustum culls, and
// each kind of part is drawn only within its own distance (a window's glass
// from anywhere, a sill or a café chair only up close).
import * as THREE from "three";

// ---------------------------------------------------------------- the kits
// The first four are the Ghent explorer's (same signs, paints and awnings); the
// rest are ours, for a site with forty shops instead of one.
const GHENT_TRIMS = ["#efe6d2", "#e2d3ac", "#f6f1e7", "#c9b78a"];
const GHENT_BRICKS = ["#8a4a36", "#9a5a40", "#7a3f30", "#a8705a"];
export const KITS = [
  { id: "coffee", sign: "COFFEE", font: "700 92px Georgia, 'Times New Roman', serif", ink: "#f3ede0",
    paints: ["#2f4a3a", "#34503c", "#2f5d62", "#3e5a3a"], stripes: ["#2f4a3a", "#efe6d2"], inside: "counter", props: "cafe", icon: "cup" },
  { id: "books", sign: "BOOKS", font: "700 92px Georgia, 'Times New Roman', serif", ink: "#f3ede0",
    paints: ["#6b2430", "#5a2a2a", "#7a3b2e", "#4a2a3a"], stripes: ["#6b2430", "#efe6d2"], inside: "shelves", props: "books", icon: "book" },
  { id: "bistro", sign: "BISTRO", font: "italic 600 92px Georgia, 'Times New Roman', serif", ink: "#f0d9a8",
    paints: ["#1f2a44", "#1a1714", "#3b2a3f", "#22313b"], stripes: ["#8a2b1f", "#f0e3c8"], inside: "lamps", props: "bistro", icon: "glass" },
  { id: "studio", sign: "STUDIO", font: "500 80px 'DM Mono', ui-monospace, Menlo, monospace", ink: "#1a1714",
    paints: ["#1d1f22", "#2b2d31", "#3a3d42"], band: "#f3f1ec", stripes: null, inside: "desks", props: null, icon: "square" },
  { id: "bakery", sign: "BAKERY", font: "700 92px Georgia, 'Times New Roman', serif", ink: "#f6efe0",
    paints: ["#8b5a2b", "#a0522d", "#6e4b3a"], stripes: ["#c9a46a", "#f6efe0"], inside: "bread", props: "cafe", icon: "loaf" },
  { id: "grocer", sign: "GROCER", font: "800 86px 'Helvetica Neue', Arial, sans-serif", ink: "#f2ecd8",
    paints: ["#3f6b35", "#2f5d3a", "#4a5a2a"], stripes: ["#3f6b35", "#f2ecd8"], inside: "produce", props: "crates", icon: "apple" },
  { id: "florist", sign: "FLOWERS", font: "italic 700 88px Georgia, 'Times New Roman', serif", ink: "#f6efe0",
    paints: ["#7a4a6b", "#5e3b5e", "#8a5a7a"], stripes: ["#c96a8a", "#f6efe0"], inside: "flowers", props: "buckets", icon: "flower" },
  { id: "hardware", sign: "HARDWARE", font: "800 80px 'Helvetica Neue', Arial, sans-serif", ink: "#f2d27a",
    paints: ["#8a2b1f", "#2b3a55", "#3a2f22"], stripes: ["#c8a23a", "#2b2b2b"], inside: "tools", props: "crates", icon: "hammer" },
  { id: "records", sign: "RECORDS", font: "700 84px 'Courier New', ui-monospace, monospace", ink: "#e9d9a8",
    paints: ["#1f2a44", "#3b2a3f", "#2a2a2a"], stripes: ["#d9b46a", "#1f2a44"], inside: "records", props: "aframe", icon: "disc" },
  { id: "tailor", sign: "TAILOR", font: "500 88px Georgia, 'Times New Roman', serif", ink: "#efe6d2",
    paints: ["#2b2d31", "#3a3d42", "#4a3a2a"], stripes: null, inside: "racks", props: null, icon: "scissors" },
];

const G = 3.5; // the ground floor's height (render.py FLOOR_TO_FLOOR_M)
const ST = G - 0.62; // top of the storefront, under the sign band
const GLASS_TOP = ST - 0.55; // top of the display glass, under the transom
const BAY = 3.3; // the explorer's pier spacing

// How far away each kind of part is still drawn (metres from the camera to its
// chunk), and how big its chunks are: big where a kind is seen from everywhere
// (fewer draw calls), small where it's only drawn up close (tighter culling).
// A storefront thins out with distance: its sign band, cornice and door read
// from 150 m ("mid"), its piers, bulkheads and trims from 75 m ("detail"),
// its capitals, brass and props only up close ("near").
const REACH = { windows: 900, glass: 260, display: 200, awnings: 150, mid: 150, letters: 130, detail: 75, near: 45, icons: 45 };
const CHUNK = { windows: 400, glass: 200, display: 200, awnings: 160, mid: 160, letters: 200, detail: 60, near: 50, icons: 120 };
const FRAME_M = 0.08; // a window's painted frame, drawn by its own quad's shader
const FRAME_COLOR = "#e8dfc8";

// The explorer's seeded random (FNV-1a into mulberry32), so a shop always looks the same.
function rngFor(seed) {
  let h = 2166136261;
  for (const ch of String(seed)) { h ^= ch.charCodeAt(0); h = Math.imul(h, 16777619); }
  return () => {
    h += 0x6d2b79f5;
    let t = h;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const pickOf = (rng, arr) => arr[Math.floor(rng() * arr.length)];

// ---------------------------------------------------------------- the table
/** The GLB's detail table as typed arrays, or null. */
export async function readDetail(parser, table) {
  const d = table && table.detail;
  if (!d || d.version !== 2) return null;
  const [w, o] = await Promise.all([
    parser.getDependency("bufferView", d.walls.bufferView),
    parser.getDependency("bufferView", d.openings.bufferView),
  ]);
  return {
    walls: new Float32Array(w, 0, d.walls.count * 9), nWalls: d.walls.count,
    openings: new Uint16Array(o, 0, d.openings.count * 6), nOpenings: d.openings.count,
    shops: d.shops.rows, strings: table.strings,
  };
}

// ---------------------------------------------------------------- textures
function canvas(w, h) {
  const c = document.createElement("canvas");
  c.width = w; c.height = h;
  return [c, c.getContext("2d")];
}
function texture(c) {
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  t.anisotropy = 4;
  return t;
}

/** Sign lettering, one kit per row, ink on transparent; and each word's width over its height. */
function lettersAtlas() {
  const rowH = 128, [c, g] = canvas(1024, rowH * KITS.length);
  const rects = [];
  KITS.forEach((k, i) => {
    g.font = k.font;
    g.fillStyle = k.ink;
    g.textAlign = "center";
    g.textBaseline = "middle";
    const w = Math.min(1000, g.measureText(k.sign).width + 24);
    g.fillText(k.sign, 512, i * rowH + rowH / 2 + 4, 1000);
    rects.push({ uv: [(512 - w / 2) / 1024, 1 - ((i + 1) * rowH) / c.height, w / 1024, rowH / c.height], aspect: w / rowH });
  });
  return { map: texture(c), rects };
}

/** Awning canvas: a kit's two stripes, or its paint for kits without. */
function stripesAtlas(paintOf) {
  const rowH = 32, [c, g] = canvas(256, rowH * KITS.length);
  KITS.forEach((k, i) => {
    const [a, b] = k.stripes || [paintOf(k), paintOf(k)];
    for (let s = 0; s < 8; s++) { g.fillStyle = s % 2 ? b : a; g.fillRect(s * 32, i * rowH, 32, rowH); }
  });
  return { map: texture(c), rect: (i) => [0, 1 - ((i + 1) * rowH) / c.height, 1, rowH / c.height] };
}

/** What you see through a shop's glass, one tile per kind of interior (after the explorer's windowTexture). */
const INSIDES = ["counter", "shelves", "lamps", "desks", "bread", "produce", "flowers", "tools", "records", "racks", "rooms"];
function insidesAtlas() {
  const tw = 256, th = 128, cols = 4, rows = Math.ceil(INSIDES.length / cols);
  const [c, g] = canvas(tw * cols, th * rows);
  const rng = rngFor("insides");
  const lamp = (x, y) => {
    const glow = g.createRadialGradient(x, y, 1, x, y, 18);
    glow.addColorStop(0, "rgba(255,248,220,1)"); glow.addColorStop(1, "rgba(255,220,150,0)");
    g.fillStyle = glow; g.beginPath(); g.arc(x, y, 18, 0, Math.PI * 2); g.fill();
  };
  const rects = {};
  INSIDES.forEach((kind, i) => {
    const x0 = (i % cols) * tw, y0 = Math.floor(i / cols) * th;
    g.save();
    g.beginPath(); g.rect(x0, y0, tw, th); g.clip();
    g.translate(x0, y0);
    const bg = { counter: "#f3d6a6", shelves: "#f1d29a", lamps: "#e8b67c", desks: "#e9eef3", bread: "#f4dcae", produce: "#efe2b8",
      flowers: "#f3e3d3", tools: "#e6dcc6", records: "#e4c89a", racks: "#ece3d3", rooms: "#e9cf9e" }[kind];
    g.fillStyle = bg; g.fillRect(0, 0, tw, th);
    if (kind === "shelves" || kind === "records" || kind === "tools") {
      const colors = kind === "records" ? ["#1a1714", "#7a2430", "#d9b46a", "#2f4a3a", "#3d4f7a"]
        : kind === "tools" ? ["#8a2b1f", "#5b6670", "#c8a23a", "#2b3a55", "#6b5a46"]
        : ["#7a2430", "#2f4a3a", "#d9b46a", "#3d4f7a", "#a8643a", "#efe6d2", "#5b3a5e"];
      for (let r = 0; r < 3; r++) {
        let x = 4;
        while (x < tw - 4) {
          const bw = kind === "records" ? 14 : 4 + rng() * 6;
          g.fillStyle = pickOf(rng, colors);
          g.fillRect(x, 10 + r * 38, bw, 24 + rng() * 6);
          x += bw + (kind === "records" ? 2 : 1);
        }
        g.fillStyle = "#5a3a1e"; g.fillRect(0, 36 + r * 38, tw, 4);
      }
    } else if (kind === "counter" || kind === "bread") {
      g.fillStyle = "#3a2a1c"; g.fillRect(90, 12, 76, 36);
      g.fillStyle = "#efe6d2"; for (let r = 0; r < 4; r++) g.fillRect(98, 18 + r * 7, 34 + (r % 2) * 20, 2);
      lamp(50, 22); lamp(206, 22);
      g.fillStyle = "#6b4526"; g.fillRect(0, 92, tw, 36);
      if (kind === "bread") { g.fillStyle = "#b8792e"; for (let k = 0; k < 9; k++) { g.beginPath(); g.ellipse(20 + k * 26, 86, 11, 6, 0, 0, Math.PI * 2); g.fill(); } }
      else { g.fillStyle = "#b9b2a6"; g.fillRect(150, 72, 34, 20); }
    } else if (kind === "lamps") {
      g.fillStyle = "#7a3c18"; for (let k = 0; k < 18; k++) g.fillRect(20 + k * 12, 46 + (k % 3), 5, 15);
      g.fillStyle = "#5a2b10"; g.fillRect(0, 62, tw, 4);
      for (let k = 0; k < 5; k++) lamp(25 + k * 51, 24);
      g.fillStyle = "#4a2410"; for (let k = 0; k < 4; k++) { g.fillRect(20 + k * 62, 100, 34, 4); g.fillRect(35 + k * 62, 104, 4, 24); }
    } else if (kind === "desks") {
      g.strokeStyle = "#9aa4ad"; g.lineWidth = 1;
      for (let k = 0; k < 4; k++) { g.fillStyle = "#ffffff"; g.fillRect(14 + k * 61, 12, 48, 34); g.strokeRect(14 + k * 61, 12, 48, 34); }
      g.fillStyle = "#5b6670"; for (let k = 0; k < 2; k++) { g.fillRect(15 + k * 125, 88, 100, 4); g.fillStyle = "#2a3036"; g.fillRect(35 + k * 125, 66, 26, 19); g.fillStyle = "#5b6670"; }
    } else if (kind === "produce" || kind === "flowers") {
      const colors = kind === "produce" ? ["#c23f2a", "#e0a030", "#6a9a3a", "#d9c04a", "#8a3a5a"] : ["#c96a8a", "#f2c94a", "#e8e0f0", "#b04a6a", "#f08a4a"];
      for (let r = 0; r < 3; r++) for (let k = 0; k < 9; k++) {
        g.fillStyle = "#7a5a3a"; g.fillRect(6 + k * 28, 44 + r * 28, 24, 20);
        g.fillStyle = pickOf(rng, colors);
        for (let b = 0; b < 4; b++) { g.beginPath(); g.arc(11 + k * 28 + b * 5, 44 + r * 28 + (b % 2) * 3, 4, 0, Math.PI * 2); g.fill(); }
      }
      lamp(64, 16); lamp(192, 16);
    } else if (kind === "racks") {
      g.fillStyle = "#3a3d42"; g.fillRect(0, 30, tw, 3);
      const colors = ["#2b2d31", "#6b2430", "#3d4f7a", "#c9b78a", "#4a3a2a"];
      for (let k = 0; k < 16; k++) { g.fillStyle = pickOf(rng, colors); g.fillRect(6 + k * 15, 33, 11, 40 + rng() * 20); }
      lamp(128, 14);
    } else { // rooms: a lived-in back room behind a ground-floor window that isn't a shop
      g.fillStyle = "#c9a878"; g.fillRect(0, 80, tw, 48);
      g.fillStyle = "#7a5a3a"; g.fillRect(40, 60, 70, 30); g.fillRect(160, 40, 36, 50);
      lamp(128, 30);
    }
    g.restore();
    rects[kind] = [x0 / c.width, 1 - (y0 + th) / c.height, tw / c.width, th / c.height];
  });
  return { map: texture(c), rects };
}

/** Blade-sign icons, light on transparent. */
function iconsAtlas() {
  const s = 128, names = [...new Set(KITS.map((k) => k.icon))];
  const [c, g] = canvas(s * names.length, s);
  const rects = {};
  names.forEach((name, i) => {
    g.save();
    g.translate(i * s + s / 2, s / 2);
    g.fillStyle = g.strokeStyle = "#f3ede0";
    g.lineWidth = 8;
    g.lineCap = g.lineJoin = "round";
    if (name === "cup") { g.fillRect(-30, -18, 46, 44); g.beginPath(); g.arc(22, 4, 14, -Math.PI / 2, Math.PI / 2); g.stroke(); }
    else if (name === "book") { g.fillRect(-36, -28, 32, 56); g.fillRect(4, -28, 32, 56); }
    else if (name === "glass") { g.beginPath(); g.moveTo(-24, -34); g.lineTo(24, -34); g.lineTo(0, 4); g.closePath(); g.fill(); g.fillRect(-3, 2, 6, 26); g.fillRect(-18, 26, 36, 6); }
    else if (name === "square") g.strokeRect(-28, -28, 56, 56);
    else if (name === "loaf") { g.beginPath(); g.ellipse(0, 6, 40, 22, 0, Math.PI, 0); g.lineTo(40, 20); g.lineTo(-40, 20); g.fill(); }
    else if (name === "apple") { g.beginPath(); g.arc(0, 8, 28, 0, Math.PI * 2); g.fill(); g.fillRect(-2, -32, 5, 16); }
    else if (name === "flower") { for (let k = 0; k < 6; k++) { g.beginPath(); g.arc(Math.cos(k) * 20, Math.sin(k) * 20 - 6, 13, 0, Math.PI * 2); g.fill(); } g.fillRect(-3, 10, 6, 34); }
    else if (name === "hammer") { g.fillRect(-30, -30, 60, 18); g.fillRect(-6, -14, 12, 50); }
    else if (name === "disc") { g.beginPath(); g.arc(0, 0, 36, 0, Math.PI * 2); g.fill(); g.fillStyle = "#1a1714"; g.beginPath(); g.arc(0, 0, 8, 0, Math.PI * 2); g.fill(); }
    else if (name === "scissors") { g.beginPath(); g.arc(-14, 20, 11, 0, Math.PI * 2); g.arc(14, 20, 11, 0, Math.PI * 2); g.stroke(); g.beginPath(); g.moveTo(-8, 10); g.lineTo(18, -34); g.moveTo(8, 10); g.lineTo(-18, -34); g.stroke(); }
    g.restore();
    rects[name] = [i / names.length, 0, 1 / names.length, 1];
  });
  return { map: texture(c), rects };
}

/**
 * One quad per window: the instance colour fills the middle (glass, or a door's
 * leaf) and a painted frame `FRAME_M` wide runs round the edge, so a window is
 * drawn once rather than a frame quad and a glass quad on top of it.
 */
function framedMaterial() {
  const m = new THREE.MeshBasicMaterial({ color: 0xffffff });
  m.onBeforeCompile = (shader) => {
    shader.uniforms.uFrame = { value: FRAME_M };
    shader.uniforms.uTrim = { value: new THREE.Color(FRAME_COLOR) };
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec2 vWin, vSize;")
      .replace("#include <begin_vertex>", `#include <begin_vertex>
        vWin = uv;
        vSize = vec2(length(instanceMatrix[0].xyz), length(instanceMatrix[1].xyz));`);
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", "#include <common>\nvarying vec2 vWin, vSize;\nuniform float uFrame;\nuniform vec3 uTrim;")
      .replace("#include <color_fragment>", `#include <color_fragment>
        vec2 d = min(vWin, 1.0 - vWin) * vSize;
        diffuseColor.rgb = mix(diffuseColor.rgb, uTrim, step(min(d.x, d.y), uFrame));`);
  };
  return m;
}

/** A material whose map is read through each instance's own rectangle of an atlas. */
function atlasMaterial(Kind, opts) {
  const m = new Kind(opts);
  m.onBeforeCompile = (shader) => {
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", "#include <common>\nattribute vec4 aUvRect;")
      .replace("#include <uv_vertex>", "#include <uv_vertex>\nvMapUv = vMapUv * aUvRect.zw + aUvRect.xy;");
  };
  return m;
}

// ---------------------------------------------------------------- instances, in chunks
// A unit box without its back (against the wall, local -z) and bottom faces,
// which nobody sees: 8 triangles instead of 12.
const BOX = (() => {
  const g = new THREE.BoxGeometry(1, 1, 1);
  const keep = g.groups.filter((_, i) => i !== 3 && i !== 5); // faces: +x, -x, +y, -y, +z, -z
  const index = keep.flatMap((grp) => Array.from(g.index.array.slice(grp.start, grp.start + grp.count)));
  g.setIndex(index);
  g.clearGroups();
  return g;
})();
const QUAD = new THREE.PlaneGeometry(1, 1);

class Parts {
  constructor(kinds) {
    this.kinds = kinds; // name -> { geometry, material, uv, reach }
    this.items = Object.fromEntries(Object.keys(kinds).map((k) => [k, { m: [], c: [], uv: [] }]));
  }
  add(kind, matrix, color, uv) {
    const it = this.items[kind];
    it.m.push(...matrix.elements);
    it.c.push(color.r, color.g, color.b);
    if (this.kinds[kind].uv) it.uv.push(...uv);
  }
  build(scene) {
    const meshes = [];
    for (const [name, it] of Object.entries(this.items)) {
      const kind = this.kinds[name];
      const n = it.c.length / 3;
      const byChunk = new Map(), size = CHUNK[name];
      for (let i = 0; i < n; i++) {
        const key = `${Math.floor(it.m[i * 16 + 12] / size)},${Math.floor(it.m[i * 16 + 14] / size)}`;
        if (!byChunk.has(key)) byChunk.set(key, []);
        byChunk.get(key).push(i);
      }
      for (const ids of byChunk.values()) {
        const geometry = kind.uv ? kind.geometry.clone() : kind.geometry;
        const im = new THREE.InstancedMesh(geometry, kind.material, ids.length);
        const col = new Float32Array(ids.length * 3), uv = kind.uv ? new Float32Array(ids.length * 4) : null;
        ids.forEach((id, k) => {
          for (let e = 0; e < 16; e++) im.instanceMatrix.array[k * 16 + e] = it.m[id * 16 + e];
          col.set(it.c.slice(id * 3, id * 3 + 3), k * 3);
          if (uv) uv.set(it.uv.slice(id * 4, id * 4 + 4), k * 4);
        });
        im.instanceColor = new THREE.InstancedBufferAttribute(col, 3);
        if (uv) geometry.setAttribute("aUvRect", new THREE.InstancedBufferAttribute(uv, 4));
        im.computeBoundingSphere();
        im.matrixAutoUpdate = false;
        im.userData.reach = kind.reach;
        im.userData.kind = name;
        if (kind.order) im.renderOrder = kind.order;
        scene.add(im);
        meshes.push(im);
      }
    }
    return meshes;
  }
}

// ---------------------------------------------------------------- building it
/**
 * Windows and storefronts from the detail table. Returns { meshes, shops,
 * update(camera) }; `shops` lists each storefront's kit and the point on the
 * pavement in front of its door, for people to visit.
 */
export function buildFrontages(scene, detail, { coarse = false, uses = null } = {}) {
  const paintOf = (k) => k.paints[0];
  const letters = lettersAtlas(), stripes = stripesAtlas(paintOf), insides = insidesAtlas(), icons = iconsAtlas();
  const lambert = new THREE.MeshLambertMaterial({ color: 0xffffff });
  const parts = new Parts({
    windows: { geometry: QUAD, material: framedMaterial(), reach: REACH.windows },
    glass: { geometry: QUAD, material: new THREE.MeshBasicMaterial({ color: 0xffffff }), reach: REACH.glass },
    display: { geometry: QUAD, material: atlasMaterial(THREE.MeshBasicMaterial, { color: 0xffffff, map: insides.map }), uv: true, reach: REACH.display },
    awnings: { geometry: QUAD, material: atlasMaterial(THREE.MeshLambertMaterial, { color: 0xffffff, map: stripes.map, side: THREE.DoubleSide }), uv: true, reach: REACH.awnings },
    mid: { geometry: BOX, material: lambert, reach: REACH.mid },
    detail: { geometry: BOX, material: lambert, reach: REACH.detail },
    near: { geometry: BOX, material: lambert, reach: REACH.near },
    letters: { geometry: QUAD, material: atlasMaterial(THREE.MeshBasicMaterial, { color: 0xffffff, map: letters.map, transparent: true, alphaTest: 0.35, depthWrite: false }), uv: true, reach: REACH.letters, order: 1 },
    icons: { geometry: QUAD, material: atlasMaterial(THREE.MeshBasicMaterial, { color: 0xffffff, map: icons.map, transparent: true, alphaTest: 0.35, side: THREE.DoubleSide }), uv: true, reach: REACH.icons },
  });

  // One wall's frame: x along it (to a viewer's right), y up, z out of the wall.
  const Y = new THREE.Vector3(0, 1, 0);
  const wallAt = (w) => {
    const W = detail.walls, o = w * 9;
    const A = new THREE.Vector3(W[o], 0, W[o + 1]), u = new THREE.Vector3(W[o + 2], 0, W[o + 3]), n = new THREE.Vector3(W[o + 4], 0, W[o + 5]);
    const right = new THREE.Vector3().crossVectors(Y, n); // = ±u; keeps every instance's basis right-handed
    const flip = right.dot(u) < 0 ? -1 : 1; // `along` runs with u; local x runs with `right`
    const basis = new THREE.Matrix4().makeBasis(right, Y, n);
    return { A, u, n, flip, basis, length: W[o + 6], public: (W[o + 7] & 1) === 1, courtyard: (W[o + 7] & 2) === 2, building: W[o + 8] };
  };
  const M = new THREE.Matrix4(), T = new THREE.Matrix4(), R = new THREE.Matrix4(), S = new THREE.Matrix4(), P = new THREE.Vector3();
  const col = new THREE.Color();
  // A part at (along, up, out) on wall `w`, sized (sx along, sy up, sz out), optionally tilted about the wall's x axis.
  const place = (w, along, up, out, sx, sy, sz, tilt = 0, yaw = 0) => {
    P.copy(w.A).addScaledVector(w.u, along).addScaledVector(w.n, out);
    P.y = up;
    M.makeTranslation(P.x, P.y, P.z).multiply(w.basis);
    if (yaw) M.multiply(R.makeRotationY(yaw));
    if (tilt) M.multiply(R.makeRotationX(tilt));
    return M.multiply(S.makeScale(sx, sy, sz));
  };
  const add = (kind, w, along, up, out, sx, sy, sz, color, uv, tilt, yaw) =>
    parts.add(kind, place(w, along, up, out, sx, sy, sz, tilt, yaw), typeof color === "string" ? col.set(color) : color, uv);

  // Which ground-floor openings a shop front replaces.
  const shopsByWall = new Map();
  for (const row of detail.shops) {
    const [wall, lo, hi] = row;
    if (!shopsByWall.has(wall)) shopsByWall.set(wall, []);
    shopsByWall.get(wall).push({ lo, hi, row });
  }
  const inShop = (wall, along) => (shopsByWall.get(wall) || []).some((s) => along > s.lo - 0.1 && along < s.hi + 0.1);

  // ---- every window and door that isn't part of a shop front
  const O = detail.openings;
  const walls = new Array(detail.nWalls);
  const glassCol = new THREE.Color(), lit = new THREE.Color("#f2c27a");
  // P9's floor uses, by building id (from the element table): a floor of work reads as
  // offices and studios, more of them lit, in cooler daylight than a home's lamps.
  const useOf = (w, floor) => (uses && uses.get(detail.strings[w.building]) || "")[floor];
  const office = new THREE.Color("#d9e8ee"), officeDim = new THREE.Color("#8fb0bd");
  for (let i = 0; i < detail.nOpenings; i++) {
    const o = i * 6;
    const wi = O[o], door = (O[o + 1] & 3) !== 0, floor = O[o + 1] >> 2; // kind: 0 window, 1 door, 2 P87 shop door
    const along = O[o + 2] / 100, bottom = O[o + 3] / 100, width = O[o + 4] / 100, height = O[o + 5] / 100;
    if (floor === 0 && inShop(wi, along)) continue;
    const w = walls[wi] || (walls[wi] = wallAt(wi));
    const r = rngFor(`${wi}:${i}`);
    const cy = bottom + height / 2;
    const framed = (color) => add("windows", w, along, cy, 0.03, width + 2 * FRAME_M, height + 2 * FRAME_M, 1, color);
    if (door) {
      framed(w.courtyard ? "#5a3a26" : "#3a2a1e"); // the leaf, framed
      add("glass", w, along, bottom + height * 0.72, 0.04, width * 0.55, height * 0.32, 1, "#d8b77a");
      continue;
    }
    if (useOf(w, floor) === "w") {
      framed(r() < 0.55 ? office : officeDim);
    } else if (floor === 0 && w.public) {
      // A ground-floor window onto a street or square: lit, a room you can see into.
      add("display", w, along, cy, 0.035, width, height, 1, col.setScalar(0.92), insides.rects.rooms);
    } else {
      glassCol.set(w.courtyard ? "#3d5260" : "#2f3e48").multiplyScalar(0.85 + 0.3 * r());
      framed(floor > 0 && r() < 0.14 ? lit : glassCol);
    }
  }

  // ---- the shop fronts (after the explorer's buildRun)
  const shops = [];
  const lastKit = new Map();
  for (const { row } of [...shopsByWall.values()].flat()) {
    const [wi, lo, hi, doorAt, doorWidth, , underArcade, p87] = row;
    const w = walls[wi] || (walls[wi] = wallAt(wi));
    const id = `${detail.strings[w.building] || w.building}:${wi}:${lo.toFixed(1)}`;
    const rng = rngFor(id);
    // Neighbours differ: no kit repeats within two doors along a wall.
    const recent = lastKit.get(wi) || [];
    const choices = KITS.map((_, k) => k).filter((k) => !recent.includes(k));
    const ki = choices[Math.floor(rng() * choices.length)];
    lastKit.set(wi, [ki, ...recent].slice(0, 2));
    const kit = KITS[ki];
    const paint = pickOf(rng, kit.paints), trim = kit.id === "studio" ? "#d9d6cf" : pickOf(rng, GHENT_TRIMS);
    const brick = pickOf(rng, GHENT_BRICKS);
    const bulk = kit.id === "studio" ? "plain" : pickOf(rng, ["panel", "panel", "brick"]);
    const pier = bulk === "brick" || rng() < 0.5 ? brick : paint;
    const awning = !!kit.stripes && !underArcade && rng() < 0.85; // a P119 arcade is shelter enough
    const awnTilt = 0.32 + rng() * 0.12;
    const L = hi - lo;
    const at = doorAt - lo;
    // Positions along the run (0..L) map to the wall's own `along`.
    const X = (x) => lo + x;
    const entryW = Math.min(2.4, Math.max(1.5, doorWidth + 0.5));
    const e0 = at - entryW / 2, e1 = at + entryW / 2;

    const nb = Math.max(1, Math.round(L / BAY));
    const bounds = Array.from({ length: nb + 1 }, (_, i) => (L * i) / nb);
    const spans = [];
    for (let i = 0; i < nb; i++) {
      const a = bounds[i], b = bounds[i + 1];
      if (b > e0 && a < e1) {
        if (e0 - a > 0.9) spans.push([a, e0 - 0.18]);
        if (b - e1 > 0.9) spans.push([e1 + 0.18, b]);
        continue;
      }
      spans.push([a, b]);
    }
    const piers = new Set(bounds.filter((x) => x <= e0 - 0.3 || x >= e1 + 0.3));
    piers.add(e0 - 0.18); piers.add(e1 + 0.18);
    for (const x of piers) {
      add("detail", w, X(x), ST / 2, 0.09, 0.36, ST, 0.22, pier);
      if (!awning) add("near", w, X(x), ST - 0.1, 0.18, 0.3, 0.26, 0.3, trim); // capitals, where no awning hides them
    }
    for (const [a0, b0] of spans) {
      const a = a0 + 0.2, b = b0 - 0.2, sw = b - a, cx = X((a + b) / 2);
      if (sw < 0.6) continue;
      add("detail", w, cx, 0.275, 0.06, sw, 0.55, 0.14, bulk === "brick" ? brick : paint); // bulkhead
      if (bulk === "panel") add("near", w, cx, 0.28, 0.14, sw - 0.2, 0.34, 0.02, trim);
      add("detail", w, cx, 0.58, 0.08, sw + 0.1, 0.07, 0.24, trim); // sill
      add("display", w, cx, (GLASS_TOP + 0.62) / 2, 0.035, sw, GLASS_TOP - 0.62, 1, col.setScalar(1), insides.rects[kit.inside]);
      if (sw > 2.4) add("near", w, cx, (GLASS_TOP + 0.62) / 2, 0.06, 0.07, GLASS_TOP - 0.62, 0.08, paint); // mullion
      add("detail", w, cx, GLASS_TOP + 0.04, 0.06, sw + 0.05, 0.08, 0.12, paint);
      add("glass", w, cx, ST - 0.26, 0.035, sw, 0.42, 1, "#4a3826"); // transom
      if (awning) {
        const d = 1.25;
        // The canvas: a quad laid along the slope (a plane faces +z; tipped by
        // -(90° - tilt) about x it faces up and out, falling away from the wall).
        add("awnings", w, cx, ST - 0.02 - Math.sin(awnTilt) * d / 2, 0.14 + Math.cos(awnTilt) * d / 2, sw + 0.15, d, 1,
          col.setScalar(1), stripes.rect(ki), awnTilt - Math.PI / 2);
        add("awnings", w, cx, ST - 0.02 - Math.sin(awnTilt) * d - 0.12, 0.14 + Math.cos(awnTilt) * d, sw + 0.15, 0.24, 1,
          col.setScalar(0.92), stripes.rect(ki)); // the valance
      }
    }
    // The entry, flush with the wall: a painted door with a glass light, side lights, a transom.
    const doorLeaf = Math.min(1.1, Math.max(0.9, doorWidth));
    add("mid", w, X(at), 1.12, 0.03, doorLeaf, 2.24, 0.06, paint);
    add("glass", w, X(at), 1.45, 0.065, doorLeaf * 0.66, 1.2, 1, "#d8b77a");
    add("near", w, X(at) + 0.36 * w.flip, 1.05, 0.08, 0.06, 0.06, 0.08, "#b8913f"); // brass
    add("detail", w, X(at), 2.3, 0.06, entryW, 0.1, 0.12, trim);
    add("glass", w, X(at), (2.36 + ST) / 2, 0.035, entryW, ST - 2.36, 1, "#4a3826");
    const side = (entryW - doorLeaf) / 2 - 0.08;
    if (side > 0.2) for (const sd of [-1, 1]) {
      add("display", w, X(at) + sd * (doorLeaf / 2 + 0.04 + side / 2), 1.2, 0.035, side, 2.0, 1, col.setScalar(1), insides.rects[kit.inside]);
    }
    // The sign band, its lettering, and the cornice over the whole run.
    add("mid", w, X(L / 2), ST + 0.28, 0.16, L - 0.1, 0.56, 0.12, kit.band || paint);
    const lr = letters.rects[ki];
    const lh = 0.36, lw = Math.min(L - 0.6, lh * lr.aspect);
    add("letters", w, X(L / 2), ST + 0.28, 0.225, lw, lw / lr.aspect, 1, "#ffffff", lr.uv);
    add("mid", w, X(L / 2), G - 0.04, 0.2, L + 0.3, 0.16, 0.42, trim);
    // A blade sign beside the door.
    const bx = X(Math.min(L - 0.3, e1 + 0.18));
    add("near", w, bx, 2.4, 0.72, 0.05, 0.05, 1.05, "#1d1b19");
    add("near", w, bx, 2.22, 0.25, 0.04, 0.4, 0.04, "#1d1b19");
    add("detail", w, bx, 2.08, 0.82, 0.03, 0.46, 0.62, paint);
    for (const sd of [-1, 1]) add("icons", w, bx + sd * 0.018 * w.flip, 2.08, 0.82, 0.5, 0.36, 1, "#ffffff", icons.rects[kit.icon], 0, sd * Math.PI / 2);
    // On the pavement in front: the kit's own props.
    const props = kit.props;
    const ahead = (x, out) => ({ x: X(x), out });
    if (props === "cafe" || props === "bistro") {
      const n = props === "bistro" ? 3 : 2;
      for (let k = 0; k < n; k++) {
        const p = ahead(((k + 0.5) * L) / n, 2.3 + (k % 2) * 0.3);
        if (Math.abs(p.x - X(at)) < 1.2) continue;
        add("near", w, p.x, 0.74, p.out, 0.72, 0.04, 0.72, props === "bistro" ? "#f2ece0" : "#e8e0cc");
        add("near", w, p.x, 0.37, p.out, 0.06, 0.72, 0.06, "#2a2622");
        for (const sd of [-1, 1]) {
          add("near", w, p.x + sd * 0.62, 0.45, p.out, 0.4, 0.04, 0.4, "#2a2622");
          add("near", w, p.x + sd * 0.8, 0.68, p.out, 0.04, 0.42, 0.4, "#2a2622");
        }
      }
    } else if (props === "books") {
      add("near", w, X(at) - 2.2 * w.flip, 0.55, 1.0, 1.4, 0.7, 0.6, "#5a3a1e");
      for (let k = 0; k < 10; k++) add("near", w, X(at) - (2.8 - k * 0.12) * w.flip, 1.03, 1.0, 0.09, 0.26, 0.2, pickOf(rng, ["#7a2430", "#2f4a3a", "#d9b46a", "#3d4f7a", "#a8643a", "#efe6d2"]));
    } else if (props === "crates" || props === "buckets") {
      const fill = props === "crates" ? ["#c23f2a", "#e0a030", "#6a9a3a", "#d9c04a"] : ["#c96a8a", "#f2c94a", "#e8e0f0", "#f08a4a"];
      for (let k = 0; k < 4; k++) {
        const x = X(at) + (k < 2 ? -1 : 1) * (1.6 + (k % 2) * 0.7);
        add("near", w, x, 0.25, 0.7, 0.6, 0.5, 0.45, props === "crates" ? "#7a5a3a" : "#5b6670");
        add("near", w, x, 0.55, 0.7, 0.52, 0.12, 0.38, pickOf(rng, fill));
      }
    } else if (props === "aframe") {
      for (const sd of [-1, 1]) add("near", w, X(at) + 1.4 * w.flip, 0.42, 1.6 + sd * 0.13, 0.55, 0.85, 0.03, "#2b2b2b", undefined, sd * 0.2);
    }
    if (rng() < 0.6) for (const sd of [-1, 1]) { // planters either side of the door
      const x = X(at) + sd * (entryW / 2 + 0.5);
      add("near", w, x, 0.25, 0.5, 0.5, 0.5, 0.5, "#6b5a46");
      add("near", w, x, 0.72, 0.5, 0.56, 0.48, 0.56, "#55704a");
    }
    const front = new THREE.Vector3().copy(w.A).addScaledVector(w.u, X(at)).addScaledVector(w.n, 1.6);
    shops.push({ kit: kit.id, door: front, wall: wi, p87: p87 === 1, building: detail.strings[w.building] || null });
  }

  const meshes = parts.build(scene);
  const scale = coarse ? 0.75 : 1;
  return {
    meshes, shops,
    /** Each chunk of each kind of part drawn only within its reach of the camera. */
    update(camera) {
      const c = camera.position;
      for (const m of meshes) {
        const s = m.boundingSphere;
        m.visible = c.distanceTo(s.center) - s.radius < m.userData.reach * scale;
      }
    },
  };
}
