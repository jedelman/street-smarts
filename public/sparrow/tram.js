// The light rail on P34's line: a two-car tram that runs in from past the
// site's edge, stops at the platform, rings its bell, waits, and runs back out
// (an end-of-line station; the cars drive from either end). Not data: the
// line and the station are (GLB extras.streetSmarts.transit, pack_glb.add_transit),
// the timetable and the tram are ours. A sparrow on its roof rides along.
import * as THREE from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

const CAR = 13.6, GAP = 0.5, LENGTH = 2 * CAR + GAP, WIDTH = 2.65;
const FLOOR = 0.35, BODY = 2.75, RAIL_TOP = 0.2;
const ROOF = RAIL_TOP + FLOOR + BODY; // where a passenger on the roof stands
// A cycle: run in, dwell at the platform, run out, lay over past the edge.
const RUN_IN = 42, DWELL = 26, RUN_OUT = 38, LAYOVER = 16, CYCLE = RUN_IN + DWELL + RUN_OUT + LAYOVER;

function part(w, h, d, x, y, z, color) {
  const g = new THREE.BoxGeometry(w, h, d).translate(x, y, z);
  const c = new THREE.Color(color);
  const col = new Float32Array(g.attributes.position.count * 3);
  for (let i = 0; i < col.length; i += 3) { col[i] = c.r; col[i + 1] = c.g; col[i + 2] = c.b; }
  g.setAttribute("color", new THREE.BufferAttribute(col, 3));
  return g;
}

/** The tram's body, along +x, in one geometry: two cars, glazing, a skirt, roof gear, a pantograph. */
function tramGeometry() {
  const parts = [];
  for (const sign of [-1, 1]) {
    const cx = sign * (CAR + GAP) / 2;
    const y0 = FLOOR;
    parts.push(part(CAR, BODY, WIDTH, cx, y0 + BODY / 2, 0, "#f1ede2")); // body
    parts.push(part(CAR - 0.6, 1.25, WIDTH + 0.02, cx, y0 + 1.75, 0, "#22323b")); // window band
    parts.push(part(CAR, 0.55, WIDTH + 0.03, cx, y0 + 0.28, 0, "#2f5a4a")); // skirt
    parts.push(part(CAR - 2, 0.12, WIDTH + 0.04, cx, y0 + 0.95, 0, "#2f5a4a")); // belt line
    parts.push(part(4.2, 0.32, 1.6, cx - sign * 3, y0 + BODY + 0.16, 0, "#9aa1a6")); // roof gear
    // The cab's windscreen at each outer end.
    parts.push(part(0.04, 1.4, WIDTH - 0.5, cx + sign * (CAR / 2 + 0.01), y0 + 1.8, 0, "#1b262d"));
    parts.push(part(0.06, 0.18, 0.3, cx + sign * (CAR / 2 + 0.02), y0 + 0.75, 0.85, "#fff4c8"));
    parts.push(part(0.06, 0.18, 0.3, cx + sign * (CAR / 2 + 0.02), y0 + 0.75, -0.85, "#fff4c8"));
  }
  parts.push(part(GAP + 0.1, BODY - 0.3, WIDTH - 0.3, 0, FLOOR + BODY / 2, 0, "#3a3f43")); // the articulation
  // The pantograph: a frame up to the (unseen) wire.
  parts.push(part(0.08, 1.1, 0.08, 1.5, FLOOR + BODY + 0.85, 0, "#4a5055"));
  parts.push(part(0.1, 0.06, 1.8, 1.5, FLOOR + BODY + 1.42, 0, "#4a5055"));
  return mergeGeometries(parts, false);
}

const smooth = (u) => u * u * (3 - 2 * u); // eases in and out: stops gently at both ends

/**
 * The tram on `line` ({points: [[x, z], ...], stations: [{at, length}]}), its
 * first arrival a few seconds after it's built. Returns { mesh, roofY,
 * update(t) -> { delta, speed, arrived, departing }, onRoof(pos), state(t) }.
 */
export function buildTram(scene, line) {
  const pts = line.points.map(([x, z]) => new THREE.Vector3(x, RAIL_TOP, z));
  const lens = [0];
  for (let i = 1; i < pts.length; i++) lens.push(lens[i - 1] + pts[i].distanceTo(pts[i - 1]));
  const total = lens[lens.length - 1];
  const station = line.stations[0];
  const stopAt = Math.min(total - LENGTH / 2, station ? station.at : total - LENGTH / 2);
  const startAt = LENGTH / 2;
  const mesh = new THREE.Mesh(tramGeometry(), new THREE.MeshLambertMaterial({ vertexColors: true }));
  scene.add(mesh);

  const at = (s, out) => {
    let i = 1;
    while (i < lens.length - 1 && lens[i] < s) i++;
    const a = pts[i - 1], b = pts[i];
    const u = THREE.MathUtils.clamp((s - lens[i - 1]) / Math.max(1e-6, lens[i] - lens[i - 1]), 0, 1);
    out.lerpVectors(a, b, u);
    return Math.atan2(-(b.z - a.z), b.x - a.x); // yaw that points +x along the line
  };
  // Start so the first arrival comes about ten seconds in.
  const offset = RUN_IN - 10;
  const sAt = (t) => {
    const c = ((t + offset) % CYCLE + CYCLE) % CYCLE;
    if (c < RUN_IN) return { s: startAt + (stopAt - startAt) * smooth(c / RUN_IN), phase: "in", c };
    if (c < RUN_IN + DWELL) return { s: stopAt, phase: "dwell", c };
    if (c < RUN_IN + DWELL + RUN_OUT) return { s: stopAt - (stopAt - startAt) * smooth((c - RUN_IN - DWELL) / RUN_OUT), phase: "out", c };
    return { s: startAt, phase: "layover", c };
  };
  const pos = new THREE.Vector3(), last = new THREE.Vector3(), delta = new THREE.Vector3();
  let lastPhase = null, first = true;
  return {
    mesh, roofY: ROOF, length: LENGTH,
    /** Moves the tram to time t; `delta` is how far it moved since last frame (for a rider). */
    update(t) {
      const { s, phase } = sAt(t);
      const yaw = at(s, pos);
      mesh.position.copy(pos);
      mesh.rotation.y = yaw;
      delta.subVectors(pos, last);
      if (first) { delta.set(0, 0, 0); first = false; }
      last.copy(pos);
      const events = { arrived: phase === "dwell" && lastPhase === "in", departing: phase === "out" && lastPhase === "dwell" };
      lastPhase = phase;
      return { delta, speed: delta.length(), phase, ...events };
    },
    /** Seconds until the next arrival at the platform (0 while it's there). */
    nextArrival(t) {
      const c = ((t + offset) % CYCLE + CYCLE) % CYCLE;
      if (c < RUN_IN) return RUN_IN - c;
      if (c < RUN_IN + DWELL) return 0;
      return CYCLE - c + RUN_IN;
    },
    /** Is `p` standing on the roof (within its footprint, at roof height)? */
    onRoof(p) {
      const local = p.clone().sub(mesh.position).applyAxisAngle(new THREE.Vector3(0, 1, 0), -mesh.rotation.y);
      return Math.abs(local.x) < LENGTH / 2 - 0.3 && Math.abs(local.z) < WIDTH / 2 - 0.1
        && p.y > ROOF - 0.45 && p.y < ROOF + 0.6;
    },
  };
}
