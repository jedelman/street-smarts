// People and benches, after Ghent Streets' explorer (jason-edelman.org,
// ghent-streets/explorer/explorer.js): the same box-built person (legs that
// swing from hip and knee, arms, a head, a cap of hair) in the same clothes
// and skin palettes, and the same bench, given legs. Instanced, so the whole
// crowd is six draw calls and the benches one. Not data: the pipeline places
// no benches and no people. The benches go where the patterns ask for sitting
// (see placeBenches in main.js).
import * as THREE from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";
import { MeshBVH } from "three-mesh-bvh";

// Ghent's palettes, as they are there.
const TOPS = [0x8a2b1f, 0x2f4a3a, 0x3d4f7a, 0xd9b46a, 0x5b3a5e, 0xe9dfc7, 0x6b5a46, 0x7a8f9a, 0xb5653a];
const BOTTOMS = [0x2b2b2b, 0x3d4f7a, 0x5a4a3a, 0x1f2a44, 0x7a6a55, 0x8c8478];
const SKINS = [0xe2b996, 0xc68a62, 0x8d5a3b, 0xf1d0b0, 0xa86f4c, 0x6b4430];
const HAIRS = [0x2a1d14, 0x4a3324, 0x8a6a3a, 0x1a1714, 0xc9b8a0];

// A 1.7 m body in Ghent's proportions; each person scales it to their height.
const H = 1.7, LEG = H * 0.47, TORSO = H * 0.31, HEAD_R = H * 0.062;
const SEAT_Y = 0.48;

const pick = (rng, xs) => xs[Math.floor(rng() * xs.length)];

/** benches: [{ x, y, z, yaw }]. One merged mesh, and a BVH so the sparrow can stand on them. */
export function buildBenches(scene, benches) {
  if (!benches.length) return null;
  const parts = [];
  const m = new THREE.Matrix4(), q = new THREE.Quaternion(), p = new THREE.Vector3(), one = new THREE.Vector3(1, 1, 1);
  for (const b of benches) {
    m.compose(p.set(b.x, b.y, b.z), q.setFromAxisAngle(new THREE.Vector3(0, 1, 0), b.yaw), one);
    const add = (geo, x, y, z) => parts.push(geo.translate(x, y, z).applyMatrix4(m));
    add(new THREE.BoxGeometry(1.6, 0.06, 0.45), 0, 0.45, 0); // seat
    add(new THREE.BoxGeometry(1.6, 0.4, 0.05), 0, 0.72, -0.2); // back
    add(new THREE.BoxGeometry(0.06, 0.42, 0.4), -0.7, 0.21, 0); // legs
    add(new THREE.BoxGeometry(0.06, 0.42, 0.4), 0.7, 0.21, 0);
  }
  const geo = mergeGeometries(parts, false);
  const mesh = new THREE.Mesh(geo, new THREE.MeshLambertMaterial({ color: 0x7a5a3a }));
  mesh.matrixAutoUpdate = false;
  scene.add(mesh);
  const collider = new THREE.BufferGeometry();
  collider.setAttribute("position", geo.attributes.position);
  collider.setIndex(geo.index);
  return { mesh, bvh: new MeshBVH(collider) };
}

function part(geo, count, scene) {
  const mesh = new THREE.InstancedMesh(geo, new THREE.MeshLambertMaterial(), count);
  mesh.frustumCulled = false;
  mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
  scene.add(mesh);
  return mesh;
}

/**
 * opts: { benches, spots: [{x,y,z}] places worth walking to, groups: [{x,y,z}] where a few stand and talk,
 *         count, rng, clearLine(a, b) -> bool, groundAt(x, z, near) -> y }
 */
export function buildPeople(scene, { benches, spots, groups, count, rng, clearLine, groundAt }) {
  // Seats: two per bench, a little back from the seat's middle.
  const seats = [];
  for (const b of benches) {
    const c = Math.cos(b.yaw), s = Math.sin(b.yaw);
    for (const lx of [-0.42, 0.42]) {
      const lz = -0.1;
      seats.push({ x: b.x + lx * c + lz * s, y: b.y, z: b.z - lx * s + lz * c, yaw: b.yaw, front: { x: b.x + 0.9 * s + lx * c, y: b.y, z: b.z + 0.9 * c - lx * s }, taken: null });
    }
  }
  const people = [];
  const makePerson = () => ({
    h: (1.6 + rng() * 0.25) / H,
    top: new THREE.Color(pick(rng, TOPS)), bottom: new THREE.Color(pick(rng, BOTTOMS)),
    skin: new THREE.Color(pick(rng, SKINS)), hair: new THREE.Color(pick(rng, HAIRS)),
    pos: new THREE.Vector3(), yaw: rng() * Math.PI * 2, state: "stand", timer: rng() * 6,
    phase: rng() * 6, speed: 1.15 + rng() * 0.35, path: null, seat: null, look: 0, crumbAt: -1e9, near: 0, sway: rng() * 6,
  });
  // A third sit, a sixth talk in groups, the rest walk.
  const free = seats.slice().sort(() => rng() - 0.5);
  for (let i = 0; i < count; i++) {
    const p = makePerson();
    if (i < count / 3 && free.length) {
      const seat = free.pop();
      sitDown(p, seat);
      p.timer = 10 + rng() * 40;
    } else if (i < count / 2 && groups.length) {
      const g = groups[Math.floor(rng() * groups.length)];
      const a = rng() * Math.PI * 2, r = 1.4 + rng() * 0.6;
      p.pos.set(g.x + Math.sin(a) * r, g.y, g.z + Math.cos(a) * r);
      p.yaw = Math.atan2(g.x - p.pos.x, g.z - p.pos.z) + (rng() - 0.5) * 0.6;
      p.state = "talk";
      p.timer = 20 + rng() * 60;
    } else if (spots.length) {
      const sp = spots[Math.floor(rng() * spots.length)];
      p.pos.set(sp.x + (rng() - 0.5) * 2, sp.y, sp.z + (rng() - 0.5) * 2);
      p.timer = rng() * 4;
    } else continue;
    people.push(p);
  }
  const n = people.length;
  const thighs = part(new THREE.BoxGeometry(0.13, LEG / 2, 0.15), n * 2, scene);
  const shins = part(new THREE.BoxGeometry(0.11, LEG / 2, 0.13), n * 2, scene);
  const torsos = part(new THREE.BoxGeometry(0.38, TORSO, 0.22), n, scene);
  const arms = part(new THREE.CylinderGeometry(0.05, 0.045, H * 0.32, 6), n * 2, scene);
  const heads = part(new THREE.SphereGeometry(HEAD_R, 10, 8), n, scene);
  const hairs = part(new THREE.SphereGeometry(HEAD_R * 1.04, 10, 6, 0, Math.PI * 2, 0, Math.PI / 2), n, scene);
  people.forEach((p, i) => {
    for (const k of [0, 1]) { thighs.setColorAt(i * 2 + k, p.bottom); shins.setColorAt(i * 2 + k, p.bottom); arms.setColorAt(i * 2 + k, p.top); }
    torsos.setColorAt(i, p.top); heads.setColorAt(i, p.skin); hairs.setColorAt(i, p.hair);
  });
  for (const mesh of [thighs, shins, torsos, arms, heads, hairs]) if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;

  function sitDown(p, seat) {
    p.seat = seat; seat.taken = p;
    p.pos.set(seat.x, seat.y, seat.z);
    p.yaw = seat.yaw;
    p.state = "sit";
  }
  function standUp(p) {
    if (p.seat) { p.pos.set(p.seat.front.x, p.seat.front.y, p.seat.front.z); p.seat.taken = null; p.seat = null; }
    p.state = "stand";
    p.timer = 0.5 + rng() * 1.5;
  }
  /** Somewhere to walk to: a bench with room, a square, a marker; in a straight, clear line. */
  function chooseWalk(p) {
    for (let tries = 0; tries < 8; tries++) {
      const wantSeat = rng() < 0.45;
      let to, seat = null;
      if (wantSeat) {
        const open = seats.filter((s) => !s.taken && Math.hypot(s.x - p.pos.x, s.z - p.pos.z) < 70);
        if (!open.length) continue;
        seat = open[Math.floor(rng() * open.length)];
        to = seat.front;
      } else {
        const near = spots.filter((s) => { const d = Math.hypot(s.x - p.pos.x, s.z - p.pos.z); return d > 6 && d < 70; });
        if (!near.length) continue;
        const sp = near[Math.floor(rng() * near.length)];
        to = { x: sp.x + (rng() - 0.5) * 3, y: sp.y, z: sp.z + (rng() - 0.5) * 3 };
      }
      if (!clearLine(p.pos, to)) continue;
      // The ground along the way, once, so walking needs no raycasts.
      const len = Math.hypot(to.x - p.pos.x, to.z - p.pos.z);
      const steps = Math.max(1, Math.ceil(len / 1.5));
      const ys = [];
      for (let k = 0; k <= steps; k++) {
        const f = k / steps;
        ys.push(groundAt(p.pos.x + (to.x - p.pos.x) * f, p.pos.z + (to.z - p.pos.z) * f, Math.max(p.pos.y, to.y)));
      }
      p.path = { from: p.pos.clone(), to: new THREE.Vector3(to.x, to.y, to.z), len, ys, d: 0, seat };
      if (seat) seat.taken = p; // reserved
      p.state = "walk";
      return true;
    }
    p.timer = 2 + rng() * 4;
    return false;
  }

  // Scratch, reused every frame (no garbage per person per frame).
  const M = new THREE.Matrix4(), H0 = new THREE.Matrix4(), A = new THREE.Matrix4(), B = new THREE.Matrix4();
  const R = new THREE.Matrix4(), T = new THREE.Matrix4(), Q = new THREE.Quaternion(), E = new THREE.Euler(), SC = new THREE.Vector3();
  const toBird = new THREE.Vector3();
  let first = true;

  return {
    count: n,
    /** Returns { crumb: true } when someone drops the sparrow a crumb. */
    update(dt, t, bird, camPos) {
      let crumb = false;
      for (let i = 0; i < n; i++) {
        const p = people[i];
        const far = p.pos.distanceToSquared(camPos) > 160 * 160;
        if (far && !first) continue; // out past the fog: leave them be
        p.timer -= dt;
        // Behavior.
        if (p.state === "walk") {
          const w = p.path;
          w.d = Math.min(w.len, w.d + p.speed * dt);
          const f = w.len > 0 ? w.d / w.len : 1;
          const k = f * (w.ys.length - 1), k0 = Math.floor(k), k1 = Math.min(w.ys.length - 1, k0 + 1);
          p.pos.set(w.from.x + (w.to.x - w.from.x) * f, w.ys[k0] + (w.ys[k1] - w.ys[k0]) * (k - k0), w.from.z + (w.to.z - w.from.z) * f);
          p.yaw = Math.atan2(w.to.x - w.from.x, w.to.z - w.from.z);
          p.phase += dt * p.speed * 5.2;
          if (f >= 1) {
            p.path = null;
            if (w.seat && w.seat.taken === p) { sitDown(p, w.seat); p.timer = 12 + rng() * 35; }
            else { p.state = "stand"; p.timer = 1 + rng() * 5; }
          }
        } else if (p.state === "sit") {
          if (p.timer <= 0) standUp(p);
        } else if (p.state === "talk") {
          if (p.timer <= 0) { p.state = "stand"; p.timer = 0.5; }
        } else if (p.timer <= 0) chooseWalk(p);

        // They notice the sparrow: heads turn to it.
        toBird.subVectors(bird.pos, p.pos);
        const dB = Math.hypot(toBird.x, toBird.z);
        let lookT = 0;
        if (dB < 3 && p.state !== "walk") {
          let a = Math.atan2(toBird.x, toBird.z) - p.yaw;
          a = Math.atan2(Math.sin(a), Math.cos(a));
          lookT = Math.max(-1.1, Math.min(1.1, a));
        }
        p.look += (lookT - p.look) * Math.min(1, dt * 4);
        // Sit still beside someone on a bench and they may drop you a crumb.
        if (p.state === "sit" && dB < 1.4 && bird.speed < 0.1 && !bird.air) {
          p.near += dt;
          if (p.near > 2.5 && t - p.crumbAt > 90) { p.crumbAt = t; crumb = true; }
        } else p.near = 0;

        // Pose (Ghent's: a negative hip angle swings a leg forward; a positive knee angle bends the shin back).
        let hipsY = LEG, hipA = [0, 0], kneeA = [0, 0], armA = [0.05, 0.05], lean = 0;
        if (p.state === "walk") {
          const s = Math.sin(p.phase);
          hipA = [-s * 0.45, s * 0.45];
          kneeA = [Math.max(0, -s) * 0.7, Math.max(0, s) * 0.7];
          armA = [-s * 0.4, s * 0.4];
          hipsY = LEG + Math.abs(Math.cos(p.phase)) * 0.03;
        } else if (p.state === "sit") {
          hipA = [-Math.PI / 2, -Math.PI / 2];
          kneeA = [Math.PI / 2, Math.PI / 2];
          armA = [-0.7, -0.7];
          hipsY = SEAT_Y / p.h;
        } else if (p.state === "talk") {
          p.sway += dt;
          lean = Math.sin(p.sway * 0.9) * 0.04;
          armA = [0.05 + Math.max(0, Math.sin(p.sway * 1.7)) * 0.5, 0.05];
        }
        Q.setFromEuler(E.set(0, p.yaw, 0));
        M.compose(p.pos, Q, SC.setScalar(p.h));
        H0.copy(M).multiply(T.makeTranslation(0, hipsY, 0)).multiply(R.makeRotationX(lean));
        for (let k = 0; k < 2; k++) {
          A.copy(H0).multiply(T.makeTranslation(k ? 0.09 : -0.09, 0, 0)).multiply(R.makeRotationX(hipA[k])); // the hip
          thighs.setMatrixAt(i * 2 + k, B.copy(A).multiply(T.makeTranslation(0, -LEG / 4, 0)));
          A.multiply(T.makeTranslation(0, -LEG / 2, 0)).multiply(R.makeRotationX(kneeA[k])).multiply(T.makeTranslation(0, -LEG / 4, 0));
          shins.setMatrixAt(i * 2 + k, A);
          A.copy(H0).multiply(T.makeTranslation(k ? 0.24 : -0.24, TORSO - 0.05, 0)).multiply(R.makeRotationX(armA[k])).multiply(T.makeTranslation(0, -H * 0.16, 0));
          arms.setMatrixAt(i * 2 + k, A);
        }
        torsos.setMatrixAt(i, B.copy(H0).multiply(T.makeTranslation(0, TORSO / 2, 0)));
        A.copy(H0).multiply(T.makeTranslation(0, TORSO + HEAD_R + 0.04, 0)).multiply(R.makeRotationY(p.look));
        heads.setMatrixAt(i, A);
        hairs.setMatrixAt(i, A.multiply(R.makeRotationX(-0.25)));
      }
      first = false;
      for (const mesh of [thighs, shins, torsos, arms, heads, hairs]) mesh.instanceMatrix.needsUpdate = true;
      return { crumb };
    },
  };
}
