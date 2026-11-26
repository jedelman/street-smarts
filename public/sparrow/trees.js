// P171's trees in the walk: the GLB's two instanced meshes (crowns and
// trunks, EXT_mesh_gpu_instancing; see tools/vibe-render/pack_glb.py), given a
// breeze, a soft shadow each, and a collider so a sparrow can perch in one.
//
// Instance i of either mesh is tree i, and tree i is element base + i in the
// GLB's element table, so a hit on a tree answers like anything else.
import * as THREE from "three";
import { MeshBVH } from "three-mesh-bvh";

const CROWN_SQUASH = 0.9; // pack_glb.CROWN_SQUASH
const CHUNK_M = 130; // chunk size: about 25 chunks on clean_baseline, 2 draws each
const NEAR_M = 60; // a chunk within this of the camera draws full crowns
// Used when a loader leaves the instance colors out (pack_glb.TREE_CROWN_COLORS).
const FORM_COLORS = { grove: "#3f6b35", avenue: "#4d7d3e", square: "#5b8c43", umbrella: "#35602e", cypress: "#a3562b" };

/**
 * From the loaded glTF scene: the crown and trunk InstancedMeshes, taken out of
 * the scene graph. `elements` is the element table. Null when there are none.
 */
export function takeTrees(gltfScene, elements) {
  let crowns = null, trunks = null;
  gltfScene.traverse((o) => {
    if (!o.isInstancedMesh) return;
    if (/crown/.test(o.name) || /crown/.test(o.material?.name || "")) crowns = o;
    else if (/trunk/.test(o.name) || /trunk/.test(o.material?.name || "")) trunks = o;
  });
  if (!crowns || !trunks) return null;
  let base = crowns.userData?.elementBase;
  if (!Number.isInteger(base)) base = elements.findIndex((e) => e.group === "trees");
  if (base < 0) return null;
  crowns.removeFromParent();
  trunks.removeFromParent();
  return { crowns, trunks, base };
}

/**
 * Trees ready to draw and touch: { meshes, bvh, list, elementOfHit(hit), bounds, update(t) }.
 * `groundAt(x, z)` gives the ground's height under a trunk, for its shadow.
 */
export function buildTrees(scene, { crowns, trunks, base }, elements, groundAt, shadowTexture) {
  const n = crowns.count;
  const m = new THREE.Matrix4(), p = new THREE.Vector3(), q = new THREE.Quaternion(), s = new THREE.Vector3();
  const list = [];
  for (let i = 0; i < n; i++) {
    crowns.getMatrixAt(i, m);
    m.decompose(p, q, s);
    // The crown's own height (s.y: CROWN_SQUASH crown radii for most trees, more for P64's tall, narrow cypress).
    list.push({ x: p.x, z: p.z, cy: p.y, r: s.x, top: p.y + s.y, el: elements[base + i] || null });
  }

  // A breeze: the upper crown sways, each tree on its own phase.
  const uTime = { value: 0 };
  const crownMat = new THREE.MeshLambertMaterial({ color: 0xffffff });
  crownMat.onBeforeCompile = (shader) => {
    shader.uniforms.uTime = uTime;
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", "#include <common>\nuniform float uTime;")
      .replace("#include <begin_vertex>", `#include <begin_vertex>
        float ph = instanceMatrix[3].x * 0.37 + instanceMatrix[3].z * 0.23;
        float lift = clamp(position.y + 0.5, 0.0, 1.5);
        transformed.x += sin(uTime * 1.3 + ph) * 0.045 * lift;
        transformed.z += cos(uTime * 1.05 + ph * 1.7) * 0.035 * lift;`);
  };
  if (!crowns.instanceColor) {
    const c = new THREE.Color();
    for (let i = 0; i < n; i++) {
      const h = Math.sin((base + i) * 12.9898) * 43758.5453;
      c.set(FORM_COLORS[list[i].el?.kind] || FORM_COLORS.grove).multiplyScalar(0.85 + 0.3 * (h - Math.floor(h)));
      crowns.setColorAt(i, c);
    }
  }
  const trunkMat = new THREE.MeshLambertMaterial({ color: trunks.material?.color || new THREE.Color("#5b4433") });

  // Drawn in chunks, so the camera's frustum culls the ones out of view, and
  // a chunk far away draws its crowns with 20 faces instead of 80.
  const lowCrown = new THREE.IcosahedronGeometry(1, 0).toNonIndexed();
  const lp = lowCrown.attributes.position;
  for (let k = 0; k < lp.count; k++) {
    const y = lp.getY(k);
    if (y < -0.35) lp.setY(k, -0.35 + (y + 0.35) * 0.5); // the same flatter underside (pack_glb.unit_crown)
  }
  lowCrown.computeVertexNormals(); // non-indexed: flat, like the near crown
  const chunks = [];
  const byChunk = new Map();
  for (let i = 0; i < n; i++) {
    const key = `${Math.floor(list[i].x / CHUNK_M)},${Math.floor(list[i].z / CHUNK_M)}`;
    if (!byChunk.has(key)) byChunk.set(key, []);
    byChunk.get(key).push(i);
  }
  const col = new THREE.Color();
  for (const ids of byChunk.values()) {
    const make = (geometry, material, src) => {
      const im = new THREE.InstancedMesh(geometry, material, ids.length);
      ids.forEach((id, k) => { src.getMatrixAt(id, m); im.setMatrixAt(k, m); });
      if (src.instanceColor) ids.forEach((id, k) => { src.getColorAt(id, col); im.setColorAt(k, col); });
      im.computeBoundingSphere();
      im.matrixAutoUpdate = false;
      scene.add(im);
      return im;
    };
    const near = make(crowns.geometry, crownMat, crowns);
    const far = make(lowCrown, crownMat, crowns);
    far.visible = false;
    const trunk = make(trunks.geometry, trunkMat, trunks);
    chunks.push({ near, far, trunk, sphere: near.boundingSphere });
  }

  // A soft shadow under each crown, on whatever ground the trunk stands on.
  const shadowGeo = new THREE.PlaneGeometry(2, 2).rotateX(-Math.PI / 2);
  const shadows = new THREE.InstancedMesh(shadowGeo,
    new THREE.MeshBasicMaterial({ map: shadowTexture, transparent: true, depthWrite: false, opacity: 0.55, color: 0x1d2a17, fog: true }), n);
  for (let i = 0; i < n; i++) {
    const t = list[i];
    t.ground = groundAt(t.x, t.z, t.cy);
    m.compose(p.set(t.x, t.ground + 0.015, t.z), q.identity(), s.set(t.r * 1.15, 1, t.r * 1.15));
    shadows.setMatrixAt(i, m);
  }
  shadows.frustumCulled = false;
  shadows.renderOrder = 1;
  scene.add(shadows);

  // The collider: a 20-face crown a little inside the drawn one, and the trunk,
  // written straight into one buffer (no per-tree geometry objects).
  const unitCrown = new THREE.IcosahedronGeometry(0.94, 0).toNonIndexed().attributes.position;
  const unitTrunk = new THREE.CylinderGeometry(1, 1, 1, 5, 1, true).translate(0, 0.5, 0).toNonIndexed().attributes.position;
  const per = unitCrown.count + unitTrunk.count;
  const pos = new Float32Array(n * per * 3), elem = new Float32Array(n * per);
  const v = new THREE.Vector3();
  let w = 0;
  for (let i = 0; i < n; i++) {
    for (const [im, unit] of [[crowns, unitCrown], [trunks, unitTrunk]]) {
      im.getMatrixAt(i, m);
      for (let k = 0; k < unit.count; k++, w++) {
        v.fromBufferAttribute(unit, k).applyMatrix4(m);
        pos[w * 3] = v.x; pos[w * 3 + 1] = v.y; pos[w * 3 + 2] = v.z;
        elem[w] = base + i;
      }
    }
  }
  const collider = new THREE.BufferGeometry();
  collider.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  collider.setAttribute("element", new THREE.BufferAttribute(elem, 1));
  const bvh = new MeshBVH(collider, { targetLeafSize: 8 });
  const elementAttr = collider.attributes.element;

  const all = [shadows];
  for (const c of chunks) all.push(c.near, c.far, c.trunk);
  return {
    meshes: all,
    bvh, list,
    elementOfHit(hit) {
      if (!hit || !hit.face) return null;
      return elements[elementAttr.getX(hit.face.a)] || null;
    },
    /** The breeze, and each chunk's crowns near or far by the camera's distance. */
    update(t, camera) {
      uTime.value = t;
      if (!camera) return;
      for (const c of chunks) {
        const d = camera.position.distanceTo(c.sphere.center) - c.sphere.radius;
        const near = d < NEAR_M;
        if (c.near.visible !== near) { c.near.visible = near; c.far.visible = !near; }
      }
    },
  };
}
