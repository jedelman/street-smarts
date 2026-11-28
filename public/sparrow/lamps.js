// Ghent's streetlights, where the City's data puts them (tools/ghent/lights.py, Dominion Energy's records): each by
// its fixture family and pole. The families are our reading of the data's fixture codes against the styles in
// Dominion's Virginia catalogue: Colonial lanterns on short poles (its Traditional Colonial stands on a 13-foot pole),
// Acorns, and Cobra and Shoebox heads on arms over the street. The shapes and the glow at dusk are ours.
//
// Instanced and chunked like the shop fronts (frontages.js's Parts): the poles from far off, the lit heads from
// further still, and the pools of light only up close.
import * as THREE from "three";
import { Parts, FULL_BOX, SHRUB, CONE, POST } from "./frontages.js";
import { lampParts } from "./lamp-parts.js";

/** Every light in `rows` ([x, z, family, pole, yaw]). Returns { meshes, count, update(camera) }. */
export function buildLamps(scene, rows, { coarse = false } = {}) {
  const lambert = new THREE.MeshLambertMaterial({ color: 0xffffff });
  const glow = new THREE.MeshBasicMaterial({ color: 0xffffff });
  const pool = new THREE.MeshBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0.16, depthWrite: false, blending: THREE.AdditiveBlending });
  const disc = new THREE.CircleGeometry(0.5, 20).rotateX(-Math.PI / 2);
  const parts = new Parts({
    pole: { geometry: POST, material: lambert, reach: 320, chunk: 320 },
    box: { geometry: FULL_BOX, material: lambert, reach: 200, chunk: 260 },
    cone: { geometry: CONE, material: lambert, reach: 200, chunk: 260 },
    glow: { geometry: FULL_BOX, material: glow, reach: 520, chunk: 400 },
    glowball: { geometry: SHRUB, material: glow, reach: 520, chunk: 400 },
    pool: { geometry: disc, material: pool, reach: 80, chunk: 140 },
  });
  const M = new THREE.Matrix4(), R = new THREE.Matrix4(), S = new THREE.Matrix4(), T = new THREE.Matrix4();
  const col = new THREE.Color(), v = new THREE.Vector3();
  for (const [x, z, family, pole, yaw] of rows) {
    R.makeRotationY(yaw || 0);
    for (const [k, px, py, pz, sx, sy, sz, c] of lampParts(family, pole)) {
      v.set(px, py, pz).applyMatrix4(R);
      M.copy(T.makeTranslation(x + v.x, v.y, z + v.z)).multiply(R).multiply(S.makeScale(sx, sy, sz));
      parts.add(k, M, col.set(c));
    }
  }
  const meshes = parts.build(scene);
  for (const m of meshes) if (m.userData.kind === "pool") m.renderOrder = 2;
  const scale = coarse ? 0.75 : 1;
  return {
    meshes, count: rows.length,
    update(camera) {
      const c = camera.position;
      for (const m of meshes) {
        const s = m.boundingSphere;
        m.visible = c.distanceTo(s.center) - s.radius < m.userData.reach * scale;
      }
    },
  };
}
