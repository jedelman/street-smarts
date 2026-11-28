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
import { ALL_BUSINESSES, chooseBusiness, growFront, growKeep } from "./shopfronts.js";
import { nameFor, faceFor, lettered, cssFont, packSigns } from "./signs.js";
import { growYard } from "./yards.js";

// ---------------------------------------------------------------- the businesses
// Which business stands at each shop front, and how its front grows: the
// L-system in shopfronts.js. KITS is kept as a name for the list.
export const KITS = ALL_BUSINESSES;

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
const REACH = { windows: 900, glass: 260, display: 200, awnings: 150, mid: 150, letters: 130, detail: 75, near: 45, icons: 45,
  yard: 170, shrub: 140, cone: 160, post: 130, rail: 80, gable: 220 };
const CHUNK = { windows: 400, glass: 200, display: 200, awnings: 160, mid: 160, letters: 200, detail: 60, near: 50, icons: 120,
  yard: 120, shrub: 100, cone: 120, post: 100, rail: 60, gable: 160 };
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
    // Ghent's house fronts (tools/ghent/build.py): what the yard grammar grows from.
    yards: (table.yards && table.yards.version === 1 && table.yards.rows) || [],
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

/**
 * Every front's sign, lettered once: white glyphs on clear in a single-channel texture (signs.js packs them), so a
 * sign's ink is its instance colour and some seven hundred names cost a quarter of what RGBA would.
 * `signs`: [{ key, text, face }]. Returns { texture, rects: Map(key -> [u, v, w, h]) }.
 */
function signsAtlas(signs) {
  const [m, mg] = canvas(8, 8);
  const measure = (text, font) => { mg.font = font; return mg.measureText(text).width; };
  const packed = packSigns(signs, measure);
  const [c, g] = canvas(packed.width, packed.height);
  g.fillStyle = "#ffffff";
  g.textBaseline = "middle";
  g.textAlign = "left";
  const rects = new Map();
  packed.rows.forEach((r, i) => {
    const sgn = signs[i];
    g.font = cssFont(sgn.face, packed.px);
    g.fillText(lettered(sgn.text, sgn.face), r.x + packed.px * 0.2, r.y + r.h / 2 + 1);
    // Rows are written top-down and a DataTexture isn't flipped: the quad's top (v = 1) reads the row's top.
    rects.set(r.key, [r.x / packed.width, (r.y + r.h) / packed.height, r.w / packed.width, -r.h / packed.height]);
  });
  const rgba = g.getImageData(0, 0, packed.width, packed.height).data;
  const alpha = new Uint8Array(packed.width * packed.height);
  for (let i = 0; i < alpha.length; i++) alpha[i] = rgba[i * 4 + 3];
  const texture = new THREE.DataTexture(alpha, packed.width, packed.height, THREE.RedFormat, THREE.UnsignedByteType);
  texture.unpackAlignment = 1;
  texture.generateMipmaps = true;
  texture.minFilter = THREE.LinearMipmapLinearFilter;
  texture.magFilter = THREE.LinearFilter;
  texture.anisotropy = 4;
  texture.needsUpdate = true;
  return { texture, rects };
}

/** Sign letters: the atlas's red channel is their coverage; their colour is each instance's ink. */
function lettersMaterial(texture) {
  const m = new THREE.MeshBasicMaterial({ color: 0xffffff, map: texture, transparent: true, alphaTest: 0.35, depthWrite: false });
  m.onBeforeCompile = (shader) => {
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", "#include <common>\nattribute vec4 aUvRect;")
      .replace("#include <uv_vertex>", "#include <uv_vertex>\nvMapUv = vMapUv * aUvRect.zw + aUvRect.xy;");
    shader.fragmentShader = shader.fragmentShader.replace("#include <map_fragment>", "diffuseColor.a *= texture2D( map, vMapUv ).r;");
  };
  return m;
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
const INSIDES = ["counter", "shelves", "lamps", "desks", "bread", "produce", "flowers", "tools", "records", "racks", "rooms",
  "bottles", "bikes", "office", "models", "school", "salon", "dental", "gallery", "yoga", "barber", "cleaners"];
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
      flowers: "#f3e3d3", tools: "#e6dcc6", records: "#e4c89a", racks: "#ece3d3", rooms: "#e9cf9e",
      bottles: "#d9b483", bikes: "#e6e2d8", office: "#eef1f2", models: "#f6f5f1", school: "#fbf3d9", salon: "#f1e6e0",
      dental: "#e3f1f0", gallery: "#f7f5ef", yoga: "#efe2c8", barber: "#ece4d8", cleaners: "#eef0f2" }[kind];
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
    } else if (kind === "bottles") { // a wine shop: racks of bottles, lamplit
      for (let r = 0; r < 3; r++) {
        g.fillStyle = "#5a3a1e"; g.fillRect(0, 34 + r * 34, tw, 4);
        for (let x = 6; x < tw - 6; x += 9) { g.fillStyle = pickOf(rng, ["#3a1a1e", "#2a3a22", "#4a1f24", "#c9b37a"]); g.fillRect(x, 12 + r * 34, 6, 22); }
      }
      lamp(64, 10); lamp(192, 10);
    } else if (kind === "bikes") { // a bike shop: bikes hung on the wall, one on the stand
      g.strokeStyle = "#2a3036"; g.lineWidth = 3;
      for (let k = 0; k < 4; k++) { const x = 34 + k * 60; g.beginPath(); g.arc(x - 14, 40, 12, 0, Math.PI * 2); g.arc(x + 14, 40, 12, 0, Math.PI * 2); g.stroke(); g.beginPath(); g.moveTo(x - 14, 40); g.lineTo(x, 26); g.lineTo(x + 14, 40); g.stroke(); }
      g.fillStyle = "#8a2b1f"; g.fillRect(90, 96, 76, 6); g.fillStyle = "#5b6670"; g.fillRect(124, 80, 8, 30);
    } else if (kind === "office") { // an accountant's: desks, screens, a plant, a filing wall
      g.fillStyle = "#c9cfd3"; g.fillRect(0, 10, tw, 40);
      for (let k = 0; k < 8; k++) { g.fillStyle = "#e9edf0"; g.fillRect(6 + k * 31, 14, 26, 32); }
      g.fillStyle = "#8a7a62"; for (let k = 0; k < 2; k++) g.fillRect(20 + k * 120, 92, 96, 6);
      g.fillStyle = "#2a3036"; for (let k = 0; k < 2; k++) g.fillRect(48 + k * 120, 70, 30, 20);
      g.fillStyle = "#55704a"; g.beginPath(); g.arc(236, 78, 12, 0, Math.PI * 2); g.fill();
    } else if (kind === "models") { // architects: white models on a long table, drawings pinned up
      g.fillStyle = "#e4e2dc"; for (let k = 0; k < 6; k++) g.fillRect(10 + k * 41, 10, 32, 24);
      g.fillStyle = "#c9c4b8"; g.fillRect(0, 86, tw, 6);
      g.fillStyle = "#ffffff"; for (let k = 0; k < 7; k++) { const h = 10 + rng() * 22; g.fillRect(14 + k * 34, 86 - h, 16 + rng() * 10, h); }
      lamp(70, 6); lamp(186, 6);
    } else if (kind === "school") { // a little school: drawings in the window, small chairs, a rug
      for (let k = 0; k < 7; k++) { g.fillStyle = pickOf(rng, ["#d9772f", "#2f7a8a", "#f2c94a", "#c23f2a", "#5a8a3a", "#8a4a8a"]); g.fillRect(8 + k * 35, 10 + (k % 2) * 6, 26, 22); }
      g.fillStyle = "#e0a030"; g.beginPath(); g.arc(200, 28, 10, 0, Math.PI * 2); g.fill();
      g.fillStyle = "#7fb0c8"; g.fillRect(30, 100, 190, 20);
      for (let k = 0; k < 4; k++) { g.fillStyle = pickOf(rng, ["#d9772f", "#2f7a8a", "#5a8a3a"]); g.fillRect(40 + k * 46, 80, 18, 14); g.fillRect(44 + k * 46, 94, 3, 8); g.fillRect(54 + k * 46, 94, 3, 8); }
    } else if (kind === "salon") { // a hairdresser's: mirrors, chairs, a lamp over each
      for (let k = 0; k < 4; k++) {
        g.fillStyle = "#c9d6dc"; g.fillRect(14 + k * 61, 14, 40, 52);
        g.fillStyle = "#1a1714"; g.fillRect(20 + k * 61, 80, 28, 22); g.fillRect(32 + k * 61, 102, 4, 18);
        lamp(34 + k * 61, 8);
      }
    } else if (kind === "dental") { // a dentist's: the chair under its lamp, a cabinet, a framed certificate
      g.fillStyle = "#2f5d62"; g.beginPath(); g.moveTo(60, 104); g.lineTo(150, 104); g.lineTo(190, 74); g.lineTo(176, 68); g.lineTo(144, 92); g.lineTo(70, 92); g.closePath(); g.fill();
      g.fillStyle = "#9aa4ad"; g.fillRect(100, 104, 8, 18);
      g.strokeStyle = "#5b6670"; g.lineWidth = 3; g.beginPath(); g.moveTo(128, 0); g.lineTo(128, 26); g.lineTo(150, 40); g.stroke();
      lamp(150, 44);
      g.fillStyle = "#ffffff"; g.fillRect(200, 36, 46, 70); g.fillStyle = "#c9d6dc"; g.fillRect(204, 40, 38, 30);
      g.fillStyle = "#b8913f"; g.fillRect(16, 18, 30, 22); g.fillStyle = "#fbf6ec"; g.fillRect(19, 21, 24, 16);
    } else if (kind === "gallery") { // a gallery: white walls, three pictures under picture lights, a bench
      const art = [["#c23f2a", "#2b4a7a"], ["#e0a030", "#1a1714"], ["#5a8a3a", "#f2c94a"]];
      art.forEach(([a, b2], k) => {
        const x = 18 + k * 80, w = 56 + (k % 2) * 8;
        lamp(x + w / 2, 10);
        g.fillStyle = "#1a1714"; g.fillRect(x - 3, 22, w + 6, 56);
        g.fillStyle = a; g.fillRect(x, 25, w, 50);
        g.fillStyle = b2; g.fillRect(x + 8 + k * 6, 34, w * 0.45, 26);
      });
      g.fillStyle = "#6b5a46"; g.fillRect(70, 104, 116, 8); g.fillRect(80, 112, 6, 14); g.fillRect(170, 112, 6, 14);
    } else if (kind === "yoga") { // a yoga studio: a wooden floor, mats laid out, plants, soft light
      g.fillStyle = "#c9a878"; g.fillRect(0, 64, tw, 64);
      const mats = ["#6b8a5a", "#c96a5a", "#4a6b7a", "#d9b46a"];
      mats.forEach((c, k) => { g.fillStyle = c; g.beginPath(); g.moveTo(20 + k * 58, 120); g.lineTo(58 + k * 58, 120); g.lineTo(52 + k * 56, 74); g.lineTo(28 + k * 56, 74); g.closePath(); g.fill(); });
      g.fillStyle = "#3f6b35"; for (const x of [10, 238]) { g.beginPath(); g.arc(x, 44, 14, 0, Math.PI * 2); g.fill(); g.fillStyle = "#8a6b4a"; g.fillRect(x - 6, 54, 12, 12); g.fillStyle = "#3f6b35"; }
      lamp(128, 16);
    } else if (kind === "barber") { // a barbershop: chairs facing mirrors, a striped pole, a checked floor
      for (let x = 0; x < tw; x += 16) for (let y = 96; y < th; y += 16) { g.fillStyle = ((x + y) / 16) % 2 ? "#1a1714" : "#f4efe4"; g.fillRect(x, y, 16, 16); }
      for (let k = 0; k < 3; k++) {
        g.fillStyle = "#c9d6dc"; g.fillRect(30 + k * 80, 14, 50, 50);
        g.fillStyle = "#7a2430"; g.fillRect(40 + k * 80, 70, 30, 22); g.fillStyle = "#b8913f"; g.fillRect(52 + k * 80, 92, 6, 8);
        lamp(55 + k * 80, 8);
      }
      g.fillStyle = "#f4efe4"; g.fillRect(4, 20, 14, 70); g.fillStyle = "#b23a2e"; for (let y = 22; y < 88; y += 14) g.fillRect(4, y, 14, 5);
    } else if (kind === "cleaners") { // a dry cleaner's: a rail of pressed clothes in plastic, a counter, a bell
      g.fillStyle = "#5b6670"; g.fillRect(0, 16, tw, 4);
      for (let x = 6; x < tw - 6; x += 13) {
        g.fillStyle = pickOf(rng, ["#2b2d31", "#6b2430", "#3d4f7a", "#c9b78a", "#efe6d2"]); g.fillRect(x, 22, 10, 46 + rng() * 14);
        g.fillStyle = "rgba(255,255,255,0.45)"; g.fillRect(x - 1, 20, 12, 52 + rng() * 10);
      }
      g.fillStyle = "#8a7a62"; g.fillRect(0, 92, tw, 36); g.fillStyle = "#b8913f"; g.beginPath(); g.arc(200, 88, 6, Math.PI, 0); g.fill();
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
  const s = 128, names = [...new Set([...KITS.map((k) => k.icon), "bike", "scooter"])];
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
    else if (name === "comb") { g.fillRect(-36, -14, 72, 12); for (let k = 0; k < 9; k++) g.fillRect(-34 + k * 8, -2, 4, 22); }
    else if (name === "bottle") { g.fillRect(-12, -6, 24, 44); g.fillRect(-5, -36, 10, 32); }
    else if (name === "ledger") { g.strokeRect(-30, -36, 60, 72); for (let k = 0; k < 4; k++) g.fillRect(-20, -22 + k * 14, 40, 4); }
    else if (name === "setsquare") { g.beginPath(); g.moveTo(-34, 34); g.lineTo(34, 34); g.lineTo(-34, -34); g.closePath(); g.stroke(); }
    else if (name === "abc") { g.font = "800 44px 'Helvetica Neue', Arial, sans-serif"; g.textAlign = "center"; g.textBaseline = "middle"; g.fillText("ABC", 0, 2); }
    else if (name === "bike") {
      g.beginPath(); g.arc(-30, 16, 20, 0, Math.PI * 2); g.arc(30, 16, 20, 0, Math.PI * 2); g.stroke();
      g.beginPath(); g.moveTo(-30, 16); g.lineTo(-8, -14); g.lineTo(22, -14); g.lineTo(30, 16); g.moveTo(-8, -14); g.lineTo(2, 16); g.lineTo(22, -14); g.moveTo(-14, -24); g.lineTo(-2, -24); g.moveTo(22, -14); g.lineTo(18, -28); g.stroke();
    }
    else if (name === "scooter") { g.beginPath(); g.arc(-26, 26, 10, 0, Math.PI * 2); g.arc(26, 26, 10, 0, Math.PI * 2); g.fill(); g.fillRect(-28, 14, 56, 6); g.fillRect(20, -34, 6, 50); g.fillRect(10, -36, 26, 6); }
    else if (name === "tooth") { g.beginPath(); g.moveTo(-30, -26); g.quadraticCurveTo(0, -42, 30, -26); g.quadraticCurveTo(34, 4, 20, 36); g.lineTo(8, 36); g.lineTo(0, 8); g.lineTo(-8, 36); g.lineTo(-20, 36); g.quadraticCurveTo(-34, 4, -30, -26); g.fill(); }
    else if (name === "frame") { g.strokeRect(-34, -30, 68, 60); g.strokeRect(-22, -18, 44, 36); }
    else if (name === "lotus") { g.beginPath(); g.arc(0, -26, 9, 0, Math.PI * 2); g.fill(); g.beginPath(); g.moveTo(-34, 0); g.lineTo(0, -10); g.lineTo(34, 0); g.moveTo(0, -10); g.lineTo(0, 14); g.moveTo(-26, 30); g.lineTo(0, 14); g.lineTo(26, 30); g.stroke(); }
    else if (name === "pole") { g.save(); g.beginPath(); g.rect(-12, -38, 24, 76); g.clip(); for (let y = -60; y < 60; y += 18) { g.beginPath(); g.moveTo(-12, y); g.lineTo(12, y - 14); g.lineTo(12, y - 6); g.lineTo(-12, y + 8); g.closePath(); g.fill(); } g.restore(); g.strokeRect(-12, -38, 24, 76); }
    else if (name === "hanger") { g.beginPath(); g.moveTo(0, -30); g.quadraticCurveTo(12, -34, 10, -22); g.quadraticCurveTo(8, -14, 0, -12); g.lineTo(-36, 20); g.lineTo(36, 20); g.lineTo(0, -12); g.stroke(); }
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
// The yards' shapes: a whole box (it stands free), a rounded shrub, a cone, a post, and a gable (a triangular prism,
// its base on y = -0.5, apex at y = 0.5, depth along z).
const FULL_BOX = new THREE.BoxGeometry(1, 1, 1);
const SHRUB = new THREE.IcosahedronGeometry(0.5, 1);
const CONE = new THREE.ConeGeometry(0.5, 1, 8);
const POST = new THREE.CylinderGeometry(0.5, 0.5, 1, 10);
const GABLE = (() => {
  const v = [-0.5, -0.5, 0.5, 0.5, -0.5, 0.5, 0, 0.5, 0.5, -0.5, -0.5, -0.5, 0.5, -0.5, -0.5, 0, 0.5, -0.5];
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(v, 3));
  g.setIndex([0, 1, 2, 4, 3, 5, 1, 4, 5, 1, 5, 2, 3, 0, 2, 3, 2, 5]);
  g.computeVertexNormals();
  return g;
})();

/**
 * Railings, fences and lattice, drawn by the fragment shader: one quad per run, its pattern in the instance's
 * aUvRect ([pattern, bar spacing in metres, bar share, 0]; yards.js RAIL): 0 balusters, 1 iron pickets, 2 lattice,
 * 3 pickets, 4 Chinese Chippendale.
 */
function railMaterial() {
  const m = new THREE.MeshLambertMaterial({ color: 0xffffff, side: THREE.DoubleSide });
  m.onBeforeCompile = (shader) => {
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", "#include <common>\nattribute vec4 aUvRect;\nvarying vec2 vR, vSz;\nvarying vec4 vPat;")
      .replace("#include <begin_vertex>", `#include <begin_vertex>
        vR = uv; vPat = aUvRect;
        vSz = vec2(length(instanceMatrix[0].xyz), length(instanceMatrix[1].xyz));`);
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", "#include <common>\nvarying vec2 vR, vSz;\nvarying vec4 vPat;")
      .replace("#include <color_fragment>", `#include <color_fragment>
        float u = vR.x * vSz.x, v = vR.y * vSz.y, sp = max(vPat.y, 0.02), bw = vPat.z;
        int pat = int(vPat.x + 0.5);
        float bands = step(vSz.y - 0.07, v) + step(v, 0.06);
        float keep = 0.0;
        if (pat == 0) keep = max(bands, step(fract(u / sp), bw));
        else if (pat == 1) keep = max(step(vSz.y - 0.04, v) + step(v, 0.05) + step(abs(v - vSz.y * 0.82), 0.02), step(fract(u / sp), bw));
        else if (pat == 2) keep = max(step(fract((u + v) / sp), bw), step(fract((u - v) / sp), bw));
        else if (pat == 3) { float f = fract(u / sp); keep = step(f, bw) * step(v, vSz.y - 0.1 * abs(f / bw * 2.0 - 1.0)) + step(abs(v - vSz.y * 0.65), 0.03) + step(abs(v - 0.15), 0.03); }
        else { float f = fract(u / sp), y = clamp((v - 0.07) / max(vSz.y - 0.14, 0.01), 0.0, 1.0); keep = max(bands, max(step(abs(f - y), 0.05), max(step(abs(1.0 - f - y), 0.05), step(f, 0.04)))); }
        if (keep < 0.5) discard;`);
  };
  return m;
}

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
  const stripes = stripesAtlas(paintOf), insides = insidesAtlas(), icons = iconsAtlas();
  const lambert = new THREE.MeshLambertMaterial({ color: 0xffffff });
  const parts = new Parts({
    windows: { geometry: QUAD, material: framedMaterial(), reach: REACH.windows },
    glass: { geometry: QUAD, material: new THREE.MeshBasicMaterial({ color: 0xffffff }), reach: REACH.glass },
    display: { geometry: QUAD, material: atlasMaterial(THREE.MeshBasicMaterial, { color: 0xffffff, map: insides.map }), uv: true, reach: REACH.display },
    awnings: { geometry: QUAD, material: atlasMaterial(THREE.MeshLambertMaterial, { color: 0xffffff, map: stripes.map, side: THREE.DoubleSide }), uv: true, reach: REACH.awnings },
    mid: { geometry: BOX, material: lambert, reach: REACH.mid },
    detail: { geometry: BOX, material: lambert, reach: REACH.detail },
    near: { geometry: BOX, material: lambert, reach: REACH.near },
    letters: { geometry: QUAD, material: null, uv: true, reach: REACH.letters, order: 1 }, // set once the signs are lettered
    icons: { geometry: QUAD, material: atlasMaterial(THREE.MeshBasicMaterial, { color: 0xffffff, map: icons.map, transparent: true, alphaTest: 0.35, side: THREE.DoubleSide }), uv: true, reach: REACH.icons },
    yard: { geometry: FULL_BOX, material: lambert, reach: REACH.yard },
    shrub: { geometry: SHRUB, material: lambert, reach: REACH.shrub },
    cone: { geometry: CONE, material: lambert, reach: REACH.cone },
    post: { geometry: POST, material: lambert, reach: REACH.post },
    rail: { geometry: QUAD, material: railMaterial(), uv: true, reach: REACH.rail },
    gable: { geometry: GABLE, material: lambert, reach: REACH.gable },
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

  // Which ground-floor openings a shop front replaces. A row a pipeline typed (tools/ghent) names its business
  // and how it meets the street: a "storefront" replaces them, like a generated plan's; any other typology
  // keeps the building's own doors and windows, and the new use shows through them.
  const shopsByWall = new Map();
  for (const row of detail.shops) {
    const [wall, lo, hi] = row;
    const typology = row[8] || null, biz = row[9] ? ALL_BUSINESSES.find((b) => b.id === row[9]) || null : null;
    const keep = !!typology && typology !== "storefront";
    if (!shopsByWall.has(wall)) shopsByWall.set(wall, []);
    shopsByWall.get(wall).push({ lo, hi, row, typology, biz, keep, windows: [], door: null });
  }
  const shopAt = (wall, along) => (shopsByWall.get(wall) || []).find((s) => along > s.lo - 0.1 && along < s.hi + 0.1) || null;

  // ---- the yards and porches (Ghent's house fronts): each grown from its row, its front windows and a seed of its own
  const O = detail.openings;
  const walls = new Array(detail.nWalls);
  const yardOf = new Map(), frontWindows = new Map();
  for (const y of detail.yards || []) yardOf.set(y[0], y);
  if (yardOf.size) {
    for (let i = 0; i < detail.nOpenings; i++) {
      const o = i * 6;
      if (!yardOf.has(O[o]) || (O[o + 1] & 3) !== 0) continue;
      if (!frontWindows.has(O[o])) frontWindows.set(O[o], []);
      frontWindows.get(O[o]).push({ x: O[o + 2] / 100, y0: O[o + 3] / 100, y1: (O[o + 3] + O[o + 5]) / 100, w: O[o + 4] / 100, floor: O[o + 1] >> 2 });
    }
  }
  const grownYards = new Map();
  for (const [wi, y] of yardOf) {
    const w = walls[wi] || (walls[wi] = wallAt(wi));
    const row = { door: y[1], depth: y[2], lotLo: y[3], lotHi: y[4], form: y[5], first: y[6], floors: y[7], built: y[8], converted: y[9] === 1 };
    const g = growYard({ row, L: w.length, windows: frontWindows.get(wi) || [], rng: rngFor(`yard:${y[10]}`) });
    grownYards.set(wi, g);
    for (const q of g.parts) add(q.k, w, q.x, q.y, q.out, q.sx, q.sy, q.sz, q.color, q.tex, q.rx || 0, q.ry || 0);
  }

  // ---- every window and door that isn't part of a shop front
  const glassCol = new THREE.Color(), lit = new THREE.Color("#f2c27a");
  // P9's floor uses, by building id (from the element table): a floor of work reads as
  // offices and studios, more of them lit, in cooler daylight than a home's lamps.
  const useOf = (w, floor) => (uses && uses.get(detail.strings[w.building]) || "")[floor];
  const office = new THREE.Color("#d9e8ee"), officeDim = new THREE.Color("#8fb0bd");
  for (let i = 0; i < detail.nOpenings; i++) {
    const o = i * 6;
    const wi = O[o], door = (O[o + 1] & 3) !== 0, floor = O[o + 1] >> 2; // kind: 0 window, 1 door, 2 P87 shop door
    const along = O[o + 2] / 100, bottom = O[o + 3] / 100, width = O[o + 4] / 100, height = O[o + 5] / 100;
    const sh = floor === 0 ? shopAt(wi, along) : null;
    if (sh && !sh.keep) continue;
    const w = walls[wi] || (walls[wi] = wallAt(wi));
    if (sh && sh.keep && sh.biz) {
      // Kept where it is, and lit: through it, what the ground floor holds now.
      const cy0 = bottom + height / 2;
      if (door) {
        sh.door = { x: along - sh.lo, top: bottom + height };
        add("windows", w, along, cy0, 0.03, width + 2 * FRAME_M, height + 2 * FRAME_M, 1, sh.biz.paints[0]);
        add("glass", w, along, bottom + height * 0.72, 0.04, width * 0.55, height * 0.32, 1, "#f2c27a");
      } else {
        sh.windows.push({ x: along - sh.lo, y0: bottom, y1: bottom + height, w: width });
        add("windows", w, along, cy0, 0.03, width + 2 * FRAME_M, height + 2 * FRAME_M, 1, "#f2c27a");
        add("display", w, along, cy0, 0.036, width, height, 1, col.setScalar(1), insides.rects[sh.biz.inside] || insides.rects.rooms);
      }
      continue;
    }
    const r = rngFor(`${wi}:${i}`);
    const cy = bottom + height / 2;
    const framed = (color) => add("windows", w, along, cy, 0.03, width + 2 * FRAME_M, height + 2 * FRAME_M, 1, color);
    if (door) {
      const yd = grownYards.get(wi);
      framed(yd ? yd.doorColor : w.courtyard ? "#5a3a26" : "#3a2a1e"); // the leaf, framed
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
  // Each front's sign: its own name and face, from a stream of its own (so the front's shape is what it was), measured
  // now and lettered once every sign is known.
  const signs = new Map(), pendingLetters = [], usedNames = new Set();
  const [mc, mg] = canvas(8, 8);
  const REF_PX = 44;
  const aspectOf = (sgn) => { mg.font = cssFont(sgn.face, REF_PX); return (mg.measureText(lettered(sgn.text, sgn.face)).width + REF_PX * 0.4) / Math.round(REF_PX * 1.32); };
  const GOLD = "#d9b45f";
  const lastBiz = new Map();
  for (const sh of [...shopsByWall.values()].flat()) {
    const { row } = sh;
    const [wi, lo, hi, doorAt, doorWidth, , underArcade, p87] = row;
    const w = walls[wi] || (walls[wi] = wallAt(wi));
    const id = `${detail.strings[w.building] || w.building}:${wi}:${lo.toFixed(1)}`;
    const rng = rngFor(id);
    // The pipeline's business where it chose one; else ours, and neighbours differ: none repeats within two doors.
    const recent = lastBiz.get(wi) || [];
    const biz = sh.biz || chooseBusiness(rng, recent);
    lastBiz.set(wi, [biz.id, ...recent].slice(0, 2));
    const bi = KITS.indexOf(biz);
    const L = hi - lo, at = doorAt - lo;
    const X = (x) => lo + x; // positions along the run (0..L) map to the wall's own `along`
    const srng = rngFor(`${id}:sign`);
    // A name nobody else on the site has: individually owned shops, not a chain (redrawn, in a fixed order, so it holds).
    let text = nameFor(biz, srng);
    for (let k = 0; k < 12 && usedNames.has(text); k++) text = nameFor(biz, srng);
    usedNames.add(text);
    const sign = { key: id, text, face: faceFor(biz, srng) };
    signs.set(id, sign);
    const aspect = aspectOf(sign);
    const grown = sh.keep
      ? growKeep({ biz, L, door: sh.door || { x: at, top: 2.6 }, windows: sh.windows, rng, flip: w.flip })
      : growFront({ biz, L, at, doorWidth, rng, arcade: !!underArcade, flip: w.flip });
    const porch = grownYards.get(wi) && grownYards.get(wi).porch;
    for (const q of grown.parts) {
      if (porch && q.k === "awnings" && X(q.x) > porch.lo - 0.3 && X(q.x) < porch.hi + 0.3) continue; // under a porch roof
      const color = typeof q.color === "number" ? col.setScalar(q.color) : q.color;
      const tex = q.tex;
      if (tex && tex.atlas === "letters") {
        const lw = Math.min(q.cap || L - 0.6, q.sy * aspect);
        pendingLetters.push({ w, along: X(q.x), y: q.y, out: q.out, lw, lh: lw / aspect, color: q.gold ? GOLD : biz.ink, key: id });
        continue;
      }
      const rect = !tex ? undefined : tex.atlas === "insides" ? insides.rects[tex.name] : tex.atlas === "icons" ? icons.rects[tex.name] : stripes.rect(bi);
      add(q.k, w, X(q.x), q.y, q.out, q.sx, q.sy, q.sz, color, rect, q.rx, q.ry);
    }
    const kit = biz;
    const front = new THREE.Vector3().copy(w.A).addScaledVector(w.u, X(at)).addScaledVector(w.n, 1.6);
    shops.push({ kit: kit.id, name: sign.text, door: front, wall: wi, p87: p87 === 1, building: row[10] || detail.strings[w.building] || null, typology: sh.typology });
  }
  // Letter every sign, then hang each one.
  if (signs.size) {
    const atlas = signsAtlas([...signs.values()]);
    parts.kinds.letters.material = lettersMaterial(atlas.texture);
    for (const l of pendingLetters) add("letters", l.w, l.along, l.y, l.out, l.lw, l.lh, 1, l.color, atlas.rects.get(l.key));
  } else {
    delete parts.kinds.letters;
    delete parts.items.letters;
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
