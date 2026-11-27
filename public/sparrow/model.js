// Loads a street-smarts GLB into a few cheap meshes, the element table, and
// one collision BVH.
//
// A packed GLB (tools/vibe-render/pack_glb.py) arrives as one mesh per
// material, with the element table in its root extras. An older, unpacked
// one (a node per solid, a primitive per face) is merged here by material,
// so it draws in ~15 calls too; its elements come from the node names.
import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";
import { MeshBVH } from "three-mesh-bvh";
import { readElements } from "./answers.js";
import { takeTrees } from "./trees.js";
import { readDetail } from "./frontages.js";

const NAME = /^(.*?)_(\d+)_(.*)$/; // render.py's f"{group}_{n}_{kind}"
// Kinds that keep their translucency here: the halos, and the ghostly
// low-confidence kinds (render.py's CONFIDENCE_ALPHA). Everything else is
// solid: you walk around it.
const SEE_THROUGH = new Set(["undecided", "building_unshaped"]);

export async function loadSite(url, onProgress) {
  const gltf = await new GLTFLoader().loadAsync(url, (e) => {
    if (onProgress) onProgress(e.lengthComputable && e.total ? e.loaded / e.total : null);
  });
  gltf.scene.updateMatrixWorld(true);
  let table = null;
  gltf.scene.traverse((o) => { if (o.userData && o.userData.streetSmarts) table = o.userData.streetSmarts; });
  // P171's trees are instanced; they're drawn and collided apart (trees.js).
  const trees = table ? takeTrees(gltf.scene, readElements(table)) : null;
  // Walls, openings and shop fronts, for the street-level detail (frontages.js).
  const detail = table ? await readDetail(gltf.parser, table).catch(() => null) : null;
  const parts = table ? packedParts(gltf, table) : looseParts(gltf);
  // Waypoints: glTF camera nodes a pipeline placed (a tour's stops), with where to look in their extras.
  const waypoints = new Map();
  gltf.scene.traverse((o) => {
    const x = o.userData;
    if (!x || !x.waypoint) return;
    const eye = o.getWorldPosition(new THREE.Vector3());
    const look = Array.isArray(x.look) ? new THREE.Vector3(...x.look) : eye.clone().add(o.getWorldDirection(new THREE.Vector3()).negate());
    waypoints.set(x.waypoint, { id: x.waypoint, eye, stand: new THREE.Vector3(eye.x, 0.2, eye.z), look, title: x.title || null, source: x.source || null });
  });
  return { ...assemble(parts), trees, detail, transit: (table && table.transit) || [], facts: (table && table.facts) || null, waypoints, overlays: (table && table.overlays) || [] };
}

// -> { elements, pieces: [{ geometry (world space, position+normal, indexed), ranges, source material }] }
function packedParts(gltf, table) {
  const elements = readElements(table);
  const pieces = [];
  gltf.scene.traverse((o) => {
    if (!o.isMesh || o.isInstancedMesh) return;
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", o.geometry.attributes.position);
    g.setAttribute("normal", o.geometry.attributes.normal);
    g.setIndex(o.geometry.index);
    g.applyMatrix4(o.matrixWorld);
    pieces.push({ geometry: g, ranges: o.userData.ranges || [], material: o.material });
  });
  return { elements, pieces };
}

function looseParts(gltf) {
  const elements = [];
  const byMaterial = new Map();
  gltf.scene.traverse((o) => {
    if (!o.isMesh) return;
    let named = o, m = null;
    while (named && !(m = NAME.exec(named.name || ""))) named = named.parent;
    let e = elements.findIndex((x) => x.node === named);
    if (e < 0) {
      e = elements.length;
      elements.push({ index: e, node: named, group: m ? m[1] : "other", kind: m ? m[3] : (o.name || ""), source: null, typology: null, floors: null });
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", o.geometry.attributes.position.clone());
    g.setAttribute("normal", o.geometry.attributes.normal.clone());
    g.setIndex(o.geometry.index ? o.geometry.index.clone() : null);
    g.applyMatrix4(o.matrixWorld);
    const slot = byMaterial.get(o.material) || { geos: [], ranges: [], tris: 0 };
    const tris = (g.index ? g.index.count : g.attributes.position.count) / 3;
    slot.ranges.push(e, slot.tris, tris);
    slot.tris += tris;
    slot.geos.push(g.index ? g : g.toNonIndexed());
    byMaterial.set(o.material, slot);
  });
  for (const el of elements) delete el.node;
  const pieces = [];
  for (const [material, slot] of byMaterial) {
    pieces.push({ geometry: mergeGeometries(slot.geos, false), ranges: slot.ranges, material });
  }
  return { elements, pieces };
}

/**
 * Walls darken a little toward the ground they stand on (a cheap stand-in for
 * the light a street loses between buildings), and each solid takes its tint.
 * Ground slabs and roofs aren't darkened: only faces that stand up.
 */
function shadeWalls(material) {
  material.onBeforeCompile = (shader) => {
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", "#include <common>\nattribute float aTint;\nvarying float vTint, vWall, vY;")
      .replace("#include <project_vertex>", `#include <project_vertex>
        vTint = aTint;
        vec3 wn = normalize(mat3(modelMatrix) * objectNormal);
        vWall = 1.0 - abs(wn.y);
        vY = (modelMatrix * vec4(transformed, 1.0)).y;`);
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", "#include <common>\nvarying float vTint, vWall, vY;")
      .replace("#include <color_fragment>", `#include <color_fragment>
        float ao = mix(0.68, 1.0, smoothstep(0.0, 2.6, vY)) * mix(1.0, 1.06, smoothstep(6.0, 20.0, vY));
        diffuseColor.rgb *= vTint * mix(1.0, ao, vWall);`);
  };
}

/** The water's clock (P64's ponds and streams ripple): main.js sets .value each frame. */
export const waterTime = { value: 0 };
/** Open water: a sheen that catches the sun, and a slow ripple in it. */
function waterMaterial(color) {
  const m = new THREE.MeshPhongMaterial({ color, specular: 0x9cc0c4, shininess: 90 });
  m.onBeforeCompile = (shader) => {
    shader.uniforms.uTime = waterTime;
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", "#include <common>\nvarying vec3 vWPos;")
      .replace("#include <begin_vertex>", "#include <begin_vertex>\nvWPos = (modelMatrix * vec4(transformed, 1.0)).xyz;");
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", "#include <common>\nuniform float uTime;\nvarying vec3 vWPos;")
      .replace("#include <normal_fragment_maps>", `#include <normal_fragment_maps>
        normal = normalize(normal + vec3(sin(vWPos.x * 0.9 + uTime * 1.3) + sin(vWPos.z * 1.7 - uTime * 0.8) * 0.5, 0.0,
                                         cos(vWPos.z * 0.8 + uTime * 1.1) + cos(vWPos.x * 1.5 + uTime * 0.6) * 0.5) * 0.05);`);
  };
  return m;
}

function assemble({ elements, pieces }) {
  const bounds = new THREE.Box3();
  const boxes = elements.map(() => new THREE.Box3());
  const meshes = [];
  const colliderGeos = [];
  const v = new THREE.Vector3();

  for (const piece of pieces) {
    const g = piece.geometry;
    const pos = g.attributes.position, idx = g.index;
    const r = piece.ranges;
    // Element of every vertex (each solid has its own vertices), for the collider.
    const vertEl = new Float32Array(pos.count).fill(-1);
    const kinds = new Set(), groups = new Set();
    for (let k = 0; k < r.length; k += 3) {
      const e = r[k], box = boxes[e];
      if (elements[e]) { kinds.add(elements[e].kind); groups.add(elements[e].group); }
      for (let t = r[k + 1] * 3, end = (r[k + 1] + r[k + 2]) * 3; t < end; t++) {
        const vi = idx ? idx.getX(t) : t;
        vertEl[vi] = e;
        box.expandByPoint(v.fromBufferAttribute(pos, vi));
      }
    }
    g.computeBoundingBox();
    g.computeBoundingSphere();
    bounds.union(g.boundingBox);

    const src = piece.material || {};
    const halo = groups.has("activity_glow");
    const seeThrough = halo || [...kinds].some((k) => SEE_THROUGH.has(k));
    const glow = groups.size > 0 && [...groups].every((g) => g === "activity" || g === "activity_glow");
    const color = src.color ? src.color.clone() : new THREE.Color("#999999");
    const isContext = kinds.size === 1 && kinds.has("context");
    if (isContext) color.lerp(new THREE.Color("#d9cfc0"), 0.35); // backdrop, not the model
    const isWater = kinds.size > 0 && [...kinds].every((k) => k === "pond" || k === "stream");
    const m = isWater ? waterMaterial(color) : new THREE.MeshLambertMaterial({ color });
    if (!isWater) shadeWalls(m, kinds);
    // A slight brightness difference per solid, so neighbors read apart (style, not data).
    const tint = new Float32Array(pos.count).fill(1);
    for (let k = 0; k < r.length; k += 3) {
      const h = Math.sin(r[k] * 12.9898) * 43758.5453;
      const v = 0.93 + 0.12 * (h - Math.floor(h));
      for (let tt = r[k + 1] * 3, end = (r[k + 1] + r[k + 2]) * 3; tt < end; tt++) tint[idx ? idx.getX(tt) : tt] = v;
    }
    g.setAttribute("aTint", new THREE.BufferAttribute(tint, 1));
    if (seeThrough && src.opacity < 0.99) {
      m.transparent = true;
      m.opacity = halo ? 0.45 : Math.max(0.35, src.opacity);
      m.depthWrite = false;
    }
    if (src.emissive && (src.emissive.r + src.emissive.g + src.emissive.b) > 0) {
      m.emissive.copy(src.emissive);
      m.emissiveIntensity = Math.min(2.5, src.emissiveIntensity || 1);
    }
    if (glow) { m.emissive.copy(color); m.emissiveIntensity = halo ? 0.6 : 0.25; }
    const mesh = new THREE.Mesh(g, m);
    mesh.userData = { ranges: r, kinds, groups, glow, halo };
    mesh.matrixAutoUpdate = false;
    meshes.push(mesh);

    // The collider: positions and element ids only.
    const cg = new THREE.BufferGeometry();
    cg.setAttribute("position", pos);
    cg.setAttribute("element", new THREE.BufferAttribute(vertEl, 1));
    cg.setIndex(idx);
    colliderGeos.push(cg);
  }

  const colliderGeometry = mergeGeometries(colliderGeos, false);
  const bvh = new MeshBVH(colliderGeometry, { targetLeafSize: 8 });
  const elementAttr = colliderGeometry.attributes.element;

  for (const [i, el] of elements.entries()) {
    el.box = boxes[i];
    el.center = boxes[i].isEmpty() ? null : boxes[i].getCenter(new THREE.Vector3());
  }
  return {
    meshes, elements, bounds, bvh,
    /** The element a raycastFirst hit landed on, or null. */
    elementOfHit(hit) {
      if (!hit || !hit.face) return null;
      const e = elementAttr.getX(hit.face.a);
      return e >= 0 ? elements[e] : null;
    },
  };
}
