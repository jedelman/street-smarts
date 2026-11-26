// Bronze birds and cats on the markers, in homage to Fernando Botero (1932-2023):
// fat, round, serene, their heads small on swollen bodies, their feet tiny.
// Ours, not his: a plump dove standing and a cat sitting with its tail
// wrapped round its paws, made of spheres, cones and a tube; none of his
// sculptures is copied. Not data: the pipeline places a marker (a post with a
// pool of light) where P126 and P61 ask for something in the middle; the walk
// swaps the post for a plinth and a figure. Instanced: three draw calls, and
// a BVH so the sparrow can stand on a bronze head.
import * as THREE from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";
import { MeshBVH } from "three-mesh-bvh";

const PLINTH = { w: 1.7, h: 0.7 };

/** A part: geometry, scaled, rotated (x then y then z), then moved. */
function part(geo, { s = [1, 1, 1], r = [0, 0, 0], t = [0, 0, 0] } = {}) {
  geo.scale(s[0], s[1], s[2]);
  if (r[0]) geo.rotateX(r[0]);
  if (r[1]) geo.rotateY(r[1]);
  if (r[2]) geo.rotateZ(r[2]);
  return geo.translate(t[0], t[1], t[2]);
}
const ball = (n = 22) => new THREE.SphereGeometry(1, n, Math.round(n * 0.7));

/** A dove, standing, facing +z, feet at y = 0: about 1.75 tall before scaling. */
function dove() {
  const tail = new THREE.ConeGeometry(0.32, 0.85, 14);
  tail.scale(1, 1, 0.3);
  return mergeGeometries([
    part(ball(), { s: [0.8, 0.72, 0.98], t: [0, 0.82, 0] }), // the body
    part(ball(), { s: [0.62, 0.56, 0.52], t: [0, 0.78, 0.42] }), // the breast, swelling forward
    part(ball(16), { s: [0.3, 0.3, 0.32], t: [0, 1.5, 0.56] }), // a small head
    part(new THREE.ConeGeometry(0.07, 0.22, 10), { r: [Math.PI / 2, 0, 0], t: [0, 1.47, 0.95] }), // the beak
    part(tail, { r: [Math.PI / 2 + 0.4, 0, 0], t: [0, 1.0, -1.0] }), // the tail, fanned and lifted
    part(ball(14), { s: [0.1, 0.34, 0.66], r: [0.2, 0, 0.12], t: [0.66, 0.95, -0.12] }), // wings, folded close
    part(ball(14), { s: [0.1, 0.34, 0.66], r: [0.2, 0, -0.12], t: [-0.66, 0.95, -0.12] }),
    part(new THREE.CylinderGeometry(0.05, 0.06, 0.24, 8), { t: [0.22, 0.12, 0.12] }), // tiny feet
    part(new THREE.CylinderGeometry(0.05, 0.06, 0.24, 8), { t: [-0.22, 0.12, 0.12] }),
  ], false);
}

/** A cat, sitting, tail round its paws, facing +z, at y = 0: about 2.4 tall. */
function cat() {
  const tailPath = new THREE.CatmullRomCurve3([
    new THREE.Vector3(0.1, 0.25, -0.75), new THREE.Vector3(0.75, 0.18, -0.45), new THREE.Vector3(0.98, 0.14, 0.15),
    new THREE.Vector3(0.62, 0.13, 0.72), new THREE.Vector3(0.05, 0.12, 0.9),
  ]);
  return mergeGeometries([
    part(ball(), { s: [0.85, 0.95, 0.8], t: [0, 0.98, -0.05] }), // the body, a pear
    part(ball(), { s: [0.98, 0.62, 0.92], t: [0, 0.6, 0.02] }), // its broad seat
    part(ball(18), { s: [0.46, 0.41, 0.43], t: [0, 2.0, 0.16] }), // the head
    part(ball(12), { s: [0.17, 0.12, 0.13], t: [0, 1.9, 0.56] }), // the muzzle
    part(new THREE.ConeGeometry(0.13, 0.3, 10), { r: [0, 0, -0.35], t: [0.25, 2.42, 0.12] }), // ears
    part(new THREE.ConeGeometry(0.13, 0.3, 10), { r: [0, 0, 0.35], t: [-0.25, 2.42, 0.12] }),
    part(ball(12), { s: [0.18, 0.13, 0.24], t: [0.23, 0.13, 0.66] }), // front paws
    part(ball(12), { s: [0.18, 0.13, 0.24], t: [-0.23, 0.13, 0.66] }),
    new THREE.TubeGeometry(tailPath, 28, 0.12, 8, false), // the tail, wrapped round
  ], false);
}

/** Positions only, in world space, for the collider; `id` tags every vertex. */
function baked(geo, matrix, id) {
  const g = (geo.index ? geo.toNonIndexed() : geo.clone()).applyMatrix4(matrix);
  for (const k of Object.keys(g.attributes)) if (k !== "position") g.deleteAttribute(k);
  g.setAttribute("sc", new THREE.BufferAttribute(new Float32Array(g.attributes.position.count).fill(id), 1));
  return g;
}

/**
 * The marker's post, gone: every vertex of its triangles pulled to its foot,
 * in the drawn mesh and the collider (they share positions). Its pool of
 * light stays.
 */
export function hidePosts(site, els) {
  const want = new Set(els);
  for (const mesh of site.meshes) {
    if (!mesh.userData.groups || !mesh.userData.groups.has("activity")) continue;
    const g = mesh.geometry, pos = g.attributes.position, idx = g.index, r = mesh.userData.ranges;
    let changed = false;
    for (let k = 0; k < r.length; k += 3) {
      const el = site.elements[r[k]];
      if (!want.has(el)) continue;
      const c = el.box.getCenter(new THREE.Vector3()), y = el.box.min.y;
      for (let t = r[k + 1] * 3, end = (r[k + 1] + r[k + 2]) * 3; t < end; t++) pos.setXYZ(idx ? idx.getX(t) : t, c.x, y, c.z);
      changed = true;
    }
    if (changed) { pos.needsUpdate = true; g.computeBoundingSphere(); }
  }
}

/**
 * markers: [{ el, x, y (its foot), z, seed (0..1) }]. Returns { meshes, bvh,
 * byElement: Map(el -> "dove" | "cat"), elementOfHit(hit) }.
 */
export function buildSculptures(scene, markers) {
  const bronze = new THREE.MeshPhongMaterial({ color: 0x5a3b23, specular: 0x6e5236, shininess: 28 }); // a dull patina, warm where the sun catches it
  const stone = new THREE.MeshLambertMaterial({ color: 0xcfc5b2 });
  const forms = { dove: dove(), cat: cat() };
  const plinthGeo = new THREE.BoxGeometry(PLINTH.w, PLINTH.h, PLINTH.w).translate(0, PLINTH.h / 2, 0);
  const which = markers.map((m) => (m.seed < 0.5 ? "dove" : "cat"));
  const count = (k) => which.filter((w) => w === k).length;
  const inst = {
    dove: new THREE.InstancedMesh(forms.dove, bronze, Math.max(1, count("dove"))),
    cat: new THREE.InstancedMesh(forms.cat, bronze, Math.max(1, count("cat"))),
    plinth: new THREE.InstancedMesh(plinthGeo, stone, Math.max(1, markers.length)),
  };
  for (const m of Object.values(inst)) { m.count = 0; scene.add(m); }

  const M = new THREE.Matrix4(), Q = new THREE.Quaternion(), P = new THREE.Vector3(), S = new THREE.Vector3(), UP = new THREE.Vector3(0, 1, 0);
  const colliders = [];
  const byElement = new Map();
  markers.forEach((m, i) => {
    const kind = which[i];
    const yaw = m.seed * 977 % (Math.PI * 2);
    const size = (kind === "dove" ? 1.25 : 0.95) * (0.9 + (m.seed * 131 % 1) * 0.25);
    Q.setFromAxisAngle(UP, yaw);
    M.compose(P.set(m.x, m.y, m.z), Q, S.set(1, 1, 1));
    inst.plinth.setMatrixAt(inst.plinth.count++, M);
    colliders.push(baked(plinthGeo, M, i));
    M.compose(P.set(m.x, m.y + PLINTH.h, m.z), Q, S.setScalar(size));
    inst[kind].setMatrixAt(inst[kind].count++, M);
    colliders.push(baked(forms[kind], M, i));
    byElement.set(m.el, kind);
  });
  for (const m of Object.values(inst)) { m.instanceMatrix.needsUpdate = true; m.computeBoundingSphere(); }

  const geo = mergeGeometries(colliders, false);
  const bvh = new MeshBVH(geo);
  const sc = geo.attributes.sc;
  return {
    meshes: Object.values(inst), bvh, byElement,
    /** The marker whose sculpture a raycast hit, or null. */
    elementOfHit(hit) {
      if (!hit || !hit.face) return null;
      const m = markers[Math.round(sc.getX(hit.face.a))];
      return m ? m.el : null;
    },
  };
}
