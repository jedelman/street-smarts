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

// Walking together, side by side: where each companion keeps to, relative to whoever leads
// (x to the leader's right, z ahead), by the group's size.
const SLOTS = {
  2: [[0.7, 0]],
  3: [[0.7, 0], [-0.7, 0]],
  4: [[0.7, 0], [-0.7, 0], [0.35, -0.9]],
};
// How walkers come: alone, in twos, threes and fours (ours).
const SIZES = [[1, 0.34], [2, 0.4], [3, 0.17], [4, 0.09]];

/**
 * opts: { benches, spots: [{x,y,z}] places worth walking to, groups: [{x,y,z}] where a few stand and talk,
 *         count, rng, clearLine(a, b) -> bool, groundAt(x, z, near) -> y }
 *
 * About a fifth sit, a fifth stand talking in groups, and the rest walk, alone or together.
 * Walkers who end up far from you, out of sight, come back on the streets behind you, so
 * wherever you go there are people about.
 */
/**
 * route (optional): (from, to) => [{x, z}, ...] or null: a way along a site's walking mask (walkmask.js), so people
 * keep to the sidewalks and cross at corners; without one they walk straight, wherever the line is clear.
 * keepTo (optional): (x, z) => whether that spot is walkable; a companion with no room beside the leader falls in
 * behind, single file.
 */
export function buildPeople(scene, { benches, spots, groups, count, rng, clearLine, groundAt, route = null, keepTo = null }) {
  // Seats: two per bench, a little back from the seat's middle.
  const seats = [];
  for (const b of benches) {
    const c = Math.cos(b.yaw), s = Math.sin(b.yaw);
    for (const lx of [-0.42, 0.42]) {
      const lz = -0.1;
      seats.push({ x: b.x + lx * c + lz * s, y: b.y, z: b.z - lx * s + lz * c, yaw: b.yaw, front: { x: b.x + 0.9 * s + lx * c, y: b.y, z: b.z + 0.9 * c - lx * s }, taken: null });
    }
  }
  /** A point within `r` metres of a spot, scattered so people don't stack, but never off the walking mask. */
  const scatter = (sp, r) => {
    const x = sp.x + (rng() - 0.5) * 2 * r, z = sp.z + (rng() - 0.5) * 2 * r;
    return !keepTo || keepTo(x, z) ? { x, y: sp.y, z } : { x: sp.x, y: sp.y, z: sp.z };
  };
  const people = [];
  const makePerson = (child = false) => ({
    h: (child ? 1.05 + rng() * 0.3 : 1.6 + rng() * 0.25) / H,
    top: new THREE.Color(pick(rng, TOPS)), bottom: new THREE.Color(pick(rng, BOTTOMS)),
    skin: new THREE.Color(pick(rng, SKINS)), hair: new THREE.Color(pick(rng, HAIRS)),
    pos: new THREE.Vector3(), yaw: rng() * Math.PI * 2, state: "stand", timer: rng() * 6,
    phase: rng() * 6, speed: 1.15 + rng() * 0.35, path: null, seat: null, look: 0, crumbAt: -1e9, near: 0, sway: rng() * 6,
    group: -1, speaking: 0, faceTo: null,
    walker: false, leader: -1, slot: null, followers: null, recycleT: rng() * 2,
  });
  const free = seats.slice().sort(() => rng() - 0.5);
  // Sitting: about a fifth, as many as the benches seat.
  const nSit = Math.min(free.length, Math.round(count * 0.2));
  for (let i = 0; i < nSit; i++) {
    const p = makePerson();
    sitDown(p, free.pop());
    p.timer = 10 + rng() * 40;
    people.push(p);
  }
  // Standing and talking: groups of two to four, each at its own spot (squares, markers, café doors).
  const anchors = groups.map((g, gi) => gi).sort(() => rng() - 0.5);
  const nTalk = anchors.length ? Math.round(count * 0.2) : 0;
  for (let made = 0, a = 0; made < nTalk && a < anchors.length * 2; a++) {
    const gi = anchors[a % anchors.length], g = groups[gi];
    const size = 2 + (rng() < 0.5 ? 1 : 0) + (rng() < 0.2 ? 1 : 0);
    const child = size > 2 && rng() < 0.3;
    for (let k = 0; k < size; k++) {
      const p = makePerson(child && k === size - 1);
      const ang = (k / size) * Math.PI * 2 + rng() * 0.6, r = 0.9 + rng() * 0.4;
      p.pos.set(g.x + Math.sin(ang) * r, g.y, g.z + Math.cos(ang) * r);
      p.yaw = Math.atan2(g.x - p.pos.x, g.z - p.pos.z) + (rng() - 0.5) * 0.5;
      p.state = "talk";
      p.group = gi;
      p.timer = 25 + rng() * 70;
      people.push(p);
    }
    made += size;
  }
  // Walking: alone, or together; a family now and then. Their groups are numbered after the spots'.
  let dyn = groups.length;
  const sizeOf = () => { let r = rng(); for (const [n, w] of SIZES) { if ((r -= w) <= 0) return n; } return 1; };
  while (people.length < count && spots.length) {
    const size = Math.min(sizeOf(), count - people.length);
    const sp = spots[Math.floor(rng() * spots.length)];
    const lead = makePerson();
    lead.walker = true;
    const at = scatter(sp, 1);
    lead.pos.set(at.x, at.y, at.z);
    lead.timer = rng() * 4;
    const li = people.push(lead) - 1;
    if (size > 1) {
      lead.group = dyn++;
      lead.followers = [];
      const family = size > 2 && rng() < 0.4;
      SLOTS[size].forEach((slot, k) => {
        const f = makePerson(family && k === size - 2);
        f.walker = true; f.leader = li; f.slot = slot; f.group = lead.group;
        f.speed = lead.speed;
        place(f, lead);
        lead.followers.push(people.push(f) - 1);
      });
    }
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

  /** A companion at their place beside the leader. */
  function place(f, lead) {
    const c = Math.cos(lead.yaw), sn = Math.sin(lead.yaw);
    f.pos.set(lead.pos.x + f.slot[0] * c + f.slot[1] * sn, lead.pos.y, lead.pos.z - f.slot[0] * sn + f.slot[1] * c);
    f.yaw = lead.yaw;
  }
  /**
   * A walker (and their companions) far away comes back on a street near the sparrow, behind
   * the camera so no one pops into view; `anywhere` (the first frame, under the opening crane
   * shot) drops the behind.
   */
  function bringBack(p, camPos, bird, anywhere = false) {
    const fx = bird.pos.x - camPos.x, fz = bird.pos.z - camPos.z, fl = Math.hypot(fx, fz) || 1;
    for (let tries = 0; tries < 24; tries++) {
      const sp = spots[Math.floor(rng() * spots.length)];
      const dx = sp.x - bird.pos.x, dz = sp.z - bird.pos.z, d = Math.hypot(dx, dz);
      if (d < (anywhere ? 8 : 25) || d > 120) continue;
      if (!anywhere && (dx * fx + dz * fz) / (d * fl) > -0.2) continue; // behind you
      // Not where someone already is: after a long flight or a warp many come back at once, and on the sidewalks
      // they'd all set off together, a parade.
      if (!anywhere && tries < 16 && people.some((q) => q !== p && q.walker && q.leader < 0 && Math.abs(q.pos.x - sp.x) < 4 && Math.abs(q.pos.z - sp.z) < 4)) continue;
      if (p.path && p.path.seat && p.path.seat.taken === p) p.path.seat.taken = null;
      p.path = null;
      const at = scatter(sp, 1);
      p.pos.set(at.x, at.y, at.z);
      p.yaw = rng() * Math.PI * 2;
      p.state = "stand";
      p.timer = anywhere ? rng() * 2 : 0.5 + rng() * 5; // and they set off at their own times
      for (const j of p.followers || []) { const f = people[j]; place(f, p); f.state = "stand"; }
      return true;
    }
    return false;
  }

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
  /** A walk along `pts` ({x, z}, at least one), with the ground every 1.5 m or so, sampled once. */
  function trail(pts, nearY, seat) {
    const xs = [], zs = [], cum = [];
    for (let i = 0; i < pts.length; i++) {
      if (i === 0) { xs.push(pts[0].x); zs.push(pts[0].z); continue; }
      const a = pts[i - 1], b = pts[i], len = Math.hypot(b.x - a.x, b.z - a.z), n = Math.max(1, Math.ceil(len / 1.5));
      for (let k = 1; k <= n; k++) { xs.push(a.x + ((b.x - a.x) * k) / n); zs.push(a.z + ((b.z - a.z) * k) / n); }
    }
    if (xs.length < 2) { xs.push(xs[0]); zs.push(zs[0]); }
    const ys = xs.map((x, i) => groundAt(x, zs[i], nearY));
    cum.push(0);
    for (let i = 1; i < xs.length; i++) cum.push(cum[i - 1] + Math.hypot(xs[i] - xs[i - 1], zs[i] - zs[i - 1]));
    return { xs, zs, ys, cum, len: cum[cum.length - 1], d: 0, k: 0, seat };
  }
  /** Somewhere to walk to: a bench with room, a square, a marker; along the sidewalks where the site has them. */
  function chooseWalk(p) {
    for (let tries = 0; tries < 8; tries++) {
      const wantSeat = !p.followers && rng() < 0.45; // a group walks on together
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
        to = scatter(sp, 1.5);
      }
      let pts;
      if (route) {
        pts = route(p.pos, to);
        if (!pts) continue;
        if (Math.hypot(pts[0].x - p.pos.x, pts[0].z - p.pos.z) > 0.05) pts.unshift({ x: p.pos.x, z: p.pos.z }); // from where they stand
      }
      else { if (!clearLine(p.pos, to)) continue; pts = [{ x: p.pos.x, z: p.pos.z }, to]; }
      // The ground along the way, once, so walking needs no raycasts.
      p.path = trail(pts, Math.max(p.pos.y, to.y), seat);
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
    /** The crowd itself, read-only: pos, state ("walk", "sit", "talk", "stand"), group, speaking. */
    people,
    /** Others standing in person i's group (talkers by the same marker), nearest first. */
    groupOf(i) {
      const p = people[i];
      if (p.group < 0 || p.state !== "talk") return [];
      return people.map((q, j) => j).filter((j) => j !== i && people[j].group === p.group && people[j].state === "talk")
        .sort((a, b) => people[a].pos.distanceToSquared(p.pos) - people[b].pos.distanceToSquared(p.pos));
    },
    /** Person i talks for `seconds`, facing `faceTo` ({x, z}) if given: walkers stop, hands move. */
    speak(i, seconds, faceTo = null) {
      const p = people[i];
      p.speaking = seconds;
      p.faceTo = faceTo;
    },
    /** Where person i's head is, for a speech bubble. */
    headOf(i, out) {
      const p = people[i];
      const sitting = p.state === "sit";
      return out.set(p.pos.x, p.pos.y + (sitting ? 1.25 : 1.75) * p.h * (H / 1.7), p.pos.z);
    },
    /** Returns { crumb: true } when someone drops the sparrow a crumb. */
    update(dt, t, bird, camPos) {
      let crumb = false;
      let routes = route ? 3 : Infinity; // a few new routes a frame at most: nobody minds waiting a moment to set off
      for (let i = 0; i < n; i++) {
        const p = people[i];
        // A walker far from the sparrow, between walks (or very far), comes back somewhere behind you.
        if (p.walker && p.leader < 0 && spots.length) {
          const d2 = p.pos.distanceToSquared(bird.pos);
          p.recycleT -= dt;
          if (first ? d2 > 120 * 120 : p.recycleT <= 0 && ((d2 > 170 * 170 && p.state !== "walk") || d2 > 260 * 260)) bringBack(p, camPos, bird, first);
          if (p.recycleT <= 0) p.recycleT = 1 + rng();
        }
        const far = p.pos.distanceToSquared(camPos) > 160 * 160;
        if (far && !first) continue; // out past the fog: leave them be
        p.timer -= dt;
        const speaking = p.speaking > 0;
        if (speaking) {
          p.speaking -= dt;
          p.sway += dt * 1.6;
          if (p.faceTo && p.state !== "sit") {
            const want = Math.atan2(p.faceTo.x - p.pos.x, p.faceTo.z - p.pos.z);
            let d = want - p.yaw;
            d = Math.atan2(Math.sin(d), Math.cos(d));
            p.yaw += d * Math.min(1, dt * 5);
          }
        }
        // Behavior (someone mid-sentence stays put; a group waits for whoever's talking).
        const groupTalking = p.followers && p.followers.some((j) => people[j].speaking > 0);
        if (p.leader >= 0) {
          // A companion: beside the leader while they walk, turned to the others when they stop.
          const L = people[p.leader];
          if (L.state === "walk" && !(L.speaking > 0) && !L.followers.some((j) => people[j].speaking > 0) && !speaking) {
            const c = Math.cos(L.yaw), sn = Math.sin(L.yaw);
            let tx = L.pos.x + p.slot[0] * c + p.slot[1] * sn, tz = L.pos.z - p.slot[0] * sn + p.slot[1] * c;
            if (keepTo && !keepTo(tx, tz)) { // no room beside: single file, behind
              const back = 0.85 * (1 + L.followers.indexOf(i));
              tx = L.pos.x - sn * back; tz = L.pos.z - c * back;
            }
            const k = Math.min(1, dt * 6);
            p.pos.x += (tx - p.pos.x) * k; p.pos.z += (tz - p.pos.z) * k; p.pos.y = L.pos.y;
            p.yaw = L.yaw;
            p.phase = L.phase + p.slot[0];
            p.state = "walk";
          } else if (!speaking) {
            if (p.state === "walk") p.state = "talk";
            const want = Math.atan2(L.pos.x - p.pos.x, L.pos.z - p.pos.z);
            let d = want - p.yaw;
            d = Math.atan2(Math.sin(d), Math.cos(d));
            p.yaw += d * Math.min(1, dt * 3);
          }
        } else if ((speaking || groupTalking) && p.state === "walk") {
          // Paused where they are.
        } else if (p.state === "walk") {
          const w = p.path;
          w.d = Math.min(w.len, w.d + p.speed * dt);
          while (w.k < w.cum.length - 2 && w.cum[w.k + 1] < w.d) w.k++;
          const k = w.k, span = w.cum[k + 1] - w.cum[k], f = span > 0 ? (w.d - w.cum[k]) / span : 1;
          p.pos.set(w.xs[k] + (w.xs[k + 1] - w.xs[k]) * f, w.ys[k] + (w.ys[k + 1] - w.ys[k]) * f, w.zs[k] + (w.zs[k + 1] - w.zs[k]) * f);
          if (span > 0) { // turning the corner, not snapping round it
            let dy = Math.atan2(w.xs[k + 1] - w.xs[k], w.zs[k + 1] - w.zs[k]) - p.yaw;
            dy = Math.atan2(Math.sin(dy), Math.cos(dy));
            p.yaw += dy * Math.min(1, dt * 7);
          }
          p.phase += dt * p.speed * 5.2;
          if (w.d >= w.len) {
            p.path = null;
            if (w.seat && w.seat.taken === p) { sitDown(p, w.seat); p.timer = 12 + rng() * 35; }
            else if (p.followers) { p.state = "talk"; p.timer = 5 + rng() * 15; } // together: they stop and talk
            else { p.state = "stand"; p.timer = 1 + rng() * 5; }
          }
        } else if (p.state === "sit") {
          if (p.timer <= 0) standUp(p);
        } else if (p.state === "talk") {
          if (p.followers && !speaking) {
            // Turned to the others.
            let cx = 0, cz = 0;
            for (const j of p.followers) { cx += people[j].pos.x; cz += people[j].pos.z; }
            const want = Math.atan2(cx / p.followers.length - p.pos.x, cz / p.followers.length - p.pos.z);
            let d = want - p.yaw;
            d = Math.atan2(Math.sin(d), Math.cos(d));
            p.yaw += d * Math.min(1, dt * 3);
          }
          if (p.timer <= 0 && !speaking && !groupTalking) { p.state = "stand"; p.timer = 0.5; }
        } else if (p.timer <= 0 && !speaking && !groupTalking) {
          if (routes-- > 0) chooseWalk(p); else p.timer = 0.1 + rng() * 0.3;
        }

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
        if (p.state === "walk" && !speaking) {
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
        if (speaking) {
          // Talking with their hands.
          lean = Math.sin(p.sway * 1.1) * 0.05;
          armA = p.state === "sit"
            ? [-0.7 - Math.max(0, Math.sin(p.sway * 2.3)) * 0.6, -0.7]
            : [-0.2 - Math.max(0, Math.sin(p.sway * 2.3)) * 0.9, -0.1 - Math.max(0, Math.sin(p.sway * 1.7 + 1)) * 0.6];
        }
        Q.setFromEuler(E.set(0, p.yaw, 0));
        // Someone walking right through the camera fills the frame: don't draw them that close.
        const tooClose = p.pos.distanceToSquared(camPos) < 1.4 * 1.4;
        M.compose(p.pos, Q, SC.setScalar(tooClose ? 0 : p.h));
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
