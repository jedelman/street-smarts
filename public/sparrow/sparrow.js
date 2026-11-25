// You: a house sparrow, about 16 cm tall, built from a dozen low-poly
// pieces. It hops on the ground, flaps and glides in the air, breathes,
// looks around, pecks, puffs up when it sits, and chirps. Faces +z.
import * as THREE from "three";

const mat = (color) => new THREE.MeshLambertMaterial({ color, flatShading: true });
const ball = (r = 1, w = 10, h = 7) => new THREE.SphereGeometry(r, w, h);

export function buildSparrow() {
  const root = new THREE.Group(); // at the feet; rotation.y = heading
  const body = new THREE.Group(); // bobs, squashes, pitches and rolls
  root.add(body);

  const brown = mat("#8d6446"), dark = mat("#5e4130"), cream = mat("#e9dfcb"), grey = mat("#8f8b84");
  const black = mat("#1d1a18"), beakM = mat("#3d3631"), leg = mat("#c8a27c");

  const torso = new THREE.Mesh(ball(), brown);
  torso.scale.set(0.052, 0.048, 0.078);
  torso.position.set(0, 0.078, 0);
  const belly = new THREE.Mesh(ball(), cream);
  belly.scale.set(0.044, 0.04, 0.062);
  belly.position.set(0, 0.068, 0.006);
  body.add(torso, belly);

  const head = new THREE.Group();
  head.position.set(0, 0.118, 0.052);
  body.add(head);
  const skull = new THREE.Mesh(ball(0.038), brown);
  const crown = new THREE.Mesh(ball(), grey);
  crown.scale.set(0.03, 0.016, 0.03);
  crown.position.set(0, 0.026, 0.004);
  const bib = new THREE.Mesh(ball(), black);
  bib.scale.set(0.02, 0.022, 0.012);
  bib.position.set(0, -0.022, 0.026);
  const cheekL = new THREE.Mesh(ball(0.014, 6, 5), cream);
  cheekL.position.set(0.028, -0.004, 0.012);
  const cheekR = cheekL.clone();
  cheekR.position.x = -0.028;
  const eyeL = new THREE.Mesh(ball(0.0075, 6, 5), black);
  eyeL.position.set(0.026, 0.008, 0.024);
  const eyeR = eyeL.clone();
  eyeR.position.x = -0.026;
  const beak = new THREE.Mesh(new THREE.ConeGeometry(0.012, 0.03, 6).rotateX(Math.PI / 2), beakM);
  beak.position.set(0, -0.002, 0.044);
  head.add(skull, crown, bib, cheekL, cheekR, eyeL, eyeR, beak);

  const wing = (side) => {
    const pivot = new THREE.Group();
    pivot.position.set(0.046 * side, 0.094, 0.004);
    const w = new THREE.Mesh(ball(), dark);
    w.scale.set(0.012, 0.034, 0.066);
    w.position.set(0.004 * side, -0.016, -0.012);
    pivot.add(w);
    body.add(pivot);
    return pivot;
  };
  const wingL = wing(1), wingR = wing(-1);

  const tail = new THREE.Mesh(new THREE.BoxGeometry(0.038, 0.008, 0.07), dark);
  tail.position.set(0, 0.088, -0.09);
  tail.rotation.x = -0.35;
  body.add(tail);

  const legs = new THREE.Group();
  for (const side of [1, -1]) {
    const l = new THREE.Mesh(new THREE.CylinderGeometry(0.004, 0.004, 0.036, 4), leg);
    l.position.set(0.017 * side, 0.018, 0.008);
    legs.add(l);
  }
  root.add(legs);

  // Animation state.
  let hop = 0, hopLift = 0, breathe = Math.random() * 6, lookYaw = 0, lookTarget = 0, nextLook = 1;
  let flap = 0, peck = 0, chirp = 0, sit = 0, squash = 0, wasAir = false;

  return {
    root, head,
    /** Eyes in world space, for the eye-view camera. */
    eyeWorld(target) { return head.localToWorld(target.set(0, 0.006, 0.03)); },
    peck() { peck = 1; },
    chirp() { chirp = 1; },
    sit(on) { sit = on ? 1 : 0; },
    land() { squash = 1; },
    /**
     * s: { dt, t, air (bool), speed (m/s), vy (m/s), turn (rad/s), climbing (bool), reduced (bool) }
     */
    update(s) {
      const dt = s.dt;
      breathe += dt * (s.air ? 5 : 2.2);
      // On the ground, moving: hops, quicker as you go faster.
      if (!s.air && s.speed > 0.15) {
        hop += dt * (5 + s.speed * 3.2);
        hopLift = Math.abs(Math.sin(hop)) * Math.min(0.035, 0.012 + s.speed * 0.008);
      } else {
        hopLift *= Math.max(0, 1 - dt * 12);
      }
      if (wasAir && !s.air) squash = 1;
      wasAir = s.air;
      squash = Math.max(0, squash - dt * 4);
      peck = Math.max(0, peck - dt * 2.2);
      chirp = Math.max(0, chirp - dt * 2.5);
      const sitT = s.speed > 0.2 || s.air ? 0 : sit;

      // Idle: looks around now and then.
      nextLook -= dt;
      if (nextLook <= 0) { lookTarget = (Math.random() - 0.5) * 1.6; nextLook = 0.8 + Math.random() * 2.2; }
      if (s.speed > 0.3 || s.air) lookTarget = 0;
      lookYaw += (lookTarget - lookYaw) * Math.min(1, dt * 14);

      const sq = 1 - 0.25 * Math.sin(squash * Math.PI) * squash;
      const puff = 1 + 0.12 * sitT + 0.02 * Math.sin(breathe);
      body.scale.set(puff / Math.sqrt(sq), puff * sq, puff / Math.sqrt(sq));
      body.position.y = hopLift - 0.012 * sitT;
      legs.scale.y = s.air ? 0.2 : 1 - 0.7 * sitT;
      legs.position.y = hopLift;

      // Flight: flap hard when climbing, glide otherwise; pitch with the climb, roll into turns.
      if (s.air) {
        flap += dt * (s.climbing ? 26 : 9);
        const amp = s.climbing ? 1.1 : 0.25 + 0.15 * Math.sin(flap * 0.13);
        const a = Math.sin(flap) * amp + (s.climbing ? 0.3 : 0.55);
        wingL.rotation.z = -a; wingR.rotation.z = a;
        body.rotation.x = THREE.MathUtils.clamp(-s.vy * 0.08, -0.5, 0.5);
        body.rotation.z = s.reduced ? 0 : THREE.MathUtils.clamp(-s.turn * 0.25, -0.6, 0.6);
      } else {
        flap = 0;
        const fold = 0.05 * Math.sin(breathe * 0.5);
        wingL.rotation.z = -fold; wingR.rotation.z = fold;
        body.rotation.x *= Math.max(0, 1 - dt * 8);
        body.rotation.z *= Math.max(0, 1 - dt * 8);
      }
      // Head: looking, pecking (a quick dip), chirping (tips up).
      const pk = Math.sin(peck * Math.PI) * (peck > 0 ? 1 : 0);
      head.rotation.set(-0.9 * pk * pk + 0.5 * Math.sin(chirp * Math.PI), lookYaw, 0);
      head.position.z = 0.052 + 0.02 * pk;
      head.position.y = 0.118 - 0.03 * pk;
    },
  };
}
