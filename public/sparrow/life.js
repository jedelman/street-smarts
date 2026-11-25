// Glints and pigeons: the small life that makes a model a place.
//
// Glints (after Dream of the Earth's glints.js): a four-point star on each
// thing in reach; the one the prompt names glints bigger, golder and steady.
// Pigeons: a flock on the open spaces, pecking, that bursts up when you rush
// them and settles back. Decoration, not data: one instanced draw call.
import * as THREE from "three";

const MAX_GLINTS = 8;

export function buildGlints(scene) {
  const pos = new Float32Array(MAX_GLINTS * 3), focus = new Float32Array(MAX_GLINTS), phase = new Float32Array(MAX_GLINTS);
  for (let i = 0; i < MAX_GLINTS; i++) phase[i] = i * 0.37;
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  geo.setAttribute("aFocus", new THREE.BufferAttribute(focus, 1));
  geo.setAttribute("aPhase", new THREE.BufferAttribute(phase, 1));
  const uTime = { value: 0 }, uScale = { value: 1 };
  const mat = new THREE.ShaderMaterial({
    uniforms: { uTime, uScale },
    vertexShader: /* glsl */ `
      attribute float aFocus, aPhase;
      uniform float uTime, uScale;
      varying float vFocus, vLight, vTurn;
      void main() {
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        float w = 0.5 + 0.5 * sin(uTime * 2.3 + aPhase * 6.283);
        float flash = pow(w, 6.0);
        float breathe = 0.5 + 0.5 * sin(uTime * 3.1);
        vLight = mix(0.35 + 0.65 * flash, 0.8 + 0.2 * breathe, aFocus);
        vFocus = aFocus;
        vTurn = mix(step(0.5, fract(uTime * 0.6 + aPhase)), 0.0, aFocus);
        gl_PointSize = mix(6.0 + 4.0 * flash, 13.0 + 2.0 * breathe, aFocus) * uScale;
      }`,
    fragmentShader: /* glsl */ `
      varying float vFocus, vLight, vTurn;
      void main() {
        vec2 p = gl_PointCoord * 2.0 - 1.0;
        if (vTurn > 0.5) p = vec2(p.x + p.y, p.y - p.x) * 0.7071;
        vec2 a = abs(p);
        float arms = max(exp(-a.y * 9.0) * (1.0 - a.x), exp(-a.x * 9.0) * (1.0 - a.y));
        float core = exp(-dot(p, p) * 10.0);
        float v = clamp(arms + core, 0.0, 1.0) * vLight;
        if (v < 0.02) discard;
        vec3 c = mix(vec3(1.0, 0.97, 0.88), vec3(1.0, 0.84, 0.38), vFocus * (1.0 - core));
        gl_FragColor = vec4(c * v, v);
      }`,
    transparent: true, depthTest: false, depthWrite: false, blending: THREE.AdditiveBlending,
  });
  const points = new THREE.Points(geo, mat);
  points.frustumCulled = false;
  points.renderOrder = 10;
  scene.add(points);
  return {
    /** spots: [{ x, y, z, focus }] (at most 8). */
    set(spots, t, pixelScale) {
      uTime.value = t;
      uScale.value = pixelScale;
      const n = Math.min(MAX_GLINTS, spots.length);
      for (let i = 0; i < n; i++) {
        pos[i * 3] = spots[i].x; pos[i * 3 + 1] = spots[i].y; pos[i * 3 + 2] = spots[i].z;
        focus[i] = spots[i].focus ? 1 : 0;
      }
      geo.setDrawRange(0, n);
      geo.attributes.position.needsUpdate = true;
      geo.attributes.aFocus.needsUpdate = true;
    },
  };
}

function pigeonGeometry() {
  // One merged, vertex-coloured low-poly pigeon, ~30 cm long, facing +z.
  const parts = [];
  const add = (geo, color, sx, sy, sz, x, y, z, rx = 0) => {
    geo.scale(sx, sy, sz).rotateX(rx).translate(x, y, z);
    const c = new THREE.Color(color), n = geo.attributes.position.count;
    const col = new Float32Array(n * 3);
    for (let i = 0; i < n; i++) { col[i * 3] = c.r; col[i * 3 + 1] = c.g; col[i * 3 + 2] = c.b; }
    geo.setAttribute("color", new THREE.BufferAttribute(col, 3));
    parts.push(geo.index ? geo.toNonIndexed() : geo);
  };
  add(new THREE.SphereGeometry(1, 8, 6), "#8e929c", 0.085, 0.08, 0.15, 0, 0.13, 0);
  add(new THREE.SphereGeometry(1, 7, 5), "#6f7684", 0.055, 0.055, 0.06, 0, 0.22, 0.11);
  add(new THREE.SphereGeometry(1, 6, 4), "#5d8f7e", 0.06, 0.05, 0.05, 0, 0.17, 0.09); // the green-purple neck sheen
  add(new THREE.ConeGeometry(0.012, 0.03, 4), "#3b3634", 1, 1, 1, 0, 0.215, 0.18, Math.PI / 2);
  add(new THREE.BoxGeometry(0.07, 0.015, 0.1), "#5c616c", 1, 1, 1, 0, 0.13, -0.15, -0.2);
  add(new THREE.SphereGeometry(1, 6, 4), "#7c818c", 0.02, 0.06, 0.12, 0.07, 0.14, -0.01);
  add(new THREE.SphereGeometry(1, 6, 4), "#7c818c", 0.02, 0.06, 0.12, -0.07, 0.14, -0.01);
  let total = 0;
  for (const p of parts) total += p.attributes.position.count;
  const position = new Float32Array(total * 3), color = new Float32Array(total * 3);
  let o = 0;
  for (const p of parts) {
    position.set(p.attributes.position.array, o * 3);
    color.set(p.attributes.color.array, o * 3);
    o += p.attributes.position.count;
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.BufferAttribute(position, 3));
  g.setAttribute("color", new THREE.BufferAttribute(color, 3));
  g.computeVertexNormals();
  return g;
}

/**
 * spots: [{ x, y, z }] places on open ground where pigeons live.
 * Returns { update(dt, t, bird, camera) -> number of pigeons that took off this frame }.
 */
export function buildPigeons(scene, spots, rng = Math.random) {
  const n = spots.length;
  const mesh = new THREE.InstancedMesh(pigeonGeometry(), new THREE.MeshLambertMaterial({ vertexColors: true, flatShading: true }), Math.max(1, n));
  mesh.count = n;
  mesh.frustumCulled = false;
  scene.add(mesh);
  const P = spots.map((s) => ({
    home: new THREE.Vector3(s.x, s.y, s.z), pos: new THREE.Vector3(s.x, s.y, s.z), vel: new THREE.Vector3(),
    heading: rng() * Math.PI * 2, state: "peck", timer: rng() * 3, bob: rng() * 6, air: 0,
  }));
  const m = new THREE.Matrix4(), q = new THREE.Quaternion(), e = new THREE.Euler(), one = new THREE.Vector3(1, 1, 1), tmp = new THREE.Vector3();
  P.forEach((p, i) => mesh.setMatrixAt(i, m.compose(p.pos, q.setFromEuler(e.set(0, p.heading, 0)), one)));

  return {
    count: n,
    update(dt, t, bird, camPos) {
      let burst = 0;
      for (let i = 0; i < n; i++) {
        const p = P[i];
        const far = p.pos.distanceToSquared(camPos) > 90 * 90;
        tmp.subVectors(p.pos, bird.pos);
        const d2 = tmp.x * tmp.x + tmp.z * tmp.z;
        if (p.state !== "fly" && d2 < (bird.speed > 1.5 || bird.air ? 2.6 : 0.8) ** 2 && Math.abs(tmp.y) < 2) {
          p.state = "fly"; p.timer = 3 + rng() * 3; burst++;
          const a = Math.atan2(tmp.x, tmp.z) + (rng() - 0.5) * 1.2;
          p.vel.set(Math.sin(a) * 4, 3.5 + rng() * 2, Math.cos(a) * 4);
        }
        if (far && p.state !== "fly") continue; // nobody's watching: leave it be
        p.timer -= dt;
        if (p.state === "fly") {
          p.vel.y -= 1.2 * dt;
          if (p.timer < 1.5) { // circle back home
            tmp.subVectors(p.home, p.pos);
            p.vel.lerp(tmp.multiplyScalar(0.9), Math.min(1, dt * 1.5));
          }
          p.pos.addScaledVector(p.vel, dt);
          p.heading = Math.atan2(p.vel.x, p.vel.z);
          p.air = 1;
          if (p.timer <= 0 && p.pos.y <= p.home.y + 0.4) { p.pos.y = p.home.y; p.state = "peck"; p.timer = 1 + rng() * 2; p.air = 0; }
          if (p.pos.y < p.home.y) p.pos.y = p.home.y;
        } else if (p.state === "walk") {
          p.pos.x += Math.sin(p.heading) * 0.35 * dt;
          p.pos.z += Math.cos(p.heading) * 0.35 * dt;
          if (p.pos.distanceToSquared(p.home) > 2.2) p.heading = Math.atan2(p.home.x - p.pos.x, p.home.z - p.pos.z);
          if (p.timer <= 0) { p.state = "peck"; p.timer = 1 + rng() * 3; }
        } else if (p.timer <= 0) {
          p.state = "walk"; p.timer = 0.6 + rng() * 1.6; p.heading += (rng() - 0.5) * 2.4;
        }
        p.bob += dt * (p.state === "walk" ? 9 : 3);
        const peck = p.state === "peck" ? Math.max(0, Math.sin(p.bob * 1.3)) ** 6 * 0.5 : 0;
        const flap = p.air ? Math.sin(t * 30 + i) * 0.15 : 0;
        e.set(peck - (p.air ? 0.15 : 0), p.heading, flap);
        q.setFromEuler(e);
        m.compose(p.pos, q, one);
        mesh.setMatrixAt(i, m);
      }
      mesh.instanceMatrix.needsUpdate = true;
      return burst;
    },
  };
}
