// The flow overlay (flows.js's model, drawn): glowing arcs over the roofs
// from where the money is (homes, desks, the tram) to the shop fronts that
// take it, thicker for more, with pulses running the way it goes, and a ring
// on each building sized by what its shop fronts take in. One draw call for
// the arcs, one for the rings; hidden until it's switched on.

import * as THREE from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";
import { topFlows } from "./flows.js";

const TINT = { home: new THREE.Color("#ffb347"), desk: new THREE.Color("#6ec6ff"), tram: new THREE.Color("#7be38a") };
const LIFT = 3; // m above a roof an arc starts and lands

const arcMaterial = () => new THREE.ShaderMaterial({
  uniforms: { time: { value: 0 }, opacity: { value: 0 } },
  vertexShader: /* glsl */ `
    attribute float along; attribute float len; attribute vec3 tint;
    varying float vAlong; varying float vLen; varying vec3 vTint;
    void main() {
      vAlong = along; vLen = len; vTint = tint;
      gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    }`,
  fragmentShader: /* glsl */ `
    uniform float time; uniform float opacity;
    varying float vAlong; varying float vLen; varying vec3 vTint;
    void main() {
      // A pulse every 36 m, running from the spender to the shops at 18 m/s.
      float p = fract((vAlong * vLen - time * 18.0) / 36.0);
      float pulse = smoothstep(0.7, 0.93, p) * (1.0 - smoothstep(0.93, 1.0, p));
      float ends = smoothstep(0.0, 0.05, vAlong) * (1.0 - smoothstep(0.95, 1.0, vAlong));
      gl_FragColor = vec4(vTint * (0.45 + 1.3 * pulse), (0.3 + 0.7 * pulse) * opacity * ends);
    }`,
  transparent: true, depthWrite: false, blending: THREE.AdditiveBlending, fog: false,
});

/**
 * `model`: computeFlows's result. `places`: Map(id -> { from: Vector3, to:
 * Vector3 }), where a building's money leaves from and where its shop fronts
 * take it in (the station's id is "station"). Returns { group, shown,
 * show(on), update(t, dt), bounds (what the arcs span), drawn: { arcs, share } }.
 */
export function buildFlowLines(scene, model, places, { coarse = false } = {}) {
  const group = new THREE.Group();
  group.visible = false;
  scene.add(group);
  const picked = topFlows(model.flows, coarse ? 24 : 42).filter((f) => places.has(f.from) && places.has(f.to));
  const sized = picked.filter((f) => f.kind !== "tram");
  const max = Math.max(1, ...sized.map((f) => f.amount));
  const radial = coarse ? 4 : 6;
  const geos = [];
  const a = new THREE.Vector3(), b = new THREE.Vector3(), c = new THREE.Vector3(), side = new THREE.Vector3();
  for (const f of picked) {
    a.copy(places.get(f.from).from);
    b.copy(places.get(f.to).to);
    const d = Math.hypot(b.x - a.x, b.z - a.z);
    if (d < 4) continue;
    // Over the roofs, higher the farther it goes; homes and desks bow to opposite sides so a pair doesn't overlap.
    side.set(-(b.z - a.z), 0, b.x - a.x).normalize().multiplyScalar(f.kind === "desk" ? 0.06 * d : f.kind === "home" ? -0.06 * d : 0);
    c.addVectors(a, b).multiplyScalar(0.5).add(side);
    c.y = Math.max(a.y, b.y) + 8 + 0.3 * d;
    const curve = new THREE.QuadraticBezierCurve3(a.clone(), c.clone(), b.clone());
    const r = f.kind === "tram" ? 0.5 : 0.5 + 2.8 * Math.sqrt(f.amount / max);
    const segs = Math.max(16, Math.min(56, Math.round(d / 8)));
    const g = new THREE.TubeGeometry(curve, segs, r, radial, false);
    const n = g.attributes.position.count, uv = g.attributes.uv;
    const along = new Float32Array(n), len = new Float32Array(n).fill(curve.getLength()), tint = new Float32Array(n * 3);
    const col = TINT[f.kind];
    for (let i = 0; i < n; i++) { along[i] = uv.getX(i); tint[i * 3] = col.r; tint[i * 3 + 1] = col.g; tint[i * 3 + 2] = col.b; }
    g.deleteAttribute("normal"); g.deleteAttribute("uv");
    g.setAttribute("along", new THREE.BufferAttribute(along, 1));
    g.setAttribute("len", new THREE.BufferAttribute(len, 1));
    g.setAttribute("tint", new THREE.BufferAttribute(tint, 3));
    geos.push(g);
  }
  const material = arcMaterial();
  if (geos.length) {
    const arcs = new THREE.Mesh(mergeGeometries(geos, false), material);
    arcs.frustumCulled = false;
    arcs.renderOrder = 3;
    group.add(arcs);
  }

  // Rings: what each building's shop fronts take in, area for area.
  const inflow = [...model.buildings.entries()].map(([id, s]) => [id, s.inHome + s.inDesk]).filter(([id, v]) => v > 0 && places.has(id));
  const top = Math.max(1, ...inflow.map(([, v]) => v));
  const ringGeos = inflow.map(([id, v]) => {
    const r = 4 + 22 * Math.sqrt(v / top), p = places.get(id).to;
    return new THREE.RingGeometry(r * 0.82, r, coarse ? 24 : 40).rotateX(-Math.PI / 2).translate(p.x, p.y + 0.2, p.z);
  });
  const ringMaterial = new THREE.MeshBasicMaterial({ color: 0xfff1c9, transparent: true, opacity: 0, depthWrite: false, blending: THREE.AdditiveBlending, side: THREE.DoubleSide, fog: false });
  if (ringGeos.length) {
    const rings = new THREE.Mesh(mergeGeometries(ringGeos, false), ringMaterial);
    rings.renderOrder = 3;
    group.add(rings);
  }

  const bounds = new THREE.Box3();
  for (const g of geos) { g.computeBoundingBox(); bounds.union(g.boundingBox); }
  let want = 0, now = 0;
  const shownAmount = sized.reduce((s, f) => s + f.amount, 0);
  const between = model.flows.filter((f) => f.kind !== "tram" && f.from !== f.to).reduce((s, f) => s + f.amount, 0);
  return {
    group, bounds,
    get shown() { return want === 1; },
    show(on) { want = on ? 1 : 0; if (on) group.visible = true; },
    update(t, dt) {
      now += (want - now) * Math.min(1, dt * 3);
      if (!want && now < 0.01) { now = 0; group.visible = false; return; }
      material.uniforms.time.value = t;
      material.uniforms.opacity.value = now;
      ringMaterial.opacity = now * (0.32 + 0.12 * Math.sin(t * 1.7));
    },
    /** How many arcs, and what share of the money spent between buildings they carry. */
    drawn: { arcs: geos.length, share: between ? shownAmount / between : 0 },
  };
}
