// What grows and stands in P64's wetland, in the walk, after the cypress
// swamps at First Landing State Park: cattails gone brown and fluffy in the
// marsh, as they are in late November, tufts of rush, lily pads on the
// blackwater, knees round every bald cypress and Spanish moss hanging from
// it, turtles sunning on logs, great blue herons and egrets in the shallows.
// The park's trail guide names the turtles on logs, the herons and egrets;
// the moss is the subtropical half of what makes the park a National
// Natural Landmark. The ponds, marsh, streams, boardwalks and cypress are
// the pipeline's (P64 Pools and Streams); everything here is ours, not
// data, placed by sampling where the plan put water and marsh.
import * as THREE from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

const rngOf = (seed) => () => { seed = (seed * 16807) % 2147483647; return seed / 2147483647; };

/** Points inside `el` (an element of the ground), found by dropping rays on its box. */
function sample(el, n, rng, drop) {
  const out = [], b = el.box;
  for (let k = 0; k < n * 3 && out.length < n; k++) {
    const x = b.min.x + rng() * (b.max.x - b.min.x), z = b.min.z + rng() * (b.max.z - b.min.z);
    const hit = drop(x, z);
    if (hit && hit.el === el) out.push(new THREE.Vector3(x, hit.y, z));
  }
  return out;
}

function heronGeometry() {
  const body = new THREE.SphereGeometry(1, 14, 10).scale(0.22, 0.2, 0.42).rotateX(-0.35).translate(0, 1.0, 0);
  const neckPath = new THREE.CatmullRomCurve3([
    new THREE.Vector3(0, 1.08, 0.3), new THREE.Vector3(0, 1.32, 0.34), new THREE.Vector3(0, 1.42, 0.24), new THREE.Vector3(0, 1.58, 0.36),
  ]);
  const neck = new THREE.TubeGeometry(neckPath, 12, 0.045, 6, false);
  const head = new THREE.SphereGeometry(1, 10, 8).scale(0.06, 0.06, 0.1).translate(0, 1.62, 0.42);
  const beak = new THREE.ConeGeometry(0.025, 0.24, 6).rotateX(Math.PI / 2).translate(0, 1.61, 0.6);
  const legs = [-0.06, 0.06].map((x) => new THREE.CylinderGeometry(0.012, 0.012, 0.9, 5).translate(x, 0.45, 0.02));
  const tail = new THREE.ConeGeometry(0.12, 0.3, 6).rotateX(-Math.PI / 2 - 0.4).translate(0, 0.92, -0.42);
  const strip = (g) => { g.deleteAttribute("uv"); return g; };
  return mergeGeometries([body, neck, head, beak, ...legs, tail].map(strip), false);
}

/**
 * site: the loaded model; trees: trees.js's list; drop(x, z) -> { y, el } or
 * null (the model only). Returns { meshes, herons, update(t) }.
 */
export function buildWetland(scene, site, trees, drop, { coarse = false } = {}) {
  const rng = rngOf(64064);
  const ponds = site.elements.filter((e) => e.group === "ground" && e.kind === "pond" && e.center);
  const marshes = site.elements.filter((e) => e.group === "ground" && e.kind === "marsh" && e.center);
  const cypress = (trees ? trees.list : []).filter((t) => t.el && t.el.kind === "cypress");
  if (!ponds.length && !marshes.length && !cypress.length) return null;
  const M = new THREE.Matrix4(), Q = new THREE.Quaternion(), P = new THREE.Vector3(), S = new THREE.Vector3(), UP = new THREE.Vector3(0, 1, 0);
  const meshes = [];
  const instanced = (geo, mat, list, place) => {
    if (!list.length) return null;
    const im = new THREE.InstancedMesh(geo, mat, list.length);
    list.forEach((it, i) => { place(it, i); im.setMatrixAt(i, M); });
    im.instanceMatrix.needsUpdate = true;
    im.computeBoundingSphere();
    scene.add(im);
    meshes.push(im);
    return im;
  };
  const area = (e) => (e.box.max.x - e.box.min.x) * (e.box.max.z - e.box.min.z);

  // Cattails in the marsh: clumps of brown stalks with their fluffy heads, and tufts of rush.
  const clumps = marshes.flatMap((e) => sample(e, Math.min(coarse ? 160 : 420, Math.round(area(e) / (coarse ? 22 : 9))), rng, drop));
  const stalks = [], tufts = [];
  for (const c of clumps) {
    if (rng() < 0.65) for (let k = 0, n = 4 + Math.floor(rng() * 6); k < n; k++) {
      const x = c.x + (rng() - 0.5) * 1.4, z = c.z + (rng() - 0.5) * 1.4;
      const under = drop(x, z); // not up through a boardwalk
      if (under && under.el && under.el.kind === "marsh") stalks.push({ x, y: under.y, z, h: 1.3 + rng() * 0.8, lean: (rng() - 0.5) * 0.25, yaw: rng() * 6.28 });
    }
    else tufts.push({ x: c.x, y: c.y, z: c.z, s: 0.8 + rng() * 0.7, yaw: rng() * 6.28 });
  }
  const stalkGeo = new THREE.CylinderGeometry(0.012, 0.018, 1, 4).translate(0, 0.5, 0);
  instanced(stalkGeo, new THREE.MeshLambertMaterial({ color: 0x9a8a5a }), stalks, (s) => {
    Q.setFromEuler(new THREE.Euler(s.lean, s.yaw, 0)); M.compose(P.set(s.x, s.y, s.z), Q, S.set(1, s.h, 1));
  });
  const headGeo = new THREE.CylinderGeometry(0.035, 0.035, 0.22, 6).translate(0, 0.11, 0);
  instanced(headGeo, new THREE.MeshLambertMaterial({ color: 0x6b4a2e }), stalks, (s) => {
    // At the top of its stalk, leaning with it.
    Q.setFromEuler(new THREE.Euler(s.lean, s.yaw, 0));
    const top = new THREE.Vector3(0, s.h * 0.82, 0).applyQuaternion(Q);
    M.compose(P.set(s.x + top.x, s.y + top.y, s.z + top.z), Q, S.set(1, 1, 1));
  });
  const tuftGeo = mergeGeometries([0, 1, 2, 3, 4, 5, 6].map((k) =>
    new THREE.ConeGeometry(0.05, 0.9, 3).translate(0, 0.45, 0).rotateZ(0.25 * Math.cos(k * 0.9)).rotateY(k * 0.9).deleteAttribute("uv")), false);
  instanced(tuftGeo, new THREE.MeshLambertMaterial({ color: 0x8a8a52 }), tufts, (t) => {
    Q.setFromAxisAngle(UP, t.yaw); M.compose(P.set(t.x, t.y, t.z), Q, S.set(t.s, t.s, t.s));
  });

  // Lily pads, mostly toward the pond's quieter side.
  const pads = ponds.flatMap((e) => sample(e, Math.min(140, Math.round(area(e) / 30)), rng, drop)).filter(() => rng() < 0.7);
  const padGeo = new THREE.CircleGeometry(1, 12, 0.35, Math.PI * 2 - 0.35).rotateX(-Math.PI / 2);
  instanced(padGeo, new THREE.MeshLambertMaterial({ color: 0x4e6f37, side: THREE.DoubleSide }), pads, (p) => {
    const s = 0.3 + rng() * 0.35;
    Q.setFromAxisAngle(UP, rng() * 6.28); M.compose(P.set(p.x, p.y + 0.012, p.z), Q, S.set(s, 1, s));
  });

  // Cypress knees: the woody cones a bald cypress pushes up from its roots.
  const knees = [];
  for (const t of cypress) {
    for (let k = 0, n = 4 + Math.floor(rng() * 6); k < n; k++) {
      const a = rng() * 6.28, d = 1.2 + rng() * 2.8;
      const x = t.x + Math.cos(a) * d, z = t.z + Math.sin(a) * d;
      const hit = drop(x, z);
      if (hit && !(hit.el && hit.el.kind === "boardwalk")) knees.push({ x, y: hit.y, z, h: 0.25 + rng() * 0.6, r: 0.07 + rng() * 0.12 });
    }
  }
  const kneeGeo = new THREE.ConeGeometry(1, 1, 7).translate(0, 0.5, 0);
  instanced(kneeGeo, new THREE.MeshLambertMaterial({ color: 0x8a7462 }), knees, (k) => {
    M.compose(P.set(k.x, k.y - 0.05, k.z), Q.identity(), S.set(k.r, k.h, k.r));
  });

  // Spanish moss: grey strands hanging from every cypress, and from the trees nearest the water.
  const nearWater = (t) => ponds.some((e) => Math.hypot(t.x - e.center.x, t.z - e.center.z) < (e.box.max.x - e.box.min.x) * 0.5 + 25);
  const mossy = (trees ? trees.list : []).filter((t) => (t.el && t.el.kind === "cypress") || (nearWater(t) && rng() < 0.35));
  const moss = [];
  for (const t of mossy) {
    const h = Math.max(0.5, t.top - t.cy);
    for (let k = 0, n = 6 + Math.floor(rng() * 7); k < n; k++) {
      const a = rng() * 6.28, d = t.r * (0.45 + rng() * 0.5);
      moss.push({ x: t.x + Math.cos(a) * d, y: t.cy - h * (0.15 + rng() * 0.45), z: t.z + Math.sin(a) * d, len: 0.8 + rng() * 1.6, yaw: rng() * 6.28 });
    }
  }
  const mossGeo = new THREE.ConeGeometry(0.16, 1, 4, 1, true).rotateX(Math.PI).translate(0, -0.5, 0); // hangs down from its top
  instanced(mossGeo, new THREE.MeshLambertMaterial({ color: 0x9ea48f, side: THREE.DoubleSide }), moss, (m) => {
    Q.setFromAxisAngle(UP, m.yaw); M.compose(P.set(m.x, m.y, m.z), Q, S.set(1, m.len, 1));
  });

  // Logs in the shallows, half sunk, with turtles sunning on them.
  const shallowsOf = (e, n) => sample(e, n * 12, rng, drop).filter((p) => [0, 1, 2, 3, 4, 5].some((k) => {
    const hit = drop(p.x + Math.cos(k) * 3, p.z + Math.sin(k) * 3);
    return hit && hit.el && hit.el.kind === "marsh";
  })).slice(0, n);
  const logs = [], turtles = [];
  for (const e of ponds) {
    for (const p of shallowsOf(e, 2 + Math.floor(rng() * 2))) {
      const len = 2.5 + rng() * 2.5, yaw = rng() * 6.28, tilt = (rng() - 0.5) * 0.12;
      logs.push({ x: p.x, y: p.y + 0.02, z: p.z, len, yaw, tilt });
      for (let k = 0, n = 1 + Math.floor(rng() * 4); k < n; k++) {
        const along = (rng() - 0.5) * len * 0.8;
        turtles.push({ x: p.x + Math.cos(yaw) * along, y: p.y + 0.2, z: p.z - Math.sin(yaw) * along, yaw: yaw + (rng() < 0.5 ? 0 : Math.PI) + (rng() - 0.5) * 0.6, s: 0.8 + rng() * 0.5 });
      }
    }
  }
  const logGeo = new THREE.CylinderGeometry(0.17, 0.2, 1, 7).rotateZ(Math.PI / 2);
  instanced(logGeo, new THREE.MeshLambertMaterial({ color: 0x4a3b2c }), logs, (l) => {
    Q.setFromEuler(new THREE.Euler(0, l.yaw, l.tilt)); M.compose(P.set(l.x, l.y, l.z), Q, S.set(l.len, 1, 1));
  });
  const turtleGeo = mergeGeometries([
    new THREE.SphereGeometry(1, 10, 6, 0, Math.PI * 2, 0, Math.PI / 2).scale(0.14, 0.07, 0.18), // the shell
    new THREE.SphereGeometry(0.035, 6, 5).translate(0, 0.02, 0.2), // the head, out in the sun
  ].map((g) => { g.deleteAttribute("uv"); return g; }), false);
  instanced(turtleGeo, new THREE.MeshLambertMaterial({ color: 0x39432a }), turtles, (t) => {
    Q.setFromAxisAngle(UP, t.yaw); M.compose(P.set(t.x, t.y, t.z), Q, S.set(t.s, t.s, t.s));
  });

  // A great blue heron in the shallows of each pond (up to four), and an egret or two, still, turning now and then.
  const herons = [];
  for (const e of ponds.slice(0, 4)) {
    const near = shallowsOf(e, 1)[0];
    if (near) herons.push({ x: near.x, y: near.y - 0.18, z: near.z, yaw: rng() * 6.28, phase: rng() * 10, s: 1 });
  }
  for (const e of ponds) {
    for (const p of shallowsOf(e, rng() < 0.5 ? 1 : 2)) herons.push({ x: p.x, y: p.y - 0.15, z: p.z, yaw: rng() * 6.28, phase: rng() * 10, s: 0.82, egret: true });
  }
  const blues = herons.filter((h) => !h.egret), whites = herons.filter((h) => h.egret);
  const heronMesh = instanced(heronGeometry(), new THREE.MeshLambertMaterial({ color: 0x7d8c99 }), blues, (h) => {
    Q.setFromAxisAngle(UP, h.yaw); M.compose(P.set(h.x, h.y, h.z), Q, S.set(1, 1, 1));
  });
  const egretMesh = instanced(heronGeometry(), new THREE.MeshLambertMaterial({ color: 0xf2efe6 }), whites, (h) => {
    Q.setFromAxisAngle(UP, h.yaw); M.compose(P.set(h.x, h.y, h.z), Q, S.setScalar(h.s));
  });

  return {
    meshes, herons, counts: { stalks: stalks.length, tufts: tufts.length, pads: pads.length, knees: knees.length, herons: blues.length, egrets: whites.length, moss: moss.length, logs: logs.length, turtles: turtles.length },
    /** The herons and egrets shift their weight and turn a little now and then. */
    update(t) {
      for (const [mesh, list] of [[heronMesh, blues], [egretMesh, whites]]) {
        if (!mesh) continue;
        list.forEach((h, i) => {
          const turn = Math.sin(t * 0.13 + h.phase) * 0.6 + (Math.sin(t * 0.7 + h.phase * 3) > 0.97 ? 0.25 : 0);
          Q.setFromAxisAngle(UP, h.yaw + turn); M.compose(P.set(h.x, h.y, h.z), Q, S.setScalar(h.s));
          mesh.setMatrixAt(i, M);
        });
        mesh.instanceMatrix.needsUpdate = true;
      }
    },
  };
}
