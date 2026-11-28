// Camera maths: the pieces a camera operator does by feel. Pure, so Node can
// test it. smoothDamp, shotWeight and bankFor come from Dream of the Earth's
// camerawork.js (jedelman/dream-of-the-earth), where they were tuned by hand.

/**
 * Critically damped spring toward a target (the classic SmoothDamp): fast to
 * start, no overshoot, arriving in about `smoothTime` seconds. Returns [value, velocity].
 */
export function smoothDamp(current, target, velocity, smoothTime, dt) {
  const omega = 2 / Math.max(1e-4, smoothTime), x = omega * dt;
  const decay = 1 / (1 + x + 0.48 * x * x + 0.235 * x * x * x);
  const change = current - target;
  const temp = (velocity + omega * change) * dt;
  return [target + (change + temp) * decay, (velocity - omega * temp) * decay];
}

/** Shortest signed angle from a to b (radians). */
export const angleTo = (a, b) => Math.atan2(Math.sin(b - a), Math.cos(b - a));

/** smoothDamp for an angle: goes the short way round. */
export function smoothDampAngle(current, target, velocity, smoothTime, dt) {
  return smoothDamp(current, current + angleTo(current, target), velocity, smoothTime, dt);
}

const smooth01 = (t) => { const c = Math.min(1, Math.max(0, t)); return c * c * (3 - 2 * c); };

/** How much an event shot counts `age` seconds in: eases in, holds, eases out by `dur`. */
export function shotWeight(age, dur, blendIn = 0.6, blendOut = 1) {
  if (age < 0 || age > dur) return 0;
  const inW = blendIn > 0 ? smooth01(age / blendIn) : 1;
  const outW = blendOut > 0 ? smooth01((dur - age) / blendOut) : 1;
  return Math.min(inW, outW);
}

/** Lean into a turn (radians of roll): more at speed, capped at 12°. turnRate in rad/s. */
export function bankFor(turnRate, speed01) {
  return Math.max(-0.21, Math.min(0.21, -turnRate * 0.3 * speed01)) + 0; // + 0: no negative zero
}

/** Field of view (degrees) for how fast you're going: a little wider at speed. */
export const fovFor = (speed01) => 60 + 12 * speed01;

/** The camera's yaw after one swing toward `want`: never more than `max` radians from where it is now. */
export const swingToward = (yaw, want, max) => yaw + Math.max(-max, Math.min(max, angleTo(yaw, want)));

/**
 * How far back the camera must stand, looking at `look` from the direction (yaw, pitch), for every point to sit on
 * screen (inside the frame's inner part: |x| <= mx, ylo <= y <= yhi in normalised coordinates, clear of the HUD).
 * Steps back from `from` by 15% at a time, to `max`. Null when no distance up to `max` shows them all: a point on the
 * camera's own side of you stays behind it however far it backs away. The direction is the walk's: the camera sits
 * at look + (sin yaw cos pitch, sin pitch, cos yaw cos pitch) * distance. fov is vertical, in degrees.
 */
export function distanceToShow(points, look, yaw, pitch, { from, max, fov, aspect, mx = 0.85, ylo = -0.8, yhi = 0.7 }) {
  const cp = Math.cos(pitch);
  const dir = [Math.sin(yaw) * cp, Math.sin(pitch), Math.cos(yaw) * cp];
  const f = [-dir[0], -dir[1], -dir[2]]; // forward, toward `look`
  const rl = Math.hypot(f[2], f[0]) || 1;
  const r = [-f[2] / rl, 0, f[0] / rl]; // right: forward x up
  const u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]]; // up: right x forward
  const ty = Math.tan((fov * Math.PI) / 360), tx = ty * aspect;
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  for (let d = Math.max(0.3, from); d <= max * 1.0001; d = Math.min(d * 1.15, d < max ? max : d * 2)) {
    const cam = [look[0] + dir[0] * d, look[1] + dir[1] * d, look[2] + dir[2] * d];
    const shows = points.every((p) => {
      const v = [p[0] - cam[0], p[1] - cam[1], p[2] - cam[2]];
      const z = dot(v, f);
      if (z < 0.2) return false;
      const x = dot(v, r) / (z * tx), y = dot(v, u) / (z * ty);
      return Math.abs(x) <= mx && y >= ylo && y <= yhi;
    });
    if (shows) return d;
    if (d >= max) break;
  }
  return null;
}
