// Walk/fly through a street-smarts model as a house sparrow.
//
// Performance first: the model arrives as ~15 meshes (see model.js), drawn
// with Lambert materials, no shadows, no environment map, no antialiasing,
// at a render scale the governor tunes to each device. Fun second, in the
// spirit of Dream of the Earth: a creature you are, a camera that moves like
// an operator, things in reach that glint and answer when touched, and a
// count of the patterns you've met.
import * as THREE from "three";
import { loadSite, waterTime } from "./model.js";
import { buildWorld, blobTexture, SUN_DIR } from "./world.js";
import { buildSparrow } from "./sparrow.js";
import { buildGlints, buildPigeons } from "./life.js";
import { buildBenches, buildPeople } from "./people.js";
import { buildTrees } from "./trees.js";
import { buildFrontages } from "./frontages.js";
import { buildLamps } from "./lamps.js";
import { seeThrough, solid, update as updateSee } from "./seethrough.js";
import { loadWalkMask } from "./walkmask.js";
import { loadSignFaces } from "./signs.js";
import { buildTram } from "./tram.js";
import { buildSculptures, hidePosts } from "./sculptures.js";
import { buildWetland } from "./wetland.js";
import { setupPanels, renderIterations, renderTours, setupHudMenu, setupPeek } from "./panels.js";
import { renderPlan } from "./sitemap.js";
import { sound } from "./sound.js";
import { smoothDamp, smoothDampAngle, angleTo, shotWeight, bankFor, fovFor, swingToward, distanceToShow } from "./camerawork.js";
import { createGovernor, median, scaleRange } from "./governor.js";
import { touchable, promptOf, answerOf, arrivalOf, patternsOf, PATTERNS } from "./answers.js";
import { makeVoices, pickOpinion, lineFor, questionLine, tidy } from "./chorus.js";
import { computeFlows, lineFor as flowLineFor, summaryLine as flowSummary } from "./flows.js";
import { buildFlowLines } from "./flowlines.js";

const $ = (id) => document.getElementById(id);
const params = new URLSearchParams(location.search);
// The site: its page names its bundle (public/sites/<id>/site.json), which says
// what the walk loads (the model and what's published beside it), its tours,
// and the modules that render its own panels. The walk is the same for every
// site; the page carries the site's words.
const SITE_URL = (document.querySelector('meta[name="street-smarts-site"]') || {}).content || "/sites/eastside-commons/site.json";
const SITE = await fetch(SITE_URL).then((r) => (r.ok ? r.json() : Promise.reject(new Error(`${r.status} for ${SITE_URL}`))))
  .catch((e) => { fail(`Couldn’t load this site (${e.message}). Try reloading.`); throw e; });
const fromSite = (url) => (url ? new URL(url, new URL(SITE_URL, location.href)).pathname : null);
// The site's model, unless the address asks for another (an archived iteration, one cluster).
const GLB_URL = params.get("glb") || fromSite(SITE.model);
// The plan's measurements, for panels like Money: published beside the model, or in the iteration's archive folder.
const NUMBERS_URL = GLB_URL === fromSite(SITE.model) ? fromSite(SITE.numbers)
  : /^\/archive\/.+\/(clean_baseline|model)\.glb$/.test(GLB_URL) ? GLB_URL.replace(/[^/]+\.glb$/, "numbers.json") : null;
const VIEWS_URL = SITE.views === false ? null : GLB_URL.replace(/\.glb$/, "_interior_views.json");
// The chorus on the whole site (one cluster is a piece of the same site): what passers-by say.
const CHORUS_URL = params.get("chorus") || fromSite(SITE.chorus);
// The build's showcase shots (the gallery): beside an archived model, or the site's own.
const SHOWCASE_URL = /^\/archive\/.+\/model\.glb$/.test(GLB_URL) ? GLB_URL.replace(/[^/]+\.glb$/, "showcase.json")
  : GLB_URL === fromSite(SITE.model) ? fromSite(SITE.showcase) : null;
const TOURS = Array.isArray(SITE.tours) ? SITE.tours : [];
const STATS = params.has("stats");
// Hero mode: embedded in the landing page. It runs the tour as attract mode and
// takes no input until the page hands it over ("Hop in"); see index.html's #hero.
const HERO = params.has("hero");
let active = !HERO;
document.body.classList.toggle("hero", HERO);
const toPage = (msg) => { if (HERO && parent !== window) parent.postMessage({ source: "street-smarts-walk", ...msg }, location.origin); };
const COARSE = matchMedia("(pointer: coarse)").matches;
const REDUCED = matchMedia("(prefers-reduced-motion: reduce)").matches;
const UP = new THREE.Vector3(0, 1, 0);
const clamp = THREE.MathUtils.clamp;

// ---------------------------------------------------------------- renderer
const canvas = $("c");
let renderer;
try {
  renderer = new THREE.WebGLRenderer({ canvas, antialias: false, stencil: false, powerPreference: "low-power" });
} catch (e) {
  fail("This needs WebGL, which this browser didn't offer.");
  throw e;
}
const range = scaleRange(devicePixelRatio, COARSE);
const governor = createGovernor(range);
renderer.setPixelRatio(governor.scale);
renderer.setSize(innerWidth, innerHeight);
const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(62, innerWidth / innerHeight, 0.05, 3000);
addEventListener("resize", () => {
  camera.aspect = innerWidth / innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(innerWidth, innerHeight);
});

function fail(msg) {
  const l = $("loading");
  l.classList.remove("hidden");
  l.innerHTML = `<p class="err">${msg}</p>`;
}
const setLoading = (msg, p) => { $("loading-msg").textContent = msg; if (p != null) toPage({ type: "progress", p }); };

// ---------------------------------------------------------------- the log
const MAX_LINES = COARSE ? 4 : 7, FADE_MS = COARSE ? 6500 : 9000;
function logLine(text, kind = "note") {
  const box = $("log");
  const line = document.createElement("div");
  line.className = `line ${kind}`;
  line.textContent = text;
  box.appendChild(line);
  while (box.children.length > MAX_LINES) box.firstChild.remove();
  setTimeout(() => line.classList.add("old"), FADE_MS);
  setTimeout(() => line.remove(), FADE_MS + 1500);
}

// ---------------------------------------------------------------- state
const bird = {
  pos: new THREE.Vector3(), vel: new THREE.Vector3(), heading: 0, turn: 0,
  air: false, onGround: false, speed: 0, under: null, groundAt: 0,
};
const keys = Object.create(null);
let site, world, sparrow, glints, pigeons, shadow, benches = [], benchSet = null, people = null, trees = null, frontages = null;
let walkMask = null; // where people walk, if the site has a mask (walkmask.js)
let shopTouchables = [];
let tram = null, tramStep = null, riding = false;
let sculptures = null; // bronze birds and cats on the markers (sculptures.js)
let wetland = null; // P64's ponds and marsh, dressed: cattails, lily pads, cypress knees, herons (wetland.js)
let panels = null, plan = null; // the flyout panels, the site map's image
let peek = null; // a panel offered in context (surface below)
let lamps = null; // streetlights (lamps.js)
let lastPlace = null; // the last place touched that has a card
let lastLot = null; // the last lot touched (a parcel), card or not: where Money and Day to day speak from
const rendered = new Set(); // the site's own panels, rendered the first time each opens
let flowModel = null, flowLines = null; // the flow overlay, built the first time it's switched on
let chorus = null, talk = null; // talk: { lines, k, speakers, who } while someone is talking
const voices = makeVoices(), heard = new Set();
let goal = null; // { point, el, stopDist, arrived(), stuckT, lastPos }
let tour = null; // { def, stops: [...], i, waitUntil }
let TOUR_LABEL = "Show me around"; // the tour button's own words, from the page
let focus = null, focusScore = Infinity;
let t = 0, lastInputT = 0, lastLookT = -10, hintGone = false;
let views = [], viewIndex = -1;
const met = new Set();
let present = [];

// ---------------------------------------------------------------- boot
(async function boot() {
  const facesReady = loadSignFaces(); // the signs' faces load alongside the model
  // Where people walk (Ghent's sidewalks and crossings), alongside the model too. Without it, they walk straight.
  const maskReady = SITE.walkmask ? loadWalkMask(fromSite(SITE.walkmask.url), SITE.walkmask).catch(() => null) : Promise.resolve(null);
  try {
    const [s, v] = await Promise.all([
      loadSite(GLB_URL, (p) => setLoading(p == null ? "Laying the streets…" : `Laying the streets… ${Math.round(p * 100)}%`, p)),
      VIEWS_URL ? fetch(VIEWS_URL).then((r) => (r.ok ? r.json() : [])).catch(() => []) : [],
    ]);
    site = s;
    views = Array.isArray(v) ? v : [];
    // A waypoint's place: the biggest building whose source it names.
    for (const w of site.waypoints ? site.waypoints.values() : []) {
      if (!w.source) continue;
      w.el = site.elements.filter((e) => e.source === w.source && e.group === "buildings" && e.center)
        .sort((a, b) => (b.area || 0) - (a.area || 0))[0] || null;
    }
  } catch (e) {
    fail(`Couldn’t load the model (${e && e.message ? e.message : "network error"}). Try reloading.`);
    return;
  }
  for (const m of site.meshes) { m.updateMatrix(); scene.add(m); }
  // With open water below the land (Ghent's Hague), the ground disc stays at the land's level and leaves the water open.
  let landY = Infinity;
  for (const m of site.meshes) if (![...m.userData.kinds].some((k) => /^(water|bank|bridge)$/.test(k))) landY = Math.min(landY, m.geometry.boundingBox.min.y);
  world = buildWorld(scene, site.bounds, site.water && isFinite(landY) ? { floorY: landY, holes: site.water.rings } : {});
  sparrow = buildSparrow();
  sparrow.root.scale.setScalar(SIZE);
  scene.add(sparrow.root);
  shadow = new THREE.Mesh(
    new THREE.PlaneGeometry(0.22 * SIZE, 0.22 * SIZE).rotateX(-Math.PI / 2),
    new THREE.MeshBasicMaterial({ map: blobTexture(), transparent: true, depthWrite: false, fog: false }),
  );
  shadow.renderOrder = 2;
  scene.add(shadow);
  glints = buildGlints(scene);
  // P171's trees, before anything that's placed around them (pigeons, benches, people).
  if (site.trees) {
    trees = buildTrees(scene, site.trees, site.elements, (x, z, fromY) => {
      const hit = castDownModel(x, fromY, z, fromY + 5);
      return hit ? hit.point.y : world.floorY;
    }, blobTexture());
    for (const tr of trees.list) {
      if (!tr.el) continue;
      tr.el.center = new THREE.Vector3(tr.x, tr.cy, tr.z);
      tr.el.box = new THREE.Box3(new THREE.Vector3(tr.x - tr.r, tr.ground, tr.z - tr.r), new THREE.Vector3(tr.x + tr.r, tr.top, tr.z + tr.r));
      tr.el.tree = tr;
    }
  }
  // The markers P126 and P61 put in the middle of things: a bronze bird or cat on a plinth, after Botero.
  {
    const els = site.elements.filter((e) => e.group === "activity" && /^activity_/.test(e.kind || "") && !/^activity_(transit|gateway)$/.test(e.kind) && e.center && !e.box.isEmpty());
    const seedOf = (s) => { let h = 2166136261; for (const ch of s || "") h = Math.imul(h ^ ch.charCodeAt(0), 16777619); return (h >>> 0) / 4294967296; };
    if (els.length) {
      hidePosts(site, els);
      sculptures = buildSculptures(scene, els.map((el, i) => ({ el, x: el.center.x, y: el.box.min.y, z: el.center.z, seed: seedOf(`${el.source}:${i}`) })));
    }
  }
  // P64's wetland: what grows and stands in it.
  wetland = buildWetland(scene, site, trees, (x, z) => {
    const hit = castDownModel(x, site.bounds.max.y + 20, z, site.bounds.max.y + 60);
    return hit ? { y: hit.point.y, el: site.elementOfHit(hit) } : null;
  }, { coarse: COARSE });
  // Every window, and storefronts at the doors that open onto a street or square.
  if (site.detail) await facesReady;
  if (site.detail) frontages = buildFrontages(scene, site.detail, { coarse: COARSE, uses: new Map(site.elements.filter((e) => e.uses).map((e) => [e.source, e.uses])) });
  // The streetlights a site's data places (Ghent's, from Dominion's records).
  if (site.detail && site.detail.lights.length) lamps = buildLamps(scene, site.detail.lights, { coarse: COARSE });
  // What stands up (buildings, roofs, canopies, trees, every front's parts and the lamps) thins out by its normals
  // where it comes between you and the camera (seethrough.js); the ground, streets and squares never do.
  const STANDING = new Set(["buildings", "canopies", "wall_niches", "context"]);
  for (const m of site.meshes) {
    const { groups, kinds } = m.userData;
    if (solid(m.material) && ([...groups].some((g) => STANDING.has(g)) || (kinds.size && [...kinds].every((k) => /^platform_(canopy|post)$/.test(k))))) seeThrough(m.material);
  }
  for (const m of [...(trees ? trees.meshes : []), ...(frontages ? frontages.meshes : []), ...(lamps ? lamps.meshes : [])]) if (solid(m.material)) seeThrough(m.material);
  // P34's light rail: a tram on the line (the line is data; the timetable is ours).
  if (site.transit.length) tram = buildTram(scene, site.transit[0]);
  pigeons = buildPigeons(scene, pigeonSpots());
  benches = placeBenches();
  benchSet = buildBenches(scene, benches);
  walkMask = await maskReady;
  people = populate();
  if (CHORUS_URL) fetch(CHORUS_URL).then((r) => (r.ok ? r.json() : null)).then((c) => { if (c && c.version === 1) chorus = c; }).catch(() => {});
  indexTouchables();
  present = [...new Set([...site.elements, ...shopTouchables].flatMap((e) => patternsOf(e)))].sort((a, b) => a - b);
  try { for (const n of JSON.parse(localStorage.getItem(metKey()) || "[]")) if (present.includes(n)) met.add(n); } catch { /* fresh */ }
  updateMet();
  spawn();
  director.crane();
  setupUI();
  $("loading").classList.add("hidden");
  if (params.has("debug")) window.__walk = { bird, camera, site, benches, people, trees, frontages, get lamps() { return lamps; }, get walkMask() { return walkMask; }, warp: warpTo, get sculptures() { return sculptures; }, get wetland() { return wetland; }, get tram() { return tram; }, get riding() { return riding; }, placeBird, get chorus() { return chorus; }, get talk() { return talk; }, eavesdrop, touch, tapAt, toggleFlows, get flows() { return flowLines && { model: flowModel, lines: flowLines }; },
    state() { return { air: bird.air, onGround: bird.onGround, y: +bird.pos.y.toFixed(2), route: goal && goal.route, up: intent.up, down: intent.down, move: +intent.move.length().toFixed(2), holdDownFor: +holdDownFor.toFixed(2) }; },
    goTo(p) { goal = { point: new THREE.Vector3(p.x, p.y, p.z), el: null, stopDist: 0.6, land: true, arrived: () => { window.__arrived = (window.__arrived || 0) + 1; } }; }, director, get goal() { return goal; }, get tour() { return tour; }, get focus() { return focus; }, governor };
  renderer.setAnimationLoop(frame);
  toPage({ type: "ready" });
})();

// A link to another place on the same page: fly there.
addEventListener("hashchange", () => {
  if (!site) return;
  const link = deepLinked();
  if (link) goToElement(link.el);
});

// The landing page drives a hero: hand over input, take it back, pause while scrolled away.
addEventListener("message", (e) => {
  if (!HERO || e.origin !== location.origin || !e.data || e.data.source !== "street-smarts-hero") return;
  const type = e.data.type;
  if (type === "activate" && !active) {
    active = true;
    document.body.classList.add("active");
    lastInputT = t;
    hintGone = false;
    $("hint").classList.remove("gone");
    setTimeout(hideHint, 9000);
  } else if (type === "deactivate" && active) {
    active = false;
    document.body.classList.remove("active");
    for (const k in keys) keys[k] = false;
    holdUp = holdDown = false;
    attractAt = t + 8;
  } else if (type === "pause" && site) {
    renderer.setAnimationLoop(null);
  } else if (type === "resume" && site) {
    last = performance.now();
    renderer.setAnimationLoop(frame);
  }
});

function metKey() { return `street-smarts-met:${GLB_URL}`; }

/** Start at the main gateway if there is one (where the paths enter), else on the open space nearest the middle. */
/**
 * A link to a place: #at=<source id> starts you there (the Ghent Streets explorer's #p=<gpin> works too, so its
 * old links land on the same lot). At a converted ground floor you stand at its door, facing it.
 */
function deepLinked() {
  const h = new URLSearchParams(location.hash.slice(1));
  const src = h.get("at") || (h.get("p") ? `parcel:${h.get("p")}` : null);
  if (!src) return null;
  const el = site.elements.filter((e) => e.source === src && e.group === "buildings" && e.center)
    .sort((a, b) => (b.area || 0) - (a.area || 0))[0] || site.elements.find((e) => e.source === src && e.center);
  if (!el) return null;
  const shop = frontages && frontages.shops.find((sh) => sh.building === src);
  if (!shop) return { el, door: null, out: null };
  // The front wall's own outward normal (the detail table's nx, nz).
  const W = site.detail && site.detail.walls, o = shop.wall * 9;
  const out = W ? new THREE.Vector3(W[o + 4], 0, W[o + 5]) : new THREE.Vector3(shop.door.x - el.center.x, 0, shop.door.z - el.center.z).normalize();
  return { el, door: shop.door, out };
}
function spawn() {
  const els = site.elements;
  const center = site.bounds.getCenter(new THREE.Vector3());
  const link = deepLinked();
  if (link) {
    const { el, door, out } = link;
    if (door) {
      // Back across the sidewalk from the door, square to the front, so it fills the view; at the door if that's
      // under a roof.
      const back = door.clone().addScaledVector(out, 4.5);
      const hit = castDownModel(back.x, site.bounds.max.y + 20, back.z, site.bounds.max.y + 60);
      const under = hit && site.elementOfHit(hit);
      placeBird(under && under.group === "buildings" ? door : back);
      bird.heading = Math.atan2(-out.x, -out.z);
    } else {
      bird.pos.copy(center);
      placeBird(approachOf(el));
      bird.heading = Math.atan2(el.center.x - bird.pos.x, el.center.z - bird.pos.z);
    }
    director.snapBehind();
    setTimeout(() => touch(el, { look: false }), 1600);
    return;
  }
  const wp = SITE.spawn && SITE.spawn.waypoint && site.waypoints && site.waypoints.get(SITE.spawn.waypoint);
  if (wp) {
    // Where the site says to start: a waypoint, facing what it looks at.
    placeBird(wp.stand);
    bird.heading = Math.atan2(wp.look.x - wp.stand.x, wp.look.z - wp.stand.z);
    director.snapBehind();
    return;
  }
  const gate = els.find((e) => e.kind === "activity_gateway" && e.group === "activity" && e.center);
  let at, face;
  if (gate) {
    const toward = new THREE.Vector3().subVectors(center, gate.center).setY(0).normalize();
    at = gate.center.clone().addScaledVector(toward, 1.6); // in reach: the gateway is the first thing that glints
    face = toward;
  } else {
    const open = els.filter((e) => e.group === "plazas" && e.center)
      .sort((a, b) => a.center.distanceToSquared(center) - b.center.distanceToSquared(center))[0];
    at = (open ? open.center : center).clone();
    face = new THREE.Vector3().subVectors(center, at).setY(0);
    if (face.lengthSq() < 1) face.set(0, 0, 1);
    face.normalize();
  }
  placeBird(at);
  bird.heading = Math.atan2(face.x, face.z);
  director.snapBehind();
}

function placeBird(at) {
  const hit = castDown(at.x, site.bounds.max.y + 5, at.z, site.bounds.max.y + 50, false); // start on the ground, not in a crown
  bird.pos.set(at.x, hit ? hit.point.y + 0.01 : world.floorY, at.z);
  bird.vel.set(0, 0, 0);
  bird.air = false;
}

// ---------------------------------------------------------------- collision helpers
const _ray = new THREE.Ray();
/**
 * The nearest hit of the model, the benches and the trees, with `el`, the
 * element it landed on (null for a bench, which carries `prop`).
 */
function firstHit(far, withTrees = true) {
  let best = site.bvh.raycastFirst(_ray, THREE.DoubleSide, 0, far);
  if (best) best.el = site.elementOfHit(best);
  const b = benchSet ? benchSet.bvh.raycastFirst(_ray, THREE.DoubleSide, 0, far) : null;
  if (b && (!best || b.distance < best.distance)) { b.prop = true; b.el = null; best = b; }
  const sc = sculptures ? sculptures.bvh.raycastFirst(_ray, THREE.DoubleSide, 0, far) : null;
  if (sc && (!best || sc.distance < best.distance)) { sc.el = sculptures.elementOfHit(sc); best = sc; }
  const tr = trees && withTrees ? trees.bvh.raycastFirst(_ray, THREE.DoubleSide, 0, far) : null;
  if (tr && (!best || tr.distance < best.distance)) { tr.el = trees.elementOfHit(tr); best = tr; }
  return best;
}
function castDown(x, y, z, far = 400, withTrees = true) {
  _ray.origin.set(x, y, z);
  _ray.direction.set(0, -1, 0);
  return firstHit(far, withTrees);
}
function castRay(origin, dir, far, withTrees = true) {
  _ray.origin.copy(origin);
  _ray.direction.copy(dir);
  return firstHit(far, withTrees);
}
/** Only the model (people plan their walks on it, and open sky means no roof). */
function castDownModel(x, y, z, far) {
  _ray.origin.set(x, y, z);
  _ray.direction.set(0, -1, 0);
  return site.bvh.raycastFirst(_ray, THREE.DoubleSide, 0, far);
}
/** Nothing of the model overhead (a canopy, an upper story). Trees don't count: they're overhead everywhere. */
function openSky(el, x = el.center.x, z = el.center.z) {
  const hit = castDownModel(x, site.bounds.max.y + 20, z, site.bounds.max.y + 60);
  const top = hit && site.elementOfHit(hit);
  return !!top && (top === el || (top.source != null && top.source === el.source));
}

// The bird's body is a capsule: radius R, from R to R + SEG above its feet.
// A plump sparrow, about 21 cm tall: real size reads as a speck from a flying camera.
const SIZE = 1.3;
const R = 0.055 * SIZE, SEG = 0.05 * SIZE, GRAVITY = 9.8;
const seg = new THREE.Line3(), cbox = new THREE.Box3(), triPt = new THREE.Vector3(), capPt = new THREE.Vector3();
const push = new THREE.Vector3(), newPos = new THREE.Vector3();
function collide(dt) {
  bird.pos.addScaledVector(bird.vel, dt);
  seg.start.set(bird.pos.x, bird.pos.y + R + SEG, bird.pos.z);
  seg.end.set(bird.pos.x, bird.pos.y + R, bird.pos.z);
  const bvhs = [site.bvh];
  if (benchSet) bvhs.push(benchSet.bvh);
  if (sculptures) bvhs.push(sculptures.bvh);
  if (trees) bvhs.push(trees.bvh);
  for (let pass = 0; pass < 2; pass++) for (const bvh of bvhs) {
    cbox.makeEmpty();
    cbox.expandByPoint(seg.start).expandByPoint(seg.end);
    cbox.min.addScalar(-R); cbox.max.addScalar(R);
    bvh.shapecast({
      intersectsBounds: (box) => box.intersectsBox(cbox),
      intersectsTriangle: (tri) => {
        const d = tri.closestPointToSegment(seg, triPt, capPt);
        if (d < R) {
          push.subVectors(capPt, triPt);
          if (push.lengthSq() < 1e-12) push.copy(UP); else push.normalize();
          seg.start.addScaledVector(push, R - d);
          seg.end.addScaledVector(push, R - d);
        }
      },
    });
  }
  newPos.set(seg.end.x, seg.end.y - R, seg.end.z);
  push.subVectors(newPos, bird.pos);
  let up = push.y > Math.abs(dt * bird.vel.y * 0.25) && push.y > 1e-5;
  bird.pos.copy(newPos);
  // The last floor, where nothing of the model is below: the land's level (the ground disc), or the water's where the
  // ground probe found water under you.
  const lastFloor = Math.max(world.groundY, Math.min(world.floorY, bird.groundAt ?? world.floorY));
  if (bird.pos.y < lastFloor) { bird.pos.y = lastFloor; up = true; }
  if (up) {
    if (bird.vel.y < 0) bird.vel.y = 0;
    return true;
  }
  if (push.lengthSq() > 1e-12) {
    push.normalize();
    bird.vel.addScaledVector(push, -push.dot(bird.vel));
  }
  return false;
}

// ---------------------------------------------------------------- moving
const intent = { move: new THREE.Vector3(), fast: false, up: false, down: false, hop: false };
const _fwd = new THREE.Vector3(), _right = new THREE.Vector3(), _tmp = new THREE.Vector3(), _tmp2 = new THREE.Vector3();
let padMove = { x: 0, y: 0 }, padLook = { x: 0, y: 0 }, padUp = false, padDown = false;
let holdUp = false, holdDown = false;
let wantHop = false; // edge-triggered: Space / A / ▲

function gatherIntent() {
  intent.move.set(0, 0, 0);
  if (!active) { intent.fast = intent.up = intent.down = intent.hop = false; wantHop = false; if (goal) steerToGoal(); return; }
  // Camera-relative: forward is away from the camera.
  _fwd.set(-Math.sin(director.yaw), 0, -Math.cos(director.yaw));
  _right.set(Math.cos(director.yaw), 0, -Math.sin(director.yaw));
  let f = 0, r = 0;
  if (keys.KeyW || keys.ArrowUp) f += 1;
  if (keys.KeyS || keys.ArrowDown) f -= 1;
  if (keys.KeyD || keys.ArrowRight) r += 1;
  if (keys.KeyA || keys.ArrowLeft) r -= 1;
  f += -padMove.y; r += padMove.x;
  intent.move.addScaledVector(_fwd, f).addScaledVector(_right, r);
  if (intent.move.lengthSq() > 1) intent.move.normalize();
  const manual = intent.move.lengthSq() > 0.01;
  intent.fast = !!(keys.ShiftLeft || keys.ShiftRight);
  intent.up = !!(keys.Space || holdUp || padUp);
  intent.down = !!(keys.KeyC || keys.ControlLeft || keys.KeyQ || holdDown || padDown);
  intent.hop = wantHop;
  wantHop = false;
  if (manual || intent.up || intent.down) {
    lastInputT = t;
    if (goal) goal = null;
    if (tour) endTour("You take it from here.");
  }
  if (!manual && goal) steerToGoal();
}

function takeOff() {
  if (bird.air) return;
  bird.air = true;
  bird.vel.y = Math.max(bird.vel.y, 2.8);
  sound.flap();
}

/**
 * The route to a goal, surveyed before flying it: whether the ground path is
 * clear enough to walk, and how high to fly to clear every roof and treetop
 * on the way (sampled straight down every 2.5 m), with room to spare.
 */
const CLEARANCE = 2.5;
function surveyRoute(g) {
  _tmp.subVectors(g.point, bird.pos);
  const horiz = Math.hypot(_tmp.x, _tmp.z);
  const dx = horiz > 1e-6 ? _tmp.x / horiz : 0, dz = horiz > 1e-6 ? _tmp.z / horiz : 0;
  // Walkable: level with the goal, and nothing in the way at sparrow height.
  _tmp2.set(dx, 0, dz);
  _tmp.set(bird.pos.x, bird.pos.y + 0.15, bird.pos.z);
  const walk = horiz < 30 && Math.abs(g.point.y - bird.pos.y) < 0.4 && !castRay(_tmp, _tmp2, Math.max(0.1, horiz - 0.3));
  let top = Math.max(g.point.y, bird.groundAt);
  const sky = site.bounds.max.y + 30;
  for (let d = 1.5; d < horiz - 1.5; d += 2.5) {
    const hit = castDown(bird.pos.x + dx * d, sky, bird.pos.z + dz * d, sky + 60);
    if (hit) top = Math.max(top, hit.point.y);
  }
  // Somewhere to come down: the goal itself, or, when something overhangs it
  // (a tree's crown, a canopy), the nearest clear ground beside it.
  let landing = g.point;
  const over = castDown(g.point.x, sky, g.point.z, sky + 60);
  if (over && over.point.y > g.point.y + 0.5) {
    search: for (const r of [2.5, 4, 6, 8]) {
      for (let k = 0; k < 12; k++) {
        const a = (k / 12) * Math.PI * 2, x = g.point.x + Math.cos(a) * r, z = g.point.z + Math.sin(a) * r;
        const h = castDown(x, sky, z, sky + 60);
        if (!h || Math.abs(h.point.y - g.point.y) > 0.35) continue;
        // Open ground all round (no wall within 0.6 m), so the way down doesn't slide along brick.
        const open = [[0.6, 0], [-0.6, 0], [0, 0.6], [0, -0.6]].every(([ox, oz]) => {
          const e = castDown(x + ox, sky, z + oz, sky + 60);
          return e && Math.abs(e.point.y - h.point.y) < 0.35;
        });
        if (open) { landing = new THREE.Vector3(x, h.point.y, z); break search; }
      }
    }
  }
  return { walk, cruise: top + CLEARANCE, landing, at: t };
}

function steerToGoal() {
  const g = goal;
  if (g.follow != null && people) g.point.copy(people.people[g.follow].pos); // a person walks on
  _tmp.subVectors(g.point, bird.pos);
  const horiz = Math.hypot(_tmp.x, _tmp.z);
  const reached = horiz < g.stopDist && (bird.air ? Math.abs(_tmp.y) < 1.2 : Math.abs(_tmp.y) < 0.6);
  if (reached && (!bird.air || g.stopDist > 0.6)) {
    goal = null;
    if (bird.air && g.land) { holdDownFor = 2.5; }
    if (g.arrived) g.arrived();
    return;
  }
  // Survey the way there now and then (it changes as you rise and as you go).
  if (!g.route || t - g.route.at > 0.6) g.route = surveyRoute(g);
  const route = g.route;
  // Stuck on the ground (a kerb too high, a bench): fly instead.
  g.stuckT = (g.stuckT || 0) + 1 / 60;
  if (!g.lastPos || g.lastPos.distanceToSquared(bird.pos) > 0.02) { g.lastPos = bird.pos.clone(); g.stuckT = 0; }
  const fly = !route.walk || g.stuckT > 0.8;
  if (fly && !bird.air) takeOff();
  // In the air, home on where to land; on the ground, on the goal.
  const aim = bird.air ? route.landing : g.point;
  _tmp.subVectors(aim, bird.pos);
  const aimHoriz = Math.hypot(_tmp.x, _tmp.z);
  const toward = _tmp2.set(_tmp.x, 0, _tmp.z).normalize();
  intent.move.copy(toward).multiplyScalar(Math.min(1, aimHoriz / (bird.air ? 5 : 1.2) + 0.15));
  intent.fast = horiz > (bird.air ? 30 : 12);
  if (bird.air) {
    // Coming down: only once the straight line to the goal is clear (so a
    // courtyard is entered from above, not through its wall).
    _tmp.set(bird.pos.x, bird.pos.y + 0.1, bird.pos.z);
    const toGoal = new THREE.Vector3().subVectors(aim, _tmp);
    const dist = toGoal.length();
    const clearDown = dist < 0.5 || !castRay(_tmp, toGoal.divideScalar(Math.max(dist, 1e-6)), Math.max(0.1, dist - 0.5));
    const descending = clearDown && aimHoriz < Math.max(4, (bird.pos.y - aim.y) * 1.6);
    if (descending) {
      intent.up = false;
      intent.down = bird.pos.y > aim.y + 0.3;
      if (aim !== g.point && aimHoriz < 0.8) holdDownFor = 0.6; // down beside the goal; walk the rest
    } else {
      // Climb to the surveyed height before pushing on, and never toward a wall.
      const below = bird.pos.y < route.cruise - 0.4;
      _tmp.set(bird.pos.x, bird.pos.y + 0.1, bird.pos.z);
      const wall = castRay(_tmp, toward, Math.min(aimHoriz, 4 + bird.speed * 0.6));
      const roof = (below || wall) && castRay(_tmp, UP, 1.4); // a canopy or an arcade overhead
      intent.up = (below || !!wall) && !roof;
      intent.down = !intent.up && !roof && bird.pos.y > route.cruise + 0.6;
      if (roof) {
        // Out from under it first: on toward the goal if that's open, else back the way you faced.
        intent.move.copy(toward).multiplyScalar(wall ? -0.7 : 0.8);
      } else if (wall) {
        intent.move.copy(toward).multiplyScalar(-0.15); // rise while easing off the brick, never into it
      } else if (below) intent.move.multiplyScalar(0.55);
    }
  }
  // No closer in six seconds: say hello from here and land.
  const progress = horiz + Math.max(0, route.cruise - bird.pos.y) * 0.5;
  if (g.best == null || progress < g.best - 0.3) { g.best = progress; g.bestT = t; }
  else if (t - g.bestT > 6) {
    goal = null;
    holdDownFor = 3;
    if (g.arrived) g.arrived();
  }
}

let holdDownFor = 0;
const before = new THREE.Vector3();
function physics(dt) {
  if (holdDownFor > 0) { holdDownFor -= dt; if (bird.air) intent.down = true; }
  const maxSpeed = bird.air ? (intent.fast ? 15 : 7.5) : (intent.fast ? 4.2 : 2.0);
  const resp = bird.air ? 2.6 : 10;
  const k = Math.min(1, resp * dt);
  bird.vel.x += (intent.move.x * maxSpeed - bird.vel.x) * k;
  bird.vel.z += (intent.move.z * maxSpeed - bird.vel.z) * k;
  if (bird.air) {
    const vyT = intent.up ? 3.4 : intent.down ? -4.5 : -0.55; // let go and you glide down
    bird.vel.y += (vyT - bird.vel.y) * Math.min(1, 3.5 * dt);
    if (intent.up && Math.random() < dt * 2) sound.flap();
  } else {
    bird.vel.y -= GRAVITY * dt;
    if (intent.hop) {
      if (bird.onGround) { bird.vel.y = 2.3; sound.hop(); }
      else takeOff(); // a second hop in the air takes flight
    } else if (intent.up && !bird.onGround && bird.vel.y < 0.5) takeOff();
  }
  before.copy(bird.pos);
  if (riding && tramStep) bird.pos.add(tramStep.delta); // carried along on the tram's roof
  const travel = bird.vel.length() * dt;
  const steps = Math.min(12, Math.max(2, Math.ceil(travel / (R * 0.7))));
  let grounded = false;
  for (let i = 0; i < steps; i++) grounded = collide(dt / steps) || grounded;
  // The tram's roof is somewhere to stand.
  riding = !!tram && bird.vel.y <= 0.3 && tram.onRoof(bird.pos);
  if (riding) {
    bird.pos.y = tram.roofY;
    if (bird.vel.y < 0) bird.vel.y = 0;
    grounded = true;
  }
  bird.onGround = grounded;
  // Up a kerb or a slab edge: the capsule can't climb 15–30 cm, so hop it.
  const wanted = Math.hypot(bird.vel.x, bird.vel.z) * dt;
  const moved = Math.hypot(bird.pos.x - before.x, bird.pos.z - before.z);
  if (!bird.air && wanted > 0.002 && moved < wanted * 0.4 && intent.move.lengthSq() > 0.01) stepUp();
  if (bird.air && grounded && bird.vel.y <= 0.05 && !intent.up) {
    bird.air = false;
    sparrow.land();
    sound.land();
  }
  // Keep inside the world and under the sky's lid.
  const c = site.bounds.getCenter(_tmp);
  const lim = Math.max(site.bounds.max.x - site.bounds.min.x, site.bounds.max.z - site.bounds.min.z) * 1.2 + 60;
  _tmp2.set(bird.pos.x - c.x, 0, bird.pos.z - c.z);
  if (_tmp2.length() > lim) { _tmp2.setLength(lim); bird.pos.x = c.x + _tmp2.x; bird.pos.z = c.z + _tmp2.z; }
  bird.pos.y = Math.min(bird.pos.y, site.bounds.max.y + 120);
  // Heading follows where you're going.
  const hs = Math.hypot(bird.vel.x, bird.vel.z);
  bird.speed = hs;
  if (hs > 0.15) {
    const target = Math.atan2(bird.vel.x, bird.vel.z);
    const d = angleTo(bird.heading, target);
    const step = d * Math.min(1, dt * (bird.air ? 4 : 12));
    bird.turn += (step / Math.max(dt, 1e-3) - bird.turn) * Math.min(1, dt * 6);
    bird.heading += step;
  } else bird.turn *= Math.max(0, 1 - dt * 6);
}

function stepUp() {
  _fwd.set(bird.vel.x, 0, bird.vel.z).normalize();
  const hit = castDown(bird.pos.x + _fwd.x * (R + 0.08), bird.pos.y + 0.42, bird.pos.z + _fwd.z * (R + 0.08), 0.44);
  if (!hit || Math.abs(hit.face.normal.y) < 0.6) return;
  const rise = hit.point.y - bird.pos.y;
  if (rise < 0.02 || rise > 0.4) return;
  bird.pos.y = hit.point.y + 0.004;
  bird.pos.addScaledVector(_fwd, 0.04);
  bird.vel.y = 0;
  sparrow.land();
  sound.hop();
}

// ---------------------------------------------------------------- ground underfoot, arrivals
let underKey = null, underSince = 0, lastArrival = "", lastArrivalT = -10;
let saidRiding = false;
function probeGround() {
  if (riding) {
    // On the tram: the shadow on its roof, and a word the first time.
    bird.groundAt = tram.roofY;
    bird.under = null;
    shadow.position.set(bird.pos.x, tram.roofY + 0.006, bird.pos.z);
    shadow.material.opacity = 1;
    shadow.scale.setScalar(1);
    if (!saidRiding && !bird.air) { saidRiding = true; logLine("Riding the tram.", "arrive"); }
    return;
  }
  saidRiding = false;
  const hit = castDown(bird.pos.x, bird.pos.y + 0.25, bird.pos.z, 400);
  bird.groundAt = hit ? hit.point.y : world.floorY;
  const el = hit && !hit.prop ? hit.el : null;
  bird.under = el;
  bird.onBench = !!(hit && hit.prop) && bird.pos.y - hit.point.y < 0.05;
  // The shadow sits on whatever is below, fading with height.
  const h = bird.pos.y - bird.groundAt;
  shadow.position.set(bird.pos.x, bird.groundAt + 0.006, bird.pos.z);
  shadow.material.opacity = clamp(1 - h / 6, 0, 1);
  shadow.scale.setScalar(1 + h * 0.15);
  shadow.visible = director.eyes < 0.5;
  // Stepping onto something new: say so, once it has settled.
  if (!bird.onGround || bird.air) return;
  const key = bird.onBench ? "bench" : el ? `${el.kind}|${el.source}` : null;
  if (key !== underKey) { underKey = key; underSince = t; return; }
  if ((!el && !bird.onBench) || t - underSince < 0.35 || underSince < 0) return;
  underSince = -1; // said
  if (el && !bird.onBench) meet(el);
  const line = bird.onBench ? "Up on a bench." : arrivalOf(el);
  if (line && (line !== lastArrival || t - lastArrivalT > 12)) {
    logLine(line, "arrive");
    lastArrival = line;
    lastArrivalT = t;
  }
}

// ---------------------------------------------------------------- patterns met
function meet(el) {
  if (!active) return; // the attract tour isn't you meeting anything
  for (const n of patternsOf(el)) {
    if (met.has(n) || !present.includes(n)) continue;
    met.add(n);
    try { localStorage.setItem(metKey(), JSON.stringify([...met])); } catch { /* not kept */ }
    logLine(`New: P${n} ${PATTERNS[n].name} (${met.size} of ${present.length})`, "new");
    sound.chime(n);
    updateMet();
  }
}
function updateMet() {
  if (!$("met-count")) return;
  $("met-count").textContent = `${met.size} / ${present.length}`;
  const list = $("met-list");
  list.innerHTML = "";
  for (const n of present) {
    const li = document.createElement("li");
    if (met.has(n)) li.innerHTML = `<b>P${n}</b> ${PATTERNS[n].name}<span>${PATTERNS[n].gist}</span>`;
    else { li.className = "unmet"; li.innerHTML = `<b>P${n}</b> <span>not met yet</span>`; }
    list.appendChild(li);
  }
}

// ---------------------------------------------------------------- reach, glints, the prompt
const CELL = 8;
const grid = new Map();
function indexTouchables() {
  const benchEls = benches.map((b) => ({
    kind: "bench", group: "props", bench: b, center: new THREE.Vector3(b.x, b.y + 0.48, b.z),
    box: new THREE.Box3(new THREE.Vector3(b.x - 0.8, b.y, b.z - 0.8), new THREE.Vector3(b.x + 0.8, b.y + 0.5, b.z + 0.8)),
  }));
  // Shop fronts: look in (a P87 shop names its pattern).
  const shopEls = (frontages ? frontages.shops : []).map((s) => ({
    group: "shops", kind: s.kit, name: s.name, p87: s.p87, source: s.building, facts: site.facts && s.building ? site.facts[s.building] || null : null,
    center: new THREE.Vector3(s.door.x, 1.2, s.door.z),
    box: new THREE.Box3(new THREE.Vector3(s.door.x - 1.2, 0, s.door.z - 1.2), new THREE.Vector3(s.door.x + 1.2, 2.6, s.door.z + 1.2)),
  }));
  shopTouchables = shopEls;
  for (const el of [...site.elements, ...benchEls, ...shopEls]) {
    if (!el.center || !(touchable(el) || el.kind === "bench")) continue;
    const b = el.box;
    for (let x = Math.floor(b.min.x / CELL); x <= Math.floor(b.max.x / CELL); x++)
      for (let z = Math.floor(b.min.z / CELL); z <= Math.floor(b.max.z / CELL); z++) {
        const k = `${x},${z}`;
        if (!grid.has(k)) grid.set(k, []);
        grid.get(k).push(el);
      }
  }
}
const REACH = { open: [0.6, 1.2], marker: [1.8, 3.2], canopy: [1.3, 3.4], garden: [0.9, 1.5], nook: [1.1, 2.6] };
const reachOf = (el) => (el.kind === "bench" ? [0.9, 0.6] : /^(plaza|common|pocket|undecided)$/.test(el.kind) ? REACH.open
  : el.kind === "roof_garden" ? REACH.garden : el.kind === "wall_niche" ? REACH.nook
  : el.kind.startsWith("canopy") ? REACH.canopy : REACH.marker);
const PERSON_REACH = 3;
const personEls = [];
/** A person as a touchable, one stable object each (focus is held by identity). */
function personEl(i) {
  const el = personEls[i] || (personEls[i] = { kind: "person", group: "people", person: i, center: new THREE.Vector3(), box: new THREE.Box3() });
  const p = people.people[i];
  el.center.set(p.pos.x, p.pos.y + 1.1, p.pos.z);
  el.box.min.set(p.pos.x - 0.3, p.pos.y, p.pos.z - 0.3);
  el.box.max.set(p.pos.x + 0.3, p.pos.y + 1.8, p.pos.z + 0.3);
  return el;
}
/** The nearest point of an element's own surface in the model to `p` (within 30 m), or null (a tree, a person). */
const _near = new THREE.Vector3();
function nearestOn(el, p, out) {
  const geo = site.bvh.geometry, idx = geo.index, elem = geo.attributes.element;
  let best = 30;
  let found = false;
  site.bvh.shapecast({
    intersectsBounds: (box) => box.distanceToPoint(p) < best,
    intersectsTriangle: (tri, i) => {
      if (elem.getX(idx.getX(i * 3)) !== el.index) return false;
      tri.closestPointToPoint(p, _near);
      const d = _near.distanceTo(p);
      if (d < best) { best = d; out.copy(_near); found = true; }
      return false;
    },
  });
  return found ? out : null;
}
function anchorOf(el, out) {
  if (el.kind === "person") return people.headOf(el.person, out).add(_tmp2.set(0, 0.3, 0));
  const b = el.box;
  if (/^(plaza|common|pocket|undecided)$/.test(el.kind)) {
    // A big space: glint where you are in it, near you.
    out.set(clamp(bird.pos.x, b.min.x + 0.3, b.max.x - 0.3), b.max.y + 0.35, clamp(bird.pos.z, b.min.z + 0.3, b.max.z - 0.3));
    if (out.distanceTo(bird.pos) < 0.6) out.copy(el.center).setY(b.max.y + 0.35);
    return out;
  }
  if (el.group === "activity") return out.set(el.center.x, b.max.y + 0.25, el.center.z);
  return out.copy(el.center).setY(b.max.y + 0.05);
}
function reachScan() {
  const cx = Math.floor(bird.pos.x / CELL), cz = Math.floor(bird.pos.z / CELL);
  const seen = new Set(), found = [];
  _fwd.set(Math.sin(bird.heading), 0, Math.cos(bird.heading));
  for (let x = cx - 1; x <= cx + 1; x++) for (let z = cz - 1; z <= cz + 1; z++) {
    for (const el of grid.get(`${x},${z}`) || []) {
      if (seen.has(el)) continue;
      seen.add(el);
      const b = el.box, [reach, vReach] = reachOf(el);
      const dx = Math.max(b.min.x - bird.pos.x, 0, bird.pos.x - b.max.x);
      const dz = Math.max(b.min.z - bird.pos.z, 0, bird.pos.z - b.max.z);
      const dh = Math.hypot(dx, dz);
      const dy = Math.max(b.min.y - bird.pos.y - 0.1, 0, bird.pos.y - b.max.y - 0.1);
      if (dh > reach || dy > vReach) continue;
      if (el.kind === "bench" && bird.onBench) continue;
      _tmp.subVectors(el.center, bird.pos).setY(0);
      const facing = _tmp.lengthSq() > 0.01 ? _tmp.normalize().dot(_fwd) : 1;
      const big = /^(plaza|common|pocket|undecided)$/.test(el.kind) ? 0.9 : 0;
      found.push({ el, score: dh + dy * 0.3 + big - facing * 0.3 });
    }
  }
  // People: eavesdrop on them.
  if (people) {
    for (let i = 0; i < people.people.length; i++) {
      const p = people.people[i];
      const dh = Math.hypot(p.pos.x - bird.pos.x, p.pos.z - bird.pos.z), dy = bird.pos.y - p.pos.y;
      if (dh > PERSON_REACH || dy < -0.6 || dy > 2.6) continue;
      const el = personEl(i);
      _tmp.subVectors(el.center, bird.pos).setY(0);
      const facing = _tmp.lengthSq() > 0.01 ? _tmp.normalize().dot(_fwd) : 1;
      // Someone right beside you is the likeliest thing you meant.
      found.push({ el, score: dh * 0.6 - 1.2 - facing * 0.5 });
    }
  }
  found.sort((a, b) => a.score - b.score);
  // Hold steady: keep the current focus unless something is clearly better.
  const cur = found.find((f) => f.el === focus);
  let next = found[0] ? found[0].el : null;
  if (cur && found[0] && cur.score < found[0].score + 0.35) next = focus;
  if (next !== focus) { focus = next; showPrompt(); }
  // Tapped something? Touch it the moment it's in reach, without waiting to get all the way there.
  if (goal && goal.autoTouch && found.some((f) => f.el === goal.el)) {
    const el = goal.el;
    goal = null;
    if (bird.air) holdDownFor = 1.2;
    if (focus !== el) { focus = el; showPrompt(); }
    touch(el);
  }
  const spots = [];
  for (const f of found.slice(0, 8)) {
    const a = anchorOf(f.el, new THREE.Vector3());
    spots.push({ x: a.x, y: a.y, z: a.z, focus: f.el === focus });
  }
  glints.set(spots, t, governor.scale);
}
function showPrompt() {
  const p = $("prompt");
  if (!focus) { p.hidden = true; return; }
  p.hidden = false;
  $("prompt-text").textContent = focus.kind === "bench" ? "Hop up on the bench"
    : focus.kind === "person" ? (talk ? "Listening…" : "Eavesdrop")
    : promptOf(focus);
}

// ---------------------------------------------------------------- touching things
let flare = 0, flareEl = null;
/** Touch an element: its words in the log, a chirp, and the camera turned to it unless `look` is false (a
 * waypoint or a link has already framed the view it wants). */
function touch(el, { look = true } = {}) {
  if (!el) { sparrow.chirp(); sound.chirp(); return; } // nothing in reach: a chirp anyway
  if (el.kind === "bench") { hopOnto(el.bench); return; }
  if (el.kind === "person") { eavesdrop(el.person); return; }
  lastInputT = t;
  const lines = answerOf(el);
  logLine(lines[0], "what");
  for (const l of lines.slice(1)) logLine(l, "pattern");
  if (el.group === "shops" && el.name) logLine(`On the sign: “${el.name}”. The name is ours.`, "note");
  if (el.facts && (el.facts.photo || el.facts.streetview || el.facts.case) && $("place-btn")) {
    // A place with a card (the assessor's photo, Street View, a case study): the Place panel shows it.
    lastPlace = el;
    $("place-btn").hidden = false;
  }
  if (el.source && el.source.startsWith("parcel:")) lastLot = el;
  // An open live panel follows what you touch (the Lot, the Money, Day to day).
  if (panels && panels.current && (SITE.live || []).includes(panels.current)) renderLive(panels.current);
  surface("touch", el);
  if (sculptures && sculptures.byElement.has(el)) {
    logLine(sculptures.byElement.get(el) === "cat"
      ? "On it, a bronze cat, sitting, round as a loaf, its tail wrapped round its paws: after Fernando Botero. Ours, not data."
      : "On it, a bronze dove, swollen and serene on tiny feet: after Fernando Botero. Ours, not data.", "note");
  }
  if (flowLines && flowLines.shown && el.source && el.group !== "shops") {
    const l = flowLineFor(flowModel, el.source);
    if (l) logLine(l, "note");
  }
  if (tram && (el.kind === "platform" || el.kind === "activity_transit")) {
    const wait = Math.round(tram.nextArrival(t));
    logLine(wait > 0 ? `The next tram is ${wait} seconds away.` : "The tram is in. Hop up on its roof.", "note");
  }
  meet(el);
  const k = el.kind;
  if (/^(plaza|common|pocket|undecided)$/.test(k) || k === "wall_niche") {
    sparrow.sit(true); sound.sit();
    setTimeout(() => { sparrow.chirp(); sound.chirp(); }, 700);
  } else if (k === "roof_garden") {
    sound.rustle();
    sparrow.peck();
    setTimeout(() => sparrow.peck(), 450);
    setTimeout(() => sparrow.peck(), 900);
  } else if (k.startsWith("activity_")) {
    sparrow.chirp(); sound.chirp();
    setTimeout(() => sound.chime(el.index), 180);
    flare = 1; flareEl = el;
  } else {
    sparrow.chirp(); sound.chirp();
  }
  if (el.center && look) director.frame(el);
}
function interact() { touch(focus); }

// ---------------------------------------------------------------- eavesdropping: the chorus, out loud
// Someone says what one of the opinions found (chorus.js); people talking in a
// group answer each other, a gripe for a love. Their lines are the opinions'
// own findings; only the openers ("Ugh.") are ours.
function eavesdrop(i) {
  lastInputT = t;
  if (talk) return; // still talking: keep listening
  if (!chorus) { logLine("They're talking, but you can't make out the words.", "note"); return; }
  const p = people.people[i];
  // What's around them: an opinion that looked at it is likelier to come up.
  const near = new Set();
  for (const e of site.elements) if (e.source && e.center && Math.abs(e.center.x - p.pos.x) < 45 && Math.abs(e.center.z - p.pos.z) < 45) near.add(e.source);
  const speakers = [i, ...people.groupOf(i).slice(0, 2)];
  // An opinion that names its answer (a chorus of objections and replies) is answered by it.
  const say = (who, against, want = null) => {
    const op = (want && chorus.opinions.find((o) => o.name === want)) || pickOpinion(chorus, { near, heard, against });
    heard.add(op.name);
    return { who, op, ...lineFor(op, { after: against, voice: chorus.voice }) };
  };
  const lines = [say(i, null)];
  if (speakers[1] != null) lines.push(say(speakers[1], lines[0].mood, lines[0].op.answer));
  if (speakers[2] != null) {
    const q = Math.random() < 0.4 ? questionLine(chorus) : null;
    lines.push(q ? { who: speakers[2], ...q } : say(speakers[2], lines[1].mood));
  }
  talk = { lines, k: -1, speakers, who: -1 };
  showPrompt();
  nextLine();
}
function nextLine() {
  if (!talk) return;
  if (++talk.k >= talk.lines.length) { endTalk(); return; }
  const line = talk.lines[talk.k];
  const others = talk.speakers.filter((j) => j !== line.who);
  const faceTo = others.length ? people.people[others[0]].pos : bird.pos; // alone, they tell the sparrow
  talk.who = line.who;
  $("bubble-text").textContent = line.text;
  $("bubble").hidden = false;
  logLine(`“${line.text}”`, "said");
  // The source, and (room permitting) the opinion's own words, so the plain version can be checked.
  logLine(line.plain && !COARSE ? `${line.cite}: “${tidy(line.finding, 110)}”` : line.cite, "pattern");
  const away = people.people[line.who].pos.distanceTo(camera.position);
  const ms = voices.say(line.speech, line.who * 7 + 3, {
    muted: sound.muted, volume: clamp(1.15 - away / 25, 0.25, 1),
    done: () => { if (!talk) return; people.speak(line.who, 0.2); setTimeout(nextLine, 380); },
  });
  people.speak(line.who, ms / 1000 + 1, faceTo);
  director.frame(personEl(line.who)); // you and whoever's talking, in one view
}
function endTalk() {
  if (talk && talk.who >= 0) people.speak(talk.who, 0);
  talk = null;
  $("bubble").hidden = true;
  showPrompt();
}
const _head = new THREE.Vector3();
/** Keeps the speech bubble over the speaker's head; ends the talk if you fly off. */
function placeBubble() {
  if (!talk || talk.who < 0) return;
  people.headOf(talk.who, _head);
  if (_head.distanceTo(bird.pos) > 30) { voices.stop(); endTalk(); return; }
  _head.y += 0.4;
  const far = _head.distanceTo(camera.position) > 45;
  camera.updateMatrixWorld();
  _head.project(camera);
  const b = $("bubble");
  if (far || _head.z > 1) { b.style.visibility = "hidden"; return; }
  b.style.visibility = "visible";
  // Off the edge of the frame (a tall talker over a small bird), it waits at the edge.
  const w = b.offsetWidth, h = b.offsetHeight;
  const x = clamp(((_head.x + 1) / 2) * innerWidth, w / 2 + 12, innerWidth - w / 2 - 12);
  const y = clamp(((1 - _head.y) / 2) * innerHeight, h + 64, innerHeight - 90);
  b.style.transform = `translate(${x}px, ${y}px) translate(-50%, -100%)`;
}

// ---------------------------------------------------------------- tap to go
const raycaster = new THREE.Raycaster();
const ndc = new THREE.Vector2();
function tapAt(x, y) {
  ndc.set((x / innerWidth) * 2 - 1, -(y / innerHeight) * 2 + 1);
  raycaster.setFromCamera(ndc, camera);
  _ray.copy(raycaster.ray);
  const hit = firstHit(2500);
  // A person under the tap (people aren't in the model's collider): go to them and eavesdrop.
  // A tap squarely on something to touch (a sculpture, a shop) wants that, unless it's right on someone.
  const OPEN = /^(plaza|common|pocket|undecided|local|pedestrian|arterial|street|meadow|track_bed|rail|marsh|pond|stream|boardwalk|bridge)(_grass)?$/;
  const onThing = !!(hit && hit.el && !OPEN.test(hit.el.kind || ""));
  const who = pickPerson(x, y, hit ? hit.distance : Infinity, onThing ? 14 : null);
  if (who >= 0) {
    if (tour) endTour("You take it from here.");
    lastInputT = t;
    const el = personEl(who);
    if (focus === el) { touch(el); return; }
    goal = { point: people.people[who].pos.clone(), el, follow: who, stopDist: 1.4, land: true, autoTouch: true, arrived: () => touch(personEl(who)) };
    pingAt(goal.point);
    return;
  }
  let point, el = null;
  if (hit) { point = hit.point.clone(); el = hit.el; }
  else {
    // The bare ground plane.
    const dy = raycaster.ray.direction.y;
    if (dy >= -1e-3) return;
    const s = (world.floorY - raycaster.ray.origin.y) / dy;
    point = raycaster.ray.at(s, new THREE.Vector3());
  }
  // Open water (Ghent's Hague): go to the nearest bank you can stand on, and look out over it from there.
  if (el && (el.kind === "water" || el.kind === "bank") && walkMask) {
    const shore = walkMask.nearest(point.x, point.z, 80);
    if (shore) {
      const h = castDownModel(shore.x, site.bounds.max.y + 20, shore.z, site.bounds.max.y + 60);
      point = new THREE.Vector3(shore.x, h ? h.point.y : world.floorY, shore.z);
    }
  }
  if (tour) endTour("You take it from here.");
  lastInputT = t;
  // Open ground (a square, a street) is somewhere to go; anything else is something to go and touch.
  const isOpen = !el || /^(plaza|common|pocket|undecided|local|pedestrian|arterial|street|meadow|track_bed|rail|bridge)(_grass)?$/.test(el.kind);
  if (el && el.tree) {
    // A tree: fly up and perch on top of its crown, then say what it is.
    const tr = el.tree;
    goal = { point: new THREE.Vector3(tr.x, tr.top, tr.z), el, stopDist: 0.6, land: true, arrived: () => touch(el) };
    pingAt(goal.point);
    return;
  }
  const target = el && el.group === "activity_glow" ? site.elements.find((e) => e.group === "activity" && e.source === el.source) || el : el;
  if (!isOpen && focus === target) { touch(target); return; } // already in reach
  goal = {
    point, el: target,
    stopDist: isOpen ? 0.25 : 1.0,
    land: true,
    autoTouch: !isOpen && !!target,
    arrived: () => { if (!isOpen) touch(target); },
  };
  pingAt(point);
}
/** The person whose head is nearest a tap on screen (within a fingertip), nearer than `wall`; -1 if none. */
const _headScreen = new THREE.Vector3();
function pickPerson(x, y, wall, tight = null) {
  if (!people) return -1;
  const slop = tight || (COARSE ? 46 : 30);
  let best = -1, bestD = slop;
  for (let i = 0; i < people.people.length; i++) {
    people.headOf(i, _headScreen);
    const dist = camera.position.distanceTo(_headScreen);
    if (dist > 140 || dist > wall + 1) continue;
    _headScreen.y -= 0.5; // aim at the chest: a tap anywhere on someone counts
    _headScreen.project(camera);
    if (_headScreen.z > 1) continue;
    const sx = (_headScreen.x + 1) / 2 * innerWidth, sy = (1 - _headScreen.y) / 2 * innerHeight;
    const d = Math.hypot(sx - x, sy - y);
    if (d < bestD) { bestD = d; best = i; }
  }
  return best;
}
const ping = { mesh: null, t: -10 };
function pingAt(p) {
  if (!ping.mesh) {
    ping.mesh = new THREE.Mesh(new THREE.RingGeometry(0.12, 0.16, 24).rotateX(-Math.PI / 2),
      new THREE.MeshBasicMaterial({ color: "#ffe2a0", transparent: true, depthWrite: false, fog: false }));
    ping.mesh.renderOrder = 3;
    scene.add(ping.mesh);
  }
  ping.mesh.position.copy(p).y += 0.02;
  ping.t = t;
}

// ---------------------------------------------------------------- the tour
/**
 * A tour from the site's bundle (site.json `tours`). Its stops are of three kinds:
 *   { pattern: n }      the nearest place, from the last stop, that pattern n made
 *                       (a generated plan's elements change with every render; its patterns don't);
 *   { element: "id" }   the element whose source record is that id (a real parcel, say);
 *   { waypoint: "id" }  a waypoint the model carries: where to stand and what to look at.
 * `max` caps the stops; `views` adds that many of the model's interior views at the end.
 */
function startTour(def = TOURS[0]) {
  if (!def) return;
  const stops = [];
  let from = bird.pos.clone();
  const used = new Set();
  const max = def.max || Infinity;
  for (const s of def.stops || []) {
    if (stops.length >= max) break;
    if (s.pattern != null) {
      const n = s.pattern;
      if (!present.includes(n)) continue;
      const cands = site.elements.filter((e) => e.center && patternsOf(e).includes(n)
        && (touchable(e) || e.group === "streets" || e.kind === "building_shaped" || e.kind === "umbrella")
        && (!/^(activity|plazas)$/.test(e.group) || openSky(e)));
      if (!cands.length) continue;
      cands.sort((a, b) => a.center.distanceToSquared(from) - b.center.distanceToSquared(from));
      const el = cands.find((c) => !used.has(c)) || cands[0];
      used.add(el);
      stops.push(el);
      from = el.center;
    } else if (s.element != null) {
      const el = site.elements.find((e) => e.source === s.element && e.center);
      if (el) { stops.push(el); from = el.center; }
    } else if (s.waypoint != null) {
      const w = site.waypoints && site.waypoints.get(s.waypoint);
      if (w) { stops.push({ waypoint: w, note: s.note }); from = w.stand; }
    }
  }
  if (views.length && def.views) stops.push(...views.slice(0, def.views).map((v, i) => ({ view: i })));
  if (!stops.length) return;
  tour = { def, stops, i: -1, waitUntil: 0 };
  $("tour").setAttribute("aria-pressed", "true");
  $("tour").textContent = "Stop the tour";
  if (active) logLine("Follow me.", "note");
  nextStop();
}
function nextStop() {
  tour.i++;
  if (tour.i >= tour.stops.length) { endTour("That’s the tour. Fly on."); return; }
  const s = tour.stops[tour.i];
  if (s.view != null) { peekInside(s.view); tour.waitUntil = t + 5; goal = null; return; }
  if (s.waypoint) {
    // Stand where the waypoint says, face what it looks at, and touch its element if it has one.
    const w = s.waypoint;
    goal = { point: w.stand.clone(), el: w.el || null, stopDist: 0.8, land: true, arrived: () => {
      bird.heading = Math.atan2(w.look.x - bird.pos.x, w.look.z - bird.pos.z);
      if (w.el) touch(w.el, { look: false }); else if (w.title) logLine(w.title, "note");
      if (s.note) logLine(s.note, "note");
      tour.waitUntil = t + (w.wait || 4.5);
    } };
    tour.waitUntil = Infinity;
    return;
  }
  const el = s;
  goal = { point: approachOf(el), el, stopDist: 1.0, land: true, arrived: () => { touch(el); tour.waitUntil = t + 3.2; } };
  tour.waitUntil = Infinity;
}
/** Where to go to meet an element: a building's foot on the side nearest you, else its anchor or middle. */
function approachOf(el) {
  const p = el.box.max.y > 1.5 && el.kind !== "building_shaped" ? anchorOf(el, new THREE.Vector3()) : el.center.clone();
  if (el.kind === "building_shaped") {
    const b = el.box;
    p.set(clamp(bird.pos.x, b.min.x - 1.2, b.max.x + 1.2), b.min.y, clamp(bird.pos.z, b.min.z - 1.2, b.max.z + 1.2));
  }
  return p;
}
/** A live panel (site.json `live`): its module renders afresh each time it opens, and when what it shows changes. */
function renderLive(name) {
  const mod = SITE.renderers && SITE.renderers[name];
  const body = $(`${name}-body`);
  if (!mod || !body) return;
  import(fromSite(mod)).then((m) => m.render(body, panelCtx()))
    .catch((e) => { body.textContent = `This panel couldn’t load (${e.message}).`; });
}
/** What a site's panel module is told: the site, the last place touched with a card, where you are, and how to go places. */
function panelCtx() {
  return { site: SITE, elements: site.elements, place: lastPlace, lot: lastLot, here: { x: bird.pos.x, z: bird.pos.z }, flyTo, goToElement, warp: warpTo,
    showcase: SHOWCASE_URL, shops: frontages ? frontages.shops : [] };
}

// ---------------------------------------------------------------- panels, offered in context
// A site's panel module may export peek(ctx): one line on what its panel has to say about what you just touched
// (ctx.touched, ctx.reason "touch") or where you've got to (ctx.reason "near"), or null. The first to answer, in the
// order of the site's `surface` list, leads a small card and the rest follow as chips (panels.js setupPeek); tapping
// opens the panel. Nothing opens by itself; a touch always asks, but a place you walk past offers itself only once.
const peekedNear = new Set();
let peekAsk = 0, nearAt = null, nearT = 0;
async function surface(reason, touched = null) {
  if (!peek || HERO || !Array.isArray(SITE.surface)) return;
  const ask = ++peekAsk;
  const ctx = { ...panelCtx(), touched, reason };
  const said = await Promise.all(SITE.surface.map(async (name) => {
    const mod = SITE.renderers && SITE.renderers[name];
    const btn = document.querySelector(`[data-open="${name}"]`);
    if (!mod || !btn) return null;
    try {
      const m = await import(fromSite(mod));
      const r = m.peek ? await m.peek(ctx) : null;
      return r && r.text ? { ...r, panel: name, title: btn.textContent.trim() } : null;
    } catch { return null; }
  }));
  if (ask !== peekAsk) return; // something newer asked meanwhile
  let items = said.filter((r) => r && r.panel !== (panels && panels.current));
  if (reason === "near") {
    items = items.filter((r) => !peekedNear.has(r.key)).slice(0, 1);
    for (const r of items) peekedNear.add(r.key);
  }
  if (items.length) peek.show(items);
}
/** Now and then while you go about: has where you've got to something to offer? */
function surfaceNear() {
  if (!peek || t - nearT < 1.2) return;
  nearT = t;
  if (nearAt && Math.hypot(bird.pos.x - nearAt.x, bird.pos.z - nearAt.z) < 5) return;
  nearAt = { x: bird.pos.x, z: bird.pos.z };
  if (!bird.air || bird.speed < 3) surface("near");
}
/** Fly to an element and touch it on arrival (a panel's list of places, say). */
function goToElement(el) {
  if (!el || !el.center) return;
  if (panels) panels.close();
  if (tour) endTour("");
  goal = { point: approachOf(el), el, stopDist: 1.0, land: true, autoTouch: true, arrived: () => touch(el) };
}
let attractAt = 4.8; // after the crane
/** The tour button, T and the pad: stop a running tour; else start the site's one tour, or offer its several. */
function tourButton() {
  if (tour) { endTour("You take it from here."); return; }
  if (TOURS.length > 1 && panels) panels.open("tours");
  else startTour();
}
function tourTick() {
  if (HERO && !active && !tour && !REDUCED && t > attractAt) startTour();
  if (!tour) return;
  if (!goal && t > tour.waitUntil) nextStop();
}
function endTour(msg) {
  tour = null;
  goal = null;
  $("tour").setAttribute("aria-pressed", "false");
  $("tour").textContent = TOUR_LABEL;
  if (msg && active) logLine(msg, "note");
  if (!active) attractAt = t + 4; // go round again
}

// ---------------------------------------------------------------- inside (cluster interiors)
function parseMeters(s) { const [x, y, z] = s.trim().split(/\s+/).map(parseFloat); return new THREE.Vector3(x, y, z); }
function peekInside(i) {
  const v = views[i];
  if (!v) return;
  viewIndex = i;
  const target = parseMeters(v.camera_target);
  const fade = $("fade");
  fade.classList.add("on");
  setTimeout(() => {
    const hit = castDown(target.x, target.y + 0.5, target.z, 6);
    bird.pos.set(target.x, hit ? hit.point.y + 0.01 : target.y - 1.5, target.z);
    bird.vel.set(0, 0, 0);
    bird.air = false;
    const orbit = v.camera_orbit.trim().split(/\s+/).map(parseFloat);
    bird.heading = THREE.MathUtils.degToRad(orbit[0]) + Math.PI;
    director.snapBehind();
    logLine("Inside, on the ground floor.", "arrive");
    fade.classList.remove("on");
  }, 350);
}

// ---------------------------------------------------------------- the camera director
const director = (() => {
  let yaw = 0, yawT = 0, yawV = 0, pitch = 0.3, pitchT = 0.3, pitchV = 0;
  let zoom = 1, dist = 1.2, distV = 0, fov = 62, fovV = 0, roll = 0, rollV = 0, eyes = 0;
  // Framing what you touched: one swing of at most 30°, slow, then back away until it's on screen (frame below).
  const SWING = THREE.MathUtils.degToRad(30), FRAME_MAX = 24;
  // No swing the camera makes by itself (following you round, framing, a shot) turns it faster than this: about
  // 34° a second, for comfort. Your own drag isn't held to it. One number, so it's easy to tune.
  const SWING_RATE = 0.6;
  const viewQ = new THREE.Quaternion(), wantQ = new THREE.Quaternion();
  let viewHeld = false;
  let hold = null, yawSlowUntil = 0, distSlowUntil = 0, lastBase = 1.15;
  const look = new THREE.Vector3(), lookV = new THREE.Vector3(), lookT = new THREE.Vector3();
  const cam = new THREE.Vector3(), head = new THREE.Vector3(), dir = new THREE.Vector3(), eye = new THREE.Vector3();
  let shot = null;
  const damp3 = (v, vv, target, st, dt) => {
    for (const c of ["x", "y", "z"]) [v[c], vv[c]] = smoothDamp(v[c], target[c], vv[c], st, dt);
  };
  const play = (frame, { dur = 3, blendIn = 0.6, blendOut = 1, start = false } = {}) => { shot = { t0: t, dur, blendIn, blendOut, frame, start }; };
  return {
    get yaw() { return yaw; },
    /** Whether a shot (the opening crane, the flows' overview) is playing: the showcase's captures wait it out. */
    get shooting() { return !!shot; },
    get eyes() { return eyes; },
    snapBehind() {
      yaw = yawT = bird.heading + Math.PI;
      look.copy(bird.pos).y += 0.1;
      lookV.set(0, 0, 0);
    },
    drag(dx, dy) {
      yawSlowUntil = 0; // your hand, not a swing: it answers at once
      yawT -= dx * 0.006;
      pitchT = clamp(pitchT + dy * 0.004, bird.air ? -0.6 : -0.12, 1.35);
      lastLookT = t; lastInputT = t;
    },
    zoomBy(f) {
      if (hold) { zoom = clamp(hold.dist / lastBase, 0.3, 45); hold = null; } // pinch from where the framing left you
      zoom = clamp(zoom * f, 0.3, 45); lastInputT = t;
    },
    zoomTo(z) { hold = null; zoom = clamp(z, 0.3, 45); lastInputT = t; },
    /** Set the tilt outright (a warp's), in radians, within a drag's limits: the same view from wherever you were. */
    pitchTo(p) { pitch = pitchT = clamp(p, bird.air ? -0.6 : -0.12, 1.35); pitchV = 0; },
    /** Swoop down from above the whole site to the bird. */
    crane() {
      if (REDUCED) return;
      const b = site.bounds, c = b.getCenter(new THREE.Vector3()), s = b.getSize(new THREE.Vector3());
      const from = new THREE.Vector3(c.x + s.x * 0.45, b.max.y + Math.max(s.x, s.z) * 0.45, c.z + s.z * 0.55);
      play(() => ({ pos: from, look: c, fov: 55 }), { dur: 4.2, blendIn: 0, blendOut: 3.4, start: true });
    },
    /** Rise over `b` (a Box3: the flows' arcs) and hold there a while, then come back. */
    overview(b) {
      if (REDUCED || !b || b.isEmpty()) return;
      const c = b.getCenter(new THREE.Vector3()), s = b.getSize(new THREE.Vector3()), span = Math.max(s.x, s.z);
      c.y = b.min.y;
      const from = new THREE.Vector3(c.x - span * 0.08, b.max.y + span * 0.35, c.z + span * 0.62);
      play(() => ({ pos: from, look: c, fov: 55 }), { dur: 9, blendIn: 1.6, blendOut: 2.4 });
    },
    /**
     * Show you the thing you touched (or whoever's talking) without swinging round it: turn toward it by at most
     * 30° (the camera goes round behind you, so you face it), slowly, then back straight away until it's on screen,
     * up to 24 m. Another touch turns another 30°. If backing away can't show it (it's on the camera's side of you), the turn is all. The framing holds
     * while you stay; moving off, flying or pinching lets it go, and the camera eases back in.
     */
    frame(el) {
      if (REDUCED || !el) return;
      const hx = bird.pos.x, hy = bird.pos.y + 0.1, hz = bird.pos.z;
      // What to show: the part of it you touched (its nearest side, at window height and the top of its ground floor:
      // a whole roofline beside you would need 30 m or a tilt up), or, if you're on it (a lot, a square) or it's
      // someone, where it glints.
      const b = el.box, pts = [];
      const side = el.kind !== "person" && b && !b.isEmpty() && nearestOn(el, new THREE.Vector3(hx, bird.pos.y + 1.2, hz), new THREE.Vector3());
      if (!side || Math.hypot(side.x - hx, side.z - hz) < 0.3) pts.push(anchorOf(el, new THREE.Vector3()));
      else pts.push(new THREE.Vector3(side.x, clamp(bird.pos.y + 1.2, b.min.y, b.max.y), side.z), new THREE.Vector3(side.x, Math.min(b.max.y, bird.pos.y + 3.5), side.z));
      const aim = pts[0];
      if (Math.hypot(aim.x - hx, aim.z - hz) > 0.6) {
        yawT = swingToward(yaw, Math.atan2(hx - aim.x, hz - aim.z), SWING);
        yawSlowUntil = t + 1.8;
      }
      const now = lastBase * zoom;
      const d = distanceToShow(pts.map((v) => [v.x, v.y, v.z]), [hx, hy, hz], yawT, pitchT,
        { from: Math.max(now, dist), max: FRAME_MAX, fov: camera.fov, aspect: camera.aspect });
      if (d && d > now * 1.05) { hold = { dist: d, x: bird.pos.x, z: bird.pos.z }; distSlowUntil = t + 4; }
    },
    update(dt) {
      const air = bird.air;
      const speed01 = clamp(bird.speed / (air ? 15 : 4.2), 0, 1);
      const following = t - lastLookT > 1.4 && bird.speed > 0.35;
      if (following) {
        const k = 1 - Math.exp(-dt * (air ? 1.6 : 0.9));
        yawT += angleTo(yawT, bird.heading + Math.PI) * k;
        if (air) pitchT += (0.18 - pitchT) * k * 0.5;
      }
      const idle = t - lastInputT > 9 && bird.speed < 0.05 && !goal && !tour;
      if (idle && !REDUCED) {
        const camDir = Math.atan2(Math.sin(yaw), Math.cos(yaw));
        const facingSun = Math.max(0, -(Math.sin(camDir) * SUN_DIR.x + Math.cos(camDir) * SUN_DIR.z));
        yawT += dt * 0.07 * (1 - 0.7 * facingSun);
      }
      const yaw0 = yaw, pitch0 = pitch;
      [yaw, yawV] = smoothDampAngle(yaw, yawT, yawV, t < yawSlowUntil ? 0.6 : 0.14, dt);
      [pitch, pitchV] = smoothDamp(pitch, pitchT, pitchV, 0.14, dt);
      if (t - lastLookT > 0.25) { // nobody's hand on it: hold the swing to SWING_RATE
        const lim = SWING_RATE * dt, dy = angleTo(yaw0, yaw), dp = pitch - pitch0;
        if (Math.abs(dy) > lim) { yaw = yaw0 + Math.sign(dy) * lim; yawV = Math.sign(dy) * SWING_RATE; }
        if (Math.abs(dp) > lim) { pitch = pitch0 + Math.sign(dp) * lim; pitchV = Math.sign(dp) * SWING_RATE; }
      }
      const attract = HERO && !active; // the hero's attract tour: higher and wider, so the place reads
      if (attract) pitchT += (0.28 - pitchT) * Math.min(1, dt * 0.8);
      const base = (air ? 1.5 + 1.9 * speed01 : 1.15) * (idle ? 1.35 : 1) * (tour ? 1.25 : 1) * (attract ? 2.4 : 1);
      lastBase = base;
      if (hold && (air || Math.hypot(bird.pos.x - hold.x, bird.pos.z - hold.z) > 2.5)) { hold = null; distSlowUntil = t + 2.5; }
      [dist, distV] = smoothDamp(dist, hold ? Math.max(base * zoom, hold.dist) : base * zoom, distV, t < distSlowUntil ? 1.2 : 0.35, dt);
      const eyesT = zoom < 0.42 ? 1 : 0;
      eyes += (eyesT - eyes) * Math.min(1, dt * 6);

      lookT.copy(bird.pos).y += 0.1;
      lookT.x += Math.sin(bird.heading) * (air ? 0.6 * speed01 : 0.06);
      lookT.z += Math.cos(bird.heading) * (air ? 0.6 * speed01 : 0.06);
      damp3(look, lookV, lookT, air ? 0.2 : 0.1, dt);

      dir.set(Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), Math.cos(yaw) * Math.cos(pitch));
      cam.copy(look).addScaledVector(dir, dist);
      let fovT = air ? (REDUCED ? 62 : fovFor(speed01)) : 62;
      let rollT = air && !REDUCED ? bankFor(bird.turn, speed01) : 0;
      const lookAt = look.clone();

      if (eyes > 0.01) {
        sparrow.eyeWorld(eye);
        cam.lerp(eye, eyes);
        lookAt.lerp(eye.clone().addScaledVector(dir, -4), eyes);
        fovT += (72 - fovT) * eyes;
      }
      if (shot) {
        const age = t - shot.t0;
        if (age > shot.dur) shot = null;
        else {
          const s = shot.frame();
          const w = shot.start ? 1 : shotWeight(age, shot.dur, shot.blendIn, shot.blendOut);
          if (shot.start && age > 0.05) shot.start = false;
          cam.lerp(s.pos, w);
          lookAt.lerp(s.look, w);
          fovT += (s.fov - fovT) * w;
          rollT *= 1 - w;
        }
      }
      // Walls, curbs and posts don't push the camera in (it jumped whenever one crossed the line): it keeps its place,
      // and what's in the way thins out instead (seethrough.js, updated below).
      head.copy(bird.pos).y += 0.1;
      // Never under the ground: the ground right below the camera (a sidewalk stands above the street), not only the
      // site's lowest. Looked for from just above the camera, so a porch roof or a crown overhead never lifts it.
      const under = castDown(cam.x, cam.y + 0.3, cam.z, 2.3, false);
      const floorY = under ? under.point.y : world.floorY;
      cam.y = Math.max(cam.y, floorY + 0.1, world.groundY + 0.04);
      [fov, fovV] = smoothDamp(fov, fovT, fovV, 0.6, dt);
      [roll, rollV] = smoothDamp(roll, rollT, rollV, 0.45, dt);
      camera.position.copy(cam);
      camera.lookAt(lookAt);
      camera.rotateZ(roll);
      // A shot (the opening crane, the flows' overview) moves the camera itself: its turn is held to SWING_RATE too,
      // through the shot and until the view has caught up after it. A cut (the crane's first frame) is a cut.
      if (shot && shot.start) viewHeld = false;
      else if (shot || viewHeld) {
        wantQ.copy(camera.quaternion);
        const gap = viewQ.angleTo(wantQ);
        if (gap > SWING_RATE * dt) { camera.quaternion.copy(viewQ).rotateTowards(wantQ, SWING_RATE * dt); viewHeld = true; }
        else viewHeld = !!shot;
      }
      viewQ.copy(camera.quaternion);
      // Near enough not to clip the ground just below a low camera.
      const near = clamp(Math.min(camera.position.distanceTo(head) * 0.25, (camera.position.y - floorY) * 0.6), 0.01, 0.4);
      if (Math.abs(camera.fov - fov) > 0.05 || Math.abs(camera.near - near) / camera.near > 0.15) {
        camera.fov = fov;
        camera.near = near;
        camera.updateProjectionMatrix();
      }
      sparrow.root.visible = eyes < 0.6;
      updateSee(camera, head, eyes < 0.5);
    },
  };
})();

// ---------------------------------------------------------------- benches and people
/**
 * Benches where the patterns ask for sitting: around each P126 "something
 * roughly in the middle", along the edges of P61 squares, facing the sun in
 * P161 sunny places, and one in each larger courtyard and common. Only on open
 * ground under open sky, and only where a whole bench (and room in front of it)
 * fits. Not data: the pipeline places no benches.
 */
function placeBenches() {
  const out = [];
  const max = COARSE ? 36 : 70;
  const open = /^(plaza|common|pocket)$/;
  const groundOf = (x, z) => {
    const hit = castDownModel(x, site.bounds.max.y + 20, z, site.bounds.max.y + 60);
    const top = hit && site.elementOfHit(hit);
    return top && open.test(top.kind) ? hit.point.y : null;
  };
  const tryBench = (x, z, yaw) => {
    if (out.length >= max || out.some((b) => (b.x - x) ** 2 + (b.z - z) ** 2 < 4)) return false;
    const c = Math.cos(yaw), s = Math.sin(yaw);
    let y = -Infinity;
    for (const [lx, lz] of [[-0.85, -0.3], [0.85, -0.3], [-0.85, 0.3], [0.85, 0.3], [0, 1.1], [0, 0]]) {
      const g = groundOf(x + lx * c + lz * s, z - lx * s + lz * c);
      if (g == null) return false;
      y = Math.max(y, g);
    }
    // Nothing standing in the bench's way at knee and shoulder height.
    for (const h of [0.3, 1.0]) {
      for (const [dx, dz] of [[c, -s], [-c, s]]) {
        const hit = castRay(_tmp.set(x, y + h, z), _tmp2.set(dx, 0, dz), 0.9);
        if (hit) return false;
      }
    }
    out.push({ x, y, z, yaw });
    return true;
  };
  const facing = (fx, fz) => Math.atan2(fx, fz);
  const els = site.elements.filter((e) => e.center);
  for (const m of els.filter((e) => e.group === "activity" && patternsOf(e).includes(126) && openSky(e))) {
    const a0 = (m.index * 2.399) % (Math.PI * 2);
    for (let k = 0; k < 3; k++) {
      const a = a0 + (k * Math.PI * 2) / 3;
      tryBench(m.center.x + Math.sin(a) * 3.2, m.center.z + Math.cos(a) * 3.2, facing(-Math.sin(a), -Math.cos(a)));
    }
  }
  for (const sq of els.filter((e) => e.group === "plazas" && patternsOf(e).includes(61))) {
    const r = Math.min(sq.box.max.x - sq.box.min.x, sq.box.max.z - sq.box.min.z) * 0.33;
    for (let k = 0; k < 4; k++) {
      const a = (k * Math.PI) / 2 + 0.3;
      tryBench(sq.center.x + Math.sin(a) * r, sq.center.z + Math.cos(a) * r, facing(-Math.sin(a), -Math.cos(a)));
    }
  }
  for (const sp of els.filter((e) => e.group === "plazas" && patternsOf(e).includes(161))) {
    tryBench(sp.center.x, sp.center.z, facing(SUN_DIR.x, SUN_DIR.z)); // facing the sun
  }
  const area = (e) => (e.box.max.x - e.box.min.x) * (e.box.max.z - e.box.min.z);
  for (const e of els.filter((e) => e.group === "plazas" && /^(plaza|common)$/.test(e.kind) && area(e) > 120).sort((a, b) => area(b) - area(a))) {
    const a = (e.index * 1.7) % (Math.PI * 2);
    tryBench(e.center.x + Math.sin(a) * 2, e.center.z + Math.cos(a) * 2, a);
  }
  return out;
}

/** People: a third on the benches, some talking by the markers, the rest walking between squares, markers, streets and benches. */
function populate() {
  let seed = 1234567;
  const rng = () => { seed = (seed * 16807) % 2147483647; return seed / 2147483647; };
  const spots = [], groups = [];
  for (const e of site.elements) {
    if (!e.center) continue;
    if (e.group === "plazas" && /^(plaza|common|pocket)$/.test(e.kind) && openSky(e)) {
      spots.push({ x: e.center.x, y: e.box.max.y, z: e.center.z });
      // A square gets a group or two standing about in it.
      const sx = e.box.max.x - e.box.min.x, sz = e.box.max.z - e.box.min.z;
      for (let k = 0, nG = sx * sz > 900 ? 2 : 1; k < nG; k++) {
        const x = e.center.x + (rng() - 0.5) * sx * 0.5, z = e.center.z + (rng() - 0.5) * sz * 0.5;
        const hit = castDownModel(x, site.bounds.max.y + 20, z, site.bounds.max.y + 60);
        if (hit && site.elementOfHit(hit) === e) groups.push({ x, y: hit.point.y, z });
      }
    }
    if (e.group === "activity" && openSky(e)) {
      groups.push({ x: e.center.x, y: e.box.min.y, z: e.center.z });
      spots.push({ x: e.center.x + 1.8, y: e.box.min.y, z: e.center.z + 1.8 });
    }
    if (e.group === "ground" && e.kind === "boardwalk") {
      // P64's boardwalks: people stroll out over the swamp.
      for (let k = 0; k < 4; k++) {
        const x = e.box.min.x + rng() * (e.box.max.x - e.box.min.x), z = e.box.min.z + rng() * (e.box.max.z - e.box.min.z);
        const hit = castDownModel(x, site.bounds.max.y + 20, z, site.bounds.max.y + 60);
        if (hit && site.elementOfHit(hit) === e) spots.push({ x, y: hit.point.y, z });
      }
    }
    if (e.group === "sidewalks") {
      // Sidewalks: plenty of places to be going.
      for (let k = 0; k < 600; k++) {
        const x = e.box.min.x + rng() * (e.box.max.x - e.box.min.x), z = e.box.min.z + rng() * (e.box.max.z - e.box.min.z);
        const hit = castDownModel(x, site.bounds.max.y + 20, z, site.bounds.max.y + 60);
        if (hit && site.elementOfHit(hit) === e) spots.push({ x, y: hit.point.y, z });
      }
    }
    if (e.group === "streets") {
      // A few points along each street, where it's open to the sky.
      for (let k = 0; k < 10; k++) {
        const x = e.box.min.x + rng() * (e.box.max.x - e.box.min.x), z = e.box.min.z + rng() * (e.box.max.z - e.box.min.z);
        const hit = castDownModel(x, site.bounds.max.y + 20, z, site.bounds.max.y + 60);
        if (hit && site.elementOfHit(hit) === e) spots.push({ x, y: hit.point.y, z });
      }
    }
  }
  // Shop doors: people stop by; outside a café or a bakery, now and then, a few stand and talk. With a walking mask,
  // each door gets a front walk to the nearest sidewalk.
  const doors = [];
  for (const s of frontages ? frontages.shops : []) {
    const hit = castDownModel(s.door.x, site.bounds.max.y + 20, s.door.z, site.bounds.max.y + 60);
    if (!hit) continue;
    if (walkMask) {
      const to = walkMask.nearest(s.door.x, s.door.z, 25);
      if (!to) continue;
      walkMask.carve(s.door.x, s.door.z, to.x, to.z, 1.2);
    }
    doors.push({ x: s.door.x, y: hit.point.y, z: s.door.z });
    if (/^(coffee|bistro|bakery)$/.test(s.kit) && rng() < 0.35) groups.push({ x: s.door.x, y: hit.point.y, z: s.door.z });
  }
  // With a walking mask, every place to be going is on it: onto the sidewalk nearby, or not at all.
  if (walkMask) {
    const onMask = [];
    for (const sp of spots) {
      const n = walkMask.nearest(sp.x, sp.z, 4);
      if (!n) continue;
      const hit = castDownModel(n.x, site.bounds.max.y + 20, n.z, site.bounds.max.y + 60);
      onMask.push({ x: n.x, y: hit ? hit.point.y : sp.y, z: n.z });
    }
    // And places along every sidewalk, evenly, so people spread out wherever you are (near the Hague, the street
    // spots and sidewalk samples above are few).
    for (let k = 0; k < 1500; k++) {
      const q = walkMask.sample(rng);
      if (!q) break;
      const hit = castDownModel(q.x, site.bounds.max.y + 20, q.z, site.bounds.max.y + 60);
      onMask.push({ x: q.x, y: hit ? hit.point.y : world.floorY, z: q.z });
    }
    spots.length = 0;
    spots.push(...onMask);
  }
  spots.push(...doors);
  if (!spots.length && !benches.length) return null;
  // About 320 people (140 on a phone); the walkers among them stay around you (people.js).
  const count = Math.min(COARSE ? 140 : 320, Math.max(12, benches.length * 2 + groups.length * 3 + Math.floor(spots.length / 2)));
  return buildPeople(scene, {
    benches, spots, groups, count, rng,
    clearLine: (a, b) => {
      const y = Math.max(a.y, b.y) + 0.9;
      const dx = b.x - a.x, dz = b.z - a.z, len = Math.hypot(dx, dz);
      if (len < 0.5) return true;
      // Nobody walks across a pond or a stream (P64's water).
      for (let k = 1, n = Math.ceil(len / 2.5); k < n; k++) {
        const hit = castDownModel(a.x + dx * k / n, site.bounds.max.y + 20, a.z + dz * k / n, site.bounds.max.y + 60);
        const el = hit && site.elementOfHit(hit);
        if (el && (el.kind === "pond" || el.kind === "stream")) return false;
      }
      _ray.origin.set(a.x, y, a.z);
      _ray.direction.set(dx / len, 0, dz / len);
      return !firstHit(len);
    },
    groundAt: (x, z, nearY) => {
      const hit = castDownModel(x, nearY + 2.5, z, 6);
      return hit ? hit.point.y : world.floorY;
    },
    // Along the sidewalks, crossing at corners, where the site has a walking mask.
    route: walkMask ? (a, b) => walkMask.path(a, b) : null,
    keepTo: walkMask ? (x, z) => walkMask.walkable(x, z) : null,
  });
}

/** Up onto the bench's seat, beside whoever's sitting. */
function hopOnto(b) {
  const c = Math.cos(b.yaw), s = Math.sin(b.yaw);
  // The end of the bench nearer you.
  const side = (bird.pos.x - b.x) * c - (bird.pos.z - b.z) * s > 0 ? 0.68 : -0.68;
  bird.pos.set(b.x + side * c + 0.05 * s, b.y + 0.49, b.z - side * s + 0.05 * c);
  bird.vel.set(0, 0, 0);
  bird.air = false;
  bird.heading = b.yaw;
  sparrow.land();
  sound.hop();
  setTimeout(() => { sparrow.chirp(); sound.chirp(); }, 350);
}

// ---------------------------------------------------------------- pigeons live on the open spaces
function pigeonSpots() {
  const open = site.elements.filter((e) => e.group === "plazas" && e.center && e.kind !== "undecided");
  const spots = [];
  let seed = 7;
  const rnd = () => { seed = (seed * 16807) % 2147483647; return seed / 2147483647; };
  const budget = COARSE ? 40 : 70;
  open.sort((a, b) => (b.box.max.x - b.box.min.x) * (b.box.max.z - b.box.min.z) - (a.box.max.x - a.box.min.x) * (a.box.max.z - a.box.min.z));
  for (const el of open) {
    if (spots.length >= budget) break;
    const flock = 2 + Math.floor(rnd() * 3);
    const cx = el.center.x + (rnd() - 0.5) * (el.box.max.x - el.box.min.x) * 0.4;
    const cz = el.center.z + (rnd() - 0.5) * (el.box.max.z - el.box.min.z) * 0.4;
    for (let i = 0; i < flock; i++) {
      const x = cx + (rnd() - 0.5) * 1.6, z = cz + (rnd() - 0.5) * 1.6;
      const hit = castDown(x, site.bounds.max.y + 20, z, site.bounds.max.y + 60);
      if (hit && hit.el === el) spots.push({ x, y: hit.point.y, z });
    }
  }
  return spots;
}

// ---------------------------------------------------------------- the frame
let last = performance.now(), frameMs = [], lastGovern = performance.now(), statsAt = 0, startedAt = performance.now();
function frame() {
  const now = performance.now();
  const raw = now - last;
  last = now;
  if (raw < 250 && now - startedAt > 2500 && !document.hidden) frameMs.push(raw);
  const dt = Math.min(raw / 1000, 0.05);
  t += dt;
  pollPad();
  if (tram) {
    tramStep = tram.update(t);
    const near = camera.position.distanceTo(tram.mesh.position);
    if ((tramStep.arrived || tramStep.departing) && near < 160) sound.bell(Math.max(0.2, 1 - near / 160));
    if (tramStep.speed > 0.02 && near < 50 && Math.random() < dt * 4) sound.rumble(Math.min(1, tramStep.speed / 0.25));
  }
  gatherIntent();
  physics(dt);
  probeGround();
  reachScan();
  tourTick();
  if (trees) trees.update(t, camera);
  if (frontages) frontages.update(camera);
  if (lamps) lamps.update(camera);
  if (flowLines) flowLines.update(t, dt);
  waterTime.value = t;
  if (wetland) wetland.update(t);
  if (plan && panels && panels.current === "map" && Math.floor(t * 10) !== Math.floor((t - dt) * 10)) drawMap();
  if (active) surfaceNear();
  sparrow.root.position.copy(bird.pos);
  sparrow.root.rotation.y = bird.heading;
  sparrow.update({ dt, t, air: bird.air, speed: bird.speed, vy: bird.vel.y, turn: bird.turn, climbing: bird.air && intent.up, reduced: REDUCED });
  if (bird.speed > 0.3 || bird.air) sparrow.sit(false);
  sparrow.root.updateMatrixWorld(true);
  if (people && people.update(dt, t, bird, camera.position).crumb) {
    logLine("Someone on the bench drops you a crumb.", "note");
    sparrow.peck();
    setTimeout(() => { sparrow.peck(); sound.chirp(); }, 500);
  }
  const burst = pigeons.update(dt, t, bird, camera.position);
  if (burst) { sound.flutter(); if (Math.random() < 0.5) setTimeout(() => sound.coo(), 400); }
  director.update(dt);
  world.follow(camera);
  placeBubble();
  // Markers breathe; a touched one flares.
  flare = Math.max(0, flare - dt * 0.8);
  for (const m of site.meshes) {
    if (!m.userData.glow) continue;
    const base = m.userData.halo ? 0.6 : 0.25;
    m.material.emissiveIntensity = base + 0.12 * Math.sin(t * 1.7) + flare * (m.userData.halo ? 1.2 : 0.8);
  }
  if (ping.mesh) {
    const a = t - ping.t;
    ping.mesh.visible = a < 0.9 && !!goal;
    ping.mesh.scale.setScalar(1 + a * 2.5);
    ping.mesh.material.opacity = Math.max(0, 1 - a / 0.9);
  }
  if (now - lastGovern > 2000) {
    lastGovern = now;
    if (frameMs.length > 20) {
      const s = governor.step(median(frameMs));
      if (s !== renderer.getPixelRatio()) { renderer.setPixelRatio(s); renderer.setSize(innerWidth, innerHeight); }
    }
    frameMs = [];
  }
  renderer.render(scene, camera);
  if (STATS && now - statsAt > 500) {
    statsAt = now;
    const info = renderer.info.render;
    $("stats").textContent = `${(1000 / Math.max(1, raw)).toFixed(0)} fps · ${info.calls} draws · ${(info.triangles / 1000).toFixed(0)}k tris · scale ${governor.scale}`;
  }
}

// ---------------------------------------------------------------- input
function pollPad() {
  if (!active) return;
  const pads = navigator.getGamepads ? navigator.getGamepads() : [];
  const p = pads && [...pads].find((x) => x && x.connected);
  if (!p) return;
  const dz = (v) => (Math.abs(v) < 0.15 ? 0 : v);
  padMove = { x: dz(p.axes[0] || 0), y: dz(p.axes[1] || 0) };
  padLook = { x: dz(p.axes[2] || 0), y: dz(p.axes[3] || 0) };
  if (padLook.x || padLook.y) director.drag(padLook.x * 14, padLook.y * 10);
  const b = (i) => !!(p.buttons[i] && p.buttons[i].pressed);
  if (b(0) && !padUp) wantHop = true;
  padUp = b(0);
  padDown = b(1);
  if (b(2) && !pollPad.x) interact();
  pollPad.x = b(2);
  if (b(3) && !pollPad.y) tourButton();
  pollPad.y = b(3);
}

function setupUI() {
  const firstGesture = () => { sound.unlock(); removeEventListener("pointerdown", firstGesture); removeEventListener("keydown", firstGesture); };
  addEventListener("pointerdown", firstGesture);
  addEventListener("keydown", firstGesture);

  addEventListener("keydown", (e) => {
    if (e.target && /INPUT|TEXTAREA/.test(e.target.tagName)) return;
    if (!active) return;
    if (!$("intro").hidden) { if (e.code === "Escape") closeIntro(false); return; } // buttons take Enter themselves
    if (HERO && e.code === "Escape") { toPage({ type: "done" }); return; }
    if (!e.repeat) {
      if (e.code === "Space") wantHop = true;
      if (e.code === "KeyE" || e.code === "Enter") interact();
      if (e.code === "KeyM") toggleMute();
      if (e.code === "KeyO") toggleFlows();
      if (e.code === "KeyT") tourButton();
      if (e.code === "KeyF") { if (bird.air) holdDownFor = 3; else takeOff(); }
      if (e.code === "KeyV") director.zoomBy(director.eyes > 0.5 ? 3 : 0.2);
      if (e.code === "Escape") { if (panels && panels.current) { panels.close(); return; } if (tour) endTour(""); goal = null; }
    }
    if (/^(Space|Arrow|Key[WASDCQEF])/.test(e.code)) e.preventDefault();
    keys[e.code] = true;
    hideHint();
  });
  addEventListener("keyup", (e) => { keys[e.code] = false; });
  addEventListener("blur", () => { for (const k in keys) keys[k] = false; });

  // Pointers: drag to look, tap to go, pinch or wheel to zoom.
  const pts = new Map();
  let pinch = 0;
  canvas.addEventListener("pointerdown", (e) => {
    if (!active) return;
    canvas.setPointerCapture(e.pointerId);
    pts.set(e.pointerId, { x: e.clientX, y: e.clientY, x0: e.clientX, y0: e.clientY, t0: performance.now(), moved: false });
    if (pts.size === 2) { const [a, b] = [...pts.values()]; pinch = Math.hypot(a.x - b.x, a.y - b.y); }
  });
  canvas.addEventListener("pointermove", (e) => {
    const p = pts.get(e.pointerId);
    if (!p) return;
    const dx = e.clientX - p.x, dy = e.clientY - p.y;
    p.x = e.clientX; p.y = e.clientY;
    if (Math.hypot(p.x - p.x0, p.y - p.y0) > 7) p.moved = true;
    if (pts.size === 2) {
      const [a, b] = [...pts.values()];
      const d = Math.hypot(a.x - b.x, a.y - b.y);
      if (pinch > 0 && d > 0) director.zoomBy(pinch / d);
      pinch = d;
      return;
    }
    if (p.moved) { director.drag(dx, dy); hideHint(); }
  });
  const up = (e) => {
    const p = pts.get(e.pointerId);
    pts.delete(e.pointerId);
    if (pts.size < 2) pinch = 0;
    if (p && !p.moved && performance.now() - p.t0 < 400 && pts.size === 0 && e.type === "pointerup") { tapAt(e.clientX, e.clientY); hideHint(); }
  };
  canvas.addEventListener("pointerup", up);
  canvas.addEventListener("pointercancel", up);
  canvas.addEventListener("wheel", (e) => { if (!active) return; e.preventDefault(); director.zoomBy(Math.exp(e.deltaY * 0.0012)); }, { passive: false });
  canvas.addEventListener("contextmenu", (e) => e.preventDefault());

  const hold = (el, on) => {
    el.addEventListener("pointerdown", (e) => { e.preventDefault(); on(true); if (el.id === "btn-up") wantHop = true; hideHint(); });
    for (const ev of ["pointerup", "pointercancel", "pointerleave"]) el.addEventListener(ev, () => on(false));
  };
  hold($("btn-up"), (v) => { holdUp = v; });
  hold($("btn-down"), (v) => { holdDown = v; });
  $("prompt").addEventListener("click", interact);
  TOUR_LABEL = $("tour").textContent;
  if (!TOURS.length) $("tour").hidden = true;
  $("tour").addEventListener("click", tourButton);
  $("mute").addEventListener("click", toggleMute);
  if (flowsReady()) { $("flows").hidden = false; $("flows").addEventListener("click", toggleFlows); }
  $("mute").setAttribute("aria-pressed", String(sound.muted));
  panels = setupPanels({
    open(name) {
      if (peek) peek.hide();
      if (name === "map") openMap();
      const mod = SITE.renderers && SITE.renderers[name];
      if (mod && (SITE.live || []).includes(name)) renderLive(name);
      else if (mod && !rendered.has(name)) {
        // The site's own panel (Money, say): its module renders into #<name>-body the first time it opens.
        rendered.add(name);
        const body = $(`${name}-body`);
        body.textContent = "Working it out…";
        Promise.all([
          import(fromSite(mod)),
          NUMBERS_URL ? fetch(NUMBERS_URL).then((r) => (r.ok ? r.json() : null)).catch(() => null) : null,
        ]).then(([m, n]) => m.render(body, { numbers: n && n.version === 1 ? n : null, site: SITE, elements: site.elements, flyTo, goToElement,
          warp: warpTo, showcase: SHOWCASE_URL, shops: frontages ? frontages.shops : [] }))
          .catch((e) => { body.textContent = `This panel couldn’t load (${e.message}).`; });
      }
      if (name === "tours") renderTours($("tours-body"), TOURS, (def) => { panels.close(); startTour(def); });
    },
  });
  setupHudMenu();
  peek = setupPeek((name) => panels.open(name));
  $("map-canvas").addEventListener("click", (e) => {
    if (!plan) return;
    const r = e.currentTarget.getBoundingClientRect();
    flyTo(plan.toWorld((e.clientX - r.left) / r.width, (e.clientY - r.top) / r.height));
  });
  if (views.length) {
    $("inside").hidden = false;
    $("inside").addEventListener("click", () => peekInside((viewIndex + 1) % views.length));
  }
  setupIterations();
  for (const a of document.querySelectorAll("[data-model]")) {
    const isCurrent = a.dataset.model === "cluster" ? GLB_URL.includes("cluster_interior") : GLB_URL.includes("clean_baseline") && !params.get("iteration");
    a.setAttribute("aria-current", isCurrent ? "page" : "false");
  }
  if (STATS) $("stats").hidden = false;
  $("hint").textContent = COARSE ? "Tap to go · drag to look · ▲ to fly" : "WASD to hop · hold Space to fly · drag to look · E to touch · T for a tour";
  $("intro-tour").addEventListener("click", () => closeIntro(true));
  $("intro-fly").addEventListener("click", () => closeIntro(false));
  $("intro-again").addEventListener("click", () => { panels.close(); openIntro(); });
  if (COARSE) $("intro-controls").innerHTML = "Tap to go, drag to look, hold &#9650; to fly; tap what glints to touch it.";
  if (active && !params.has("debug") && !introSeen()) openIntro();
  else if (active) setTimeout(hideHint, 14000);
  else { hintGone = true; $("hint").classList.add("gone"); }
}
/**
 * Earlier iterations of the plan, from /archive/index.json (worker/index.js
 * serves the archive from R2; tools/vibe-render/archive_milestone.py adds
 * to it). Choosing one reloads the walk on that iteration's model and its
 * chorus. Without an archive (a local copy, say) the menu stays hidden.
 */
function setupIterations() {
  const ITERATION = params.get("iteration");
  if (!SITE.archive) return;
  fetch(fromSite(SITE.archive)).then((r) => (r.ok ? r.json() : null)).then((idx) => {
    const its = idx && Array.isArray(idx.iterations) ? idx.iterations : [];
    if (!its.length || HERO) return;
    renderIterations($("its-body"), its, its.some((i) => i.id === ITERATION) ? ITERATION : null, location.pathname);
    $("its-btn").hidden = false;
    const short = (d) => new Date(d + "T12:00:00").toLocaleDateString("en-US", { month: "short", day: "numeric" });
    const k = its.findIndex((i) => i.id === ITERATION);
    if (k >= 0) {
      const it = its[k], note = $("iteration-note");
      note.innerHTML = "";
      const b = document.createElement("b"); b.textContent = `Iteration ${k + 1} of ${its.length}: ${it.title}`;
      note.append(b, document.createTextNode(` (${short(it.date)}${it.pr ? `, PR #${it.pr}` : ""}). ${it.summary} `));
      const back = document.createElement("a"); back.href = location.pathname; back.textContent = "Back to now";
      note.append(back);
      note.hidden = false;
      logLine(`You're walking iteration ${k + 1} of ${its.length}: ${it.title}.`, "note");
    }
  }).catch(() => {});
}
/**
 * The site map: the scene from straight above, rendered the first time the panel opens
 * (sitemap.js), with the sparrow drawn on it while it's open.
 */
function openMap() {
  if (!plan) {
    const box = new THREE.Box3();
    for (const e of site.elements) if (e.center && e.kind !== "context" && !e.box.isEmpty()) box.union(e.box);
    if (box.isEmpty()) box.copy(site.bounds);
    box.expandByVector(new THREE.Vector3(15, 0, 15));
    plan = renderPlan(renderer, scene, box, COARSE ? 640 : 900);
    drawOverlays(plan, site.overlays || []);
    const cv = $("map-canvas");
    cv.width = plan.image.width; cv.height = plan.image.height;
  }
  drawMap();
}
function drawMap() {
  const cv = $("map-canvas"), ctx = cv.getContext("2d");
  ctx.drawImage(plan.image, 0, 0);
  const [u, v] = plan.toMap(bird.pos.x, bird.pos.z), x = u * cv.width, y = v * cv.height, s = cv.width / 300;
  // Where you're looking: a soft wedge along the camera's heading.
  const look = Math.atan2(bird.pos.x - camera.position.x, bird.pos.z - camera.position.z);
  ctx.fillStyle = "rgba(255, 210, 122, 0.28)";
  ctx.beginPath(); ctx.moveTo(x, y); ctx.arc(x, y, 26 * s, Math.PI / 2 - look - 0.5, Math.PI / 2 - look + 0.5); ctx.closePath(); ctx.fill();
  ctx.fillStyle = "#ffd27a"; ctx.strokeStyle = "#2a2116"; ctx.lineWidth = 2 * s;
  ctx.beginPath(); ctx.arc(x, y, 5 * s, 0, Math.PI * 2); ctx.fill(); ctx.stroke();
}
/**
 * What a pipeline asks the map to draw over the plan (the model's `extras.streetSmarts.overlays`, glTF x/z):
 * lines round areas, dots and numbered labels, baked into the plan's image once, at map scale, with a legend
 * from their labels in #map-legend.
 */
function drawOverlays(plan, overlays) {
  if (!overlays.length) return;
  // Sizes are in the pixels you see: the image is drawn about 380 px wide in the panel.
  const g = plan.image.getContext("2d"), W = plan.image.width, H = plan.image.height, s = W / 380;
  const at = ([x, z]) => { const [u, v] = plan.toMap(x, z); return [u * W, v * H]; };
  for (const o of overlays) {
    g.save();
    if (o.rings) {
      g.strokeStyle = o.color; g.lineWidth = (o.width || 2) * s; g.lineJoin = "round";
      g.setLineDash((o.dash || []).map((d) => d * s));
      g.shadowColor = "rgba(0,0,0,0.45)"; g.shadowBlur = 2 * s;
      for (const ring of o.rings) {
        g.beginPath();
        ring.forEach((p, i) => { const [x, y] = at(p); if (i) g.lineTo(x, y); else g.moveTo(x, y); });
        g.closePath(); g.stroke();
      }
    }
    if (o.points) {
      g.fillStyle = o.color;
      for (const p of o.points) { const [x, y] = at(p); g.beginPath(); g.arc(x, y, (o.radius || 2) * s, 0, Math.PI * 2); g.fill(); }
    }
    if (o.labels) {
      g.font = `700 ${Math.round(11 * s)}px system-ui, sans-serif`; g.textAlign = "center"; g.textBaseline = "middle";
      for (const l of o.labels) {
        const [x, y] = at(l.at);
        g.fillStyle = o.fill || "#fff"; g.strokeStyle = o.color; g.lineWidth = 1.5 * s;
        g.beginPath(); g.arc(x, y, 8 * s, 0, Math.PI * 2); g.fill(); g.stroke();
        g.fillStyle = o.color; g.fillText(l.text, x, y + 0.5 * s);
      }
    }
    g.restore();
  }
  const legend = $("map-legend");
  if (legend) {
    legend.innerHTML = "";
    for (const o of overlays) {
      const li = document.createElement("li"), sw = document.createElement("span");
      sw.className = `sw ${o.rings ? "line" : o.labels ? "label" : "dot"}`;
      sw.style.setProperty("--c", o.color);
      if (o.dash) sw.classList.add("dash");
      li.append(sw, document.createTextNode(o.label));
      legend.append(li);
    }
  }
}
/** From the map: a blink, and you're there, on the ground or the roof below. */
function flyTo(p) {
  const hit = castDown(p.x, site.bounds.max.y + 40, p.z, site.bounds.max.y + 120);
  if (!hit) return;
  $("fade").classList.add("on");
  setTimeout(() => {
    goal = null;
    if (tour) endTour("");
    placeBird({ x: p.x, y: hit.point.y + 0.3, z: p.z });
    director.snapBehind();
    $("fade").classList.remove("on");
    if (COARSE) panels.close();
  }, 360);
}
/**
 * From the gallery: a blink, and you're standing where a showcase shot was taken, looking where it looked.
 * `shot`: { stand: [x, z], look: [x, y, z], pitch, zoom } in glTF metres (tools/ghent/showcase.mjs writes them).
 */
function warpTo(shot, done) {
  $("fade").classList.add("on");
  setTimeout(() => {
    goal = null;
    if (tour) endTour("");
    const [sx, sz] = shot.stand, [lx, , lz] = shot.look;
    placeBird({ x: sx, y: 0, z: sz });
    bird.heading = Math.atan2(lx - sx, lz - sz);
    director.snapBehind();
    director.zoomTo(shot.zoom || 1);
    // A shot's pitch is in drag units from the walk's resting tilt (0.3 rad; showcase.mjs writes -60): set outright,
    // not nudged from wherever the camera was, so a shot looks the same however you came to it.
    director.pitchTo(0.3 + (shot.pitch || 0) * 0.004);
    $("fade").classList.remove("on");
    if (done) done();
  }, 360);
}
/** The intro: once per browser (remembered if storage allows), and again from How to play. */
const INTRO_KEY = SITE.id === "eastside-commons" ? "street-smarts-intro-seen" : `street-smarts-intro-seen:${SITE.id}`; // each site's intro shows once
function introSeen() { try { return localStorage.getItem(INTRO_KEY) === "1"; } catch { return false; } }
function openIntro() {
  $("intro").hidden = false;
  hintGone = true; $("hint").classList.add("gone");
  setTimeout(() => $("intro-tour").focus(), 50);
}
function closeIntro(tourToo) {
  $("intro").hidden = true;
  try { localStorage.setItem(INTRO_KEY, "1"); } catch { /* private window: it'll show again next time */ }
  lastInputT = t;
  if (tourToo && !tour) tourButton();
  else { hintGone = false; $("hint").classList.remove("gone"); setTimeout(hideHint, 9000); }
}
function hideHint() {
  if (hintGone) return;
  hintGone = true;
  $("hint").classList.add("gone");
}
/**
 * The flow overlay needs each building's footprint (models rendered since
 * the overlay came in carry it) and the shop fronts; without them there's
 * no button.
 */
function flowsReady() {
  return !!(frontages && frontages.shops.length && site.elements.some((e) => e.area != null && e.uses));
}
/** Where the neighbourhood's money goes (flows.js), drawn over the roofs (flowlines.js). */
function toggleFlows() {
  if (!flowsReady()) return;
  if (!flowLines) {
    const LIFT = 3;
    const byId = new Map();
    for (const e of site.elements) {
      if (e.area == null || !e.source || !e.box || e.box.isEmpty()) continue;
      const b = byId.get(e.source) || { id: e.source, area: e.area, uses: e.uses, box: new THREE.Box3() };
      b.box.union(e.box);
      byId.set(e.source, b);
    }
    const buildings = [...byId.values()].map((b) => {
      const c = b.box.getCenter(new THREE.Vector3());
      return { id: b.id, area: b.area, uses: b.uses, x: c.x, z: c.z, top: b.box.max.y };
    });
    const shops = frontages.shops.map((s) => ({ building: s.building, x: s.door.x, z: s.door.z }));
    let station = null;
    const line = site.transit[0];
    if (line && line.stations && line.stations.length) {
      const pts = line.points;
      let left = line.stations[0].at;
      for (let i = 1; i < pts.length && !station; i++) {
        const seg = Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]);
        if (left <= seg || i === pts.length - 1) {
          const u = seg ? Math.min(1, left / seg) : 0;
          station = { x: pts[i - 1][0] + (pts[i][0] - pts[i - 1][0]) * u, z: pts[i - 1][1] + (pts[i][1] - pts[i - 1][1]) * u };
        }
        left -= seg;
      }
    }
    flowModel = computeFlows({ buildings, shops, station });
    const places = new Map();
    for (const b of buildings) {
      const mine = shops.filter((s) => s.building === b.id);
      const to = mine.length
        ? new THREE.Vector3(mine.reduce((a, s) => a + s.x, 0) / mine.length, b.top + LIFT, mine.reduce((a, s) => a + s.z, 0) / mine.length)
        : new THREE.Vector3(b.x, b.top + LIFT, b.z);
      places.set(b.id, { from: new THREE.Vector3(b.x, b.top + LIFT, b.z), to });
    }
    if (station) {
      const p = new THREE.Vector3(station.x, (tram ? tram.roofY : 4) + LIFT, station.z);
      places.set("station", { from: p, to: p });
    }
    flowLines = buildFlowLines(scene, flowModel, places, { coarse: COARSE });
  }
  const on = !flowLines.shown;
  flowLines.show(on);
  $("flows").setAttribute("aria-pressed", String(on));
  $("flows-legend").hidden = !on;
  if (on) {
    logLine(flowSummary(flowModel), "what");
    logLine("Where it goes is our guess: each spends at the shop fronts within a five-minute walk, more at nearer ones. Touch a building to see what its shops take in.", "note");
    director.overview(flowLines.bounds);
  }
}
function toggleMute() {
  const m = sound.toggle();
  if (m && globalThis.speechSynthesis) speechSynthesis.cancel(); // a line in progress stops; the words stay on screen
  $("mute").setAttribute("aria-pressed", String(m));
  $("mute").textContent = m ? "♪ off" : "♪";
}
