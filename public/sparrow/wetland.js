// What grows and stands in P64's wetland, in the walk: cattails gone brown
// and fluffy in the marsh, as they are in late November, tufts of rush,
// lily pads on the ponds, knees round every bald cypress, and a great blue
// heron in the shallows of each pond. The ponds, marsh, streams and cypress
// are the pipeline's (P64 Pools and Streams); everything here is ours, not
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
    if (rng() < 0.65) for (let k = 0, n = 4 + Math.floor(rng() * 6); k < n; k++)
      stalks.push({ x: c.x + (rng() - 0.5) * 1.4, y: c.y, z: c.z + (rng() - 0.5) * 1.4, h: 1.3 + rng() * 0.8, lean: (rng() - 0.5) * 0.25, yaw: rng() * 6.28 });
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
      if (hit) knees.push({ x, y: hit.y, z, h: 0.25 + rng() * 0.6, r: 0.07 + rng() * 0.12 });
    }
  }
  const kneeGeo = new THREE.ConeGeometry(1, 1, 7).translate(0, 0.5, 0);
  instanced(kneeGeo, new THREE.MeshLambertMaterial({ color: 0x8a7462 }), knees, (k) => {
    M.compose(P.set(k.x, k.y - 0.05, k.z), Q.identity(), S.set(k.r, k.h, k.r));
  });

  // A great blue heron in the shallows of each pond (up to four), still, now and then lowering its head.
  const herons = [];
  for (const e of ponds.slice(0, 4)) {
    // Near the edge: a point in the pond with marsh a couple of metres off.
    const near = sample(e, 40, rng, drop).find((p) => [0, 1, 2, 3, 4, 5].some((k) => {
      const hit = drop(p.x + Math.cos(k) * 2.5, p.z + Math.sin(k) * 2.5);
      return hit && hit.el && hit.el.kind === "marsh";
    }));
    if (near) herons.push({ x: near.x, y: near.y - 0.18, z: near.z, yaw: rng() * 6.28, phase: rng() * 10 });
  }
  const heronMesh = instanced(heronGeometry(), new THREE.MeshLambertMaterial({ color: 0x7d8c99 }), herons, (h) => {
    Q.setFromAxisAngle(UP, h.yaw); M.compose(P.set(h.x, h.y, h.z), Q, S.set(1, 1, 1));
  });

  return {
    meshes, herons, counts: { stalks: stalks.length, tufts: tufts.length, pads: pads.length, knees: knees.length, herons: herons.length },
    /** The herons shift their weight and turn a little now and then. */
    update(t) {
      if (!heronMesh) return;
      herons.forEach((h, i) => {
        const turn = Math.sin(t * 0.13 + h.phase) * 0.6 + (Math.sin(t * 0.7 + h.phase * 3) > 0.97 ? 0.25 : 0);
        Q.setFromAxisAngle(UP, h.yaw + turn); M.compose(P.set(h.x, h.y, h.z), Q, S.set(1, 1, 1));
        heronMesh.setMatrixAt(i, M);
      });
      heronMesh.instanceMatrix.needsUpdate = true;
    },
  };
}
